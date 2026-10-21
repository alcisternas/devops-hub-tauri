// ─────────────────────────────────────────────────────────────────────────
// Credenciales de Jira
//
// Mismo patron que SonarQube: solo hace falta el TOKEN (Bearer, sin correo),
// con su propio item en el llavero (devops-hub-jira).
//
// El token necesita los scopes read:jira-work y write:jira-work.
// ─────────────────────────────────────────────────────────────────────────

use serde::Serialize;

use super::super::eks::config::RefreshError;
use super::super::llavero::Llavero;
use super::client::{mensaje_error, JiraClient, PROYECTO_JIRA};

const SERVICE: &str = "devops-hub-jira";
const KEY_TOKEN: &str = "token";

static LLAVERO: Llavero = Llavero::nuevo(SERVICE, KEY_TOKEN);

#[derive(Debug, Serialize)]
pub struct JiraCredStatus {
    pub almacen_disponible: bool,
    pub guardadas: bool,
    pub validas: bool,
    pub detalle: String,
}

// Lectura para uso interno del modulo.
pub async fn obtener_token() -> Result<String, RefreshError> {
    // spawn_blocking: el helper es sincrono y puede esperar el candado.
    let lectura = tokio::task::spawn_blocking(|| LLAVERO.leer())
        .await
        .map_err(|e| RefreshError::new("leer_token_jira", e.to_string()))?;

    match lectura {
        Ok(Some(t)) => Ok(t),
        Ok(None) => Err(RefreshError::new(
            "leer_token_jira",
            "No hay token de Jira guardado en este equipo.",
        )),
        Err(e) => Err(RefreshError::new("leer_token_jira", e)),
    }
}

// OJO: con un token vencido o revocado Jira NO responde 401. Responde 200 con
// una lista vacia, porque trata la peticion como anonima. Por eso validar el
// token con "la busqueda funciono" es una trampa: el token roto parece sano
// y el modulo diria "no hay tickets pendientes" para siempre.
//
// Lo que si distingue es buscar algo que existe seguro: el proyecto de
// solicitudes tiene tickets, asi que una busqueda vacia sobre el delata un
// token que no sirve o una cuenta sin acceso.
async fn validar(token: &str) -> Result<(bool, String), RefreshError> {
    let client = JiraClient::new(token).await?;
    let jql = format!("project = \"{}\"", PROYECTO_JIRA);
    let (codigo, cuerpo) = client
        .get(
            "/rest/api/3/search/jql",
            &[("jql", jql.as_str()), ("maxResults", "1"), ("fields", "summary")],
        )
        .await?;

    if codigo == 401 || codigo == 403 {
        return Ok((
            false,
            format!(
                "Token rechazado (HTTP {}). Puede estar vencido, revocado o sin los scopes read:jira-work y write:jira-work.",
                codigo
            ),
        ));
    }
    if codigo != 200 {
        return Ok((
            false,
            format!("Respuesta inesperada de Jira (HTTP {}): {}", codigo, mensaje_error(&cuerpo)),
        ));
    }

    let hay_tickets = serde_json::from_str::<serde_json::Value>(&cuerpo)
        .ok()
        .and_then(|v| v.get("issues").and_then(|i| i.as_array()).map(|a| !a.is_empty()))
        .unwrap_or(false);

    if hay_tickets {
        Ok((true, format!("Autenticado en Jira — proyecto {} visible", PROYECTO_JIRA)))
    } else {
        Ok((
            false,
            format!(
                "El token no ve ningun ticket del proyecto {}: probablemente vencio, fue revocado o la cuenta perdio acceso.",
                PROYECTO_JIRA
            ),
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Comandos
// ─────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn jira_cred_status() -> Result<JiraCredStatus, RefreshError> {
    let lectura = tokio::task::spawn_blocking(|| LLAVERO.leer())
        .await
        .map_err(|e| RefreshError::new("leer_token_jira", e.to_string()))?;

    match lectura {
        Ok(None) => Ok(JiraCredStatus {
            almacen_disponible: true,
            guardadas: false,
            validas: false,
            detalle: "No hay token de Jira guardado en este equipo.".into(),
        }),
        Ok(Some(token)) => {
            let (validas, detalle) = validar(&token).await?;
            Ok(JiraCredStatus {
                almacen_disponible: true,
                guardadas: true,
                validas,
                detalle,
            })
        }
        Err(e) => Ok(JiraCredStatus {
            almacen_disponible: false,
            guardadas: false,
            validas: false,
            detalle: format!("No se pudo acceder al almacen de credenciales: {}", e),
        }),
    }
}

#[tauri::command]
pub async fn jira_cred_save(token: String) -> Result<JiraCredStatus, RefreshError> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(RefreshError::new("validar_entrada", "El token es obligatorio."));
    }

    // Se valida antes de guardar: guardar un token que no sirve esconde el
    // error hasta el proximo uso.
    let (validas, detalle) = validar(&token).await?;
    if !validas {
        return Ok(JiraCredStatus {
            almacen_disponible: true,
            guardadas: false,
            validas: false,
            detalle,
        });
    }

    let copia = token.clone();
    // El token ya fue validado arriba: el helper no valida nada por si mismo.
    let guardado = tokio::task::spawn_blocking(move || LLAVERO.guardar(&copia))
        .await
        .map_err(|e| RefreshError::new("guardar_token_jira", e.to_string()))?;

    match guardado {
        Ok(()) => Ok(JiraCredStatus {
            almacen_disponible: true,
            guardadas: true,
            validas: true,
            detalle: format!("{} — guardado en este equipo.", detalle),
        }),
        // El token sirve pero no se pudo guardar. La sesion actual puede
        // seguir trabajando; lo que se pierde es recordarlo.
        Err(e) => Ok(JiraCredStatus {
            almacen_disponible: false,
            guardadas: false,
            validas: true,
            detalle: format!("Token valido, pero no se pudo guardar: {}", e),
        }),
    }
}

#[tauri::command]
pub async fn jira_cred_delete() -> Result<(), RefreshError> {
    tokio::task::spawn_blocking(|| LLAVERO.borrar())
        .await
        .map_err(|e| RefreshError::new("borrar_token_jira", e.to_string()))?
        .map_err(|e| RefreshError::new("borrar_token_jira", e))
}
