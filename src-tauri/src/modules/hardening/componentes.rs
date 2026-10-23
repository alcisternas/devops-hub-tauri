// Catalogo de componentes por ecosistema, deteccion a partir del escaneo y
// atribucion de cada hallazgo a su componente.
// Port de hardening/components.py de Container Hardening Studio.

use std::collections::BTreeMap;

use serde::Serialize;

use super::escaner::{Escaneo, Hallazgo, Paquete};

pub const NODE_GLOBAL: &str = "usr/local/lib/node_modules/";

#[derive(Debug, Clone, Copy)]
pub struct DefComponente {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    // node | system | security
    pub group: &'static str,
    // paquetes apk que lo constituyen (y que se instalan si se selecciona)
    pub apk: &'static [&'static str],
    // rutas (sin "/" inicial) de los paquetes de lenguaje que lo constituyen
    pub path_prefixes: &'static [&'static str],
    pub requires: &'static [&'static str],
}

pub const CATALOGO_NODE: [DefComponente; 12] = [
    DefComponente {
        id: "npm", label: "npm", group: "node", apk: &[], requires: &[],
        description: "Gestor de paquetes de Node.js (npm install, npm ci, npm run).",
        path_prefixes: &["usr/local/lib/node_modules/npm/"],
    },
    DefComponente {
        id: "npx", label: "npx", group: "node", apk: &[], requires: &["npm"],
        description: "Ejecutor de paquetes incluido con npm.",
        path_prefixes: &[],
    },
    DefComponente {
        id: "corepack", label: "Corepack", group: "node", apk: &[], requires: &[],
        description: "Administra yarn/pnpm de forma reproducible.",
        path_prefixes: &["usr/local/lib/node_modules/corepack/"],
    },
    DefComponente {
        id: "yarn", label: "Yarn", group: "node", apk: &[], requires: &[],
        description: "Gestor de paquetes Yarn.",
        path_prefixes: &["opt/yarn", "usr/local/lib/node_modules/yarn/"],
    },
    DefComponente {
        id: "pnpm", label: "pnpm", group: "node", apk: &[], requires: &[],
        description: "Gestor de paquetes pnpm.",
        path_prefixes: &["usr/local/lib/node_modules/pnpm/"],
    },
    DefComponente {
        id: "build_tools", label: "Herramientas de compilación", group: "system", path_prefixes: &[],
        description: "gcc, g++, make y cabeceras para módulos nativos.",
        apk: &["build-base", "gcc", "g++", "make", "musl-dev"], requires: &["shell", "python"],
    },
    DefComponente {
        id: "python", label: "Python (compilaciones nativas)", group: "system", path_prefixes: &[], requires: &[],
        description: "Requerido por node-gyp.",
        apk: &["python3"],
    },
    DefComponente {
        id: "git", label: "Git", group: "system", path_prefixes: &[], requires: &[],
        description: "Control de versiones (dependencias git).",
        apk: &["git"],
    },
    DefComponente {
        id: "curl", label: "curl", group: "system", path_prefixes: &[], requires: &[],
        description: "Cliente HTTP de línea de comandos.",
        apk: &["curl"],
    },
    DefComponente {
        id: "ca_certs", label: "Certificados CA", group: "security", path_prefixes: &[], requires: &[],
        description: "Necesarios para conexiones HTTPS/TLS.",
        apk: &["ca-certificates-bundle"],
    },
    DefComponente {
        id: "shell", label: "Shell (/bin/sh)", group: "system", path_prefixes: &[], requires: &[],
        description: "Intérprete de comandos y utilidades BusyBox.",
        apk: &["busybox", "busybox-binsh"],
    },
    DefComponente {
        id: "pkg_manager", label: "Gestor de paquetes del sistema (apk)", group: "system", path_prefixes: &[], requires: &[],
        description: "Permite instalar paquetes en tiempo de ejecución.",
        apk: &["apk-tools", "alpine-keys", "libapk"],
    },
];

// Opciones de otros ecosistemas, solo informativas: el generador soporta Node/Alpine.
pub fn opciones_otros_ecosistemas(eco: &str) -> &'static [&'static str] {
    match eco {
        "python" => &["pip", "setuptools", "herramientas de compilación"],
        "java" => &["JDK completo", "JRE", "Maven", "Gradle"],
        "go" => &["Compilador Go", "Binario final"],
        "dotnet" => &["SDK", "Runtime"],
        _ => &["Paquetes y herramientas instaladas"],
    }
}

pub fn catalogo_para(ecosistema: &str) -> &'static [DefComponente] {
    if ecosistema == "node" {
        &CATALOGO_NODE
    } else {
        &[]
    }
}

pub fn detectar_ecosistema(env: &BTreeMap<String, String>) -> &'static str {
    if env.contains_key("NODE_VERSION") {
        "node"
    } else if env.contains_key("PYTHON_VERSION") {
        "python"
    } else if env.contains_key("JAVA_VERSION") || env.contains_key("JAVA_HOME") {
        "java"
    } else if env.contains_key("GOLANG_VERSION") {
        "go"
    } else if env.contains_key("DOTNET_VERSION") || env.contains_key("ASPNET_VERSION") {
        "dotnet"
    } else {
        "generic"
    }
}

// Componente del catalogo (o categoria) al que pertenece un hallazgo.
pub fn propietario_de(v: &Hallazgo, ecosistema: &str) -> String {
    let ruta = v.pkg_path.trim_start_matches('/');
    let catalogo = catalogo_para(ecosistema);
    if v.class == "lang-pkgs" && !ruta.is_empty() {
        for comp in catalogo {
            if comp.path_prefixes.iter().any(|p| ruta.starts_with(p)) {
                return comp.id.to_string();
            }
        }
        if let Some(resto) = ruta.strip_prefix(NODE_GLOBAL) {
            return format!("global:{}", resto.split('/').next().unwrap_or(""));
        }
        return "app".to_string();
    }
    if v.class == "os-pkgs" {
        for comp in catalogo {
            if comp.apk.contains(&v.pkg_name.as_str()) {
                return comp.id.to_string();
            }
        }
        return "os-base".to_string();
    }
    "unknown".to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ComponenteDetectado {
    pub id: String,
    pub label: String,
    pub description: String,
    pub group: String,
    pub present: bool,
    // Se puede incluir aunque no exista en la imagen origen (paquete apk).
    // Campo propio del hub: Studio no lo tiene.
    pub installable: bool,
    pub version: Option<String>,
    pub packages: usize,
    pub high_critical: usize,
    pub unfixed_high_critical: usize,
    pub vulnerabilities: usize,
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PaqueteSuelto {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UbicacionLenguaje {
    pub path: String,
    pub packages: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Detectados {
    pub components: Vec<ComponenteDetectado>,
    pub other_os_packages: Vec<PaqueteSuelto>,
    pub other_language_locations: Vec<UbicacionLenguaje>,
    // Hay paquetes de lenguaje fuera del catalogo: imagen derivada con su propia aplicacion.
    pub derived_image_warning: bool,
}

pub fn detectar_componentes(scan: &Escaneo, ecosistema: &str) -> Detectados {
    let catalogo = catalogo_para(ecosistema);
    let es_hc = |v: &&&Hallazgo| v.severity == "HIGH" || v.severity == "CRITICAL";
    let mut componentes: Vec<ComponenteDetectado> = Vec::new();

    for comp in catalogo {
        let mut miembros: Vec<&Paquete> = if !comp.apk.is_empty() {
            scan.packages.iter().filter(|p| comp.apk.contains(&p.name.as_str()) && p.class == "os-pkgs").collect()
        } else {
            scan.packages.iter().filter(|p| comp.path_prefixes.iter().any(|pre| p.path.starts_with(pre))).collect()
        };
        if comp.id == "npx" {
            let npm = scan.packages.iter().find(|p| p.name == "npm" && p.path.starts_with("usr/local/lib/node_modules/npm/"));
            miembros = npm.into_iter().collect();
        }
        let suyos: Vec<&Hallazgo> = scan.vulnerabilities.iter().filter(|v| propietario_de(v, ecosistema) == comp.id).collect();

        let version = match miembros.iter().find(|p| p.name == comp.id) {
            Some(top) => Some(top.version.clone()),
            None if !miembros.is_empty() && !comp.apk.is_empty() => Some(miembros[0].version.clone()),
            None => None,
        };
        componentes.push(ComponenteDetectado {
            id: comp.id.into(),
            label: comp.label.into(),
            description: comp.description.into(),
            group: comp.group.into(),
            present: !miembros.is_empty(),
            installable: !comp.apk.is_empty(),
            version,
            packages: miembros.len(),
            high_critical: suyos.iter().filter(es_hc).count(),
            unfixed_high_critical: suyos.iter().filter(es_hc).filter(|v| v.fixed_version.is_none()).count(),
            vulnerabilities: suyos.len(),
            requires: comp.requires.iter().map(|s| s.to_string()).collect(),
        });
    }

    let apk_catalogo: Vec<&str> = catalogo.iter().flat_map(|c| c.apk.iter().copied()).collect();
    let otros_os: Vec<PaqueteSuelto> = scan
        .packages
        .iter()
        .filter(|p| p.class == "os-pkgs" && !apk_catalogo.contains(&p.name.as_str()))
        .map(|p| PaqueteSuelto { name: p.name.clone(), version: p.version.clone() })
        .collect();

    let mut otros_lenguaje: BTreeMap<String, usize> = BTreeMap::new();
    for p in &scan.packages {
        let cubierto = catalogo.iter().any(|c| c.path_prefixes.iter().any(|pre| p.path.starts_with(pre)));
        if p.class == "lang-pkgs" && !cubierto {
            let cima = match p.path.find("/node_modules/") {
                Some(i) => p.path[..i].to_string(),
                None => p.path.clone(),
            };
            *otros_lenguaje.entry(cima).or_insert(0) += 1;
        }
    }

    Detectados {
        components: componentes,
        other_os_packages: otros_os,
        derived_image_warning: !otros_lenguaje.is_empty(),
        other_language_locations: otros_lenguaje.into_iter().map(|(path, packages)| UbicacionLenguaje { path, packages }).collect(),
    }
}
