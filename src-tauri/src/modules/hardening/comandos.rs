// Comandos Tauri del modulo Hardening.
//
// Flujo (fase 1: analizar, elegir opciones y generar el Dockerfile):
//   hardening_herramientas  que escaneres y que runtime de contenedores hay
//   hardening_perfiles      perfiles incluidos
//   hardening_analizar      resuelve la imagen, la escanea y deja una sesion
//   hardening_evaluar       con una seleccion de componentes: impacto + plan
//   hardening_generar       Dockerfile endurecido (resuelve el constructor)
//   hardening_guardar       escribe el contexto de construccion en disco
//
// La sesion guarda el escaneo en memoria para que evaluar (que se llama con
// cada casilla que el usuario cambia) no repita la red ni el escaneo.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, State};

use super::super::eks::config::RefreshError;
use super::super::eks::permissions::emit_progress;
use super::componentes::{detectar_componentes, detectar_ecosistema, opciones_otros_ecosistemas, propietario_de, Detectados};
use super::config::{adaptar, config_desde_perfil, perfiles, ConfigHardening, Perfil};
use super::dockerfile::{generar_node_alpine, DocDockerfile, EntradaDockerfile, ImagenOrigen};
use super::escaner::{escanear, version_trivy, Escaner, Escaneo, InfoEscaner, SistemaOperativo};
use super::impacto::{analizar, Analisis};
use super::planificador::{construir_plan, recomendar, Plan, Recomendacion};
use super::referencia::parsear_referencia;
use super::publicacion::{pasos as pasos_publicacion, script as script_publicacion, DestinoEcr, PasoPublicacion};
use super::registro::{resolver, resolver_constructor, ImagenResuelta};

const MODULO: &str = "hardening";
const REGISTRO_NPM_POR_DEFECTO: &str = "https://registry.npmjs.org/";
const PERFIL_INICIAL_NODE: &str = "Node.js manteniendo npm";
const MAX_SESIONES: usize = 8;
const FIRMA_DOCKERFILE: &str = "DevOps Hub (motor de Container Hardening Studio)";

fn err(etapa: &str, mensaje: impl Into<String>) -> RefreshError {
    RefreshError::new(etapa, mensaje)
}

// ─────────────────────────────────────────────────────────────────────────
// Sesiones
// ─────────────────────────────────────────────────────────────────────────

struct Sesion {
    imagen: ImagenResuelta,
    escaneo: Escaneo,
    detectados: Detectados,
    ecosistema: String,
    generado: Option<Generado>,
}

#[derive(Clone)]
struct Generado {
    doc: DocDockerfile,
    pasos: Vec<PasoPublicacion>,
    script: String,
}

#[derive(Default)]
pub struct EstadoHardening {
    sesiones: Mutex<(Vec<String>, HashMap<String, Sesion>)>,
}

impl EstadoHardening {
    fn guardar(&self, id: String, sesion: Sesion) {
        let mut g = self.sesiones.lock().unwrap();
        g.0.push(id.clone());
        g.1.insert(id, sesion);
        while g.0.len() > MAX_SESIONES {
            let viejo = g.0.remove(0);
            g.1.remove(&viejo);
        }
    }

    fn con<T>(&self, id: &str, f: impl FnOnce(&mut Sesion) -> Result<T, RefreshError>) -> Result<T, RefreshError> {
        let mut g = self.sesiones.lock().unwrap();
        match g.1.get_mut(id) {
            Some(s) => f(s),
            None => Err(err("sesion", "La sesión de análisis ya no existe; vuelva a analizar la imagen.")),
        }
    }
}

fn id_nuevo() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:x}", nanos)
}

// ─────────────────────────────────────────────────────────────────────────
// Herramientas
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct Herramienta {
    pub instalada: bool,
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Herramientas {
    pub trivy: Herramienta,
    pub scout: Herramienta,
    // Docker (o compatible): necesario para construir y probar la imagen, no para analizar.
    pub docker: Herramienta,
}

async fn salida_de(programa: &str, args: &[&str]) -> Option<String> {
    let fut = tokio::process::Command::new(programa).args(args).kill_on_drop(true).output();
    let s = tokio::time::timeout(Duration::from_secs(15), fut).await.ok()?.ok()?;
    if !s.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&s.stdout).trim().to_string())
}

#[tauri::command]
pub async fn hardening_herramientas() -> Herramientas {
    let trivy = version_trivy().await.and_then(|v| v.get("Version").and_then(|x| x.as_str()).map(String::from));
    let docker = salida_de("docker", &["version", "--format", "{{.Client.Version}}"]).await;
    let scout = match &docker {
        Some(_) => salida_de("docker", &["scout", "version"]).await.map(|t| t.lines().next().unwrap_or("").to_string()),
        None => None,
    };
    Herramientas {
        trivy: Herramienta { instalada: trivy.is_some(), version: trivy },
        scout: Herramienta { instalada: scout.is_some(), version: scout },
        docker: Herramienta { instalada: docker.is_some(), version: docker },
    }
}

#[tauri::command]
pub fn hardening_perfiles() -> Vec<Perfil> {
    perfiles()
}

// ─────────────────────────────────────────────────────────────────────────
// Analizar
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct HallazgoVista {
    pub id: String,
    pub severity: String,
    pub pkg_name: String,
    pub installed_version: String,
    pub fixed_version: Option<String>,
    pub class: String,
    pub pkg_path: String,
    // componente del catalogo al que pertenece (npm, shell, os-base...)
    pub owner: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct Soporte {
    // El generador de Dockerfile cubre Node.js sobre Alpine.
    pub generador: bool,
    pub motivo: Option<String>,
    // Componentes tipicos del ecosistema, solo informativos cuando no hay generador.
    pub componentes_tipicos: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Evaluacion {
    // Seleccion ya ajustada a lo que existe en la imagen.
    pub adaptada: ConfigHardening,
    // Componentes pedidos que no existen en la imagen origen y se descartaron.
    pub descartados: Vec<String>,
    pub analisis: Analisis,
    pub plan: Plan,
    pub recomendaciones: Vec<Recomendacion>,
}

#[derive(Debug, Serialize)]
pub struct ResultadoAnalisis {
    pub id_sesion: String,
    pub imagen: ImagenResuelta,
    pub scanner: InfoEscaner,
    pub os: SistemaOperativo,
    pub counts: std::collections::BTreeMap<String, usize>,
    pub ecosistema: String,
    pub hallazgos: Vec<HallazgoVista>,
    pub detectados: Detectados,
    pub soporte: Soporte,
    pub perfil_inicial: String,
    pub config_inicial: ConfigHardening,
    pub evaluacion: Evaluacion,
}

fn soporte_de(ecosistema: &str, os: &SistemaOperativo) -> Soporte {
    if ecosistema != "node" {
        return Soporte {
            generador: false,
            motivo: Some(format!("El generador de Dockerfile solo cubre Node.js; esta imagen se detectó como «{}». Se muestra el análisis de vulnerabilidades.", ecosistema)),
            componentes_tipicos: opciones_otros_ecosistemas(ecosistema).iter().map(|s| s.to_string()).collect(),
        };
    }
    if os.family.as_deref() != Some("alpine") {
        return Soporte {
            generador: false,
            motivo: Some(format!(
                "El generador de Dockerfile solo cubre bases Alpine; esta imagen usa «{}». Se muestra el análisis de vulnerabilidades.",
                os.family.as_deref().unwrap_or("desconocida")
            )),
            componentes_tipicos: Vec::new(),
        };
    }
    Soporte { generador: true, motivo: None, componentes_tipicos: Vec::new() }
}

fn evaluar_interno(sesion: &Sesion, cfg: &ConfigHardening) -> Result<Evaluacion, RefreshError> {
    cfg.validar().map_err(|m| err("configuracion", m))?;
    let (adaptada, descartados) = adaptar(cfg, &sesion.detectados);
    Ok(Evaluacion {
        analisis: analizar(&adaptada, &sesion.escaneo),
        plan: construir_plan(&adaptada, &sesion.escaneo, 1, None),
        recomendaciones: recomendar(&adaptada, &sesion.escaneo),
        adaptada,
        descartados,
    })
}

// Arma el resultado del analisis y la sesion a partir de la imagen resuelta y
// su escaneo. Sin red: es la parte que se prueba con el escaneo guardado.
fn construir_analisis(imagen: ImagenResuelta, escaneo: Escaneo) -> Result<(ResultadoAnalisis, Sesion), RefreshError> {
    // El entorno de la imagen sale del registro; el del escaneo es respaldo.
    let mut entorno = escaneo.image.env.clone();
    entorno.extend(imagen.env.clone());
    let ecosistema = detectar_ecosistema(&entorno).to_string();
    let detectados = detectar_componentes(&escaneo, &ecosistema);

    // Una imagen de desarrollo casi siempre necesita npm: se parte de ese perfil.
    let base = if ecosistema == "node" {
        config_desde_perfil(PERFIL_INICIAL_NODE).unwrap_or_default()
    } else {
        ConfigHardening { ecosystem: ecosistema.clone(), ..ConfigHardening::default() }
    };

    let sesion = Sesion { imagen: imagen.clone(), escaneo, detectados: detectados.clone(), ecosistema: ecosistema.clone(), generado: None };
    let evaluacion = evaluar_interno(&sesion, &base)?;
    let hallazgos = sesion
        .escaneo
        .vulnerabilities
        .iter()
        .map(|v| HallazgoVista {
            id: v.id.clone(),
            severity: v.severity.clone(),
            pkg_name: v.pkg_name.clone(),
            installed_version: v.installed_version.clone(),
            fixed_version: v.fixed_version.clone(),
            class: v.class.clone(),
            pkg_path: v.pkg_path.clone(),
            owner: propietario_de(v, &ecosistema),
            title: v.title.clone(),
        })
        .collect();
    let resultado = ResultadoAnalisis {
        id_sesion: id_nuevo(),
        imagen,
        scanner: sesion.escaneo.scanner.clone(),
        os: sesion.escaneo.os.clone(),
        counts: sesion.escaneo.counts.clone(),
        soporte: soporte_de(&ecosistema, &sesion.escaneo.os),
        ecosistema,
        hallazgos,
        detectados,
        perfil_inicial: if base.ecosystem == "node" { PERFIL_INICIAL_NODE.into() } else { base.profile.clone() },
        config_inicial: evaluacion.adaptada.clone(),
        evaluacion,
    };
    Ok((resultado, sesion))
}

#[tauri::command]
pub async fn hardening_analizar(
    app: AppHandle,
    estado: State<'_, EstadoHardening>,
    referencia: String,
    plataforma: String,
    escaner: String,
) -> Result<ResultadoAnalisis, RefreshError> {
    let motor = Escaner::desde_texto(&escaner).map_err(|m| err("escaner", m))?;
    let r = parsear_referencia(&referencia).map_err(|m| err("referencia", m))?;

    emit_progress(MODULO, &app, "Resolviendo la imagen en el registro", &r.mostrar(), 1, 3, "running");
    let imagen = match resolver(&r, &plataforma).await {
        Ok(i) => i,
        Err(m) => {
            emit_progress(MODULO, &app, "No se pudo resolver la imagen", &r.mostrar(), 1, 3, "error");
            return Err(err("resolver_imagen", m));
        }
    };
    emit_progress(MODULO, &app, &format!("Imagen fijada: {}", imagen.pinned_ref), &r.mostrar(), 1, 3, "ok");

    emit_progress(MODULO, &app, "Escaneando vulnerabilidades (puede tardar varios minutos la primera vez)", &r.mostrar(), 2, 3, "running");
    let escaneo = match escanear(motor, &imagen.pinned_ref, &plataforma).await {
        Ok(e) => e,
        Err(m) => {
            emit_progress(MODULO, &app, "El escaneo falló", &r.mostrar(), 2, 3, "error");
            return Err(err("escanear", m));
        }
    };
    let total_hc = escaneo.counts.get("CRITICAL").copied().unwrap_or(0) + escaneo.counts.get("HIGH").copied().unwrap_or(0);
    emit_progress(
        MODULO,
        &app,
        &format!("Escaneo terminado: {} hallazgos, {} HIGH/CRITICAL", escaneo.vulnerabilities.len(), total_hc),
        &r.mostrar(),
        2,
        3,
        "ok",
    );

    let (resultado, sesion) = construir_analisis(imagen, escaneo)?;
    estado.guardar(resultado.id_sesion.clone(), sesion);
    emit_progress(MODULO, &app, "Análisis listo: elija qué conservar", &r.mostrar(), 3, 3, "ok");
    Ok(resultado)
}

#[tauri::command]
pub fn hardening_evaluar(estado: State<'_, EstadoHardening>, id_sesion: String, config: ConfigHardening) -> Result<Evaluacion, RefreshError> {
    estado.con(&id_sesion, |s| evaluar_interno(s, &config))
}

// ─────────────────────────────────────────────────────────────────────────
// Generar y guardar
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ResultadoGeneracion {
    pub doc: DocDockerfile,
    pub builder_ref: String,
    pub etiqueta_sugerida: String,
    // Destino en ECR ya resuelto (con el tag completado y validado).
    pub destino_ecr: DestinoEcr,
    pub pasos: Vec<PasoPublicacion>,
    pub evaluacion: Evaluacion,
}

fn etiqueta_sugerida(img: &ImagenResuelta) -> String {
    let nombre = img.repository.rsplit('/').next().unwrap_or("imagen");
    let version = img.tag.clone().unwrap_or_else(|| img.manifest_digest.chars().skip(7).take(12).collect());
    format!("{}:{}-hardened", nombre, version)
}

// Comprobaciones previas, antes de gastar red en resolver el constructor.
fn exigir_generable(evaluacion: &Evaluacion) -> Result<(), RefreshError> {
    let errores: Vec<&str> = evaluacion.analisis.issues.iter().filter(|i| i.level == "error").map(|i| i.message.as_str()).collect();
    if !errores.is_empty() {
        return Err(err("configuracion", format!("Configuración inválida: {}", errores.join("; "))));
    }
    if !evaluacion.plan.blockers.is_empty() {
        let b: Vec<String> = evaluacion.plan.blockers.iter().map(|b| format!("{} ({})", b.reason, b.cves.join(", "))).collect();
        return Err(err(
            "bloqueo",
            format!("Con esta selección no existe una estrategia que cumpla la política: {}. Elija una alternativa (por ejemplo, autorizar eliminar el componente).", b.join(" | ")),
        ));
    }
    Ok(())
}

// Genera el Dockerfile con el constructor ya resuelto. Sin red.
fn generar_documento(
    sesion: &Sesion,
    evaluacion: Evaluacion,
    builder_ref: String,
    registro: &str,
    pruebas_red: bool,
    nonce: &str,
    destino: &DestinoEcr,
) -> Result<(ResultadoGeneracion, String), RefreshError> {
    let imagen = &sesion.imagen;
    let origen = ImagenOrigen {
        pinned_ref: imagen.pinned_ref.clone(),
        env: imagen.env.clone(),
        os_family: sesion.escaneo.os.family.clone(),
        os_name: sesion.escaneo.os.name.clone(),
    };
    let doc = generar_node_alpine(&EntradaDockerfile {
        cfg: &evaluacion.adaptada,
        plan: &evaluacion.plan,
        imagen: &origen,
        detectados: &sesion.detectados,
        builder_ref: &builder_ref,
        network_tests: pruebas_red,
        npm_registry: registro,
        nonce,
        generador: FIRMA_DOCKERFILE,
    })
    .map_err(|e| err("generar", e.to_string()))?;

    let etiqueta = etiqueta_sugerida(imagen);
    let nombre = imagen.repository.rsplit('/').next().unwrap_or("imagen");
    let tag_origen = imagen.tag.clone().unwrap_or_else(|| imagen.manifest_digest.chars().skip(7).take(12).collect());
    let destino = destino.resuelto(nombre, &tag_origen).map_err(|m| err("destino_ecr", m))?;
    let pasos = pasos_publicacion(&etiqueta, &destino);
    let script = script_publicacion(&etiqueta, &destino, &imagen.pinned_ref, &doc.sha256);
    Ok((ResultadoGeneracion { doc, builder_ref, etiqueta_sugerida: etiqueta, destino_ecr: destino, pasos, evaluacion }, script))
}

#[tauri::command]
pub async fn hardening_generar(
    app: AppHandle,
    estado: State<'_, EstadoHardening>,
    id_sesion: String,
    config: ConfigHardening,
    npm_registry: Option<String>,
    pruebas_red: Option<bool>,
    destino_ecr: Option<DestinoEcr>,
) -> Result<ResultadoGeneracion, RefreshError> {
    // El destino se valida antes de gastar red.
    let destino = destino_ecr.unwrap_or_default();
    // Se copia lo necesario de la sesion para no retener el candado durante la red.
    let (evaluacion, nombre_os, plataforma, referencia) = estado.con(&id_sesion, |s| {
        if s.ecosistema != "node" {
            return Err(err("generar", "El generador solo cubre imágenes Node.js."));
        }
        let ev = evaluar_interno(s, &config)?;
        exigir_generable(&ev)?;
        Ok((ev, s.escaneo.os.name.clone(), s.imagen.platform.clone(), s.imagen.reference.clone()))
    })?;

    emit_progress(MODULO, &app, "Resolviendo la imagen base del constructor", &referencia, 1, 2, "running");
    let builder_ref = resolver_constructor(nombre_os.as_deref(), evaluacion.adaptada.builder_base_override.as_deref(), &plataforma)
        .await
        .map_err(|m| err("constructor", m))?;

    let registro = npm_registry.unwrap_or_else(|| REGISTRO_NPM_POR_DEFECTO.to_string());
    let resultado = estado.con(&id_sesion, |s| {
        let (r, script) = generar_documento(s, evaluacion, builder_ref, &registro, pruebas_red.unwrap_or(true), &id_nuevo(), &destino)?;
        s.generado = Some(Generado { doc: r.doc.clone(), pasos: r.pasos.clone(), script });
        Ok(r)
    })?;
    emit_progress(MODULO, &app, "Dockerfile generado", &referencia, 2, 2, "ok");
    Ok(resultado)
}

fn carpeta_base() -> PathBuf {
    let casa = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_else(|_| ".".into());
    PathBuf::from(casa).join("devops-hub-hardening")
}

fn nombre_seguro(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '-' }).collect()
}

// Escribe el Dockerfile y los archivos que necesita al lado, listo para
// `docker buildx build .` desde esa carpeta.
#[tauri::command]
pub fn hardening_guardar(estado: State<'_, EstadoHardening>, id_sesion: String, directorio: Option<String>) -> Result<String, RefreshError> {
    let (generado, imagen) = estado.con(&id_sesion, |s| match &s.generado {
        Some(g) => Ok((g.clone(), s.imagen.clone())),
        None => Err(err("guardar", "Primero genere el Dockerfile.")),
    })?;

    let destino = match directorio.filter(|d| !d.trim().is_empty()) {
        Some(d) => PathBuf::from(d),
        None => {
            let marca = chrono::Local::now().format("%Y%m%d-%H%M%S");
            let tag = imagen.tag.clone().unwrap_or_else(|| "digest".into());
            carpeta_base().join(format!("{}-{}-{}", nombre_seguro(imagen.repository.rsplit('/').next().unwrap_or("imagen")), nombre_seguro(&tag), marca))
        }
    };
    std::fs::create_dir_all(&destino).map_err(|e| err("guardar", format!("No se pudo crear {}: {}", destino.display(), e)))?;

    let escribir = |ruta: PathBuf, contenido: &str| -> Result<(), RefreshError> {
        if let Some(padre) = ruta.parent() {
            std::fs::create_dir_all(padre).map_err(|e| err("guardar", format!("No se pudo crear {}: {}", padre.display(), e)))?;
        }
        std::fs::write(&ruta, contenido).map_err(|e| err("guardar", format!("No se pudo escribir {}: {}", ruta.display(), e)))
    };

    escribir(destino.join("Dockerfile"), &generado.doc.text)?;
    for (rel, contenido) in &generado.doc.files {
        // Las rutas las genera el modulo, pero no se confia: nada fuera de la carpeta.
        if rel.starts_with('/') || rel.split('/').any(|t| t == ".." || t.is_empty()) {
            return Err(err("guardar", format!("Ruta de archivo no válida: {}", rel)));
        }
        escribir(destino.join(rel), contenido)?;
    }
    escribir(destino.join("publicar.sh"), &generado.script)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let ruta = destino.join("publicar.sh");
        std::fs::set_permissions(&ruta, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| err("guardar", format!("No se pudo marcar como ejecutable {}: {}", ruta.display(), e)))?;
    }
    let comandos: Vec<String> = generado.pasos.iter().map(|p| format!("# {}\n{}\n", p.titulo, p.comando)).collect();
    escribir(
        destino.join("COMANDOS.txt"),
        &format!(
            "Imagen origen: {}\nHuella del Dockerfile (sha256): {}\n\nTodo en uno: ./publicar.sh (credenciales de AWS en la terminal).\nO paso a paso, desde esta carpeta:\n\n{}",
            imagen.pinned_ref,
            generado.doc.sha256,
            comandos.join("\n")
        ),
    )?;
    Ok(destino.to_string_lossy().to_string())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::Value;

    const FIXTURE: &str = include_str!("testdata/trivy_node26_alpine324.json");
    const ORO: &str = include_str!("testdata/oro.json");

    fn imagen_y_escaneo() -> (ImagenResuelta, Escaneo) {
        let oro: Value = serde_json::from_str(ORO).unwrap();
        let raw: Value = serde_json::from_str(FIXTURE).unwrap();
        let escaneo = super::super::escaner::normalizar_trivy(&raw, &format!("node@{}", oro["digest"].as_str().unwrap()));
        let i = &oro["image"];
        let imagen = ImagenResuelta {
            reference: i["reference"].as_str().unwrap().into(),
            registry: "docker.io".into(),
            repository: "library/node".into(),
            tag: Some("26.12.0-alpine3.24".into()),
            platform: "linux/amd64".into(),
            available_platforms: vec!["linux/amd64".into(), "linux/arm64".into()],
            index_digest: None,
            manifest_digest: oro["digest"].as_str().unwrap().into(),
            pinned_ref: i["pinned_ref"].as_str().unwrap().into(),
            created: None,
            os: Some("linux".into()),
            architecture: Some("amd64".into()),
            size_bytes: 62_903_952,
            layers: 3,
            env: i["env"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect(),
            user: String::new(),
            entrypoint: None,
            cmd: None,
            workdir: "/".into(),
            labels: Default::default(),
        };
        (imagen, escaneo)
    }

    #[test]
    fn el_analisis_parte_del_perfil_que_conserva_npm() {
        let (imagen, escaneo) = imagen_y_escaneo();
        let (r, _) = construir_analisis(imagen, escaneo).unwrap();
        assert_eq!(r.ecosistema, "node");
        assert!(r.soporte.generador);
        assert_eq!(r.perfil_inicial, "Node.js manteniendo npm");
        assert!(r.config_inicial.conserva("npm") && r.config_inicial.conserva("npx"));
        // Corepack no existe en Node 26: la seleccion inicial ya viene ajustada.
        assert!(!r.config_inicial.conserva("corepack"));
        assert_eq!(r.hallazgos.len(), 13);
        assert!(r.hallazgos.iter().any(|h| h.owner == "npm"));
    }

    #[test]
    fn genera_el_dockerfile_sin_red_y_bloquea_lo_invalido() {
        let (imagen, escaneo) = imagen_y_escaneo();
        let (r, sesion) = construir_analisis(imagen, escaneo).unwrap();
        let builder = "docker.io/library/alpine@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();

        exigir_generable(&r.evaluacion).unwrap();
        let (g, script) = generar_documento(&sesion, evaluar_interno(&sesion, &r.config_inicial).unwrap(), builder.clone(), REGISTRO_NPM_POR_DEFECTO, true, "t1", &DestinoEcr::default()).unwrap();
        assert!(g.doc.text.contains("FROM scratch AS final"));
        assert_eq!(g.etiqueta_sugerida, "node:26.12.0-alpine3.24-hardened");
        assert_eq!(g.pasos.len(), 8);
        assert_eq!(g.destino_ecr.tag, "node-26.12.0-alpine3.24-sec");
        assert!(script.contains("841964127262") && script.contains("node-26.12.0-alpine3.24-sec"));

        // Un destino invalido se rechaza sin llegar a generar nada.
        let malo = DestinoEcr { cuenta: "12".into(), ..DestinoEcr::default() };
        let ev = evaluar_interno(&sesion, &r.config_inicial).unwrap();
        assert!(generar_documento(&sesion, ev, builder.clone(), REGISTRO_NPM_POR_DEFECTO, true, "t1", &malo).is_err());

        // Un conflicto de configuracion se rechaza antes de tocar la red.
        let mut mala = r.config_inicial.clone();
        mala.keep.insert("npm".into(), false);
        mala.allow_install_deps = true;
        let ev = evaluar_interno(&sesion, &mala).unwrap();
        assert!(exigir_generable(&ev).is_err(), "instalar dependencias sin npm debe rechazarse");

        // Una configuracion no valida (usuario con caracteres peligrosos) ni siquiera se evalua.
        let mut peligrosa = r.config_inicial.clone();
        peligrosa.run_user = "x; rm -rf /".into();
        assert!(evaluar_interno(&sesion, &peligrosa).is_err());
        let mut ruta = r.config_inicial.clone();
        ruta.workdir = "/app; touch /pwn".into();
        assert!(evaluar_interno(&sesion, &ruta).is_err());
    }

    // Escribe las respuestas reales de los comandos para probar la interfaz sin el backend:
    // `HARDENING_VOLCAR=/ruta cargo test --lib hardening::comandos -- --ignored volcar`
    #[test]
    #[ignore]
    fn volcar_respuestas_para_la_interfaz() {
        let dir = std::env::var("HARDENING_VOLCAR").expect("define HARDENING_VOLCAR");
        let (imagen, escaneo) = imagen_y_escaneo();
        let (r, sesion) = construir_analisis(imagen, escaneo).unwrap();
        std::fs::write(format!("{}/analisis.json", dir), serde_json::to_string(&r).unwrap()).unwrap();
        let builder = "docker.io/library/alpine@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        let (g, _) = generar_documento(&sesion, evaluar_interno(&sesion, &r.config_inicial).unwrap(), builder, REGISTRO_NPM_POR_DEFECTO, true, "demo", &DestinoEcr::default()).unwrap();
        std::fs::write(format!("{}/generacion.json", dir), serde_json::to_string(&g).unwrap()).unwrap();
        std::fs::write(format!("{}/perfiles.json", dir), serde_json::to_string(&perfiles()).unwrap()).unwrap();

        // Evaluaciones de las selecciones que la interfaz produce en la demostracion.
        let inicial = r.config_inicial.clone();
        let mut sin_npm = inicial.clone();
        sin_npm.profile = "Personalizado".into();
        sin_npm.keep.insert("npm".into(), false);
        sin_npm.keep.insert("npx".into(), false);
        sin_npm.allow_install_deps = false;
        let mut sin_npm_ni_shell = sin_npm.clone();
        sin_npm_ni_shell.keep.insert("shell".into(), false);
        let mut variantes: Vec<ConfigHardening> = vec![inicial, sin_npm, sin_npm_ni_shell];
        variantes.extend(perfiles().into_iter().map(|p| p.config));
        let mut autorizado = r.config_inicial.clone();
        autorizado.authorize_remove_components = vec!["npm".into()];
        variantes.push(autorizado);
        let pares: Vec<serde_json::Value> = variantes
            .iter()
            .filter_map(|c| evaluar_interno(&sesion, c).ok().map(|e| serde_json::json!({"config": c, "evaluacion": e})))
            .collect();
        std::fs::write(format!("{}/evaluaciones.json", dir), serde_json::to_string(&pares).unwrap()).unwrap();
    }
}
