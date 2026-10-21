// ─────────────────────────────────────────────────────────────────────────
// Procesar un ticket de Jira de punta a punta
//
// Orden y criterio, heredados de la automatizacion que corria por terminal:
//
//   1. Releer y validar el ticket. La lista que ve el usuario puede tener
//      minutos; si cambio, mejor enterarse ahora.
//   2. Verificar en Bitbucket que el proyecto existe y que NINGUN repo del
//      ticket existe todavia. Si alguno existe se aborta el ticket completo:
//      no se crea nada a medias.
//   3. Crear con el motor del modulo Bitbucket (bitbucket_crear_repos): commit
//      inicial, Pipelines, ramas y SonarQube. Para repos de Base de Datos se
//      piden SIN Pipelines ni SonarQube.
//   4. Solo si TODOS los repos quedaron bien: etiquetas y cierre del ticket.
//      Con un fallo no se toca el ticket; se informa y se completa a mano.
//   5. La evidencia (comentario + imagen) es un comando aparte porque la
//      imagen la dibuja la interfaz con canvas, que ya trae sus fuentes.
// ─────────────────────────────────────────────────────────────────────────

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use super::super::bitbucket::client::{mensaje_error as bb_error, url_repo, BBClient};
use super::super::bitbucket::credentials as bb_credentials;
use super::super::bitbucket::full_run::{bitbucket_crear_repos, ConfigRun, ResumenRun};
use super::super::bitbucket::repos::{self as bb_repos, EstadoRepo};
use super::super::eks::config::RefreshError;
use super::super::eks::permissions::emit_progress;
use super::client::{mensaje_error, JiraClient};
use super::credentials;
use super::tickets::{etiqueta_pais, leer_ticket, pais_a_codigo, TicketPendiente, ESTADOS_PROCESABLES};

// Tag con el que la consola global identifica estas lineas: se reutiliza el
// del modulo Bitbucket, que es de donde cuelga esta pantalla.
const MODULO: &str = "bitbucket";
const PASOS: u32 = 5;

// Etiquetas que ya existen en Jira. Jira crea una etiqueta nueva con cualquier
// texto distinto, asi que solo se agregan estas (la de pais va sin tilde).
const ETIQUETAS_FIJAS: [&str; 2] = ["ACN", "DEVOPS-CHAPTER"];

// Resolucion con la que el equipo cierra estos tickets.
const RESOLUCION_CIERRE: &str = "Listo";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpcionesProceso {
    pub workspace: String,
    // Importar los repos en SonarQube (se ignora en repos de Base de Datos).
    pub sonar: bool,
    pub etiquetas: bool,
    pub cerrar: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenciaRepo {
    pub nombre: String,
    pub url: String,
    pub ramas: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultadoTicket {
    pub clave: String,
    pub es_bd: bool,
    pub resumen: ResumenRun,
    // true si se intentaron etiquetas/cierre; false si algun repo no quedo bien.
    pub acciones_jira: bool,
    pub motivo_sin_acciones: Option<String>,
    pub etiquetas_agregadas: Vec<String>,
    pub etiquetas_error: Option<String>,
    // "no_pedido" | "cerrado" | "error" | "omitido"
    pub cierre_estado: String,
    pub cierre_detalle: String,
    // Datos para la evidencia (vacio si no hubo repos creados).
    pub proyecto_etiqueta: String,
    pub ramas: Vec<String>,
    pub evidencia: Vec<EvidenciaRepo>,
}

// ─────────────────────────────────────────────────────────────────────────
// Reglas puras (con pruebas)
// ─────────────────────────────────────────────────────────────────────────

// Transiciones segun el estado actual. Se buscan por NOMBRE y no por id: los
// ids son internos del flujo de trabajo y el nombre es lo que ve el equipo.
pub fn pasos_cierre(estado: &str) -> Vec<&'static str> {
    match estado {
        "Asignado" => vec!["Comenzar progreso", "Marcar como hecho"],
        "Trabajo en curso" => vec!["Marcar como hecho"],
        _ => vec![],
    }
}

// Etiquetas que faltan en el ticket: nunca se repite ni se quita una.
pub fn etiquetas_faltantes(codigo_pais: &str, actuales: &[String]) -> Vec<String> {
    let mut deseadas: Vec<String> = ETIQUETAS_FIJAS.iter().map(|s| s.to_string()).collect();
    deseadas.push(etiqueta_pais(codigo_pais).to_string());
    deseadas.into_iter().filter(|e| !actuales.contains(e)).collect()
}

// "master, development y release"
fn unir_es(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => format!("{} y {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

// Cuerpo del comentario al solicitante, en wiki markup de Jira (API v2). Un
// solo repo va con la URL sola; varios, en lista con su nombre.
pub fn cuerpo_comentario(
    evidencia: &[EvidenciaRepo],
    proyecto: &str,
    ramas: &[String],
    imagen: Option<&str>,
) -> String {
    let mut partes: Vec<String> = Vec::new();

    if evidencia.len() == 1 {
        partes.push(format!(
            "Se crea el repositorio solicitado en el proyecto {} de Bitbucket.",
            proyecto
        ));
        partes.push(String::new());
        partes.push(format!("[{}]", evidencia[0].url));
    } else {
        partes.push(format!(
            "Se crean los repositorios solicitados en el proyecto {} de Bitbucket.",
            proyecto
        ));
        partes.push(String::new());
        partes.push("*Repositorios*".to_string());
        for e in evidencia {
            // \\ es el salto de linea dentro de un item de lista en wiki markup.
            partes.push(format!("* {} \\\\ [{}]", e.nombre, e.url));
        }
    }

    partes.push(String::new());
    partes.push(format!("Ramas: {}", unir_es(ramas)));
    if let Some(nombre) = imagen {
        partes.push(String::new());
        partes.push(format!("!{}!", nombre));
    }
    partes.join("\n")
}

// Codifica todo salvo los caracteres sin significado especial en una URL.
fn urlencode(s: &str) -> String {
    let mut salida = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => salida.push(b as char),
            otro => salida.push_str(&format!("%{:02X}", otro)),
        }
    }
    salida
}

// Master primero, luego la rama de desarrollo y por ultimo el resto.
fn ordenar_ramas(ramas: &mut [String]) {
    let peso = |r: &str| match r {
        "master" => 0,
        "development" | "develop" => 1,
        "release" => 2,
        _ => 3,
    };
    ramas.sort_by(|a, b| peso(a).cmp(&peso(b)).then(a.cmp(b)));
}

// ─────────────────────────────────────────────────────────────────────────
// Pasos contra Bitbucket
// ─────────────────────────────────────────────────────────────────────────

// Devuelve la clave REAL del proyecto en Bitbucket. Se busca por clave exacta
// y, si no existe, por nombre exacto. Nunca por parecido: un repo en el
// proyecto equivocado es peor que un error claro.
async fn verificar_proyecto(
    bb: &BBClient,
    workspace: &str,
    clave: &str,
    nombre: Option<&str>,
) -> Result<String, RefreshError> {
    let (codigo, cuerpo) = bb
        .get(&format!("/workspaces/{}/projects/{}", workspace, clave))
        .await?;
    match codigo {
        200 => return Ok(clave.to_string()),
        404 => {}
        otro => {
            return Err(RefreshError::new(
                "verificar_proyecto",
                format!("HTTP {}: {}", otro, bb_error(&cuerpo)),
            ))
        }
    }

    if let Some(n) = nombre {
        let consulta = urlencode(&format!("name=\"{}\"", n));
        let (codigo, cuerpo) = bb
            .get(&format!("/workspaces/{}/projects?q={}&pagelen=10", workspace, consulta))
            .await?;
        if codigo == 200 {
            let v: Value = serde_json::from_str(&cuerpo).unwrap_or(Value::Null);
            if let Some(lista) = v.get("values").and_then(|x| x.as_array()) {
                if lista.len() == 1 {
                    if let Some(k) = lista[0].get("key").and_then(|k| k.as_str()) {
                        return Ok(k.to_string());
                    }
                }
            }
        }
    }

    Err(RefreshError::new(
        "verificar_proyecto",
        format!(
            "El proyecto '{}' ('{}') no existe en el workspace '{}' de Bitbucket.",
            clave,
            nombre.unwrap_or(""),
            workspace
        ),
    ))
}

// Falla si ALGUN repo ya existe: el ticket se procesa completo o no se toca.
async fn verificar_repos_libres(
    bb: &BBClient,
    workspace: &str,
    repos: &[String],
) -> Result<(), RefreshError> {
    let mut existentes: Vec<String> = Vec::new();
    for r in repos {
        match bb_repos::verificar(bb, workspace, r).await? {
            EstadoRepo::NoExiste => {}
            EstadoRepo::Existe(_) => existentes.push(r.clone()),
            EstadoRepo::ErrorAlVerificar(m) => {
                return Err(RefreshError::new("verificar_repos", format!("{}: {}", r, m)))
            }
        }
    }
    if existentes.is_empty() {
        Ok(())
    } else {
        Err(RefreshError::new(
            "repos_existentes",
            format!(
                "Estos repositorios ya existen en Bitbucket: {}. No se creo ninguno de este ticket.",
                existentes.join(", ")
            ),
        ))
    }
}

// Nombre, URL y ramas reales de cada repo creado, leidos de Bitbucket: la
// evidencia muestra lo que existe, no lo que se supone que se creo.
async fn recolectar_evidencia(
    bb: &BBClient,
    workspace: &str,
    repos: &[String],
) -> Result<Vec<EvidenciaRepo>, RefreshError> {
    let mut salida: Vec<EvidenciaRepo> = Vec::new();
    for r in repos {
        let (codigo, cuerpo) = bb.get(&format!("/repositories/{}/{}", workspace, r)).await?;
        if codigo != 200 {
            return Err(RefreshError::new(
                "leer_evidencia",
                format!("{}: HTTP {}: {}", r, codigo, bb_error(&cuerpo)),
            ));
        }
        let v: Value = serde_json::from_str(&cuerpo).unwrap_or(Value::Null);
        let nombre = v.get("name").and_then(|n| n.as_str()).unwrap_or(r).to_string();
        let url = url_repo(&cuerpo)
            .unwrap_or_else(|| format!("https://bitbucket.org/{}/{}", workspace, r));

        let (codigo, cuerpo) = bb
            .get(&format!("/repositories/{}/{}/refs/branches?pagelen=50", workspace, r))
            .await?;
        if codigo != 200 {
            return Err(RefreshError::new(
                "leer_evidencia",
                format!("{}: ramas HTTP {}: {}", r, codigo, bb_error(&cuerpo)),
            ));
        }
        let v: Value = serde_json::from_str(&cuerpo).unwrap_or(Value::Null);
        let mut ramas: Vec<String> = v
            .get("values")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|b| b.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        ordenar_ramas(&mut ramas);

        salida.push(EvidenciaRepo { nombre, url, ramas });
    }
    Ok(salida)
}

// ─────────────────────────────────────────────────────────────────────────
// Pasos contra Jira
// ─────────────────────────────────────────────────────────────────────────

async fn agregar_etiquetas(
    jira: &JiraClient,
    clave: &str,
    etiquetas: &[String],
) -> Result<(), RefreshError> {
    let operaciones: Vec<Value> = etiquetas.iter().map(|e| json!({ "add": e })).collect();
    let (codigo, cuerpo) = jira
        .put_json(
            &format!("/rest/api/3/issue/{}", clave),
            json!({ "update": { "labels": operaciones } }),
        )
        .await?;
    if codigo == 200 || codigo == 204 {
        Ok(())
    } else {
        Err(RefreshError::new(
            "agregar_etiquetas",
            format!("HTTP {}: {}", codigo, mensaje_error(&cuerpo)),
        ))
    }
}

async fn transicionar(jira: &JiraClient, clave: &str, nombre: &str) -> Result<(), RefreshError> {
    let (codigo, cuerpo) = jira
        .get(&format!("/rest/api/3/issue/{}/transitions", clave), &[])
        .await?;
    if codigo != 200 {
        return Err(RefreshError::new(
            "cerrar_ticket",
            format!("HTTP {}: {}", codigo, mensaje_error(&cuerpo)),
        ));
    }
    let v: Value = serde_json::from_str(&cuerpo).unwrap_or(Value::Null);
    let disponibles: Vec<(String, String)> = v
        .get("transitions")
        .and_then(|t| t.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    Some((
                        t.get("id")?.as_str()?.to_string(),
                        t.get("name")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();

    let id = match disponibles.iter().find(|(_, n)| n == nombre) {
        Some((id, _)) => id.clone(),
        None => {
            return Err(RefreshError::new(
                "cerrar_ticket",
                format!(
                    "La transicion '{}' no esta disponible. Disponibles: {}.",
                    nombre,
                    disponibles.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>().join(", ")
                ),
            ))
        }
    };

    let mut payload = json!({ "transition": { "id": id } });
    if nombre == "Marcar como hecho" {
        payload["fields"] = json!({ "resolution": { "name": RESOLUCION_CIERRE } });
    }

    let (codigo, cuerpo) = jira
        .post_json(&format!("/rest/api/3/issue/{}/transitions", clave), payload)
        .await?;
    if codigo == 200 || codigo == 204 {
        Ok(())
    } else {
        Err(RefreshError::new(
            "cerrar_ticket",
            format!("'{}' HTTP {}: {}", nombre, codigo, mensaje_error(&cuerpo)),
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Comandos
// ─────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn jira_procesar_ticket(
    app: AppHandle,
    clave: String,
    opciones: OpcionesProceso,
) -> Result<ResultadoTicket, RefreshError> {
    // ── 1. Releer y validar
    emit_progress(MODULO, &app, "Jira: leyendo ticket", &clave, 1, PASOS, "running");
    let token = credentials::obtener_token().await?;
    let jira = JiraClient::new(&token).await?;
    let ticket: TicketPendiente = leer_ticket(&jira, &clave).await?;

    if !ticket.errores.is_empty() {
        return Err(RefreshError::new("validar_ticket", ticket.errores.join(" | ")));
    }
    if !ESTADOS_PROCESABLES.contains(&ticket.estado.as_str()) {
        return Err(RefreshError::new(
            "ticket_no_procesable",
            format!("El ticket esta en '{}'; solo se procesan los que estan en Asignado o Trabajo en curso.", ticket.estado),
        ));
    }
    let codigo_pais = pais_a_codigo(ticket.pais.as_deref())
        .ok_or_else(|| RefreshError::new("validar_ticket", "Pais no reconocido."))?;
    let clave_proyecto = ticket
        .project_key
        .clone()
        .ok_or_else(|| RefreshError::new("validar_ticket", "El ticket no trae la clave del proyecto."))?;

    // ── 2. Verificar en Bitbucket
    emit_progress(MODULO, &app, "Jira: verificando Bitbucket", &clave, 2, PASOS, "running");
    let (email, token_bb) = bb_credentials::obtener_credenciales().await?;
    let bb = BBClient::new(&email, &token_bb)?;
    let proyecto = verificar_proyecto(
        &bb,
        &opciones.workspace,
        &clave_proyecto,
        ticket.project_name.as_deref(),
    )
    .await?;
    verificar_repos_libres(&bb, &opciones.workspace, &ticket.repos).await?;

    // ── 3. Crear con el motor del modulo Bitbucket
    emit_progress(MODULO, &app, "Jira: creando repositorios", &clave, 3, PASOS, "running");
    let resumen = bitbucket_crear_repos(
        app.clone(),
        ConfigRun {
            workspace: opciones.workspace.clone(),
            project_key: proyecto.clone(),
            repos: ticket.repos.clone(),
            pais: codigo_pais.to_string(),
            // Los repos de Base de Datos van sin Pipelines ni SonarQube.
            activar_pipelines: !ticket.es_bd,
            crear_ramas: true,
            crear_sonar: opciones.sonar && !ticket.es_bd,
        },
    )
    .await?;

    let rama_dev = bb_repos::rama_desarrollo(codigo_pais);
    let ramas_esperadas: Vec<String> = vec!["master".into(), rama_dev.into(), "release".into()];
    let proyecto_etiqueta = format!("{} ({})", proyecto, ticket.project_name.clone().unwrap_or_default());

    // Los avisos de SonarQube no bloquean: el repo sirve y se completa a mano.
    let todos_bien = resumen.resultados.iter().all(|r| {
        r.estado == "creado" && r.advertencias.iter().all(|a| a.starts_with("SonarQube:"))
    });

    if !todos_bien {
        emit_progress(MODULO, &app, "Jira: ticket sin tocar", &clave, 3, PASOS, "error");
        return Ok(ResultadoTicket {
            clave,
            es_bd: ticket.es_bd,
            resumen,
            acciones_jira: false,
            motivo_sin_acciones: Some(
                "Algun repositorio no quedo completo; el ticket no se modifico. Revisar los resultados y completar a mano.".into(),
            ),
            etiquetas_agregadas: vec![],
            etiquetas_error: None,
            cierre_estado: "omitido".into(),
            cierre_detalle: "No se intento: hay repositorios con errores.".into(),
            proyecto_etiqueta,
            ramas: ramas_esperadas,
            evidencia: vec![],
        });
    }

    // ── 4. Etiquetas y cierre
    let mut etiquetas_agregadas: Vec<String> = Vec::new();
    let mut etiquetas_error: Option<String> = None;
    if opciones.etiquetas {
        emit_progress(MODULO, &app, "Jira: etiquetas", &clave, 4, PASOS, "running");
        let faltan = etiquetas_faltantes(codigo_pais, &ticket.etiquetas);
        if !faltan.is_empty() {
            match agregar_etiquetas(&jira, &clave, &faltan).await {
                Ok(()) => etiquetas_agregadas = faltan,
                Err(e) => etiquetas_error = Some(e.mensaje),
            }
        }
    }

    let (cierre_estado, cierre_detalle) = if !opciones.cerrar {
        ("no_pedido".to_string(), "No se pidio cerrar el ticket.".to_string())
    } else {
        emit_progress(MODULO, &app, "Jira: cerrando ticket", &clave, 5, PASOS, "running");
        let pasos = pasos_cierre(&ticket.estado);
        if pasos.is_empty() {
            ("omitido".to_string(), format!("No se sabe cerrar un ticket en estado '{}'.", ticket.estado))
        } else {
            let mut resultado = ("cerrado".to_string(), format!("Finalizado ({}).", pasos.join(" → ")));
            for paso in pasos {
                if let Err(e) = transicionar(&jira, &clave, paso).await {
                    resultado = ("error".to_string(), e.mensaje);
                    break;
                }
            }
            resultado
        }
    };

    // ── Evidencia: lo que existe en Bitbucket ahora. Si no se puede leer, el
    // ticket ya quedo bien; solo se informa y la evidencia se omite.
    let evidencia = recolectar_evidencia(&bb, &opciones.workspace, &ticket.repos)
        .await
        .unwrap_or_default();

    let estado_evento = if etiquetas_error.is_some() || cierre_estado == "error" { "error" } else { "ok" };
    emit_progress(MODULO, &app, "Jira: listo", &clave, PASOS, PASOS, estado_evento);

    Ok(ResultadoTicket {
        clave,
        es_bd: ticket.es_bd,
        resumen,
        acciones_jira: true,
        motivo_sin_acciones: None,
        etiquetas_agregadas,
        etiquetas_error,
        cierre_estado,
        cierre_detalle,
        proyecto_etiqueta,
        ramas: ramas_esperadas,
        evidencia,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeticionEvidencia {
    pub clave: String,
    pub proyecto_etiqueta: String,
    pub ramas: Vec<String>,
    pub evidencia: Vec<EvidenciaRepo>,
    // PNG en base64, dibujado por la interfaz. None = comentario sin imagen.
    pub imagen_png_base64: Option<String>,
}

// Adjunta la imagen y publica la respuesta al solicitante (visible para el
// cliente, como "Responder al cliente"). Si la imagen no se puede adjuntar no
// se comenta: el texto apunta a una imagen que no existiria.
#[tauri::command]
pub async fn jira_publicar_evidencia(peticion: PeticionEvidencia) -> Result<(), RefreshError> {
    if peticion.evidencia.is_empty() {
        return Err(RefreshError::new("publicar_evidencia", "No hay repositorios para documentar."));
    }

    let token = credentials::obtener_token().await?;
    let jira = JiraClient::new(&token).await?;

    let nombre_imagen = format!("evidencia-{}.png", peticion.clave);
    let usa_imagen = peticion.imagen_png_base64.is_some();

    if let Some(b64) = &peticion.imagen_png_base64 {
        let bytes = BASE64
            .decode(b64.trim())
            .map_err(|e| RefreshError::new("publicar_evidencia", format!("Imagen invalida: {}", e)))?;
        let (codigo, cuerpo) = jira
            .adjuntar(
                &format!("/rest/api/3/issue/{}/attachments", peticion.clave),
                &nombre_imagen,
                "image/png",
                &bytes,
            )
            .await?;
        if codigo != 200 {
            return Err(RefreshError::new(
                "adjuntar_imagen",
                format!("HTTP {}: {}", codigo, mensaje_error(&cuerpo)),
            ));
        }
    }

    let cuerpo = cuerpo_comentario(
        &peticion.evidencia,
        &peticion.proyecto_etiqueta,
        &peticion.ramas,
        if usa_imagen { Some(nombre_imagen.as_str()) } else { None },
    );

    // API v2 porque acepta wiki markup; la propiedad sd.public.comment hace el
    // comentario visible para el cliente en Jira Service Management.
    let (codigo, respuesta) = jira
        .post_json(
            &format!("/rest/api/2/issue/{}/comment", peticion.clave),
            json!({
                "body": cuerpo,
                "properties": [{ "key": "sd.public.comment", "value": { "internal": false } }]
            }),
        )
        .await?;
    if codigo == 201 || codigo == 200 {
        Ok(())
    } else {
        Err(RefreshError::new(
            "publicar_comentario",
            format!("HTTP {}: {}", codigo, mensaje_error(&respuesta)),
        ))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn evid(nombre: &str) -> EvidenciaRepo {
        EvidenciaRepo {
            nombre: nombre.into(),
            url: format!("https://bitbucket.org/banco_ripley/{}", nombre),
            ramas: vec![],
        }
    }

    #[test]
    fn cierre_depende_del_estado() {
        assert_eq!(pasos_cierre("Asignado"), vec!["Comenzar progreso", "Marcar como hecho"]);
        assert_eq!(pasos_cierre("Trabajo en curso"), vec!["Marcar como hecho"]);
        assert!(pasos_cierre("Finalizada").is_empty());
        assert!(pasos_cierre("Bloqueado").is_empty());
    }

    #[test]
    fn etiquetas_faltantes_nunca_repite_y_usa_pais_sin_tilde() {
        assert_eq!(
            etiquetas_faltantes("CL", &["ACN".into(), "CHILE".into(), "DEVOPS-CHAPTER".into()]),
            Vec::<String>::new()
        );
        assert_eq!(
            etiquetas_faltantes("CL", &["ACN".into(), "CHILE".into()]),
            vec!["DEVOPS-CHAPTER".to_string()]
        );
        assert_eq!(
            etiquetas_faltantes("PE", &[]),
            vec!["ACN".to_string(), "DEVOPS-CHAPTER".to_string(), "PERU".to_string()]
        );
        // Una etiqueta ajena no se toca ni se quita.
        assert_eq!(
            etiquetas_faltantes("CL", &["OTRA".into()]),
            vec!["ACN".to_string(), "DEVOPS-CHAPTER".to_string(), "CHILE".to_string()]
        );
    }

    #[test]
    fn comentario_con_varios_repos_es_igual_al_que_se_valido() {
        let esperado = "Se crean los repositorios solicitados en el proyecto CRC (clave-unica) de Bitbucket.\n\
\n\
*Repositorios*\n\
* br-java-cu-claveunica-domain \\\\ [https://bitbucket.org/banco_ripley/br-java-cu-claveunica-domain]\n\
* br-java-cu-claveunica-hsm \\\\ [https://bitbucket.org/banco_ripley/br-java-cu-claveunica-hsm]\n\
\n\
Ramas: master, development y release\n\
\n\
!evidencia-SCDO-4140.png!";
        let ramas = vec!["master".to_string(), "development".to_string(), "release".to_string()];
        let obtenido = cuerpo_comentario(
            &[evid("br-java-cu-claveunica-domain"), evid("br-java-cu-claveunica-hsm")],
            "CRC (clave-unica)",
            &ramas,
            Some("evidencia-SCDO-4140.png"),
        );
        assert_eq!(obtenido, esperado);
    }

    #[test]
    fn comentario_con_un_repo_va_en_singular_y_sin_lista() {
        let ramas = vec!["master".to_string(), "develop".to_string(), "release".to_string()];
        let obtenido = cuerpo_comentario(&[evid("br-java-cu-claveunica-hsm")], "CRC (clave-unica)", &ramas, None);
        assert_eq!(
            obtenido,
            "Se crea el repositorio solicitado en el proyecto CRC (clave-unica) de Bitbucket.\n\n\
[https://bitbucket.org/banco_ripley/br-java-cu-claveunica-hsm]\n\n\
Ramas: master, develop y release"
        );
    }

    #[test]
    fn ramas_se_ordenan_master_desarrollo_release() {
        let mut r = vec!["release".to_string(), "development".to_string(), "master".to_string(), "hotfix".to_string()];
        ordenar_ramas(&mut r);
        assert_eq!(r, vec!["master", "development", "release", "hotfix"]);
        let mut p = vec!["release".to_string(), "develop".to_string(), "master".to_string()];
        ordenar_ramas(&mut p);
        assert_eq!(p, vec!["master", "develop", "release"]);
    }

    #[test]
    fn urlencode_codifica_comillas_espacios_e_igual() {
        assert_eq!(urlencode("name=\"clave unica\""), "name%3D%22clave%20unica%22");
        assert_eq!(urlencode("abc-123_x.y~z"), "abc-123_x.y~z");
    }
}
