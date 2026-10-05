// ─────────────────────────────────────────────────────────────────────────
// Subredes GCP — catálogo para el service.yaml de Cloud Run
//
// El service.yaml de Cloud Run necesita la red y la subred compartidas del
// proyecto. Esos datos viven en tres proyectos de networking, uno por
// ambiente, y la forma de obtenerlos es la misma que usa el equipo a mano:
//
//   gcloud compute networks subnets list --project=<proyecto> --format=json
//
// Se consulta con gcloud en vez de la API REST de Compute por el mismo
// criterio que ya usa el módulo de Clusters: delegar a la CLI, que ya está
// instalada y autenticada en el equipo de quien usa el hub. Pedir la API
// directa obligaría igual a sacar un token con gcloud, así que no habría
// ganado nada y habría sumado código.
//
// El resultado se guarda en caché en disco. La consulta tarda varios
// segundos y los datos cambian poco, así que el módulo abre con la caché y
// solo vuelve a preguntar cuando el usuario lo pide — por ejemplo si el
// proyecto es nuevo y su subred todavía no aparece.
// ─────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use super::super::eks::config::RefreshError;
use super::super::eks::permissions::emit_progress;

// Tag con el que la consola global identifica las líneas de este módulo.
const MODULO: &str = "cicd";

// Los tres proyectos de networking. El primer valor es la clave de ambiente
// que usa el formulario; el segundo, el proyecto real. Ojo con release: el
// ambiente se llama "release" en el formulario pero "pp" en GCP.
const NETWORKING: [(&str, &str); 3] = [
    ("dev", "br-gcp-networking-dev"),
    ("release", "br-gcp-networking-pp"),
    ("prod", "br-gcp-networking-prod"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subnet {
    // Nombre corto de la subred, tal como lo espera el service.yaml.
    pub n: String,
    // Región. No es fija por ambiente: varía por proyecto, incluso dentro
    // del mismo ambiente. Por eso se guarda junto a cada subred y no se
    // deduce de ninguna regla.
    pub r: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogoSubredes {
    pub dev: Vec<Subnet>,
    pub release: Vec<Subnet>,
    pub prod: Vec<Subnet>,
}

impl CatalogoSubredes {
    fn set(&mut self, env: &str, lista: Vec<Subnet>) {
        match env {
            "dev" => self.dev = lista,
            "release" => self.release = lista,
            _ => self.prod = lista,
        }
    }

    fn total(&self) -> usize {
        self.dev.len() + self.release.len() + self.prod.len()
    }
}

// Lo que se guarda en disco. Se versiona el formato para poder descartar
// una caché vieja sin adivinar si el contenido sirve.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEnDisco {
    formato: u32,
    actualizado: String,
    catalogo: CatalogoSubredes,
}

const FORMATO_ACTUAL: u32 = 1;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoSubredes {
    // "cache"      hay caché en disco y es la que se devuelve
    // "sin_cache"  nunca se consultó en este equipo
    // "consulta"   recién consultado en vivo
    pub origen: String,
    // Fecha ISO de la última consulta en vivo. None cuando no hay caché.
    pub actualizado: Option<String>,
    pub catalogo: CatalogoSubredes,
    // Texto para mostrar en pantalla.
    pub detalle: String,
    // Un mensaje por proyecto que falló. Si está vacío, ningún proyecto
    // falló — no significa que haya datos.
    pub errores: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────
// Caché en disco
//
// Vive en el directorio de datos de la aplicación, el que Tauri resuelve
// por sistema operativo. No se usa ninguna ruta fija ni ~/: el directorio
// se pregunta, no se asume.
// ─────────────────────────────────────────────────────────────────────────
fn ruta_cache(app: &AppHandle) -> Result<PathBuf, RefreshError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| RefreshError::new("ruta_cache", e.to_string()))?
        .join("cicd");
    Ok(dir.join("subnets.json"))
}

fn leer_cache(app: &AppHandle) -> Option<CacheEnDisco> {
    let ruta = ruta_cache(app).ok()?;
    let texto = std::fs::read_to_string(ruta).ok()?;
    let cache: CacheEnDisco = serde_json::from_str(&texto).ok()?;
    // Una caché de otro formato se ignora como si no existiera: el usuario
    // puede volver a consultar y sobrescribirla, que es lo que corresponde.
    if cache.formato != FORMATO_ACTUAL {
        return None;
    }
    Some(cache)
}

fn guardar_cache(app: &AppHandle, catalogo: &CatalogoSubredes) -> Result<String, String> {
    let ruta = ruta_cache(app).map_err(|e| e.mensaje)?;
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let actualizado = chrono::Local::now().to_rfc3339();
    let cache = CacheEnDisco {
        formato: FORMATO_ACTUAL,
        actualizado: actualizado.clone(),
        catalogo: catalogo.clone(),
    };
    let json = serde_json::to_string_pretty(&cache).map_err(|e| e.to_string())?;
    std::fs::write(&ruta, json).map_err(|e| e.to_string())?;
    Ok(actualizado)
}

// ─────────────────────────────────────────────────────────────────────────
// Filtro y parseo
// ─────────────────────────────────────────────────────────────────────────

// Reglas confirmadas con los datos reales de los tres proyectos:
//
//   subnet1-<proyecto>-<sufijo>   forma estándar, es la que se busca
//   br-gcp-<proyecto>-<sufijo>    forma irregular de unos pocos proyectos
//
// Todo lo demás se descarta porque no es específico de un proyecto:
// subnet2 y superiores son subredes secundarias compartidas, y las gke-*
// son de los clusters, no de los servicios.
fn es_subred_de_proyecto(nombre: &str) -> bool {
    if nombre.starts_with("gke-") {
        return false;
    }
    nombre.starts_with("subnet1-") || nombre.starts_with("br-gcp-")
}

// gcloud devuelve la región como URL completa
// (".../projects/x/regions/us-east4"). Solo interesa el último segmento.
fn basename(valor: &str) -> String {
    valor.rsplit('/').next().unwrap_or(valor).to_string()
}

fn parsear_salida(json: &str) -> Result<Vec<Subnet>, String> {
    let raw: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let arr = raw
        .as_array()
        .ok_or_else(|| "La respuesta de gcloud no es una lista".to_string())?;

    let mut lista: Vec<Subnet> = arr
        .iter()
        .filter_map(|item| {
            let n = item.get("name")?.as_str()?.to_string();
            if !es_subred_de_proyecto(&n) {
                return None;
            }
            let r = basename(item.get("region").and_then(|v| v.as_str()).unwrap_or(""));
            Some(Subnet { n, r })
        })
        .collect();

    // Orden estable por región y después por nombre, para que dos consultas
    // seguidas produzcan el mismo archivo y se pueda comparar.
    lista.sort_by(|a, b| a.r.cmp(&b.r).then(a.n.cmp(&b.n)));
    Ok(lista)
}

async fn consultar_proyecto(proyecto: &str) -> Result<Vec<Subnet>, String> {
    let salida = tokio::process::Command::new("gcloud")
        .args([
            "compute",
            "networks",
            "subnets",
            "list",
            &format!("--project={}", proyecto),
            "--format=json",
        ])
        .output()
        .await
        .map_err(|e| {
            format!(
                "No se pudo ejecutar gcloud: {}. Verifica que esté instalado y en el PATH.",
                e
            )
        })?;

    if !salida.status.success() {
        let err = String::from_utf8_lossy(&salida.stderr);
        // gcloud escribe el motivo real en stderr: permisos, sesión vencida,
        // proyecto inexistente. Se recorta para que entre en una línea de la
        // consola sin perder la parte útil.
        return Err(err.trim().chars().take(300).collect());
    }

    parsear_salida(&String::from_utf8_lossy(&salida.stdout))
}

// ─────────────────────────────────────────────────────────────────────────
// Comandos que React invoca
// ─────────────────────────────────────────────────────────────────────────

// Se llama al abrir el módulo. Nunca consulta en vivo ni pide nada: solo
// informa qué hay guardado. Si no hay caché, el frontend usa su catálogo
// semilla y lo dice en pantalla.
#[tauri::command]
pub async fn cicd_subnets_leer(app: AppHandle) -> Result<EstadoSubredes, RefreshError> {
    match leer_cache(&app) {
        Some(cache) => {
            let total = cache.catalogo.total();
            Ok(EstadoSubredes {
                origen: "cache".to_string(),
                actualizado: Some(cache.actualizado),
                catalogo: cache.catalogo,
                detalle: format!("{} subredes en caché local.", total),
                errores: vec![],
            })
        }
        None => Ok(EstadoSubredes {
            origen: "sin_cache".to_string(),
            actualizado: None,
            catalogo: CatalogoSubredes::default(),
            detalle: "Sin caché en este equipo — se usa el catálogo incluido en la aplicación."
                .to_string(),
            errores: vec![],
        }),
    }
}

// Consulta los tres proyectos y guarda el resultado.
//
// Criterio ante fallas: un proyecto que falla no cancela los otros dos. Con
// la sesión de gcloud vencida fallan los tres y el mensaje lo deja claro;
// pero si solo falla producción por permisos, tener dev y release
// actualizados sigue siendo mejor que no tener nada. Para no perder lo que
// ya estaba, un ambiente que falla conserva lo que tenía en caché.
#[tauri::command]
pub async fn cicd_subnets_actualizar(app: AppHandle) -> Result<EstadoSubredes, RefreshError> {
    let previo = leer_cache(&app);
    let mut catalogo = previo
        .as_ref()
        .map(|c| c.catalogo.clone())
        .unwrap_or_default();

    let mut errores: Vec<String> = Vec::new();
    let total = NETWORKING.len() as u32;

    for (i, (env, proyecto)) in NETWORKING.iter().enumerate() {
        let n = (i + 1) as u32;
        emit_progress(MODULO, &app, "Consultando subredes", proyecto, n, total, "running");

        match consultar_proyecto(proyecto).await {
            Ok(lista) => {
                emit_progress(
                    MODULO,
                    &app,
                    &format!("{} subredes", lista.len()),
                    proyecto,
                    n,
                    total,
                    "ok",
                );
                catalogo.set(env, lista);
            }
            Err(e) => {
                emit_progress(MODULO, &app, "Error al consultar", proyecto, n, total, "error");
                errores.push(format!("{}: {}", proyecto, e));
            }
        }
    }

    // Los tres fallaron y no había nada guardado: no hay resultado que
    // devolver, así que es un error y no un éxito vacío.
    if errores.len() == NETWORKING.len() && previo.is_none() {
        return Err(RefreshError::new(
            "consultar_subredes",
            format!(
                "No se pudo consultar ninguno de los proyectos de networking.\n{}",
                errores.join("\n")
            ),
        ));
    }

    let (actualizado, aviso) = match guardar_cache(&app, &catalogo) {
        Ok(fecha) => (Some(fecha), String::new()),
        // Los datos sirven igual en esta sesión; lo que se pierde es
        // recordarlos para la próxima.
        Err(e) => (
            previo.as_ref().map(|c| c.actualizado.clone()),
            format!(" No se pudo guardar la caché: {}", e),
        ),
    };

    let detalle = format!(
        "{} subredes (dev {}, release {}, prod {}).{}",
        catalogo.total(),
        catalogo.dev.len(),
        catalogo.release.len(),
        catalogo.prod.len(),
        aviso
    );

    Ok(EstadoSubredes {
        origen: "consulta".to_string(),
        actualizado,
        catalogo,
        detalle,
        errores,
    })
}
