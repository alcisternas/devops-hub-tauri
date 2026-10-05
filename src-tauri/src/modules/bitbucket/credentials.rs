// ─────────────────────────────────────────────────────────────────────────
// Credenciales de Bitbucket — modulo Bitbucket Repo Creator
//
// Guarda el par <correo, token de Atlassian> en el almacen seguro del
// sistema operativo y valida que sirvan contra la API real de Bitbucket.
// ─────────────────────────────────────────────────────────────────────────

use keyring::Entry;
use serde::{Deserialize, Serialize};

// RefreshError vive hoy en el modulo eks porque fue el primero en
// necesitarlo, pero el tipo es generico y los tres modulos ya lo comparten.
// Se reutiliza en vez de declarar uno nuevo, asi React sigue parseando
// {etapa, mensaje} con la misma funcion parseErr para todo el hub.
use super::super::eks::config::RefreshError;
use super::client::{mensaje_error, BBClient};

// Nombre del servicio en el almacen del sistema. Prefijo "devops-hub-"
// para que todo lo que guarde esta app quede agrupado y visible junto en
// Keychain / Credential Manager, y no choque con items de otras
// herramientas instaladas en el mismo equipo.
pub const SERVICE: &str = "devops-hub-bitbucket";

// UNA sola entrada con ambos valores adentro, no una por valor.
//
// La version anterior guardaba correo y token como items separados, y eso
// hacia que cada operacion pidiera la clave del llavero DOS veces: una por
// item. Juntarlos deja una sola lectura y por lo tanto una sola solicitud.
//
// El nombre de usuario de la entrada es fijo porque al abrir la app
// todavia no sabemos cual es el correo del usuario: si lo usaramos como
// clave de busqueda habria que adivinarlo para poder leer.
const KEY_CRED: &str = "credenciales";

// Lo que se guarda adentro de esa entrada, serializado como JSON.
#[derive(Debug, Serialize, Deserialize)]
struct Credenciales {
    email: String,
    token: String,
}

// ─────────────────────────────────────────────────────────────────────────
// Estado que ve React
// ─────────────────────────────────────────────────────────────────────────
#[derive(Debug, Serialize)]
pub struct CredStatus {
    // El almacen del sistema respondio. En Linux puede venir false si no
    // hay un servicio de llavero corriendo (equipo sin escritorio).
    pub almacen_disponible: bool,
    // Hay credenciales guardadas en el almacen.
    pub guardadas: bool,
    // Correo asociado, para mostrarlo en pantalla. Nunca el token.
    pub email: Option<String>,
    // Las credenciales guardadas funcionan contra la API ahora mismo.
    pub validas: bool,
    // Texto para mostrarle al usuario: por que fallo, o como esta todo.
    pub detalle: String,
}

// ─────────────────────────────────────────────────────────────────────────
// Acceso al almacen del sistema
//
// Estas funciones son SINCRONAS y bloquean el hilo donde corren. No es un
// descuido: la libreria accede al almacen de forma bloqueante en las tres
// plataformas, y en Linux cada llamada viaja por D-Bus a otro proceso, lo
// que puede tardar cientos de milisegundos. Por eso los comandos de mas
// abajo las ejecutan dentro de spawn_blocking, para no congelar la interfaz.
// ─────────────────────────────────────────────────────────────────────────

fn entrada() -> Result<Entry, String> {
    Entry::new(SERVICE, KEY_CRED).map_err(|e| e.to_string())
}

fn leer_sync() -> Result<Option<Credenciales>, String> {
    let guardado = match entrada()?.get_password() {
        Ok(v) => v,
        // NoEntry significa "nunca se guardo nada": es el caso normal de la
        // primera vez, no un error. Cualquier otro error SI se propaga,
        // porque ahi el almacen existe pero algo anda mal.
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };

    // Si lo guardado no se puede interpretar, se trata como "no hay nada"
    // en vez de como error: el usuario puede volver a ingresar sus datos y
    // sobrescribirlo, que es justamente lo que corresponde hacer.
    Ok(serde_json::from_str::<Credenciales>(&guardado).ok())
}

fn guardar_sync(email: &str, token: &str) -> Result<(), String> {
    let cred = Credenciales {
        email: email.to_string(),
        token: token.to_string(),
    };
    let json = serde_json::to_string(&cred).map_err(|e| e.to_string())?;
    entrada()?.set_password(&json).map_err(|e| e.to_string())
}

fn borrar_sync() -> Result<(), String> {
    match entrada()?.delete_credential() {
        Ok(()) => Ok(()),
        // Borrar algo que no existe deja el sistema en el estado deseado
        // igual, asi que no se trata como falla.
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// Lectura para uso interno de otros archivos del modulo (full_run).
// Falla con un mensaje claro si no hay nada guardado, porque llegar hasta
// ahi sin credenciales es un error de flujo, no una situacion normal.
pub async fn obtener_credenciales() -> Result<(String, String), RefreshError> {
    let lectura = tokio::task::spawn_blocking(leer_sync)
        .await
        .map_err(|e| RefreshError::new("leer_credenciales", e.to_string()))?;

    match lectura {
        Ok(Some(c)) => Ok((c.email, c.token)),
        Ok(None) => Err(RefreshError::new(
            "leer_credenciales",
            "No hay credenciales guardadas en este equipo. Ingresalas antes de continuar.",
        )),
        Err(e) => Err(RefreshError::new("leer_credenciales", e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Validacion contra la API real
// ─────────────────────────────────────────────────────────────────────────

// Se consulta el listado de repositorios del workspace en vez del perfil
// del usuario. Es la misma verificacion que hacia la herramienta anterior,
// y comprueba dos cosas de una sola vez: que el token sea valido Y que
// tenga acceso al workspace donde se van a crear los repositorios.
// pagelen=1 pide un solo resultado: alcanza para saber si responde.
async fn validar(email: &str, token: &str, workspace: &str) -> Result<(bool, String), RefreshError> {
    let client = BBClient::new(email, token)?;
    let (codigo, cuerpo) = client
        .get(&format!("/repositories/{}?pagelen=1", workspace))
        .await?;

    let resultado = match codigo {
        200 => (true, format!("Autenticado como {}", email)),
        401 => (
            false,
            "Credenciales rechazadas (401). El token puede estar vencido o revocado.".to_string(),
        ),
        403 => (
            false,
            "Sin permisos suficientes (403). Revisa que el token tenga los scopes requeridos."
                .to_string(),
        ),
        404 => (
            false,
            format!("El workspace '{}' no existe o no es visible con este token (404).", workspace),
        ),
        otro => (
            false,
            format!("Respuesta inesperada de Bitbucket (HTTP {}): {}", otro, mensaje_error(&cuerpo)),
        ),
    };

    Ok(resultado)
}

// ─────────────────────────────────────────────────────────────────────────
// Comandos que React invoca
// ─────────────────────────────────────────────────────────────────────────

// Se llama al abrir el modulo. Lee lo guardado y lo prueba contra la API.
// Nunca pide nada al usuario: solo reporta en que estado esta todo.
#[tauri::command]
pub async fn bitbucket_cred_status(workspace: String) -> Result<CredStatus, RefreshError> {
    let lectura = tokio::task::spawn_blocking(leer_sync)
        .await
        .map_err(|e| RefreshError::new("leer_credenciales", e.to_string()))?;

    match lectura {
        // El almacen respondio y no hay nada guardado todavia.
        Ok(None) => Ok(CredStatus {
            almacen_disponible: true,
            guardadas: false,
            email: None,
            validas: false,
            detalle: "No hay credenciales guardadas en este equipo.".to_string(),
        }),

        // Hay credenciales: se prueban contra la API.
        Ok(Some(c)) => {
            let (validas, detalle) = validar(&c.email, &c.token, &workspace).await?;
            Ok(CredStatus {
                almacen_disponible: true,
                guardadas: true,
                email: Some(c.email),
                validas,
                detalle,
            })
        }

        // El almacen mismo fallo. En Linux este es el caso de "no hay
        // servicio de llavero corriendo". Se informa sin romper el modulo:
        // el usuario igual puede ingresar credenciales, solo que no
        // quedaran guardadas para la proxima vez.
        Err(e) => Ok(CredStatus {
            almacen_disponible: false,
            guardadas: false,
            email: None,
            validas: false,
            detalle: format!("No se pudo acceder al almacen de credenciales del sistema: {}", e),
        }),
    }
}

// Valida PRIMERO y guarda despues. El orden importa: guardar un token que
// no sirve dejaria al usuario con credenciales rotas en el llavero y la
// falla recien aparecería en el proximo uso.
#[tauri::command]
pub async fn bitbucket_cred_save(
    email: String,
    token: String,
    workspace: String,
) -> Result<CredStatus, RefreshError> {
    let email = email.trim().to_string();
    let token = token.trim().to_string();

    if email.is_empty() || token.is_empty() {
        return Err(RefreshError::new(
            "validar_entrada",
            "El correo y el token son obligatorios.",
        ));
    }

    let (validas, detalle) = validar(&email, &token, &workspace).await?;

    if !validas {
        // No se guarda nada. Se devuelve el motivo para mostrarlo en pantalla.
        return Ok(CredStatus {
            almacen_disponible: true,
            guardadas: false,
            email: Some(email),
            validas: false,
            detalle,
        });
    }

    let email_para_guardar = email.clone();
    let token_para_guardar = token.clone();
    let guardado = tokio::task::spawn_blocking(move || {
        guardar_sync(&email_para_guardar, &token_para_guardar)
    })
    .await
    .map_err(|e| RefreshError::new("guardar_credenciales", e.to_string()))?;

    match guardado {
        Ok(()) => Ok(CredStatus {
            almacen_disponible: true,
            guardadas: true,
            email: Some(email),
            validas: true,
            detalle: format!("{} — guardadas en este equipo.", detalle),
        }),

        // Las credenciales sirven pero no se pudieron guardar. La sesion
        // actual puede seguir trabajando; lo que se pierde es recordarlas.
        Err(e) => Ok(CredStatus {
            almacen_disponible: false,
            guardadas: false,
            email: Some(email),
            validas: true,
            detalle: format!(
                "Credenciales validas, pero no se pudieron guardar en el sistema: {}",
                e
            ),
        }),
    }
}

// Permite cambiar de cuenta o limpiar un token vencido sin tener que ir a
// buscar el item a mano en Keychain o Credential Manager.
#[tauri::command]
pub async fn bitbucket_cred_delete() -> Result<(), RefreshError> {
    tokio::task::spawn_blocking(borrar_sync)
        .await
        .map_err(|e| RefreshError::new("borrar_credenciales", e.to_string()))?
        .map_err(|e| RefreshError::new("borrar_credenciales", e))
}
