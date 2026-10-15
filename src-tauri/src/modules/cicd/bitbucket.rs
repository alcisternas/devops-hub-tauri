// ─────────────────────────────────────────────────────────────────────────
// Subida de los archivos generados a Bitbucket
//
// Reemplaza el copiar y pegar: los mismos archivos que muestra la vista
// previa se escriben en el repositorio con un commit por repositorio.
//
// Reutiliza el cliente HTTP y las credenciales del módulo Bitbucket. No
// hay un servicio nuevo en el llavero: la convención del proyecto es uno
// por sistema externo, no uno por módulo, y Bitbucket ya tiene el suyo.
//
// La operación está partida en dos comandos a propósito. Verificar solo
// lee, así que se puede ejecutar sin riesgo y mostrar qué va a pasar;
// subir escribe, y ocurre recién después de que el usuario confirma.
// ─────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::super::bitbucket::client::{mensaje_error, BBClient};
use super::super::bitbucket::credentials::obtener_credenciales;
use super::super::eks::config::RefreshError;
use super::super::eks::permissions::emit_progress;

const MODULO: &str = "cicd";

// Hasta dónde se prueba al buscar un nombre libre. Veinte es de sobra: si
// hay veinte copias de un archivo, el problema no se arregla con otra.
const MAX_SUFIJOS: u32 = 20;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Accion {
    // Escribir en la ruta original, exista o no.
    Sobrescribir,
    // Buscar un nombre libre agregando .new, .new2, etc.
    Renombrar,
    // No subir este archivo.
    Omitir,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArchivoRepo {
    pub ruta: String,
    pub contenido: String,
    pub accion: Accion,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoArchivos {
    pub repo: String,
    pub archivos: Vec<ArchivoRepo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificacionRepo {
    pub repo: String,
    pub repo_existe: bool,
    pub rama_existe: bool,
    // Rutas que ya tienen un archivo en la rama destino.
    pub conflictos: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoRepo {
    pub repo: String,
    pub ok: bool,
    // Enlace al commit cuando salió bien.
    pub url: Option<String>,
    pub subidos: Vec<String>,
    // (ruta pedida, ruta efectiva) de los archivos renombrados.
    pub renombrados: Vec<(String, String)>,
    pub omitidos: Vec<String>,
    pub error: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────
// Lecturas
// ─────────────────────────────────────────────────────────────────────────

async fn repo_existe(client: &BBClient, ws: &str, repo: &str) -> Result<bool, RefreshError> {
    let (codigo, _) = client.get(&format!("/repositories/{}/{}", ws, repo)).await?;
    Ok(codigo == 200)
}

async fn rama_existe(
    client: &BBClient,
    ws: &str,
    repo: &str,
    rama: &str,
) -> Result<bool, RefreshError> {
    let (codigo, _) = client
        .get(&format!("/repositories/{}/{}/refs/branches/{}", ws, repo, rama))
        .await?;
    Ok(codigo == 200)
}

async fn archivo_existe(
    client: &BBClient,
    ws: &str,
    repo: &str,
    rama: &str,
    ruta: &str,
) -> Result<bool, RefreshError> {
    let (codigo, _) = client
        .get(&format!("/repositories/{}/{}/src/{}/{}", ws, repo, rama, ruta))
        .await?;
    Ok(codigo == 200)
}

// Primer nombre libre de la serie ruta.new, ruta.new2, ruta.new3...
//
// Si los veinte están ocupados devuelve None y ese archivo se omite: es
// preferible dejarlo fuera y avisar que escribir encima de algo.
async fn nombre_libre(
    client: &BBClient,
    ws: &str,
    repo: &str,
    rama: &str,
    ruta: &str,
) -> Result<Option<String>, RefreshError> {
    for i in 1..=MAX_SUFIJOS {
        let candidato = if i == 1 {
            format!("{}.new", ruta)
        } else {
            format!("{}.new{}", ruta, i)
        };
        if !archivo_existe(client, ws, repo, rama, &candidato).await? {
            return Ok(Some(candidato));
        }
    }
    Ok(None)
}

// ─────────────────────────────────────────────────────────────────────────
// Comandos
// ─────────────────────────────────────────────────────────────────────────

// Verificación previa. No escribe nada: revisa que el repositorio y la
// rama existan y qué archivos ya están, para poder mostrar la lista
// completa de conflictos y decidir una sola vez por cada uno.
//
// Un repositorio que falla no detiene a los demás; solo un corte de red
// aborta todo, porque en ese caso los siguientes fallarían igual.
#[tauri::command]
pub async fn cicd_bb_verificar(
    app: AppHandle,
    workspace: String,
    rama: String,
    repos: Vec<RepoArchivos>,
) -> Result<Vec<VerificacionRepo>, RefreshError> {
    let (email, token) = obtener_credenciales().await?;
    let client = BBClient::new(&email, &token)?;

    let total = repos.len() as u32;
    let mut salida = Vec::new();

    for (i, item) in repos.iter().enumerate() {
        let n = (i + 1) as u32;
        emit_progress(MODULO, &app, "Verificando repositorio", &item.repo, n, total, "running");

        if !repo_existe(&client, &workspace, &item.repo).await? {
            emit_progress(MODULO, &app, "El repositorio no existe", &item.repo, n, total, "error");
            salida.push(VerificacionRepo {
                repo: item.repo.clone(),
                repo_existe: false,
                rama_existe: false,
                conflictos: vec![],
                error: Some("El repositorio no existe en este workspace.".to_string()),
            });
            continue;
        }

        if !rama_existe(&client, &workspace, &item.repo, &rama).await? {
            emit_progress(
                MODULO,
                &app,
                &format!("Sin la rama {}", rama),
                &item.repo,
                n,
                total,
                "error",
            );
            salida.push(VerificacionRepo {
                repo: item.repo.clone(),
                repo_existe: true,
                rama_existe: false,
                conflictos: vec![],
                error: Some(format!(
                    "El repositorio no tiene la rama {}. Hay que crearla antes de subir.",
                    rama
                )),
            });
            continue;
        }

        let mut conflictos = Vec::new();
        for archivo in &item.archivos {
            if archivo_existe(&client, &workspace, &item.repo, &rama, &archivo.ruta).await? {
                conflictos.push(archivo.ruta.clone());
            }
        }

        emit_progress(
            MODULO,
            &app,
            &if conflictos.is_empty() {
                "Sin conflictos".to_string()
            } else {
                format!("{} archivos ya existen", conflictos.len())
            },
            &item.repo,
            n,
            total,
            "ok",
        );

        salida.push(VerificacionRepo {
            repo: item.repo.clone(),
            repo_existe: true,
            rama_existe: true,
            conflictos,
            error: None,
        });
    }

    Ok(salida)
}

// Subida. Un commit por repositorio con todos sus archivos, igual que el
// flujo manual (un `git add .` y un `git commit`). Uno por archivo daría
// ocho commits por repositorio en Kubernetes con tres ambientes.
//
// Las decisiones de conflicto llegan resueltas desde la pantalla; acá solo
// se aplica la que corresponda a cada archivo.
#[tauri::command]
pub async fn cicd_bb_subir(
    app: AppHandle,
    workspace: String,
    rama: String,
    mensaje: String,
    repos: Vec<RepoArchivos>,
) -> Result<Vec<ResultadoRepo>, RefreshError> {
    let (email, token) = obtener_credenciales().await?;
    let client = BBClient::new(&email, &token)?;

    let total = repos.len() as u32;
    let mut salida = Vec::new();

    for (i, item) in repos.iter().enumerate() {
        let n = (i + 1) as u32;
        emit_progress(MODULO, &app, "Subiendo archivos", &item.repo, n, total, "running");

        // Resolución de rutas: los renombrados necesitan una consulta más
        // para encontrar el primer nombre libre.
        let mut campos: Vec<(String, String)> = Vec::new();
        let mut subidos = Vec::new();
        let mut renombrados = Vec::new();
        let mut omitidos = Vec::new();

        for archivo in &item.archivos {
            match archivo.accion {
                Accion::Omitir => omitidos.push(archivo.ruta.clone()),
                Accion::Sobrescribir => {
                    campos.push((archivo.ruta.clone(), archivo.contenido.clone()));
                    subidos.push(archivo.ruta.clone());
                }
                Accion::Renombrar => {
                    match nombre_libre(&client, &workspace, &item.repo, &rama, &archivo.ruta).await? {
                        Some(ruta) => {
                            campos.push((ruta.clone(), archivo.contenido.clone()));
                            renombrados.push((archivo.ruta.clone(), ruta));
                        }
                        None => omitidos.push(archivo.ruta.clone()),
                    }
                }
            }
        }

        if campos.is_empty() {
            emit_progress(MODULO, &app, "Nada por subir", &item.repo, n, total, "ok");
            salida.push(ResultadoRepo {
                repo: item.repo.clone(),
                ok: true,
                url: None,
                subidos,
                renombrados,
                omitidos,
                error: None,
            });
            continue;
        }

        // El mensaje y la rama viajan como campos del mismo formulario que
        // los archivos. Sin "branch", Bitbucket escribe en la rama
        // principal del repositorio, que no es a donde va esto.
        let mut partes: Vec<(&str, &str)> = vec![("message", mensaje.as_str()), ("branch", rama.as_str())];
        for (ruta, contenido) in &campos {
            partes.push((ruta.as_str(), contenido.as_str()));
        }

        let (codigo, cuerpo) = client
            .post_multipart(&format!("/repositories/{}/{}/src", workspace, item.repo), &partes)
            .await?;

        // Un commit correcto responde 201. Bitbucket también usa 200 en
        // algunos casos, así que se aceptan los dos.
        if codigo == 201 || codigo == 200 {
            emit_progress(
                MODULO,
                &app,
                &format!("{} archivos subidos", campos.len()),
                &item.repo,
                n,
                total,
                "ok",
            );
            salida.push(ResultadoRepo {
                repo: item.repo.clone(),
                ok: true,
                url: Some(format!(
                    "https://bitbucket.org/{}/{}/src/{}",
                    workspace, item.repo, rama
                )),
                subidos,
                renombrados,
                omitidos,
                error: None,
            });
        } else {
            let detalle = mensaje_error(&cuerpo);
            emit_progress(
                MODULO,
                &app,
                &format!("Error {}: {}", codigo, detalle),
                &item.repo,
                n,
                total,
                "error",
            );
            salida.push(ResultadoRepo {
                repo: item.repo.clone(),
                ok: false,
                url: None,
                subidos: vec![],
                renombrados: vec![],
                omitidos,
                error: Some(format!("HTTP {} — {}", codigo, detalle)),
            });
        }
    }

    Ok(salida)
}
