// Modelo de configuracion de hardening y perfiles reutilizables.
// Port de hardening/config.py de Container Hardening Studio.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::componentes::{catalogo_para, Detectados};
use super::validar::{es_nombre_apk, es_ruta_absoluta, es_usuario};

pub const IDS_COMPONENTES_NODE: [&str; 12] = [
    "npm", "npx", "corepack", "yarn", "pnpm", "build_tools", "python", "git", "curl", "ca_certs", "shell", "pkg_manager",
];

fn d_ecosistema() -> String { "node".into() }
fn d_perfil() -> String { "Personalizado".into() }
fn d_true() -> bool { true }
fn d_usuario() -> String { "node".into() }
fn d_uid() -> u32 { 1000 }
fn d_workdir() -> String { "/app".into() }
fn d_intentos() -> u32 { 3 }

// Seleccion del usuario. keep[id]=true conserva (o instala) el componente en la imagen final.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigHardening {
    #[serde(default = "d_ecosistema")]
    pub ecosystem: String,
    #[serde(default = "d_perfil")]
    pub profile: String,
    #[serde(default)]
    pub keep: BTreeMap<String, bool>,
    #[serde(default = "d_true")]
    pub runtime_only: bool,
    #[serde(default = "d_true")]
    pub non_root: bool,
    #[serde(default)]
    pub allow_install_deps: bool,
    #[serde(default = "d_usuario")]
    pub run_user: String,
    #[serde(default = "d_uid")]
    pub run_uid: u32,
    #[serde(default = "d_workdir")]
    pub workdir: String,
    #[serde(default = "d_intentos")]
    pub max_attempts: u32,
    #[serde(default)]
    pub extra_apk: Vec<String>,
    // rutas adicionales de la imagen original a conservar
    #[serde(default)]
    pub preserve_paths: Vec<String>,
    // Autorizaciones explicitas: sin ellas el planificador solo SUGIERE estos cambios.
    #[serde(default)]
    pub authorize_remove_components: Vec<String>,
    // actualizar npm dentro de su misma rama mayor (no cambia Node.js)
    #[serde(default = "d_true")]
    pub allow_npm_upgrade: bool,
    #[serde(default)]
    pub authorize_base_change: bool,
    #[serde(default)]
    pub authorize_runtime_change: bool,
    // p. ej. "alpine:3.25" (requiere authorize_base_change)
    #[serde(default)]
    pub builder_base_override: Option<String>,
    #[serde(default)]
    pub network_tests: Option<bool>,
}

impl Default for ConfigHardening {
    fn default() -> Self {
        serde_json::from_str("{}").expect("la configuracion por defecto siempre es valida")
    }
}

impl ConfigHardening {
    pub fn conserva(&self, id: &str) -> bool {
        self.keep.get(id).copied().unwrap_or(false)
    }

    // Lo que pydantic validaba al construir la configuracion en Python.
    pub fn validar(&self) -> Result<(), String> {
        if !(1..=8).contains(&self.max_attempts) {
            return Err("max_attempts debe estar entre 1 y 8".into());
        }
        for n in &self.extra_apk {
            if !es_nombre_apk(n) {
                return Err(format!("Nombre de paquete apk inválido: {:?}", n));
            }
        }
        for p in &self.preserve_paths {
            if !es_ruta_absoluta(p) || p.split('/').any(|t| t == "..") {
                return Err(format!("Ruta inválida: {:?}", p));
            }
        }
        if !es_usuario(&self.run_user) {
            return Err("Nombre de usuario inválido".into());
        }
        // Studio no valida el directorio de trabajo y lo escribe sin comillas en el
        // Dockerfile; aqui se restringe al mismo alfabeto que las demas rutas.
        if !es_ruta_absoluta(&self.workdir) || self.workdir.split('/').any(|t| t == "..") {
            return Err(format!("Directorio de trabajo inválido: {:?}", self.workdir));
        }
        Ok(())
    }
}

fn todos_apagados() -> BTreeMap<String, bool> {
    IDS_COMPONENTES_NODE.iter().map(|id| (id.to_string(), false)).collect()
}

fn mantener(ids: &[&str]) -> BTreeMap<String, bool> {
    let mut m = todos_apagados();
    for id in ids {
        m.insert(id.to_string(), true);
    }
    m
}

#[derive(Debug, Clone, Serialize)]
pub struct Perfil {
    pub nombre: String,
    pub descripcion: String,
    pub config: ConfigHardening,
}

// Perfiles incluidos, en el orden en que se muestran.
pub fn perfiles() -> Vec<Perfil> {
    let base = |keep: BTreeMap<String, bool>, runtime_only: bool, allow_install_deps: bool| ConfigHardening {
        keep,
        runtime_only,
        non_root: true,
        allow_install_deps,
        ..ConfigHardening::default()
    };
    let p = |nombre: &str, descripcion: &str, mut config: ConfigHardening| {
        config.profile = nombre.to_string();
        Perfil { nombre: nombre.to_string(), descripcion: descripcion.to_string(), config }
    };

    vec![
        p("Runtime mínimo", "Solo Node.js, certificados CA y usuario no root. Sin npm, shell ni gestor de paquetes.",
          base(mantener(&["ca_certs"]), true, false)),
        p("Imagen de producción", "Runtime mínimo con shell para diagnóstico controlado.",
          base(mantener(&["ca_certs", "shell"]), true, false)),
        p("Imagen para compilación", "npm, npx, Corepack, shell, Python y herramientas de compilación para dependencias nativas.",
          base(mantener(&["npm", "npx", "corepack", "shell", "ca_certs", "python", "build_tools", "git"]), false, true)),
        p("Imagen de desarrollo", "Todas las herramientas de desarrollo Node.js conservadas.",
          base(mantener(&["npm", "npx", "corepack", "yarn", "pnpm", "shell", "ca_certs", "git", "curl", "pkg_manager"]), false, true)),
        p("Perfil corporativo bancario", "Runtime mínimo con controles BANK-STRICT: sin shell, sin gestor de paquetes, usuario no root.",
          ConfigHardening { max_attempts: 3, ..base(mantener(&["ca_certs"]), true, false) }),
        p("Node.js manteniendo npm", "Conserva npm y npx e intenta corregir sus dependencias (escenario B).",
          base(mantener(&["npm", "npx", "ca_certs", "shell"]), false, true)),
    ]
}

pub fn config_desde_perfil(nombre: &str) -> Option<ConfigHardening> {
    perfiles().into_iter().find(|p| p.nombre == nombre).map(|p| p.config)
}

// Completa claves ausentes y aplica implicaciones triviales (npx depende de npm).
pub fn normalizar(cfg: &ConfigHardening) -> ConfigHardening {
    let mut keep = todos_apagados();
    for (k, v) in &cfg.keep {
        keep.insert(k.clone(), *v);
    }
    if !keep.get("npm").copied().unwrap_or(false) {
        keep.insert("npx".into(), false);
    }
    ConfigHardening { keep, ..cfg.clone() }
}

// Desactiva los componentes de Node que no existen en la imagen origen (por
// ejemplo Corepack en Node 26) y que no se pueden instalar. Devuelve la
// configuracion ajustada y los ids que se descartaron.
pub fn adaptar(cfg: &ConfigHardening, detectados: &Detectados) -> (ConfigHardening, Vec<String>) {
    let mut keep = normalizar(cfg).keep;
    let mut descartados: Vec<String> = Vec::new();

    for comp in catalogo_para(&cfg.ecosystem) {
        let presente = detectados.components.iter().find(|c| c.id == comp.id).map(|c| c.present).unwrap_or(false);
        if keep.get(comp.id).copied().unwrap_or(false) && comp.apk.is_empty() && !presente {
            keep.insert(comp.id.to_string(), false);
            descartados.push(comp.id.to_string());
        }
    }
    if !keep.get("npm").copied().unwrap_or(false) {
        keep.insert("npx".into(), false);
    }
    let mut salida = ConfigHardening { keep, ..cfg.clone() };
    if !descartados.is_empty() && !salida.conserva("npm") && salida.allow_install_deps {
        salida.allow_install_deps = false;
    }
    (salida, descartados)
}
