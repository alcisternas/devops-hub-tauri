// ─────────────────────────────────────────────────────────────────────────
// Crear el proyecto en SonarQube Cloud y vincularlo al repo de Bitbucket.
//
// El flujo fue verificado contra la instancia real del banco antes de
// escribir esto (sondeo + prueba de escritura):
//
//   1. POST api/projects/create      → el proyecto nace CON la key correcta,
//                                       lo que elimina el paso manual de
//                                       "Update key". La respuesta incluye
//                                       project.uuid, que es el id interno:
//                                       se aprovecha y se ahorra una llamada.
//   2. POST dop-translation/project-bindings con {projectId, repositoryId}
//                                     → HTTP 201 y el proyecto queda con
//                                       alm = bitbucketcloud, igual que uno
//                                       importado desde la interfaz.
//
// repositoryId es el UUID de Bitbucket CON LLAVES, ej "{77e7e6b1-...}".
// Pasar el nombre del repo devuelve 200 con lista vacia: no falla, pero no
// vincula nada. Por eso el UUID se toma de la respuesta de Bitbucket al
// crear el repositorio, no se construye.
// ─────────────────────────────────────────────────────────────────────────

use serde_json::json;

use super::super::eks::config::RefreshError;
use super::client::{campo_str, mensaje_error, SonarClient, ORG};

pub struct ResultadoSonar {
    // "creado" | "ya_existia" | "error"
    pub estado: String,
    pub detalle: String,
    pub bindeado: bool,
}

// La key del proyecto en Sonar es siempre PROYECTO_repositorio.
pub fn sonar_key(project_key: &str, repo: &str) -> String {
    format!("{}_{}", project_key, repo)
}

// Devuelve el id interno si el proyecto ya existe, o None si no existe.
// Se usa navigation/component porque resuelve las dos cosas a la vez:
// existencia e id. (No figura en la lista publica de endpoints, pero
// responde: esa lista no es exhaustiva.)
async fn buscar_existente(
    client: &SonarClient,
    key: &str,
) -> Result<Option<String>, RefreshError> {
    let (codigo, cuerpo) = client
        .get_v1(&format!(
            "/api/navigation/component?component={}&organization={}",
            urlencode(key),
            ORG
        ))
        .await?;

    match codigo {
        200 => Ok(campo_str(&cuerpo, &["id"])),
        404 => Ok(None),
        _ => Err(RefreshError::new(
            "buscar_proyecto_sonar",
            format!("HTTP {}: {}", codigo, mensaje_error(&cuerpo)),
        )),
    }
}

async fn crear_proyecto(client: &SonarClient, key: &str) -> Result<String, RefreshError> {
    // El nombre visible es el mismo texto que la key, como en los proyectos
    // que ya existen en la organizacion.
    let (codigo, cuerpo) = client
        .post_v1_form(
            "/api/projects/create",
            &[("organization", ORG), ("project", key), ("name", key)],
        )
        .await?;

    if codigo == 200 || codigo == 201 {
        return campo_str(&cuerpo, &["project", "uuid"]).ok_or_else(|| {
            RefreshError::new(
                "crear_proyecto_sonar",
                "El proyecto se creo pero la respuesta no trajo su id interno.",
            )
        });
    }

    Err(RefreshError::new(
        "crear_proyecto_sonar",
        format!("HTTP {}: {}", codigo, mensaje_error(&cuerpo)),
    ))
}

async fn tiene_binding(client: &SonarClient, project_id: &str) -> Result<bool, RefreshError> {
    let (codigo, cuerpo) = client
        .get_v2(&format!(
            "/dop-translation/project-bindings?projectId={}",
            urlencode(project_id)
        ))
        .await?;

    if codigo != 200 {
        // No poder consultarlo no es motivo para abortar: se intenta crear
        // el binding igual y ahi se vera si ya existia.
        return Ok(false);
    }

    let total = serde_json::from_str::<serde_json::Value>(&cuerpo)
        .ok()
        .and_then(|v| v.get("page")?.get("total")?.as_u64())
        .unwrap_or(0);

    Ok(total > 0)
}

async fn crear_binding(
    client: &SonarClient,
    project_id: &str,
    repo_uuid: &str,
) -> Result<(), RefreshError> {
    // Sin devOpsPlatform: con el UUID alcanza, SonarQube deduce la plataforma
    // y el slug. Verificado en la prueba real (HTTP 201).
    let (codigo, cuerpo) = client
        .post_v2_json(
            "/dop-translation/project-bindings",
            json!({ "projectId": project_id, "repositoryId": repo_uuid }),
        )
        .await?;

    match codigo {
        200 | 201 | 204 => Ok(()),
        409 => Ok(()), // ya estaba vinculado: el estado final es el deseado
        _ => Err(RefreshError::new(
            "vincular_bitbucket",
            format!("HTTP {}: {}", codigo, mensaje_error(&cuerpo)),
        )),
    }
}

// Orquesta los pasos para un repositorio. Nunca devuelve Err por un fallo de
// Sonar: el repositorio de Bitbucket ya existe y eso no se invalida porque
// el proyecto de analisis no se haya podido crear. Se reporta y se sigue.
pub async fn crear_y_vincular(
    client: &SonarClient,
    project_key: &str,
    repo: &str,
    repo_uuid: &str,
) -> ResultadoSonar {
    let key = sonar_key(project_key, repo);

    let (project_id, ya_existia) = match buscar_existente(client, &key).await {
        Ok(Some(id)) => (id, true),
        Ok(None) => match crear_proyecto(client, &key).await {
            Ok(id) => (id, false),
            Err(e) => {
                return ResultadoSonar {
                    estado: "error".into(),
                    detalle: e.mensaje,
                    bindeado: false,
                }
            }
        },
        Err(e) => {
            return ResultadoSonar {
                estado: "error".into(),
                detalle: e.mensaje,
                bindeado: false,
            }
        }
    };

    // Un proyecto que ya existia puede estar sin vincular. Se revisa y, si
    // le falta, se vincula: es justamente el caso que habria que arreglar.
    if ya_existia && tiene_binding(client, &project_id).await.unwrap_or(false) {
        return ResultadoSonar {
            estado: "ya_existia".into(),
            detalle: format!("{} ya existia y ya estaba vinculado.", key),
            bindeado: true,
        };
    }

    match crear_binding(client, &project_id, repo_uuid).await {
        Ok(()) => ResultadoSonar {
            estado: if ya_existia { "ya_existia".into() } else { "creado".into() },
            detalle: if ya_existia {
                format!("{} ya existia; se vinculo al repositorio.", key)
            } else {
                format!("{} creado y vinculado.", key)
            },
            bindeado: true,
        },
        Err(e) => ResultadoSonar {
            // El proyecto existe pero sin vinculo: es utilizable, aunque sin
            // decoracion de pull requests hasta que se vincule a mano.
            estado: "error".into(),
            detalle: format!("Proyecto {} disponible, pero sin vincular: {}", key, e.mensaje),
            bindeado: false,
        },
    }
}

// Codificacion minima para los parametros que se arman aca. Las keys de
// proyecto solo admiten letras, digitos y - _ . : asi que alcanza con
// cubrir los dos caracteres que tendrian significado en una URL.
fn urlencode(s: &str) -> String {
    s.replace('%', "%25").replace('&', "%26").replace('+', "%2B").replace(' ', "%20")
}
