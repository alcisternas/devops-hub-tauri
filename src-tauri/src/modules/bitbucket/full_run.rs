// ─────────────────────────────────────────────────────────────────────────
// Orquestacion: recorre la lista de repositorios aplicando las operaciones
// de repos.rs en orden, y va informando el avance a la consola global.
//
// Criterio heredado de la herramienta anterior: si un repositorio falla o ya
// existia, se omite y se sigue con el siguiente. Nunca se aborta el lote
// completo por uno malo — con 10 repositorios, que el numero 3 falle no es
// razon para no crear los 7 restantes.
// ─────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::super::eks::config::RefreshError;
use super::super::eks::permissions::emit_progress;
use super::client::BBClient;
use super::credentials;
use super::repos::{self, EstadoRepo};

// Tag con el que la consola global identifica las lineas de este modulo.
const MODULO: &str = "bitbucket";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRun {
    pub workspace: String,
    pub project_key: String,
    pub repos: Vec<String>,
    pub pais: String,
    pub activar_pipelines: bool,
    pub crear_ramas: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoRepo {
    pub repo: String,
    // "creado" | "ya_existia" | "error"
    pub estado: String,
    pub url: Option<String>,
    pub detalle: String,
    // Advertencias de pasos que fallaron sin impedir la creacion del repo
    // (por ejemplo Pipelines). El repo existe, pero algo quedo pendiente.
    pub advertencias: Vec<String>,
    // Key que tendra que llevar el proyecto en SonarQube, para la tabla del
    // paso manual posterior.
    pub sonar_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumenRun {
    pub total: u32,
    pub creados: u32,
    pub omitidos: u32,
    pub con_error: u32,
    pub rama_desarrollo: String,
    pub resultados: Vec<ResultadoRepo>,
}

#[tauri::command]
pub async fn bitbucket_crear_repos(
    app: AppHandle,
    config: ConfigRun,
) -> Result<ResumenRun, RefreshError> {
    if config.repos.is_empty() {
        return Err(RefreshError::new(
            "validar_entrada",
            "La lista de repositorios esta vacia.",
        ));
    }

    // Las credenciales se leen y validan UNA vez antes de tocar nada. Si el
    // token no sirve, es mejor saberlo ahora que a mitad del lote con
    // repositorios ya creados.
    let (email, token) = credentials::obtener_credenciales().await?;
    let client = BBClient::new(&email, &token)?;

    let rama_dev = repos::rama_desarrollo(&config.pais);
    let total = config.repos.len() as u32;
    let mut resultados: Vec<ResultadoRepo> = Vec::new();

    for (i, repo) in config.repos.iter().enumerate() {
        let n = (i + 1) as u32;
        let sonar_key = format!("{}_{}", config.project_key, repo);

        emit_progress(MODULO, &app, "Verificando", repo, n, total, "running");

        // ── Paso 1: ¿ya existe?
        match repos::verificar(&client, &config.workspace, repo).await {
            Ok(EstadoRepo::Existe(url)) => {
                emit_progress(MODULO, &app, "Ya existia — omitido", repo, n, total, "error");
                resultados.push(ResultadoRepo {
                    repo: repo.clone(),
                    estado: "ya_existia".into(),
                    url: Some(url),
                    detalle: "El repositorio ya existe en el workspace. Se omitio.".into(),
                    advertencias: vec![],
                    sonar_key,
                });
                continue;
            }
            Ok(EstadoRepo::ErrorAlVerificar(msg)) => {
                emit_progress(MODULO, &app, "Error al verificar", repo, n, total, "error");
                resultados.push(ResultadoRepo {
                    repo: repo.clone(),
                    estado: "error".into(),
                    url: None,
                    detalle: msg,
                    advertencias: vec![],
                    sonar_key,
                });
                continue;
            }
            // Falla de red: aca si se corta el lote. Si no hay conexion, los
            // repositorios siguientes van a fallar igual y seguir solo
            // llenaria el resumen de errores identicos.
            Err(e) => return Err(e),
            Ok(EstadoRepo::NoExiste) => {}
        }

        // ── Paso 2: crear
        emit_progress(MODULO, &app, "Creando repositorio", repo, n, total, "running");
        let url = match repos::crear(&client, &config.workspace, repo, &config.project_key).await {
            Ok(url) => url,
            Err(e) => {
                emit_progress(MODULO, &app, "Error al crear", repo, n, total, "error");
                resultados.push(ResultadoRepo {
                    repo: repo.clone(),
                    estado: "error".into(),
                    url: None,
                    detalle: e.mensaje,
                    advertencias: vec![],
                    sonar_key,
                });
                continue;
            }
        };

        // ── Paso 3: commit inicial
        emit_progress(MODULO, &app, "Commit inicial", repo, n, total, "running");
        if let Err(e) = repos::commit_inicial(&client, &config.workspace, repo).await {
            // El repositorio quedo creado pero vacio: sin commit no se pueden
            // crear las ramas, asi que no tiene sentido intentar los pasos
            // siguientes. Se reporta como error con la URL, para que se pueda
            // revisar o borrar a mano.
            emit_progress(MODULO, &app, "Error en commit inicial", repo, n, total, "error");
            resultados.push(ResultadoRepo {
                repo: repo.clone(),
                estado: "error".into(),
                url: Some(url),
                detalle: format!(
                    "Repositorio creado pero sin commit inicial: {}. Sin commit no se pueden crear las ramas.",
                    e.mensaje
                ),
                advertencias: vec![],
                sonar_key,
            });
            continue;
        }

        // ── Pasos 4 y 5: desde aca, lo que falle es una advertencia. El
        // repositorio ya existe y es utilizable; esto se puede completar a
        // mano sin rehacer nada.
        let mut advertencias: Vec<String> = Vec::new();

        if config.activar_pipelines {
            emit_progress(MODULO, &app, "Activando Pipelines", repo, n, total, "running");
            if let Err(e) = repos::activar_pipelines(&client, &config.workspace, repo).await {
                advertencias.push(format!("Pipelines: {}", e.mensaje));
            }
        }

        if config.crear_ramas {
            for rama in [rama_dev, "release"] {
                emit_progress(
                    MODULO,
                    &app,
                    &format!("Creando rama {}", rama),
                    repo,
                    n,
                    total,
                    "running",
                );
                if let Err(e) = repos::crear_rama(&client, &config.workspace, repo, rama).await {
                    advertencias.push(format!("Rama {}", e.mensaje));
                }
            }
        }

        let estado_evento = if advertencias.is_empty() { "ok" } else { "error" };
        emit_progress(MODULO, &app, "Listo", repo, n, total, estado_evento);

        resultados.push(ResultadoRepo {
            repo: repo.clone(),
            estado: "creado".into(),
            url: Some(url),
            detalle: if advertencias.is_empty() {
                "Creado correctamente.".into()
            } else {
                "Creado con pasos pendientes.".into()
            },
            advertencias,
            sonar_key,
        });
    }

    let creados = resultados.iter().filter(|r| r.estado == "creado").count() as u32;
    let omitidos = resultados.iter().filter(|r| r.estado == "ya_existia").count() as u32;
    let con_error = resultados.iter().filter(|r| r.estado == "error").count() as u32;

    Ok(ResumenRun {
        total,
        creados,
        omitidos,
        con_error,
        rama_desarrollo: rama_dev.to_string(),
        resultados,
    })
}
