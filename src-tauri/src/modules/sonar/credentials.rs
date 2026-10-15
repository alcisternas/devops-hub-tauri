// ─────────────────────────────────────────────────────────────────────────
// Credenciales de SonarQube Cloud
//
// Mismo patron que el modulo de Bitbucket, con dos diferencias:
//   - Servicio propio: devops-hub-sonarqube (convencion devops-hub-<sistema>)
//   - Solo hace falta el TOKEN. SonarQube usa Bearer, no necesita correo.
// ─────────────────────────────────────────────────────────────────────────

use serde::Serialize;

use super::super::eks::config::RefreshError;
use super::super::llavero::Llavero;
use super::client::{mensaje_error, SonarClient, ORG};

// Convencion del hub: devops-hub-<sistema>. Un item por sistema, para poder
// rotar el token de uno sin tocar el del otro.
const SERVICE: &str = "devops-hub-sonarqube";
const KEY_TOKEN: &str = "token";

// Toda la logica de llavero y cache vive en el helper compartido
// (modules/llavero.rs), igual que en Bitbucket y en lo que venga despues.
static LLAVERO: Llavero = Llavero::nuevo(SERVICE, KEY_TOKEN);

#[derive(Debug, Serialize)]
pub struct SonarCredStatus {
    pub almacen_disponible: bool,
    pub guardadas: bool,
    pub validas: bool,
    pub detalle: String,
    // Nombre legible de la organizacion, cuando el token da acceso.
    pub organizacion: Option<String>,
}

// Lectura para uso interno del modulo (la usa bitbucket::full_run).
pub async fn obtener_token() -> Result<String, RefreshError> {
    // spawn_blocking: el helper es sincrono y puede esperar el candado.
    let lectura = tokio::task::spawn_blocking(|| LLAVERO.leer())
        .await
        .map_err(|e| RefreshError::new("leer_token_sonar", e.to_string()))?;

    match lectura {
        Ok(Some(t)) => Ok(t),
        Ok(None) => Err(RefreshError::new(
            "leer_token_sonar",
            "No hay token de SonarQube guardado en este equipo.",
        )),
        Err(e) => Err(RefreshError::new("leer_token_sonar", e)),
    }
}

// Se valida consultando la organizacion en vez del endpoint generico de
// autenticacion. Asi se comprueban dos cosas de una sola vez: que el token
// sirva Y que de acceso a la organizacion donde se crean los proyectos.
async fn validar(token: &str) -> Result<(bool, Option<String>, String), RefreshError> {
    let client = SonarClient::new(token)?;
    let (codigo, cuerpo) = client
        .get_v1(&format!("/api/organizations/search?organizations={}", ORG))
        .await?;

    if codigo == 401 {
        return Ok((false, None, "Token rechazado (401). Puede estar vencido o revocado.".into()));
    }
    if codigo != 200 {
        return Ok((
            false,
            None,
            format!("Respuesta inesperada de SonarQube (HTTP {}): {}", codigo, mensaje_error(&cuerpo)),
        ));
    }

    let nombre = serde_json::from_str::<serde_json::Value>(&cuerpo)
        .ok()
        .and_then(|v| {
            v.get("organizations")?
                .as_array()?
                .first()?
                .get("name")?
                .as_str()
                .map(String::from)
        });

    match nombre {
        Some(n) => Ok((true, Some(n.clone()), format!("Autenticado en {}", n))),
        None => Ok((
            false,
            None,
            format!("El token es valido pero no da acceso a la organizacion '{}'.", ORG),
        )),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Comandos
// ─────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn sonar_cred_status() -> Result<SonarCredStatus, RefreshError> {
    let lectura = tokio::task::spawn_blocking(|| LLAVERO.leer())
        .await
        .map_err(|e| RefreshError::new("leer_token_sonar", e.to_string()))?;

    match lectura {
        Ok(None) => Ok(SonarCredStatus {
            almacen_disponible: true,
            guardadas: false,
            validas: false,
            detalle: "No hay token de SonarQube guardado en este equipo.".into(),
            organizacion: None,
        }),
        Ok(Some(token)) => {
            let (validas, organizacion, detalle) = validar(&token).await?;
            Ok(SonarCredStatus {
                almacen_disponible: true,
                guardadas: true,
                validas,
                detalle,
                organizacion,
            })
        }
        Err(e) => Ok(SonarCredStatus {
            almacen_disponible: false,
            guardadas: false,
            validas: false,
            detalle: format!("No se pudo acceder al almacen de credenciales: {}", e),
            organizacion: None,
        }),
    }
}

#[tauri::command]
pub async fn sonar_cred_save(token: String) -> Result<SonarCredStatus, RefreshError> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(RefreshError::new("validar_entrada", "El token es obligatorio."));
    }

    // Se valida antes de guardar, por el mismo motivo que en Bitbucket:
    // guardar un token que no sirve esconde el error hasta el proximo uso.
    let (validas, organizacion, detalle) = validar(&token).await?;
    if !validas {
        return Ok(SonarCredStatus {
            almacen_disponible: true,
            guardadas: false,
            validas: false,
            detalle,
            organizacion: None,
        });
    }

    let copia = token.clone();
    // El token ya fue validado arriba: el helper no valida nada por si mismo.
    let guardado = tokio::task::spawn_blocking(move || LLAVERO.guardar(&copia))
        .await
        .map_err(|e| RefreshError::new("guardar_token_sonar", e.to_string()))?;

    match guardado {
        Ok(()) => Ok(SonarCredStatus {
            almacen_disponible: true,
            guardadas: true,
            validas: true,
            detalle: format!("{} — guardado en este equipo.", detalle),
            organizacion,
        }),
        // El token sirve pero no se pudo guardar. La sesion actual puede
        // seguir trabajando; lo que se pierde es recordarlo.
        Err(e) => Ok(SonarCredStatus {
            almacen_disponible: false,
            guardadas: false,
            validas: true,
            detalle: format!("Token valido, pero no se pudo guardar: {}", e),
            organizacion,
        }),
    }
}

#[tauri::command]
pub async fn sonar_cred_delete() -> Result<(), RefreshError> {
    tokio::task::spawn_blocking(|| LLAVERO.borrar())
        .await
        .map_err(|e| RefreshError::new("borrar_token_sonar", e.to_string()))?
        .map_err(|e| RefreshError::new("borrar_token_sonar", e))
}
