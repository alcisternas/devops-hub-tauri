// Analisis de impacto de la configuracion: consecuencias funcionales y efecto
// proyectado sobre las vulnerabilidades.
// Port de hardening/impact.py de Container Hardening Studio.
//
// Es una evaluacion PRELIMINAR basada en el escaneo de origen: nunca declara
// cumplimiento. Solo el reescaneo de la imagen construida puede aprobarla.

use std::collections::{BTreeSet, HashSet};

use serde::Serialize;

use super::componentes::{catalogo_para, propietario_de};
use super::config::{normalizar, ConfigHardening};
use super::escaner::{bloqueantes, version_requerida, Escaneo, Hallazgo};

pub fn efectos_al_eliminar(id: &str) -> &'static [&'static str] {
    match id {
        "npm" => &[
            "npm no estará disponible en la imagen final.",
            "No podrá ejecutar npm install.",
            "No podrá ejecutar npm ci.",
            "No podrá ejecutar npm run.",
            "Algunas vulnerabilidades asociadas con npm podrían desaparecer (se confirma al reescanear).",
            "La imagen seguirá pudiendo ejecutar JavaScript con Node.js.",
        ],
        "npx" => &["npx no estará disponible; no se podrán ejecutar paquetes con npx."],
        "corepack" => &["corepack no estará disponible; yarn y pnpm gestionados por Corepack dejarán de funcionar."],
        "yarn" => &["yarn no estará disponible."],
        "pnpm" => &["pnpm no estará disponible."],
        "build_tools" => &["No se podrán compilar módulos nativos (node-gyp) dentro de la imagen."],
        "python" => &["node-gyp no podrá compilar módulos nativos sin Python."],
        "git" => &["No se podrán instalar dependencias desde repositorios git."],
        "curl" => &["curl no estará disponible; los healthchecks basados en curl fallarán."],
        "ca_certs" => &["Las conexiones HTTPS/TLS a servicios externos fallarán por falta de certificados CA de confianza."],
        "shell" => &[
            "No habrá /bin/sh: no se podrá abrir una shell interactiva (docker exec ... sh).",
            "Los entrypoints y healthchecks escritos como scripts de shell no funcionarán; use formato exec (JSON).",
            "npm run y los scripts de ciclo de vida de npm requieren shell.",
        ],
        "pkg_manager" => &["apk no estará disponible en tiempo de ejecución (no se podrán instalar paquetes)."],
        _ => &[],
    }
}

fn notas_al_conservar(id: &str) -> Option<&'static [&'static str]> {
    match id {
        "npm" => Some(&["npm se conserva: sus dependencias internas serán analizadas y se intentará corregirlas sin cambiar la versión de Node.js."]),
        "shell" => Some(&["Se conserva BusyBox: amplía la superficie de ataque y los paquetes a escanear."]),
        "pkg_manager" => Some(&["Conservar apk contradice el principio de runtime inmutable."]),
        "git" => Some(&["Git añade paquetes del sistema adicionales a la superficie de ataque."]),
        "curl" => Some(&["curl añade paquetes (libcurl, nghttp2, brotli…) a la superficie de ataque."]),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Problema {
    // error bloquea la generacion; warning solo informa
    pub level: String,
    pub component: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Efecto {
    pub component: String,
    pub label: String,
    pub kept: bool,
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HallazgoQuitado {
    pub id: String,
    pub severity: String,
    pub package: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Corregible {
    pub package: String,
    pub version: String,
    pub cves: Vec<String>,
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SinCorreccion {
    pub package: String,
    pub version: String,
    pub path: String,
    pub cves: Vec<String>,
    pub owner: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Analisis {
    // invalid | likely_blocked | needs_remediation | candidate
    pub verdict: String,
    pub message: String,
    pub issues: Vec<Problema>,
    pub effects: Vec<Efecto>,
    pub vulnerabilities_removed: Vec<HallazgoQuitado>,
    pub remaining_high_critical: usize,
    pub fixable: Vec<Corregible>,
    pub unfixable: Vec<SinCorreccion>,
    pub normalized: ConfigHardening,
}

// Reglas de coherencia.
pub fn validar_config(cfg: &ConfigHardening) -> Vec<Problema> {
    let c = normalizar(cfg);
    let mut p: Vec<Problema> = Vec::new();
    let mut add = |level: &str, msg: String, comp: &str| {
        p.push(Problema { level: level.into(), component: comp.into(), message: msg });
    };

    for comp in catalogo_para(&c.ecosystem) {
        if c.conserva(comp.id) {
            for req in comp.requires {
                if !c.conserva(req) {
                    add("error", format!("{} requiere «{}», que está desactivado.", comp.label, req), comp.id);
                }
            }
        }
    }
    if c.allow_install_deps && !c.conserva("npm") {
        add("error", "Permitir instalación de dependencias requiere conservar npm.".into(), "npm");
    }
    if c.allow_install_deps && !c.conserva("ca_certs") {
        add("error", "Instalar dependencias requiere certificados CA para HTTPS.".into(), "ca_certs");
    }
    if c.runtime_only {
        for prohibido in ["build_tools", "python", "git", "pkg_manager", "yarn", "pnpm"] {
            if c.conserva(prohibido) {
                add("error", format!("«Imagen solo runtime» es incompatible con conservar {}.", prohibido), prohibido);
            }
        }
        if c.allow_install_deps {
            add("error", "«Imagen solo runtime» es incompatible con instalar dependencias.".into(), "npm");
        }
    }
    if c.conserva("npm") && !c.conserva("shell") {
        add("warning", "npm sin shell: npm run y los scripts de instalación no funcionarán (npm install de paquetes JS puros sí).".into(), "shell");
    }
    if (c.conserva("yarn") || c.conserva("pnpm")) && !c.conserva("corepack") && c.conserva("pnpm") {
        add("warning", "pnpm normalmente se activa mediante Corepack.".into(), "corepack");
    }
    if !c.non_root {
        add("warning", "Ejecutar como root requiere justificación aprobada según BANK-STRICT.".into(), "");
    }
    if !c.conserva("ca_certs") {
        add("warning", "Sin certificados CA no habrá conexiones TLS verificables.".into(), "ca_certs");
    }
    p
}

// Ids de componentes (claves de keep) que la configuracion apaga.
pub fn apagados(c: &ConfigHardening) -> HashSet<String> {
    c.keep.iter().filter(|(_, k)| !**k).map(|(id, _)| id.clone()).collect()
}

// (hallazgos que permanecerian, hallazgos que desaparecerian por eliminar componentes)
fn repartir<'a>(scan: &'a Escaneo, c: &ConfigHardening) -> (Vec<&'a Hallazgo>, Vec<&'a Hallazgo>) {
    let off = apagados(c);
    let mut quedan = Vec::new();
    let mut se_van = Vec::new();
    for v in &scan.vulnerabilities {
        if off.contains(&propietario_de(v, &c.ecosystem)) {
            se_van.push(v);
        } else {
            quedan.push(v);
        }
    }
    (quedan, se_van)
}

// Agrupa conservando el orden de primera aparicion (como un dict de Python).
fn agrupar<'a, K: PartialEq>(items: Vec<(K, &'a Hallazgo)>) -> Vec<(K, Vec<&'a Hallazgo>)> {
    let mut grupos: Vec<(K, Vec<&'a Hallazgo>)> = Vec::new();
    for (k, v) in items {
        match grupos.iter_mut().find(|(gk, _)| *gk == k) {
            Some((_, lista)) => lista.push(v),
            None => grupos.push((k, vec![v])),
        }
    }
    grupos
}

pub fn analizar(cfg: &ConfigHardening, scan: &Escaneo) -> Analisis {
    let c = normalizar(cfg);
    let (quedan, se_van) = repartir(scan, &c);
    let catalogo = catalogo_para(&c.ecosystem);

    let mut efectos: Vec<Efecto> = Vec::new();
    for comp in catalogo {
        if !c.conserva(comp.id) {
            efectos.push(Efecto {
                component: comp.id.into(),
                label: comp.label.into(),
                kept: false,
                messages: efectos_al_eliminar(comp.id).iter().map(|s| s.to_string()).collect(),
            });
        } else if let Some(notas) = notas_al_conservar(comp.id) {
            efectos.push(Efecto {
                component: comp.id.into(),
                label: comp.label.into(),
                kept: true,
                messages: notas.iter().map(|s| s.to_string()).collect(),
            });
        }
    }

    // Proyeccion: hallazgos bloqueantes que permanecen, agrupados por paquete y ruta
    let quedan_bloq: Vec<&Hallazgo> = quedan.iter().copied().filter(|v| v.severity == "HIGH" || v.severity == "CRITICAL").collect();
    let grupos = agrupar(
        quedan_bloq
            .iter()
            .map(|v| ((v.pkg_name.clone(), v.pkg_path.clone(), v.installed_version.clone()), *v))
            .collect(),
    );
    let mut sin_correccion: Vec<SinCorreccion> = Vec::new();
    let mut corregibles: Vec<Corregible> = Vec::new();
    for ((nombre, ruta, instalada), vs) in &grupos {
        let owner = propietario_de(vs[0], &c.ecosystem);
        if vs.iter().any(|v| v.fixed_version.is_none()) {
            sin_correccion.push(SinCorreccion {
                package: nombre.clone(),
                version: instalada.clone(),
                path: ruta.clone(),
                cves: vs.iter().filter(|v| v.fixed_version.is_none()).map(|v| v.id.clone()).collect(),
                owner,
            });
            continue;
        }
        if vs[0].class == "lang-pkgs" && version_requerida(instalada, vs).is_none() {
            sin_correccion.push(SinCorreccion {
                package: nombre.clone(),
                version: instalada.clone(),
                path: ruta.clone(),
                cves: vs.iter().map(|v| v.id.clone()).collect(),
                owner,
            });
            continue;
        }
        corregibles.push(Corregible {
            package: nombre.clone(),
            version: instalada.clone(),
            cves: vs.iter().map(|v| v.id.clone()).collect(),
            owner,
        });
    }

    let problemas = validar_config(&c);
    let hay_errores = problemas.iter().any(|i| i.level == "error");
    let quitados: Vec<&str> = catalogo
        .iter()
        .filter(|comp| !c.conserva(comp.id) && scan.vulnerabilities.iter().any(|v| propietario_de(v, &c.ecosystem) == comp.id))
        .map(|comp| comp.id)
        .collect();

    let (veredicto, mut mensaje) = if hay_errores {
        ("invalid", "La configuración tiene conflictos que deben resolverse antes de generar el Dockerfile.".to_string())
    } else if !sin_correccion.is_empty() {
        let pk: BTreeSet<String> = sin_correccion.iter().map(|u| format!("{} {}", u.package, u.version)).collect();
        let pk = pk.into_iter().collect::<Vec<_>>().join(", ");
        (
            "likely_blocked",
            format!(
                "Con esta selección permanecen hallazgos HIGH/CRITICAL sin corrección publicada ({}). \
                 Es probable que la política bloquee la imagen; se intentarán estrategias compatibles y, si no existen, se informará el bloqueo.",
                pk
            ),
        )
    } else if !quedan_bloq.is_empty() {
        (
            "needs_remediation",
            "Permanecen hallazgos HIGH/CRITICAL con corrección disponible; se aplicarán actualizaciones y se verificará mediante \
             construcción, pruebas y reescaneo."
                .to_string(),
        )
    } else {
        (
            "candidate",
            "No quedan hallazgos HIGH/CRITICAL atribuibles a los componentes conservados según el escaneo de origen. \
             Se requieren construcción, pruebas y escaneo para comprobar el cumplimiento."
                .to_string(),
        )
    };
    if !quitados.is_empty() {
        let nombres: Vec<&str> = quitados.iter().filter_map(|id| catalogo.iter().find(|c| c.id == *id).map(|c| c.label)).collect();
        mensaje = format!("Has elegido eliminar {}. {}", nombres.join(", "), mensaje);
    }

    Analisis {
        verdict: veredicto.into(),
        message: mensaje,
        issues: problemas,
        effects: efectos,
        vulnerabilities_removed: se_van
            .iter()
            .map(|v| HallazgoQuitado { id: v.id.clone(), severity: v.severity.clone(), package: v.pkg_name.clone() })
            .collect(),
        remaining_high_critical: bloqueantes(&quedan.iter().map(|v| (*v).clone()).collect::<Vec<_>>()).len(),
        fixable: corregibles,
        unfixable: sin_correccion,
        normalized: c,
    }
}
