// ─────────────────────────────────────────────────────────────────────────
// Tickets de Jira — busqueda de pendientes, lectura y validacion
//
// Reglas portadas de la automatizacion que corria por terminal (probadas
// contra los tickets reales del equipo):
//
// - Los nombres de repositorio NO estan en campos del ticket: viven en las
//   respuestas del formulario "ABM de Repositorios Bitbucket" (API de
//   formularios). Los campos Repositorio 1-5 del ticket vienen vacios.
// - El campo "Proyecto de Bitbucket" llega como "CLAVE - nombre". La clave
//   se conserva TAL CUAL, sin pasarla a mayusculas: existe una real en
//   minusculas (peru_control_de_cambios).
// - Los nombres de repositorio se llevan siempre a minusculas.
// - "Lenguaje / Tecnologia" = "Base de Datos" marca los repos de BD, que se
//   crean SIN Pipelines ni SonarQube.
// - Solo se procesan solicitudes de tipo Alta.
// ─────────────────────────────────────────────────────────────────────────

use serde::Serialize;
use serde_json::Value;

use super::super::eks::config::RefreshError;
use super::client::{mensaje_error, JiraClient, PROYECTO_JIRA};
use super::credentials;

const TIPO_SOLICITUD_JIRA: &str = "Alta, Baja y Modificación de Repositorios Bitbucket";
const NOMBRE_FORMULARIO: &str = "ABM de Repositorios Bitbucket";

// Un ticket ya cerrado o cancelado no se vuelve a procesar.
pub const ESTADOS_PROCESABLES: [&str; 2] = ["Asignado", "Trabajo en curso"];

// Identificadores de los campos personalizados. Se descubrieron leyendo un
// ticket real; son propios de esta instancia de Jira.
const CF_PROYECTO_BITBUCKET: &str = "customfield_14685";
const CF_TIPO_SOLICITUD: &str = "customfield_10969";
const CF_LENGUAJE: &str = "customfield_12792";
const CF_PAIS: &str = "customfield_12891";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketPendiente {
    pub clave: String,
    pub resumen: String,
    pub estado: String,
    pub etiquetas: Vec<String>,
    // Texto crudo del campo, ej "CRC - clave-unica".
    pub proyecto: Option<String>,
    pub project_key: Option<String>,
    pub project_name: Option<String>,
    pub repos: Vec<String>,
    pub tipo: Option<String>,
    pub pais: Option<String>,
    pub lenguaje: Option<String>,
    pub es_bd: bool,
    // Motivos por los que el ticket no se puede procesar. Vacio = valido.
    pub errores: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────
// Reglas puras (sin red): se prueban abajo con datos reales
// ─────────────────────────────────────────────────────────────────────────

// Quita tildes y pasa a minusculas para comparar valores escritos a mano:
// "Peru" y "Perú" deben dar lo mismo.
fn sin_acentos(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
            'ñ' | 'Ñ' => 'n',
            otro => otro,
        })
        .collect::<String>()
        .to_lowercase()
}

// Codigo de pais con el que trabaja el modulo Bitbucket: CL o PE.
pub fn pais_a_codigo(pais: Option<&str>) -> Option<&'static str> {
    match sin_acentos(pais?.trim()).as_str() {
        "chile" => Some("CL"),
        "peru" => Some("PE"),
        _ => None,
    }
}

// Etiqueta de pais que ya existe en Jira. Sin tilde: Jira crea una etiqueta
// nueva con cualquier texto distinto, y la existente es PERU, no PERÚ.
pub fn etiqueta_pais(codigo: &str) -> &'static str {
    if codigo == "PE" {
        "PERU"
    } else {
        "CHILE"
    }
}

pub fn es_base_de_datos(lenguaje: Option<&str>) -> bool {
    lenguaje
        .map(|l| sin_acentos(l.trim()) == "base de datos")
        .unwrap_or(false)
}

// "CRC - clave-unica" -> (Some("CRC"), Some("clave-unica")).
// Si no viene en ese formato -> (None, Some(valor)): no hay clave confiable.
pub fn parsear_proyecto(raw: Option<&str>) -> (Option<String>, Option<String>) {
    let raw = match raw {
        Some(r) if !r.trim().is_empty() => r.trim(),
        _ => return (None, None),
    };

    if let Some((clave, nombre)) = raw.split_once(" - ") {
        let clave = clave.trim();
        if !clave.is_empty() && !clave.contains(char::is_whitespace) {
            return (Some(clave.to_string()), Some(nombre.trim().to_string()));
        }
    }
    (None, Some(raw.to_string()))
}

// Letras, numeros, punto, guion y guion bajo; sin espacios ni barras. Esto
// rechaza lo que a veces se pega por error: una URL en vez del nombre.
pub fn nombre_repo_valido(nombre: &str) -> bool {
    let largo = nombre.chars().count();
    if !(2..=100).contains(&largo) {
        return false;
    }
    let empieza_bien = nombre.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false);
    empieza_bien
        && nombre
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

fn es_etiqueta_repo(etiqueta: &str) -> bool {
    match etiqueta.to_lowercase().strip_prefix("repositorio") {
        Some(resto) => {
            let r = resto.trim();
            !r.is_empty() && r.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

// Nombres de repositorio de las respuestas del formulario: en minusculas, sin
// vacios y sin duplicados (dos que solo difieran en mayusculas serian el mismo
// repo y el segundo fallaria por "ya existe").
pub fn repos_desde_respuestas(respuestas: &Value) -> Vec<String> {
    let mut repos: Vec<String> = Vec::new();
    if let Some(lista) = respuestas.as_array() {
        for item in lista {
            let etiqueta = item.get("label").and_then(|l| l.as_str()).unwrap_or("").trim();
            if !es_etiqueta_repo(etiqueta) {
                continue;
            }
            if let Some(respuesta) = item.get("answer").and_then(|a| a.as_str()) {
                let nombre = respuesta.trim().to_lowercase();
                if !nombre.is_empty() && !repos.contains(&nombre) {
                    repos.push(nombre);
                }
            }
        }
    }
    repos
}

// Los campos de Jira llegan de varias formas: texto, objeto {value|name|key}
// (listas desplegables) o lista de objetos.
fn campo_texto(fields: &Value, id: &str) -> Option<String> {
    let desde_objeto = |o: &serde_json::Map<String, Value>, claves: &[&str]| {
        claves
            .iter()
            .find_map(|k| o.get(*k).and_then(|v| v.as_str()).map(String::from))
    };

    match fields.get(id)? {
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        Value::Object(o) => desde_objeto(o, &["value", "name", "key"]),
        Value::Array(a) => match a.first()? {
            Value::String(s) => Some(s.clone()),
            Value::Object(o) => desde_objeto(o, &["value", "name"]),
            _ => None,
        },
        _ => None,
    }
}

// Motivos por los que un ticket no se puede procesar automaticamente.
pub fn validar(t: &TicketPendiente) -> Vec<String> {
    let mut errores: Vec<String> = Vec::new();

    match t.tipo.as_deref().map(str::trim) {
        None | Some("") => errores.push(
            "El campo 'Tipo de Solicitud' esta vacio. Solo se procesan solicitudes de tipo Alta.".into(),
        ),
        Some(tipo) if tipo.to_lowercase() != "alta" => errores.push(format!(
            "Tipo de solicitud es '{}'. Este proceso solo maneja Alta.",
            tipo
        )),
        _ => {}
    }

    match t.pais.as_deref().map(str::trim) {
        None | Some("") => errores.push("El campo 'Pais' esta vacio. Debe indicarse Chile o Peru.".into()),
        Some(p) if pais_a_codigo(Some(p)).is_none() => {
            errores.push(format!("Pais '{}' no reconocido. Se esperaba Chile o Peru.", p))
        }
        _ => {}
    }

    // Sin este campo no se sabe si los repos son de BD (sin Pipelines ni
    // SonarQube) o de codigo, y adivinar dejaria Pipelines donde no deben.
    if t.lenguaje.as_deref().map(str::trim).unwrap_or("").is_empty() {
        errores.push(
            "El campo 'Lenguaje / Tecnologia' esta vacio; se necesita para saber si son repositorios de Base de Datos."
                .into(),
        );
    }

    if t.project_key.is_none() {
        errores.push(format!(
            "El campo 'Proyecto de Bitbucket' ('{}') esta vacio o no tiene el formato 'CLAVE - nombre'.",
            t.proyecto.as_deref().unwrap_or("")
        ));
    }

    if t.repos.is_empty() {
        errores.push(
            "No se encontro ningun nombre de repositorio en el ticket (los campos Repositorio 1-5 estan todos vacios)."
                .into(),
        );
    } else {
        for r in &t.repos {
            if !nombre_repo_valido(r) {
                errores.push(format!(
                    "Nombre de repositorio invalido: '{}'. Debe ser un nombre (letras, numeros, '.', '_' o '-'), no una URL ni contener espacios.",
                    r
                ));
            }
        }
    }

    errores
}

// ─────────────────────────────────────────────────────────────────────────
// Lectura desde Jira
// ─────────────────────────────────────────────────────────────────────────

// Tickets asignados a la cuenta del token, en un estado procesable.
async fn claves_pendientes(client: &JiraClient) -> Result<Vec<String>, RefreshError> {
    let jql = format!(
        "project = \"{}\" AND assignee = currentUser() AND status in (\"{}\", \"{}\") AND \"Request Type\" = \"{}\" ORDER BY created ASC",
        PROYECTO_JIRA, ESTADOS_PROCESABLES[0], ESTADOS_PROCESABLES[1], TIPO_SOLICITUD_JIRA
    );

    let mut claves: Vec<String> = Vec::new();
    let mut pagina: Option<String> = None;

    loop {
        let mut query: Vec<(&str, String)> = vec![
            ("jql", jql.clone()),
            ("maxResults", "50".to_string()),
            ("fields", "summary".to_string()),
        ];
        if let Some(p) = &pagina {
            query.push(("nextPageToken", p.clone()));
        }
        let query_ref: Vec<(&str, &str)> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();

        let (codigo, cuerpo) = client.get("/rest/api/3/search/jql", &query_ref).await?;
        if codigo != 200 {
            return Err(RefreshError::new(
                "buscar_tickets",
                format!("HTTP {}: {}", codigo, mensaje_error(&cuerpo)),
            ));
        }

        let v: Value = serde_json::from_str(&cuerpo)
            .map_err(|e| RefreshError::new("buscar_tickets", e.to_string()))?;
        if let Some(lista) = v.get("issues").and_then(|i| i.as_array()) {
            for issue in lista {
                if let Some(k) = issue.get("key").and_then(|k| k.as_str()) {
                    claves.push(k.to_string());
                }
            }
        }

        pagina = v.get("nextPageToken").and_then(|t| t.as_str()).map(String::from);
        if pagina.is_none() {
            break;
        }
    }
    Ok(claves)
}

// Respuestas del formulario ABM del ticket. Un ticket sin formulario no es un
// error de lectura: simplemente no tiene repositorios que leer.
async fn respuestas_formulario(client: &JiraClient, clave: &str) -> Result<Value, RefreshError> {
    let vacio = Value::Array(Vec::new());

    let (codigo, cuerpo) = client.get_forms(&format!("/issue/{}/form", clave)).await?;
    if codigo == 404 {
        return Ok(vacio);
    }
    if codigo != 200 {
        return Err(RefreshError::new(
            "leer_formulario",
            format!("{}: HTTP {}: {}", clave, codigo, mensaje_error(&cuerpo)),
        ));
    }

    let formularios: Value = serde_json::from_str(&cuerpo)
        .map_err(|e| RefreshError::new("leer_formulario", e.to_string()))?;
    let id = formularios
        .as_array()
        .and_then(|lista| {
            lista
                .iter()
                .find(|f| f.get("name").and_then(|n| n.as_str()) == Some(NOMBRE_FORMULARIO))
        })
        .and_then(|f| f.get("id").and_then(|i| i.as_str()).map(String::from));

    let id = match id {
        Some(i) => i,
        None => return Ok(vacio),
    };

    let (codigo, cuerpo) = client
        .get_forms(&format!("/issue/{}/form/{}/format/answers", clave, id))
        .await?;
    if codigo != 200 {
        return Err(RefreshError::new(
            "leer_formulario",
            format!("{}: HTTP {}: {}", clave, codigo, mensaje_error(&cuerpo)),
        ));
    }
    serde_json::from_str(&cuerpo).map_err(|e| RefreshError::new("leer_formulario", e.to_string()))
}

// Lee un ticket completo y lo valida. Tambien lo usa procesar.rs, que vuelve
// a leerlo justo antes de actuar: la lista que ve el usuario puede tener
// minutos y el ticket pudo cambiar.
pub async fn leer_ticket(client: &JiraClient, clave: &str) -> Result<TicketPendiente, RefreshError> {
    let (codigo, cuerpo) = client.get(&format!("/rest/api/3/issue/{}", clave), &[]).await?;
    if codigo != 200 {
        return Err(RefreshError::new(
            "leer_ticket",
            format!("{}: HTTP {}: {}", clave, codigo, mensaje_error(&cuerpo)),
        ));
    }
    let v: Value = serde_json::from_str(&cuerpo)
        .map_err(|e| RefreshError::new("leer_ticket", e.to_string()))?;
    let fields = v.get("fields").cloned().unwrap_or(Value::Null);

    let proyecto = campo_texto(&fields, CF_PROYECTO_BITBUCKET);
    let (project_key, project_name) = parsear_proyecto(proyecto.as_deref());
    let lenguaje = campo_texto(&fields, CF_LENGUAJE);
    let respuestas = respuestas_formulario(client, clave).await?;

    let mut ticket = TicketPendiente {
        clave: clave.to_string(),
        resumen: fields.get("summary").and_then(|s| s.as_str()).unwrap_or("").to_string(),
        estado: fields
            .get("status")
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string(),
        etiquetas: fields
            .get("labels")
            .and_then(|l| l.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        proyecto,
        project_key,
        project_name,
        repos: repos_desde_respuestas(&respuestas),
        tipo: campo_texto(&fields, CF_TIPO_SOLICITUD),
        pais: campo_texto(&fields, CF_PAIS),
        es_bd: es_base_de_datos(lenguaje.as_deref()),
        lenguaje,
        errores: Vec::new(),
    };
    ticket.errores = validar(&ticket);
    Ok(ticket)
}

#[tauri::command]
pub async fn jira_tickets_pendientes() -> Result<Vec<TicketPendiente>, RefreshError> {
    let token = credentials::obtener_token().await?;
    let client = JiraClient::new(&token).await?;

    let mut tickets: Vec<TicketPendiente> = Vec::new();
    for clave in claves_pendientes(&client).await? {
        tickets.push(leer_ticket(&client, &clave).await?);
    }
    Ok(tickets)
}

// ─────────────────────────────────────────────────────────────────────────
// Pruebas — usan valores reales de tickets ya procesados
// ─────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    fn ticket_base() -> TicketPendiente {
        TicketPendiente {
            clave: "SCDO-1".into(),
            resumen: "x".into(),
            estado: "Asignado".into(),
            etiquetas: vec![],
            proyecto: Some("CRC - clave-unica".into()),
            project_key: Some("CRC".into()),
            project_name: Some("clave-unica".into()),
            repos: vec!["br-java-cu-claveunica-domain".into()],
            tipo: Some("Alta".into()),
            pais: Some("Chile".into()),
            lenguaje: Some("Java".into()),
            es_bd: false,
            errores: vec![],
        }
    }

    #[test]
    fn proyecto_conserva_la_clave_tal_cual() {
        assert_eq!(
            parsear_proyecto(Some("CRC - clave-unica")),
            (Some("CRC".into()), Some("clave-unica".into()))
        );
        assert_eq!(
            parsear_proyecto(Some("PE_ECCI - PE_eqcci")),
            (Some("PE_ECCI".into()), Some("PE_eqcci".into()))
        );
        // Clave en minusculas: existe de verdad y no se debe pasar a mayusculas.
        assert_eq!(
            parsear_proyecto(Some("peru_control_de_cambios - PERU Control de Cambios")),
            (Some("peru_control_de_cambios".into()), Some("PERU Control de Cambios".into()))
        );
        assert_eq!(
            parsear_proyecto(Some("AAFT - AI Agents Factory [TEST]")),
            (Some("AAFT".into()), Some("AI Agents Factory [TEST]".into()))
        );
    }

    #[test]
    fn proyecto_sin_formato_no_da_clave() {
        assert_eq!(parsear_proyecto(Some("Solo nombre")), (None, Some("Solo nombre".into())));
        assert_eq!(parsear_proyecto(Some("   ")), (None, None));
        assert_eq!(parsear_proyecto(None), (None, None));
    }

    #[test]
    fn pais_acepta_con_y_sin_tilde() {
        assert_eq!(pais_a_codigo(Some("Chile")), Some("CL"));
        assert_eq!(pais_a_codigo(Some("Perú")), Some("PE"));
        assert_eq!(pais_a_codigo(Some("PERU")), Some("PE"));
        assert_eq!(pais_a_codigo(Some(" peru ")), Some("PE"));
        assert_eq!(pais_a_codigo(Some("Argentina")), None);
        assert_eq!(pais_a_codigo(None), None);
    }

    #[test]
    fn etiqueta_de_pais_va_sin_tilde() {
        assert_eq!(etiqueta_pais("PE"), "PERU");
        assert_eq!(etiqueta_pais("CL"), "CHILE");
    }

    #[test]
    fn base_de_datos_se_reconoce_sin_importar_forma() {
        for v in ["Base de Datos", "base de datos", " BASE DE DATOS ", "Base de datos"] {
            assert!(es_base_de_datos(Some(v)), "{}", v);
        }
        for v in ["Java", "Node", "Otros", ""] {
            assert!(!es_base_de_datos(Some(v)), "{}", v);
        }
        assert!(!es_base_de_datos(None));
    }

    #[test]
    fn nombres_de_repo_reales_son_validos_y_las_urls_no() {
        for n in [
            "br-java-cu-claveunica-domain",
            "br-ms-sd00026-oi00116-archivo-trasmitir-v1",
            "br-db-gestor-campanas-camp-adm-ddl",
            "wsin_chk_hash_pres",
        ] {
            assert!(nombre_repo_valido(n), "{}", n);
        }
        for n in [
            "https://bitbucket.org/banco_ripley/workspace/projects/ARQ",
            "con espacio",
            "",
            "a",
            "-empieza-con-guion",
        ] {
            assert!(!nombre_repo_valido(n), "{}", n);
        }
    }

    #[test]
    fn repos_salen_en_minusculas_sin_duplicados_y_solo_de_las_etiquetas_repositorio() {
        let respuestas = json!([
            {"label": "Sistema APM Asociado", "answer": "Clave Unica"},
            {"label": "Repositorio 1", "answer": "br-java-cu-claveunica-ws-tam"},
            {"label": "Repositorio 2", "answer": "br-java-cu-HSMDummy"},
            {"label": "Repositorio 3", "answer": "BR-JAVA-CU-HSMDUMMY"},
            {"label": "Repositorio 4", "answer": "  "},
            {"label": "Repositorio 5"},
            {"label": "Proyecto de Bitbucket", "answer": "CRC - clave-unica"}
        ]);
        assert_eq!(
            repos_desde_respuestas(&respuestas),
            vec!["br-java-cu-claveunica-ws-tam".to_string(), "br-java-cu-hsmdummy".to_string()]
        );
    }

    #[test]
    fn campo_texto_entiende_cada_forma_de_jira() {
        let f = json!({
            "a": "  Chile ",
            "b": {"value": "Alta", "id": "1"},
            "c": [{"value": "Java"}],
            "d": null,
            "e": ""
        });
        assert_eq!(campo_texto(&f, "a"), Some("Chile".into()));
        assert_eq!(campo_texto(&f, "b"), Some("Alta".into()));
        assert_eq!(campo_texto(&f, "c"), Some("Java".into()));
        assert_eq!(campo_texto(&f, "d"), None);
        assert_eq!(campo_texto(&f, "e"), None);
        assert_eq!(campo_texto(&f, "no_existe"), None);
    }

    #[test]
    fn ticket_correcto_no_tiene_errores() {
        assert!(validar(&ticket_base()).is_empty());
    }

    #[test]
    fn solo_se_procesan_altas() {
        let mut t = ticket_base();
        t.tipo = Some("Modificación".into());
        assert_eq!(validar(&t).len(), 1);
        t.tipo = None;
        assert_eq!(validar(&t).len(), 1);
    }

    #[test]
    fn pais_y_lenguaje_son_obligatorios() {
        let mut t = ticket_base();
        t.pais = Some("Argentina".into());
        assert_eq!(validar(&t).len(), 1);
        t.pais = Some("Perú".into());
        assert!(validar(&t).is_empty());
        t.lenguaje = None;
        assert_eq!(validar(&t).len(), 1);
    }

    #[test]
    fn proyecto_sin_clave_y_repos_invalidos_se_informan() {
        let mut t = ticket_base();
        t.project_key = None;
        t.proyecto = Some("Solo nombre".into());
        t.repos = vec!["https://bitbucket.org/x".into(), "ok-repo".into()];
        let e = validar(&t);
        assert_eq!(e.len(), 2, "{:?}", e);

        t.repos = vec![];
        assert!(validar(&t).iter().any(|m| m.contains("ningun nombre de repositorio")));
    }
}
