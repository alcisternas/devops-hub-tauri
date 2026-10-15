// Credenciales de Bitbucket — modulo Bitbucket Repo Creator
//
// Guarda el par <correo, token de Atlassian> en el almacen seguro del
// sistema operativo y valida que sirvan contra la API real de Bitbucket.

use serde::{Deserialize, Serialize};
use super::super::llavero::Llavero;

use super::super::eks::config::RefreshError;
use super::client::{mensaje_error, BBClient};

// Lo que se guarda en el llavero, serializado como JSON.
#[derive(Debug, Serialize, Deserialize)]
struct Credenciales {
    email: String,
    token: String,
}

// Instancia compartida con caché en memoria (via llavero.rs).
// Un solo ítem JSON para correo y token: una lectura al Keychain por sesión,
// sin importar cuántos módulos pidan las credenciales de Bitbucket.
static LLAVERO: Llavero = Llavero::nuevo("devops-hub-bitbucket", "credenciales");

// Estado que ve React
#[derive(Debug, Serialize)]
pub struct CredStatus {
    pub almacen_disponible: bool,
    pub guardadas: bool,
    pub email: Option<String>,
    pub validas: bool,
    pub detalle: String,
}

// Lectura para uso interno de otros archivos del modulo (full_run, cicd).
pub async fn obtener_credenciales() -> Result<(String, String), RefreshError> {
    let lectura = tokio::task::spawn_blocking(|| LLAVERO.leer())
        .await
        .map_err(|e| RefreshError::new("leer_credenciales", e.to_string()))?;

    match lectura {
        Ok(Some(json)) => {
            let c = serde_json::from_str::<Credenciales>(&json)
                .map_err(|e| RefreshError::new("leer_credenciales", e.to_string()))?;
            Ok((c.email, c.token))
        }
        Ok(None) => Err(RefreshError::new(
            "leer_credenciales",
            "No hay credenciales guardadas en este equipo. Ingresalas antes de continuar.",
        )),
        Err(e) => Err(RefreshError::new("leer_credenciales", e)),
    }
}

// Validacion contra la API real.
// Se consulta el listado de repositorios del workspace: comprueba a la vez
// que el token sea valido Y que tenga acceso al workspace donde se crean repos.
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

// Comandos que React invoca

// Se llama al abrir el modulo. Lee lo guardado y lo prueba contra la API.
#[tauri::command]
pub async fn bitbucket_cred_status(workspace: String) -> Result<CredStatus, RefreshError> {
    let lectura = tokio::task::spawn_blocking(|| LLAVERO.leer())
        .await
        .map_err(|e| RefreshError::new("leer_credenciales", e.to_string()))?;

    match lectura {
        Ok(None) => Ok(CredStatus {
            almacen_disponible: true,
            guardadas: false,
            email: None,
            validas: false,
            detalle: "No hay credenciales guardadas en este equipo.".to_string(),
        }),
        Ok(Some(json)) => {
            // JSON malformado se trata igual que "sin credenciales".
            let c = match serde_json::from_str::<Credenciales>(&json).ok() {
                None => return Ok(CredStatus {
                    almacen_disponible: true,
                    guardadas: false,
                    email: None,
                    validas: false,
                    detalle: "No hay credenciales guardadas en este equipo.".to_string(),
                }),
                Some(c) => c,
            };
            let (validas, detalle) = validar(&c.email, &c.token, &workspace).await?;
            Ok(CredStatus {
                almacen_disponible: true,
                guardadas: true,
                email: Some(c.email),
                validas,
                detalle,
            })
        }
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
// no sirve dejaria al usuario con credenciales rotas en el llavero.
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
        let cred = Credenciales { email: email_para_guardar, token: token_para_guardar };
        let json = serde_json::to_string(&cred).map_err(|e| e.to_string())?;
        LLAVERO.guardar(&json)
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

// Permite cambiar de cuenta o limpiar un token vencido.
#[tauri::command]
pub async fn bitbucket_cred_delete() -> Result<(), RefreshError> {
    tokio::task::spawn_blocking(|| LLAVERO.borrar())
        .await
        .map_err(|e| RefreshError::new("borrar_credenciales", e.to_string()))?
        .map_err(|e| RefreshError::new("borrar_credenciales", e))
}
