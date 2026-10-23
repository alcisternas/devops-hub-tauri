// Planificador de remediacion y asistente de recomendaciones.
// Port de hardening/planner.py de Container Hardening Studio.
//
// El plan se deriva EXCLUSIVAMENTE de hallazgos reales del escaner. Que un
// paquete figure en package.json o en el filesystem no se considera
// correccion: solo cuenta el reescaneo de la imagen construida.

use std::collections::{BTreeMap, BTreeSet};

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use super::componentes::{catalogo_para, detectar_componentes, propietario_de, DefComponente};
use super::config::{normalizar, ConfigHardening};
use super::escaner::{bloqueantes, elegir_version_corregida, Escaneo, Hallazgo};
use super::impacto::apagados;
use super::version::{parsear, Version};

const ORDEN_SEVERIDAD: [&str; 5] = ["CRITICAL", "HIGH", "MEDIUM", "LOW", "UNKNOWN"];

fn rango_severidad(s: &str) -> usize {
    ORDEN_SEVERIDAD.iter().position(|x| *x == s).unwrap_or(ORDEN_SEVERIDAD.len())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Alternativa {
    pub kind: String,
    pub description: String,
    pub requires_authorization: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Bloqueo {
    pub package: String,
    pub version: String,
    pub path: String,
    pub owner: String,
    pub cves: Vec<String>,
    pub severity: String,
    pub reason: String,
    pub alternatives: Vec<Alternativa>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Estrategia {
    pub kind: String,
    pub title: String,
    pub detail: String,
    // alta | media | baja
    pub confidence: String,
    pub addresses: Vec<String>,
    pub keeps_required_versions: bool,
    pub requires_authorization: bool,
}

// Mapa que conserva el orden de insercion (un dict de Python): el orden se ve
// en el Dockerfile y en patches.json.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapaOrdenado(pub Vec<(String, String)>);

impl MapaOrdenado {
    pub fn get(&self, clave: &str) -> Option<&String> {
        self.0.iter().find(|(k, _)| k == clave).map(|(_, v)| v)
    }

    pub fn poner(&mut self, clave: &str, valor: String) {
        match self.0.iter_mut().find(|(k, _)| k == clave) {
            Some((_, v)) => *v = valor,
            None => self.0.push((clave.to_string(), valor)),
        }
    }

    pub fn valores(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(_, v)| v)
    }
}

impl Serialize for MapaOrdenado {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ParcheNpm {
    pub name: String,
    // {"5": "5.0.11", "2": "2.1.6"}
    pub fixes_by_major: MapaOrdenado,
    pub cves: Vec<String>,
    pub cross_major: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Plan {
    pub attempt: u32,
    pub apk_packages: Vec<String>,
    pub apk_pins: BTreeMap<String, String>,
    pub npm_patches: Vec<ParcheNpm>,
    // rama mayor de npm a instalar (p. ej. "11")
    pub npm_upgrade: Option<String>,
    // eliminaciones autorizadas por el usuario
    pub removed_components: Vec<String>,
    pub strategies: Vec<Estrategia>,
    pub blockers: Vec<Bloqueo>,
}

impl Plan {
    fn vacio(intento: u32) -> Self {
        Plan {
            attempt: intento,
            apk_packages: Vec::new(),
            apk_pins: BTreeMap::new(),
            npm_patches: Vec::new(),
            npm_upgrade: None,
            removed_components: Vec::new(),
            strategies: Vec::new(),
            blockers: Vec::new(),
        }
    }
}

fn alternativas(owner: &str, etiqueta: &str) -> Vec<Alternativa> {
    let alt = |kind: &str, description: String, aut: bool| Alternativa { kind: kind.into(), description, requires_authorization: aut };
    if ["npm", "corepack", "yarn", "pnpm"].contains(&owner) || owner.starts_with("global:") {
        return vec![
            alt("remove_component", format!("Eliminar {} de la imagen final (se perderá la funcionalidad asociada).", etiqueta), true),
            alt("vendor_fix", "Esperar una versión del componente que incorpore la corrección o publicar un fix interno verificado.".into(), false),
            alt("change_runtime", "Usar otra versión de Node.js cuyo npm incluya una dependencia corregida.".into(), true),
        ];
    }
    vec![
        alt("change_base", "Cambiar a una versión de la distribución base que incorpore la corrección (p. ej. otra versión menor de Alpine).".into(), true),
        alt("replace_distro", "Sustituir la distribución base por otra (Debian slim, distroless) con aprobación explícita.".into(), true),
        alt("remove_component", "Eliminar el componente si no es necesario para la funcionalidad seleccionada.".into(), true),
    ]
}

fn mayor_de(v: &str) -> Option<u64> {
    parsear(v).map(|p| p.mayor())
}

fn etiqueta_de(owner: &str, comps: &[DefComponente], por_defecto: &str) -> String {
    comps.iter().find(|c| c.id == owner).map(|c| c.label.to_string()).unwrap_or_else(|| por_defecto.to_string())
}

// Agrupa conservando el orden de primera aparicion.
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

fn ids_ordenados<'a>(vs: impl IntoIterator<Item = &'a &'a Hallazgo>) -> Vec<String> {
    vs.into_iter().map(|v| v.id.clone()).collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn construir_plan(cfg: &ConfigHardening, scan: &Escaneo, intento: u32, previo: Option<&Plan>) -> Plan {
    let c = normalizar(cfg);
    let comps = catalogo_para(&c.ecosystem);
    let mut plan = Plan::vacio(intento);

    // La autorizacion solo habilita eliminar un componente cuando es la unica via; nunca se elimina de entrada.
    plan.removed_components = match previo {
        None => Vec::new(),
        Some(p) => p.removed_components.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect(),
    };

    let activo = |id: &str, quitados: &[String]| c.conserva(id) && !quitados.iter().any(|q| q == id);

    let mut apk: Vec<String> = Vec::new();
    for comp in comps {
        if activo(comp.id, &plan.removed_components) {
            apk.extend(comp.apk.iter().map(|s| s.to_string()));
        }
    }
    apk.extend(c.extra_apk.iter().cloned());
    plan.apk_packages = apk.into_iter().collect::<BTreeSet<_>>().into_iter().collect();

    let vulns = bloqueantes(&scan.vulnerabilities);
    let quitados_por_duenio: BTreeSet<&str> = comps.iter().filter(|d| !activo(d.id, &plan.removed_components)).map(|d| d.id).collect();
    let relevantes: Vec<&Hallazgo> =
        vulns.iter().copied().filter(|v| !quitados_por_duenio.contains(propietario_de(v, &c.ecosystem).as_str())).collect();

    // ---- paquetes del sistema
    for v in relevantes.iter().filter(|x| x.class == "os-pkgs") {
        if v.fixed_version.is_some() {
            if let Some(nueva) = v.fixed_versions.first() {
                // Comparacion de texto, igual que en Studio.
                let entrada = plan.apk_pins.entry(v.pkg_name.clone()).or_insert_with(|| nueva.clone());
                if nueva > entrada {
                    *entrada = nueva.clone();
                }
            }
        }
    }
    if !plan.apk_pins.is_empty() {
        let pines: Vec<String> = plan.apk_pins.iter().map(|(k, v)| format!("{}>={}", k, v)).collect();
        let afectados: BTreeSet<String> =
            relevantes.iter().filter(|v| plan.apk_pins.contains_key(&v.pkg_name)).map(|v| v.id.clone()).collect();
        plan.strategies.push(Estrategia {
            kind: "update_os_packages".into(),
            title: "Actualizar paquetes del sistema operativo".into(),
            detail: format!(
                "La imagen final se reconstruye desde cero con los paquetes actuales del repositorio de Alpine y se verifica que \
                 las versiones corregidas queden instaladas: {}",
                pines.join(", ")
            ),
            confidence: "alta".into(),
            addresses: afectados.into_iter().collect(),
            keeps_required_versions: true,
            requires_authorization: false,
        });
    }

    // ---- dependencias npm
    let grupos_npm = agrupar(
        relevantes
            .iter()
            .filter(|x| x.class == "lang-pkgs")
            .map(|v| ((v.pkg_name.clone(), v.installed_version.clone()), *v))
            .collect(),
    );
    let mut parches: BTreeMap<String, ParcheNpm> = BTreeMap::new();
    let mut sin_arreglo: Vec<(String, Vec<&Hallazgo>)> = Vec::new();
    for ((nombre, instalada), vs) in &grupos_npm {
        let mayor = mayor_de(instalada);
        let mut necesarias: Vec<(Version, String)> = Vec::new();
        let mut ok = true;
        for v in vs {
            match elegir_version_corregida(instalada, &v.fixed_versions).and_then(|fx| parsear(&fx).map(|pv| (pv, fx))) {
                Some(par) => necesarias.push(par),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        let owner = propietario_de(vs[0], &c.ecosystem);
        let mayor = match (ok, mayor) {
            (true, Some(m)) => m,
            _ => {
                match sin_arreglo.iter_mut().find(|(o, _)| *o == owner) {
                    Some((_, lista)) => lista.extend(vs.iter().copied()),
                    None => sin_arreglo.push((owner, vs.clone())),
                }
                continue;
            }
        };
        let (tope, tope_txt) = necesarias.iter().max_by(|a, b| a.0.cmp(&b.0)).cloned().expect("hay al menos una version");
        let p = parches
            .entry(nombre.clone())
            .or_insert_with(|| ParcheNpm { name: nombre.clone(), fixes_by_major: MapaOrdenado::default(), cves: Vec::new(), cross_major: false });
        let clave = mayor.to_string();
        let supera = match p.fixes_by_major.get(&clave).and_then(|x| parsear(x)) {
            Some(prev) => tope > prev,
            None => true,
        };
        if supera {
            p.fixes_by_major.poner(&clave, tope_txt);
        }
        let mut cves: BTreeSet<String> = p.cves.iter().cloned().collect();
        cves.extend(vs.iter().map(|v| v.id.clone()));
        p.cves = cves.into_iter().collect();
        p.cross_major = p.cross_major || tope.mayor() != mayor;
    }
    plan.npm_patches = parches.into_values().collect();
    if !plan.npm_patches.is_empty() {
        let cves: BTreeSet<String> = plan.npm_patches.iter().flat_map(|p| p.cves.iter().cloned()).collect();
        let cruzan = plan.npm_patches.iter().any(|p| p.cross_major);
        let lista: Vec<String> = plan
            .npm_patches
            .iter()
            .map(|p| format!("{}→{}", p.name, p.fixes_by_major.valores().cloned().collect::<Vec<_>>().join("/")))
            .collect();
        plan.strategies.push(Estrategia {
            kind: "update_dependencies".into(),
            title: "Sustituir dependencias vulnerables por versiones corregidas".into(),
            detail: format!(
                "Se reemplazan en el árbol de npm las versiones vulnerables por la versión corregida más cercana de la misma rama mayor: {}. \
                 Se descargan con verificación de integridad del registro y se validan con pruebas funcionales.",
                lista.join(", ")
            ),
            confidence: if cruzan { "baja" } else { "media" }.into(),
            addresses: cves.into_iter().collect(),
            keeps_required_versions: true,
            requires_authorization: false,
        });
    }

    // ---- bloqueos sin correccion: intentar actualizar npm completo (una vez) y, si no, informar
    for (owner, vs) in &sin_arreglo {
        let etiqueta = etiqueta_de(owner, comps, owner);
        let pk: Vec<String> =
            vs.iter().map(|v| format!("{} {}", v.pkg_name, v.installed_version)).collect::<BTreeSet<_>>().into_iter().collect();
        let ya_actualizo = previo.map(|p| p.npm_upgrade.is_some()).unwrap_or(false);
        if owner == "npm" && c.conserva("npm") && !plan.removed_components.iter().any(|r| r == "npm") && !ya_actualizo {
            let version_npm = scan
                .packages
                .iter()
                .find(|p| p.name == "npm" && p.path.starts_with("usr/local/lib/node_modules/npm/"))
                .map(|p| p.version.clone());
            let mayor = version_npm.as_deref().and_then(mayor_de);
            if let (Some(m), true) = (mayor, c.allow_npm_upgrade) {
                plan.npm_upgrade = Some(m.to_string());
                plan.strategies.push(Estrategia {
                    kind: "update_package_manager".into(),
                    title: format!("Actualizar npm a la última versión {}.x", m),
                    detail: format!(
                        "No existe corrección publicada para {}. Una versión más reciente de npm podría incorporar una \
                         dependencia distinta o corregida; se instalará y se reescaneará. Si el hallazgo persiste, el hardening quedará bloqueado.",
                        pk.join(", ")
                    ),
                    confidence: "baja".into(),
                    addresses: ids_ordenados(vs.iter().map(|v| v)),
                    keeps_required_versions: true,
                    requires_authorization: false,
                });
                continue;
            }
        }
        if plan.removed_components.iter().any(|r| r == owner) {
            continue;
        }
        if c.authorize_remove_components.iter().any(|a| a == owner) {
            plan.removed_components.push(owner.clone());
            plan.strategies.push(Estrategia {
                kind: "remove_component".into(),
                title: format!("Eliminar {} (autorizado por el usuario)", etiqueta),
                detail: format!("{} no tiene corrección; el usuario autorizó eliminar el componente.", pk.join(", ")),
                confidence: "alta".into(),
                addresses: ids_ordenados(vs.iter().map(|v| v)),
                keeps_required_versions: true,
                requires_authorization: true,
            });
            continue;
        }
        let peor = vs.iter().map(|v| v.severity.as_str()).min_by_key(|s| rango_severidad(s)).unwrap_or("UNKNOWN");
        plan.blockers.push(Bloqueo {
            package: vs[0].pkg_name.clone(),
            version: vs[0].installed_version.clone(),
            path: vs[0].pkg_path.clone(),
            owner: owner.clone(),
            cves: ids_ordenados(vs.iter().map(|v| v)),
            severity: peor.into(),
            reason: format!(
                "Sin versión corregida publicada para {}. Con {} conservado no existe una estrategia compatible que cumpla la política.",
                pk.join(", "),
                etiqueta
            ),
            alternatives: alternativas(owner, &etiqueta),
        });
    }

    // ---- hallazgos del sistema sin correccion
    for v in relevantes.iter().filter(|v| v.class == "os-pkgs" && v.fixed_version.is_none()) {
        let owner = propietario_de(v, &c.ecosystem);
        let etiqueta = etiqueta_de(&owner, comps, "sistema base");
        let es_componente = comps.iter().any(|d| d.id == owner);
        if es_componente && c.authorize_remove_components.iter().any(|a| *a == owner) {
            if !plan.removed_components.contains(&owner) {
                plan.removed_components.push(owner.clone());
            }
            continue;
        }
        plan.blockers.push(Bloqueo {
            package: v.pkg_name.clone(),
            version: v.installed_version.clone(),
            path: String::new(),
            owner: owner.clone(),
            cves: vec![v.id.clone()],
            severity: v.severity.clone(),
            reason: format!("{} {} no tiene corrección publicada en el repositorio de Alpine ({}).", v.pkg_name, v.installed_version, etiqueta),
            alternatives: alternativas(&owner, &etiqueta),
        });
    }
    plan
}

// Configuracion efectiva tras eliminaciones autorizadas por el usuario.
pub fn config_efectiva(cfg: &ConfigHardening, plan: &Plan) -> ConfigHardening {
    let mut keep = normalizar(cfg).keep;
    for r in &plan.removed_components {
        keep.insert(r.clone(), false);
        if r == "npm" {
            keep.insert("npx".into(), false);
        }
    }
    let permite = cfg.allow_install_deps && keep.get("npm").copied().unwrap_or(false);
    ConfigHardening { keep, allow_install_deps: permite, ..cfg.clone() }
}

// ─────────────────────────────────────────────────────────────────────────
// Asistente de recomendaciones
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Recomendacion {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub confidence: String,
    pub resolves: Vec<String>,
    pub changes: Vec<String>,
    pub affected: Vec<String>,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub incompatibilities: Vec<String>,
    pub keeps_required_versions: bool,
    pub requires_authorization: bool,
}

fn rec(id: &str, kind: &str, title: &str, confidence: &str) -> Recomendacion {
    Recomendacion {
        id: id.into(),
        kind: kind.into(),
        title: title.into(),
        confidence: confidence.into(),
        resolves: Vec::new(),
        changes: Vec::new(),
        affected: Vec::new(),
        pros: Vec::new(),
        cons: Vec::new(),
        incompatibilities: Vec::new(),
        keeps_required_versions: true,
        requires_authorization: false,
    }
}

fn txt(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn ids_de<'a>(vs: impl IntoIterator<Item = &'a Hallazgo>) -> Vec<String> {
    vs.into_iter().map(|v| v.id.clone()).collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn recomendar(cfg: &ConfigHardening, scan: &Escaneo) -> Vec<Recomendacion> {
    let c = normalizar(cfg);
    let det = detectar_componentes(scan, &c.ecosystem);
    let det_por_id = |id: &str| det.components.iter().find(|d| d.id == id);
    let vulns: Vec<&Hallazgo> = scan.vulnerabilities.iter().collect();
    let hc: Vec<&Hallazgo> = vulns.iter().copied().filter(|v| v.severity == "HIGH" || v.severity == "CRITICAL").collect();
    let off = apagados(&c);
    let dueno = |v: &Hallazgo| propietario_de(v, &c.ecosystem);
    let mut recs: Vec<Recomendacion> = Vec::new();

    let os_fix: Vec<&Hallazgo> = vulns.iter().copied().filter(|v| v.class == "os-pkgs" && v.fixed_version.is_some()).collect();
    if !os_fix.is_empty() {
        let mut r = rec("update-os", "update_os_packages", "Actualizar paquetes del sistema operativo", "alta");
        r.resolves = ids_de(os_fix.iter().copied());
        r.changes = os_fix
            .iter()
            .map(|v| format!("{} {} → {}", v.pkg_name, v.installed_version, v.fixed_version.clone().unwrap_or_default()))
            .collect();
        r.affected = os_fix.iter().map(|v| v.pkg_name.clone()).collect::<BTreeSet<_>>().into_iter().collect();
        r.pros = txt(&["Corrección oficial del proveedor de la distribución.", "Sin cambios funcionales."]);
        r.cons = txt(&["Depende de que la versión corregida esté ya publicada en el repositorio."]);
        recs.push(r);
    }

    let lang_fix: Vec<&Hallazgo> = vulns.iter().copied().filter(|v| v.class == "lang-pkgs" && v.fixed_version.is_some()).collect();
    if !lang_fix.is_empty() {
        let duenos: Vec<String> = lang_fix.iter().map(|v| dueno(v)).collect::<BTreeSet<_>>().into_iter().collect();
        let mut r = rec("update-deps", "update_dependencies", "Actualizar dependencias vulnerables", "media");
        r.resolves = ids_de(lang_fix.iter().copied());
        r.changes = lang_fix
            .iter()
            .map(|v| format!("{} {} → {}", v.pkg_name, v.installed_version, v.fixed_versions.first().cloned().unwrap_or_default()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        r.affected = duenos.clone();
        r.incompatibilities = txt(&["Una versión corregida de otra rama mayor puede romper la API usada por npm; se valida con pruebas funcionales."]);
        r.pros = txt(&["Mantiene Node.js y npm conservados."]);
        r.cons = txt(&["Reemplaza archivos dentro del árbol de npm (modificación no oficial) hasta que npm publique un release corregido."]);
        recs.push(r);
        if duenos.iter().any(|o| det_por_id(o).is_some() && !c.conserva(o)) {
            let mut r = rec(
                "removal-resolves-deps",
                "remove_unneeded_tools",
                "Las dependencias afectadas pertenecen a componentes que no conserva",
                "alta",
            );
            r.changes = duenos.iter().filter(|o| det_por_id(o).is_some() && !c.conserva(o)).map(|o| format!("Se omite {}", o)).collect();
            r.affected = duenos.clone();
            r.pros = txt(&["Esas vulnerabilidades dejan de formar parte de la imagen."]);
            r.cons = txt(&["Se pierde la funcionalidad del componente."]);
            recs.push(r);
        }
    }

    let sin_arreglo: Vec<&Hallazgo> = hc.iter().copied().filter(|v| v.fixed_version.is_none() && !off.contains(&dueno(v))).collect();
    let duenos_sin: BTreeSet<String> = sin_arreglo.iter().map(|v| dueno(v)).collect();
    for owner in &duenos_sin {
        let mios: Vec<&Hallazgo> = sin_arreglo.iter().copied().filter(|v| dueno(v) == *owner).collect();
        let etiqueta = det_por_id(owner).map(|d| d.label.clone()).unwrap_or_else(|| owner.clone());
        let mut r = rec(&format!("remove-{}", owner), "remove_unneeded_tools", &format!("Eliminar {} (sin corrección publicada)", etiqueta), "alta");
        r.resolves = ids_de(mios.iter().copied());
        r.affected = vec![owner.clone()];
        r.changes = vec![format!("{} no se incluye en la imagen final", etiqueta)];
        r.pros = txt(&["Única vía compatible cuando no existe parche."]);
        r.cons = vec![format!("Se pierde la funcionalidad de {}.", etiqueta)];
        if owner == "npm" {
            r.cons.push("Sin npm install / npm ci / npm run.".into());
        }
        r.requires_authorization = true;
        recs.push(r);
    }

    let mut r = rec("runtime-minimal", "minimal_runtime", "Construir una imagen runtime mínima (rootfs limpio)", "alta");
    r.affected = txt(&["imagen completa"]);
    r.changes = txt(&[
        "Nueva raíz con solo Node.js y las bibliotecas que exige su binario (calculadas con scanelf).",
        "Las capas heredadas de la imagen original no forman parte del artefacto final.",
    ]);
    r.resolves = ids_de(vulns.iter().copied().filter(|v| off.contains(&dueno(v))));
    r.pros = txt(&["Reduce paquetes y superficie de ataque.", "Elimina archivos vulnerables heredados de capas anteriores."]);
    r.cons = txt(&["Los componentes no seleccionados dejan de existir en la imagen."]);
    r.incompatibilities = txt(&["Scripts que asuman shell o utilidades BusyBox no funcionarán si se elimina la shell."]);
    recs.push(r);

    let mut r = rec("multi-stage", "multi_stage", "Separar construcción y ejecución con multi-stage build", "alta");
    r.affected = txt(&["Dockerfile"]);
    r.changes = txt(&["Etapas: src (origen fijado por digest) → rootfs (ensamblado) → final (FROM scratch) → test."]);
    r.pros = txt(&["Las herramientas de construcción no llegan al artefacto final."]);
    r.cons = txt(&["Dockerfile más extenso."]);
    recs.push(r);

    if c.non_root {
        let mut r = rec("non-root", "non_root", "Ejecutar como usuario no root", "alta");
        r.affected = txt(&["runtime"]);
        r.changes = vec![format!("USER {}:{} ({}), directorio de trabajo {} con permisos mínimos.", c.run_uid, c.run_uid, c.run_user, c.workdir)];
        r.pros = txt(&["Reduce el impacto de una explotación."]);
        r.cons = txt(&["Puertos <1024 requieren configuración adicional."]);
        recs.push(r);
    }
    if det_por_id("shell").map(|d| d.present).unwrap_or(false) && !c.conserva("shell") {
        let mut r = rec("drop-shell", "remove_unneeded_tools", "Eliminar shell y utilidades BusyBox", "alta");
        r.affected = txt(&["shell"]);
        r.changes = txt(&["BusyBox y /bin/sh no se instalan en la imagen final."]);
        r.resolves = ids_de(vulns.iter().copied().filter(|v| dueno(v) == "shell"));
        r.pros = txt(&["Dificulta la post-explotación."]);
        r.cons = txt(&["Sin acceso shell para depuración; entrypoints en formato exec."]);
        recs.push(r);
    }
    if det_por_id("pkg_manager").map(|d| d.present).unwrap_or(false) && !c.conserva("pkg_manager") {
        let mut r = rec("drop-apk", "reduce_attack_surface", "Eliminar apk-tools del runtime", "alta");
        r.affected = txt(&["pkg_manager"]);
        r.changes = txt(&["apk-tools no se instala; se conserva la base de datos de paquetes para auditoría/SBOM."]);
        r.pros = txt(&["Imagen inmutable."]);
        r.cons = txt(&["No se pueden instalar paquetes en el contenedor en ejecución."]);
        recs.push(r);
    }
    if sin_arreglo.iter().any(|v| dueno(v) == "os-base") {
        let mut r = rec("change-base", "migrate_base", "Migrar a una versión base con corrección", "baja");
        r.affected = txt(&["base"]);
        r.requires_authorization = true;
        r.keeps_required_versions = false;
        r.changes = txt(&["Evaluar la siguiente versión de Alpine o una distribución alternativa."]);
        r.pros = txt(&["Puede incorporar el parche."]);
        r.cons = txt(&["Cambia el sistema operativo base; requiere autorización y revalidación completa."]);
        recs.push(r);
    }
    let mut r = rec("distroless", "distroless", "Evaluar imagen distroless", "baja");
    r.requires_authorization = true;
    r.keeps_required_versions = false;
    r.affected = txt(&["base"]);
    r.changes = txt(&["Las imágenes distroless de Node.js se basan en Debian (glibc), no en Alpine (musl)."]);
    r.pros = txt(&["Sin shell ni gestor de paquetes por defecto."]);
    r.cons = txt(&["Cambia el sistema operativo y puede no ofrecer la versión exacta de Node.js exigida."]);
    r.incompatibilities = txt(&["Módulos nativos compilados para musl no son compatibles con glibc."]);
    recs.push(r);
    recs
}
