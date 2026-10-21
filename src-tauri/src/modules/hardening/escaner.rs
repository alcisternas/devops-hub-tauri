// ─────────────────────────────────────────────────────────────────────────
// Escaner de vulnerabilidades — ejecucion controlada y normalizacion
//
// Port de scanner/trivy.py de Container Hardening Studio.
//
// Reglas inamovibles (politica BANK-STRICT):
//   * nunca se pasa --ignore-unfixed, --vex, --ignorefile ni --skip-* de
//     vulnerabilidades: lo que no tiene parche TAMBIEN cuenta;
//   * se evaluan todas las severidades;
//   * los argumentos realmente usados se devuelven como evidencia.
//
// Los campos llevan el mismo nombre (snake_case) que en Python para poder
// comparar la salida con la del motor de referencia.
// ─────────────────────────────────────────────────────────────────────────

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::validar::{es_digest, es_plataforma};
use super::version::{parsear, Version};

pub const SEVERIDADES: [&str; 5] = ["CRITICAL", "HIGH", "MEDIUM", "LOW", "UNKNOWN"];
const FLAGS_PROHIBIDOS: [&str; 6] = ["--ignore-unfixed", "--vex", "--ignore-status", "--ignore-policy", "--skip-dirs", "--skip-files"];
const TIMEOUT_ESCANEO_SEGUNDOS: u64 = 600;

// ─────────────────────────────────────────────────────────────────────────
// Motor de escaneo elegible
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escaner {
    Trivy,
    DockerScout,
}

impl Escaner {
    pub fn desde_texto(t: &str) -> Result<Self, String> {
        match t.trim().to_lowercase().as_str() {
            "trivy" => Ok(Escaner::Trivy),
            "scout" | "docker-scout" | "docker scout" => Ok(Escaner::DockerScout),
            otro => Err(format!("Escaner desconocido: {:?}. Opciones: trivy, scout.", otro)),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Modelo normalizado (independiente del escaner)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hallazgo {
    pub uid: usize,
    pub id: String,
    pub severity: String,
    pub pkg_name: String,
    pub installed_version: String,
    pub fixed_version: Option<String>,
    pub fixed_versions: Vec<String>,
    pub status: String,
    pub cvss: Option<f64>,
    pub title: String,
    pub url: String,
    pub target: String,
    pub ecosystem: String,
    pub class: String,
    pub pkg_path: String,
    pub data_source: String,
    pub layer_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Paquete {
    pub name: String,
    pub version: String,
    pub class: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SistemaOperativo {
    pub family: Option<String>,
    pub name: Option<String>,
    pub eosl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InfoEscaner {
    pub name: String,
    pub version: Option<String>,
    pub vulnerability_db: Option<Value>,
    pub args: Vec<String>,
    pub ignore_unfixed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoImagen {
    pub image_id: Option<String>,
    pub repo_digests: Vec<String>,
    pub size: Option<u64>,
    pub architecture: Option<String>,
    // Variables de entorno de la imagen (NODE_VERSION, PATH...), ya separadas en clave/valor.
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Escaneo {
    pub label: String,
    pub scanner: InfoEscaner,
    pub image: InfoImagen,
    pub os: SistemaOperativo,
    pub vulnerabilities: Vec<Hallazgo>,
    pub packages: Vec<Paquete>,
    pub counts: BTreeMap<String, usize>,
}

fn texto(v: &Value, clave: &str) -> String {
    v.get(clave).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn mejor_cvss(v: &Value) -> Option<f64> {
    let mut puntajes: Vec<f64> = Vec::new();
    if let Some(fuentes) = v.get("CVSS").and_then(|c| c.as_object()) {
        for fuente in fuentes.values() {
            for clave in ["V3Score", "V40Score", "V2Score"] {
                if let Some(p) = fuente.get(clave).and_then(|x| x.as_f64()) {
                    puntajes.push(p);
                    break;
                }
            }
        }
    }
    puntajes.into_iter().fold(None, |acc, p| Some(acc.map_or(p, |a: f64| a.max(p))))
}

pub fn versiones_corregidas(fixed: Option<&str>) -> Vec<String> {
    fixed
        .unwrap_or("")
        .split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

pub fn resumir(vulns: &[Hallazgo], n_paquetes: usize) -> BTreeMap<String, usize> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    for s in SEVERIDADES {
        m.insert(s.to_string(), vulns.iter().filter(|v| v.severity == s).count());
    }
    let corregibles = vulns.iter().filter(|v| v.fixed_version.is_some()).count();
    m.insert("total".into(), vulns.len());
    m.insert("fixable".into(), corregibles);
    m.insert("unfixed".into(), vulns.len() - corregibles);
    m.insert(
        "high_critical_unfixed".into(),
        vulns.iter().filter(|v| (v.severity == "HIGH" || v.severity == "CRITICAL") && v.fixed_version.is_none()).count(),
    );
    m.insert("packages".into(), n_paquetes);
    m
}

// Convierte el JSON de Trivy al modelo normalizado.
pub fn normalizar_trivy(raw: &Value, label: &str) -> Escaneo {
    let meta = raw.get("Metadata").cloned().unwrap_or(Value::Null);
    let diff_ids: Vec<String> = meta
        .get("DiffIDs")
        .and_then(|d| d.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let indice_capa = |diff: Option<&str>| diff.and_then(|d| diff_ids.iter().position(|x| x == d));

    let mut vulns: Vec<Hallazgo> = Vec::new();
    let mut pkgs: Vec<Paquete> = Vec::new();

    if let Some(resultados) = raw.get("Results").and_then(|r| r.as_array()) {
        for res in resultados {
            let rtype = texto(res, "Type");
            let rclass = texto(res, "Class");
            if let Some(lista) = res.get("Packages").and_then(|p| p.as_array()) {
                for p in lista {
                    pkgs.push(Paquete {
                        name: texto(p, "Name"),
                        version: texto(p, "Version"),
                        class: rclass.clone(),
                        path: texto(p, "FilePath"),
                    });
                }
            }
            if let Some(lista) = res.get("Vulnerabilities").and_then(|v| v.as_array()) {
                for v in lista {
                    let fixed = v.get("FixedVersion").and_then(|f| f.as_str()).filter(|f| !f.is_empty());
                    let severidad = v.get("Severity").and_then(|s| s.as_str()).unwrap_or("UNKNOWN").to_uppercase();
                    let capa = v.get("Layer").and_then(|l| l.get("DiffID")).and_then(|d| d.as_str());
                    vulns.push(Hallazgo {
                        uid: 0,
                        id: texto(v, "VulnerabilityID"),
                        severity: severidad,
                        pkg_name: texto(v, "PkgName"),
                        installed_version: texto(v, "InstalledVersion"),
                        fixed_version: fixed.map(String::from),
                        fixed_versions: versiones_corregidas(fixed),
                        status: texto(v, "Status"),
                        cvss: mejor_cvss(v),
                        title: texto(v, "Title"),
                        url: texto(v, "PrimaryURL"),
                        target: texto(res, "Target"),
                        ecosystem: rtype.clone(),
                        class: rclass.clone(),
                        pkg_path: texto(v, "PkgPath"),
                        data_source: v.get("DataSource").map(|d| texto(d, "Name")).unwrap_or_default(),
                        layer_index: indice_capa(capa),
                    });
                }
            }
        }
    }
    for (i, v) in vulns.iter_mut().enumerate() {
        v.uid = i;
    }

    let os_meta = meta.get("OS").cloned().unwrap_or(Value::Null);
    let cfg = meta.get("ImageConfig").cloned().unwrap_or(Value::Null);
    let mut env: BTreeMap<String, String> = BTreeMap::new();
    if let Some(lista) = cfg.get("config").and_then(|c| c.get("Env")).and_then(|e| e.as_array()) {
        for item in lista.iter().filter_map(|x| x.as_str()) {
            let (k, v) = item.split_once('=').unwrap_or((item, ""));
            env.insert(k.to_string(), v.to_string());
        }
    }
    let conteos = resumir(&vulns, pkgs.len());

    Escaneo {
        label: label.to_string(),
        scanner: InfoEscaner {
            name: "trivy".into(),
            version: raw.get("Trivy").and_then(|t| t.get("Version")).and_then(|v| v.as_str()).map(String::from),
            vulnerability_db: None,
            args: vec![],
            ignore_unfixed: false,
        },
        image: InfoImagen {
            image_id: meta.get("ImageID").and_then(|x| x.as_str()).map(String::from),
            repo_digests: meta
                .get("RepoDigests")
                .and_then(|d| d.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            size: meta.get("Size").and_then(|s| s.as_u64()),
            architecture: cfg.get("architecture").and_then(|a| a.as_str()).map(String::from),
            env,
        },
        os: SistemaOperativo {
            family: os_meta.get("Family").and_then(|x| x.as_str()).map(String::from),
            name: os_meta.get("Name").and_then(|x| x.as_str()).map(String::from),
            eosl: os_meta.get("EOSL").and_then(|x| x.as_bool()).unwrap_or(false),
        },
        vulnerabilities: vulns,
        packages: pkgs,
        counts: conteos,
    }
}

pub fn bloqueantes(vulns: &[Hallazgo]) -> Vec<&Hallazgo> {
    vulns.iter().filter(|v| v.severity == "HIGH" || v.severity == "CRITICAL").collect()
}

// ─────────────────────────────────────────────────────────────────────────
// Eleccion de la version que corrige un hallazgo
// ─────────────────────────────────────────────────────────────────────────

// Version corregida mas cercana a la instalada: de la misma rama mayor si
// existe; si no, la minima superior. None si ninguna sirve.
pub fn elegir_version_corregida(instalada: &str, corregidas: &[String]) -> Option<String> {
    let actual = parsear(instalada);
    let candidatas: Vec<(&String, Version)> = corregidas.iter().filter_map(|f| parsear(f).map(|pv| (f, pv))).collect();
    let actual = match actual {
        Some(a) if !candidatas.is_empty() => a,
        _ => return corregidas.first().cloned(),
    };
    let misma_rama = candidatas.iter().filter(|(_, pv)| pv.mayor() == actual.mayor() && *pv >= actual).min_by(|a, b| a.1.cmp(&b.1));
    if let Some((f, _)) = misma_rama {
        return Some((*f).clone());
    }
    candidatas.iter().filter(|(_, pv)| *pv > actual).min_by(|a, b| a.1.cmp(&b.1)).map(|(f, _)| (*f).clone())
}

// Version minima que corrige TODAS las vulnerabilidades de un paquete.
// Devuelve (version, cruza_rama_mayor); None si alguna no tiene correccion.
pub fn version_requerida(instalada: &str, vulns: &[&Hallazgo]) -> Option<(String, bool)> {
    let mut elegidas: Vec<(Version, String)> = Vec::new();
    for v in vulns {
        let fx = elegir_version_corregida(instalada, &v.fixed_versions)?;
        let pv = parsear(&fx)?;
        elegidas.push((pv, fx));
    }
    let (tope, nombre) = elegidas.iter().max_by(|a, b| a.0.cmp(&b.0))?.clone();
    let cruza = parsear(instalada).map(|c| tope.mayor() != c.mayor()).unwrap_or(false);
    Some((nombre, cruza))
}

// ─────────────────────────────────────────────────────────────────────────
// Ejecucion de Trivy
// ─────────────────────────────────────────────────────────────────────────

pub fn argumentos_trivy(objetivo: &str, plataforma: &str, salida: &str) -> Result<Vec<String>, String> {
    if !es_plataforma(plataforma) {
        return Err(format!("Plataforma no valida: {:?}", plataforma));
    }
    if let Some((_, d)) = objetivo.rsplit_once('@') {
        if !es_digest(d) {
            return Err("La imagen debe estar fijada por un digest sha256 valido.".into());
        }
    } else {
        return Err("La imagen debe estar fijada por digest antes de escanearla.".into());
    }

    let args: Vec<String> = [
        "image", "--quiet", "--skip-version-check", "--format", "json",
        "--timeout", &format!("{}s", TIMEOUT_ESCANEO_SEGUNDOS),
        "--scanners", "vuln", "--list-all-pkgs", "--severity", &SEVERIDADES.join(","),
        "--image-src", "remote", "--platform", plataforma,
        "--output", salida,
        objetivo,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // Defensa en profundidad: ningun flag que oculte hallazgos.
    if let Some(malo) = args.iter().find(|a| FLAGS_PROHIBIDOS.contains(&a.as_str())) {
        return Err(format!("Flag prohibido por politica: {}", malo));
    }
    Ok(args)
}

fn directorio_temporal() -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("devops-hub-trivy-{}-{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear el directorio temporal: {}", e))?;
    Ok(dir)
}

// Entorno minimo: el escaneo no hereda variables que puedan cambiar su
// comportamiento (TRIVY_*, configuracion de ignorados, etc.).
fn entorno_minimo() -> Vec<(String, String)> {
    ["PATH", "HOME", "TMPDIR", "HTTPS_PROXY", "HTTP_PROXY", "NO_PROXY", "SSL_CERT_FILE"]
        .iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
        .collect()
}

pub async fn version_trivy() -> Option<Value> {
    let salida = tokio::process::Command::new("trivy").args(["version", "--format", "json"]).output().await.ok()?;
    if !salida.status.success() {
        return None;
    }
    serde_json::from_slice(&salida.stdout).ok()
}

// Escanea una imagen de un registro (referencia ya fijada por digest).
pub async fn escanear_trivy(referencia_fijada: &str, plataforma: &str) -> Result<Escaneo, String> {
    // cwd vacio: Trivy no puede leer un .trivyignore del entorno.
    let dir = directorio_temporal()?;
    let reporte = dir.join("report.json");
    let args = argumentos_trivy(referencia_fijada, plataforma, &reporte.to_string_lossy())?;

    let mut cmd = tokio::process::Command::new("trivy");
    cmd.args(&args).current_dir(&dir).env_clear().envs(entorno_minimo()).kill_on_drop(true);

    let resultado = tokio::time::timeout(Duration::from_secs(TIMEOUT_ESCANEO_SEGUNDOS + 60), cmd.output()).await;
    let limpiar = || {
        let _ = std::fs::remove_dir_all(&dir);
    };

    let salida = match resultado {
        Err(_) => {
            limpiar();
            return Err("Trivy supero el tiempo maximo de escaneo.".into());
        }
        Ok(Err(e)) => {
            limpiar();
            return Err(format!("No se pudo ejecutar Trivy: {}. Esta instalado y en el PATH?", e));
        }
        Ok(Ok(s)) => s,
    };

    if !salida.status.success() || !reporte.exists() {
        let err = String::from_utf8_lossy(&salida.stderr).to_string();
        limpiar();
        let cola: String = err.chars().rev().take(1500).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(format!("Trivy fallo (codigo {:?}): {}", salida.status.code(), cola.trim()));
    }

    let texto_json = std::fs::read_to_string(&reporte).map_err(|e| format!("No se pudo leer el reporte de Trivy: {}", e));
    limpiar();
    let raw: Value = serde_json::from_str(&texto_json?).map_err(|e| format!("Reporte de Trivy invalido: {}", e))?;

    let mut escaneo = normalizar_trivy(&raw, referencia_fijada);
    let ver = version_trivy().await;
    escaneo.scanner.version = ver.as_ref().and_then(|v| v.get("Version")).and_then(|v| v.as_str()).map(String::from).or(escaneo.scanner.version);
    escaneo.scanner.vulnerability_db = ver.as_ref().and_then(|v| v.get("VulnerabilityDB").cloned());
    escaneo.scanner.args = args.into_iter().filter(|a| !a.starts_with('/')).collect();
    Ok(escaneo)
}

// Docker Scout no ofrece una salida JSON de vulnerabilidades como Trivy: sus
// formatos son texto, SARIF, SPDX, markdown y GitLab. Hasta contar con una salida
// real de `docker scout cves --format sarif` contra la que probar el lector, no
// se simula: se informa claramente.
pub async fn escanear_scout(_referencia: &str, _plataforma: &str) -> Result<Escaneo, String> {
    Err("Docker Scout aun no esta soportado en este modulo: falta validar el lector de su salida SARIF contra una ejecucion real. Use Trivy.".into())
}

pub async fn escanear(escaner: Escaner, referencia_fijada: &str, plataforma: &str) -> Result<Escaneo, String> {
    match escaner {
        Escaner::Trivy => escanear_trivy(referencia_fijada, plataforma).await,
        Escaner::DockerScout => escanear_scout(referencia_fijada, plataforma).await,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    const FIXTURE: &str = include_str!("testdata/trivy_node26_alpine324.json");
    const ORO: &str = include_str!("testdata/oro.json");

    fn oro() -> Value {
        serde_json::from_str(ORO).unwrap()
    }

    pub fn escaneo_fixture() -> Escaneo {
        let raw: Value = serde_json::from_str(FIXTURE).unwrap();
        let digest = oro()["digest"].as_str().unwrap().to_string();
        normalizar_trivy(&raw, &format!("node@{}", digest))
    }

    #[test]
    fn la_normalizacion_coincide_con_la_de_python() {
        let esc = escaneo_fixture();
        let oro = oro();
        let o = &oro["scan"];

        assert_eq!(serde_json::to_value(&esc.counts).unwrap(), o["counts"], "conteos");
        assert_eq!(esc.packages.len() as u64, o["paquetes"].as_u64().unwrap());
        assert_eq!(esc.os.family, o["os"]["family"].as_str().map(String::from));
        assert_eq!(esc.os.name, o["os"]["name"].as_str().map(String::from));
        assert_eq!(esc.os.eosl, o["os"]["eosl"].as_bool().unwrap());

        let esperadas = o["vulns"].as_array().unwrap();
        assert_eq!(esc.vulnerabilities.len(), esperadas.len());
        for (v, e) in esc.vulnerabilities.iter().zip(esperadas) {
            let obtenido = json!({
                "id": v.id, "severity": v.severity, "pkg_name": v.pkg_name, "installed_version": v.installed_version,
                "fixed_version": v.fixed_version, "fixed_versions": v.fixed_versions, "class": v.class,
                "pkg_path": v.pkg_path, "ecosystem": v.ecosystem, "status": v.status,
            });
            assert_eq!(&obtenido, e, "hallazgo {}", v.id);
        }
    }

    #[test]
    fn el_digest_y_las_variables_salen_del_escaneo() {
        let esc = escaneo_fixture();
        assert_eq!(esc.image.env.get("NODE_VERSION").map(String::as_str), Some("26.12.0"));
        assert!(esc.image.env.contains_key("PATH"));
    }

    #[test]
    fn corregir_elige_la_mas_cercana_de_la_misma_rama() {
        let f = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(elegir_version_corregida("6.20.0", &f(&["6.28.1", "7.5.0"])), Some("6.28.1".into()));
        assert_eq!(elegir_version_corregida("5.0.1", &f(&["5.0.11", "5.0.5"])), Some("5.0.5".into()));
        // sin version de la misma rama: la minima superior (cruza rama mayor)
        assert_eq!(elegir_version_corregida("3.1.0", &f(&["5.0.0", "4.0.2"])), Some("4.0.2".into()));
        assert_eq!(elegir_version_corregida("9.0.0", &f(&["5.0.0"])), None);
        assert_eq!(elegir_version_corregida("1.0.0", &[]), None);
    }

    #[test]
    fn los_argumentos_nunca_ocultan_hallazgos() {
        let h = format!("docker.io/library/node@sha256:{}", "a".repeat(64));
        let args = argumentos_trivy(&h, "linux/amd64", "/tmp/r.json").unwrap();
        for prohibido in FLAGS_PROHIBIDOS {
            assert!(!args.iter().any(|a| a == prohibido), "{}", prohibido);
        }
        assert!(args.iter().any(|a| a == "--list-all-pkgs"));
        let sev = args.iter().position(|a| a == "--severity").unwrap();
        assert_eq!(args[sev + 1], "CRITICAL,HIGH,MEDIUM,LOW,UNKNOWN");
        assert_eq!(args.last().unwrap(), &h);
    }

    #[test]
    fn rechaza_imagenes_sin_digest_y_plataformas_raras() {
        assert!(argumentos_trivy("docker.io/library/node:latest", "linux/amd64", "/tmp/r").is_err());
        let h = format!("docker.io/library/node@sha256:{}", "a".repeat(64));
        assert!(argumentos_trivy(&h, "windows/amd64", "/tmp/r").is_err());
        assert!(argumentos_trivy("docker.io/library/node@sha256:xyz", "linux/amd64", "/tmp/r").is_err());
    }

    #[test]
    fn el_escaner_se_elige_por_nombre() {
        assert_eq!(Escaner::desde_texto("Trivy").unwrap(), Escaner::Trivy);
        assert_eq!(Escaner::desde_texto("scout").unwrap(), Escaner::DockerScout);
        assert!(Escaner::desde_texto("grype").is_err());
    }
}
