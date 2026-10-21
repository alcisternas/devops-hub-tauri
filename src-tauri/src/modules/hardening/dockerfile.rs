// Generador de Dockerfile endurecido para imagenes Node.js sobre Alpine.
// Port de hardening/dockerfile.py de Container Hardening Studio.
//
// Estrategia: «rootfs limpio». La imagen final se ensambla en una etapa
// intermedia (`rootfs`) a partir de
//   1. el binario de Node.js de la imagen origen (fijada por digest),
//   2. SOLO los paquetes apk que el binario exige (se calculan con scanelf,
//      no se adivinan) mas los componentes elegidos,
//   3. los componentes node elegidos (npm, corepack, yarn...), opcionalmente
//      con dependencias corregidas,
// y se publica con `FROM scratch`. Las capas heredadas de la imagen original
// NO forman parte del artefacto final.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::componentes::{catalogo_para, Detectados};
use super::config::{normalizar, ConfigHardening};
use super::planificador::{config_efectiva, Plan};
use super::validar::{es_nombre_apk, es_nombre_npm, es_referencia_fijada, es_semver, es_url_registro_npm, es_version_pkg, shell_quote};

pub const VERSION_GENERADOR: &str = "1.0";

const APPLY_PATCHES_JS: &str = include_str!("assets/apply_patches.js");
const NODE_SUITE_JS: &str = include_str!("assets/node_suite.js");

// tipo imita el nombre de la excepcion de Studio para poder comparar los mensajes.
#[derive(Debug, Clone)]
pub struct ErrorGeneracion {
    pub tipo: &'static str,
    pub mensaje: String,
}

impl fmt::Display for ErrorGeneracion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.tipo, self.mensaje)
    }
}

fn generacion(m: impl Into<String>) -> ErrorGeneracion {
    ErrorGeneracion { tipo: "GenerationError", mensaje: m.into() }
}

fn validacion(m: impl Into<String>) -> ErrorGeneracion {
    ErrorGeneracion { tipo: "ValidationFailure", mensaje: m.into() }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Explicacion {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PruebaEsperada {
    pub id: String,
    pub title: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MetaDockerfile {
    pub src_ref: String,
    pub builder_ref: String,
    pub alpine: String,
    pub node_version: String,
    pub effective_keep: BTreeMap<String, bool>,
    pub removed_by_authorization: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocDockerfile {
    pub text: String,
    pub explanations: Vec<Explicacion>,
    // ruta relativa al contexto de construccion -> contenido
    pub files: BTreeMap<String, String>,
    pub expected_tests: Vec<PruebaEsperada>,
    pub sha256: String,
    pub meta: MetaDockerfile,
}

// Lo que el generador necesita saber de la imagen origen.
pub struct ImagenOrigen {
    // nombre@sha256:...
    pub pinned_ref: String,
    pub env: BTreeMap<String, String>,
    pub os_family: Option<String>,
    pub os_name: Option<String>,
}

pub struct EntradaDockerfile<'a> {
    pub cfg: &'a ConfigHardening,
    pub plan: &'a Plan,
    pub imagen: &'a ImagenOrigen,
    pub detectados: &'a Detectados,
    // referencia de alpine fijada por digest (nombre@sha256:...)
    pub builder_ref: &'a str,
    pub network_tests: bool,
    pub npm_registry: &'a str,
    pub nonce: &'a str,
    // Quien firma el comentario de cabecera del Dockerfile.
    pub generador: &'a str,
}

struct Constructor {
    lineas: Vec<String>,
    explicaciones: Vec<Explicacion>,
}

impl Constructor {
    fn bloque(&mut self, explicacion: &str, lineas: Vec<String>) {
        let inicio = self.lineas.len() + 1;
        self.lineas.extend(lineas);
        self.explicaciones.push(Explicacion { start: inicio, end: self.lineas.len(), text: explicacion.to_string() });
    }

    fn vacia(&mut self) {
        self.lineas.push(String::new());
    }
}

fn v(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// "3.24.2" -> "3.24"
fn alpine_menor(nombre: Option<&str>) -> Result<String, ErrorGeneracion> {
    let n = nombre.unwrap_or("");
    let digitos1: String = n.chars().take_while(|c| c.is_ascii_digit()).collect();
    let resto = &n[digitos1.len()..];
    if !digitos1.is_empty() {
        if let Some(r) = resto.strip_prefix('.') {
            let digitos2: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digitos2.is_empty() {
                return Ok(format!("{}.{}", digitos1, digitos2));
            }
        }
    }
    Err(generacion("No se pudo determinar la versión de Alpine de la imagen origen."))
}

fn comprobar_seleccion(cfg: &ConfigHardening, det: &Detectados) -> Result<(), ErrorGeneracion> {
    for comp in catalogo_para(&cfg.ecosystem) {
        let presente = det.components.iter().find(|d| d.id == comp.id).map(|d| d.present).unwrap_or(false);
        if cfg.conserva(comp.id) && comp.apk.is_empty() && !presente {
            return Err(generacion(format!(
                "{} está seleccionado pero no existe en la imagen origen y no es instalable automáticamente.",
                comp.label
            )));
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────
// JSON al estilo de Python (json.dumps): separadores ", " y ": ", ASCII puro.
// Se necesita para que el texto embebido y la huella coincidan con Studio.
// ─────────────────────────────────────────────────────────────────────────

fn py_cadena(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x7f => o.push(c),
            c => {
                let mut buf = [0u16; 2];
                for unidad in c.encode_utf16(&mut buf) {
                    o.push_str(&format!("\\u{:04x}", unidad));
                }
            }
        }
    }
    o.push('"');
    o
}

fn py_valor(valor: &Value) -> String {
    match valor {
        Value::String(s) => py_cadena(s),
        Value::Array(a) => format!("[{}]", a.iter().map(py_valor).collect::<Vec<_>>().join(", ")),
        otro => otro.to_string(),
    }
}

fn py_objeto(pares: &[(&str, Value)]) -> String {
    format!("{{{}}}", pares.iter().map(|(k, val)| format!("{}: {}", py_cadena(k), py_valor(val))).collect::<Vec<_>>().join(", "))
}

pub fn generar_node_alpine(e: &EntradaDockerfile) -> Result<DocDockerfile, ErrorGeneracion> {
    let c = normalizar(&config_efectiva(e.cfg, e.plan));
    if c.ecosystem != "node" {
        return Err(generacion("Solo se soporta el ecosistema Node.js en esta versión."));
    }
    let familia = e.imagen.os_family.as_deref();
    if familia != Some("alpine") {
        return Err(generacion(format!(
            "La distribución base «{}» aún no está soportada por el generador (solo Alpine). \
             Cambiar de distribución requiere autorización y una estrategia dedicada.",
            familia.unwrap_or("None")
        )));
    }
    let src_ref = e.imagen.pinned_ref.as_str();
    for r in [src_ref, e.builder_ref] {
        if !es_referencia_fijada(r) {
            return Err(validacion(format!("Referencia no fijada por digest: '{}'", r)));
        }
    }
    if e.cfg.builder_base_override.is_some() && !e.cfg.authorize_base_change {
        return Err(generacion("Cambiar la imagen base del constructor requiere authorize_base_change."));
    }
    if !es_url_registro_npm(e.npm_registry) {
        return Err(validacion("URL de registro npm inválida"));
    }
    comprobar_seleccion(&c, e.detectados)?;

    let node_version = match e.imagen.env.get("NODE_VERSION") {
        Some(nv) if es_semver(nv) => nv.clone(),
        _ => return Err(generacion("No se pudo determinar NODE_VERSION de la imagen origen.")),
    };
    let alpine = alpine_menor(e.imagen.os_name.as_deref())?;
    let (usuario, uid) = (c.run_user.clone(), c.run_uid);
    let npm_activo = c.conserva("npm");
    let usa_parche_npm = npm_activo && (!e.plan.npm_patches.is_empty() || e.plan.npm_upgrade.is_some());
    let shell = c.conserva("shell");
    let registro_q = shell_quote(e.npm_registry);

    // ---- paquetes apk
    for p in &e.plan.apk_packages {
        if !es_nombre_apk(p) {
            return Err(validacion(format!("Paquete apk inválido: '{}'", p)));
        }
    }
    let mut base: Vec<String> = v(&["alpine-baselayout-data", "musl"]);
    if c.conserva("ca_certs") {
        base.push("ca-certificates-bundle".into());
    }
    let nombres_apk: Vec<String> = base.into_iter().chain(e.plan.apk_packages.iter().cloned()).collect::<BTreeSet<_>>().into_iter().collect();
    let pines: Vec<(&String, &String)> = e.plan.apk_pins.iter().filter(|(k, val)| es_nombre_apk(k) && es_version_pkg(val)).collect();

    let mut b = Constructor { lineas: Vec::new(), explicaciones: Vec::new() };
    b.bloque(
        "Sintaxis moderna de BuildKit (necesaria para --mount).",
        vec![
            "# syntax=docker/dockerfile:1.7".into(),
            format!("# Generado por {} v{}. Estrategia: rootfs limpio multi-stage.", e.generador, VERSION_GENERADOR),
        ],
    );
    b.vacia();
    b.bloque(
        "Imagen origen fijada por digest: garantiza reproducibilidad y que se construya exactamente lo escaneado.",
        vec![format!("FROM {} AS src", src_ref)],
    );
    b.vacia();

    if usa_parche_npm {
        let mut pasos: Vec<String> = vec!["    set -eu; mkdir -p /out; cp -a /usr/local/lib/node_modules/npm /out/npm;".into()];
        if let Some(mayor) = &e.plan.npm_upgrade {
            let mayor: u64 = mayor.parse().map_err(|_| validacion(format!("Rama mayor de npm inválida: '{}'", mayor)))?;
            pasos.push(format!(
                "    npm install -g --no-audit --no-fund --prefix /tmp/npmup --registry {} npm@{}; \
                 rm -rf /out/npm; cp -a /tmp/npmup/lib/node_modules/npm /out/npm;",
                registro_q, mayor
            ));
        }
        for p in &e.plan.npm_patches {
            if !es_nombre_npm(&p.name) || !p.fixes_by_major.valores().all(|x| es_semver(x)) {
                return Err(validacion(format!("Parche npm inválido: {}", p.name)));
            }
        }
        let mut lineas = vec![
            "FROM src AS npmfix".to_string(),
            "COPY patches/apply_patches.js patches/patches.json /patch/".to_string(),
            "RUN \\".to_string(),
        ];
        lineas.extend(pasos.iter().map(|s| format!("{} \\", s)));
        lineas.push(format!("    node /patch/apply_patches.js /out/npm /patch/patches.json {}", registro_q));
        let explicacion = format!(
            "Etapa auxiliar: prepara el árbol de npm. {}sustituye dependencias vulnerables por versiones corregidas con verificación de integridad (npm pack) \
             y comprueba que sus dependencias se resuelven.",
            if e.plan.npm_upgrade.is_some() { "Actualiza npm dentro de su rama mayor y " } else { "" }
        );
        b.bloque(&explicacion, lineas);
        b.vacia();
    }

    // ---- ensamblado del rootfs
    let mut montajes: Vec<&str> = vec!["--mount=from=src,source=/,target=/src,ro"];
    if usa_parche_npm {
        montajes.push("--mount=from=npmfix,source=/out,target=/npmfix,ro");
    }
    let mut s: Vec<String> = vec![
        "set -eu;".into(),
        "apk add --no-cache pax-utils >/dev/null;".into(),
        "mkdir -p /rootfs/usr/local/bin /rootfs/usr/local/lib/node_modules /rootfs/etc /rootfs/tmp /rootfs/opt;".into(),
        "cp -a /src/usr/local/bin/node /rootfs/usr/local/bin/node;".into(),
        // bibliotecas exigidas por el binario (soname -> paquete apk), nunca adivinadas
        "needed=\"$(scanelf --needed --nobanner --format '%n' /rootfs/usr/local/bin/node | tr ',' '\\n' | grep -v '^$' | sort -u | sed 's/^/so:/')\";"
            .into(),
        "echo \"CHS_NEEDED_LIBS: $needed\";".into(),
        format!(
            "apk add --root /rootfs --initdb --no-scripts --no-cache --keys-dir /etc/apk/keys --repositories-file /etc/apk/repositories {} $needed;",
            nombres_apk.join(" ")
        ),
    ];
    if !pines.is_empty() {
        s.push(
            concat!(
                "pin_check() { pkg=$1; min=$2; if apk info --root /rootfs -e \"$pkg\" >/dev/null 2>&1; then ",
                "inst=$(apk info --root /rootfs -v \"$pkg\" | head -n1); inst=${inst#\"$pkg\"-}; ",
                "if [ \"$(apk version -t \"$inst\" \"$min\")\" = \"<\" ]; then echo \"CHS_PIN_VIOLATION $pkg $inst < $min\"; exit 1; fi; fi; };"
            )
            .into(),
        );
        for (k, val) in &pines {
            s.push(format!("pin_check {} {};", shell_quote(k), shell_quote(val)));
        }
    }
    if shell {
        s.push("chroot /rootfs /bin/busybox --install -s;".into());
    }
    if npm_activo {
        let origen_npm = if usa_parche_npm { "/npmfix/npm" } else { "/src/usr/local/lib/node_modules/npm" };
        s.push(format!("cp -a {} /rootfs/usr/local/lib/node_modules/npm;", origen_npm));
        s.push("ln -s ../lib/node_modules/npm/bin/npm-cli.js /rootfs/usr/local/bin/npm;".into());
        if c.conserva("npx") {
            s.push("ln -s ../lib/node_modules/npm/bin/npx-cli.js /rootfs/usr/local/bin/npx;".into());
        }
    }
    if c.conserva("corepack") {
        s.push("cp -a /src/usr/local/lib/node_modules/corepack /rootfs/usr/local/lib/node_modules/corepack;".into());
        s.push("ln -s ../lib/node_modules/corepack/dist/corepack.js /rootfs/usr/local/bin/corepack;".into());
        if c.conserva("pnpm") {
            s.push("ln -s ../lib/node_modules/corepack/dist/pnpm.js /rootfs/usr/local/bin/pnpm;".into());
        }
    }
    if c.conserva("yarn") {
        s.push("cp -a /src/opt/yarn-v* /rootfs/opt/;".into());
        s.push(
            "for y in /rootfs/opt/yarn-v*; do ln -s \"../../opt/$(basename \"$y\")/bin/yarn\" /rootfs/usr/local/bin/yarn; break; done;".into(),
        );
    }
    for ruta in &e.cfg.preserve_paths {
        let q = shell_quote(ruta);
        s.push(format!("mkdir -p \"/rootfs$(dirname {})\"; cp -a \"/src\"{} \"/rootfs\"{};", q, q, q));
    }
    let shell_login = if shell { "/bin/sh" } else { "/sbin/nologin" };
    if c.non_root {
        s.push(format!("echo '{}:x:{}:' >> /rootfs/etc/group;", usuario, uid));
        s.push(format!("echo '{}:x:{}:{}:{}:/home/{}:{}' >> /rootfs/etc/passwd;", usuario, uid, uid, usuario, usuario, shell_login));
        s.push(format!("install -d -o {} -g {} -m 0755 /rootfs/home/{};", uid, uid, usuario));
        s.push(format!("install -d -o {} -g {} -m 0750 /rootfs{};", uid, uid, c.workdir));
    } else {
        s.push(format!("mkdir -p /rootfs{}; mkdir -p /rootfs/root;", c.workdir));
    }
    s.push("chmod 1777 /rootfs/tmp;".into());
    s.push("find /rootfs -xdev -type f \\( -perm -4000 -o -perm -2000 \\) -exec chmod u-s,g-s {} +;".into());
    s.push("echo CHS_ROOTFS_READY".into());

    let ultimo = s.len() - 1;
    let mut run_lineas = vec![format!("RUN {} \\", montajes.join(" "))];
    run_lineas.extend(s[..ultimo].iter().map(|x| format!("    {} \\", x)));
    run_lineas.push(format!("    {}", s[ultimo]));
    let mut lineas_rootfs = vec![format!("FROM {} AS rootfs", e.builder_ref)];
    lineas_rootfs.extend(run_lineas);
    b.bloque(
        "Etapa de ensamblado: el rootfs se construye desde cero con solo lo necesario. Las bibliotecas que exige node se \
         resuelven con scanelf; se eliminan bits setuid/setgid y se crea el usuario sin privilegios.",
        lineas_rootfs,
    );
    b.vacia();

    // ---- etapa final
    let env_path = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
    let (nombre_base, digest_base) = src_ref.split_once('@').expect("la referencia fijada contiene @");
    let perfil: String = e.cfg.profile.chars().filter(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == ' ' || *ch == '.' || *ch == '-').take(60).collect();
    let etiquetas: Vec<(&str, String)> = vec![
        ("org.opencontainers.image.title", "hardened-node".into()),
        ("org.opencontainers.image.base.name", nombre_base.into()),
        ("org.opencontainers.image.base.digest", digest_base.into()),
        ("io.chs.generator", VERSION_GENERADOR.into()),
        ("io.chs.profile", perfil),
        ("io.chs.node.version", node_version.clone()),
    ];
    let mut final_l = vec![
        "FROM scratch AS final".to_string(),
        "COPY --from=rootfs /rootfs/ /".to_string(),
        format!("ENV NODE_VERSION={} PATH={} HOME=/home/{}", node_version, env_path, if c.non_root { usuario.as_str() } else { "root" }),
    ];
    final_l.extend(etiquetas.iter().map(|(k, val)| format!("LABEL {}=\"{}\"", k, val)));
    final_l.push(format!("WORKDIR {}", c.workdir));
    if c.non_root {
        final_l.push(format!("USER {}:{}", uid, uid));
    }
    final_l.push("CMD [\"node\"]".into());
    b.bloque(
        "Imagen final desde scratch: solo el rootfs ensamblado. Usuario no root, directorio de trabajo con permisos mínimos y \
         metadatos OCI (origen y digest base) para trazabilidad.",
        final_l,
    );
    b.vacia();

    // ---- pruebas funcionales (dentro de la imagen final, sin Docker daemon, como pasos RUN en formato exec)
    let herramientas: Vec<Value> = [("curl", "curl"), ("git", "git"), ("python3", "python")]
        .iter()
        .filter(|(_, comp)| c.conserva(comp))
        .map(|(t, _)| Value::String(t.to_string()))
        .collect();
    let esperado: Vec<(&str, Value)> = vec![
        ("node_version", Value::from(node_version.clone())),
        ("non_root", Value::from(c.non_root)),
        ("uid", Value::from(uid)),
        ("npm", Value::from(npm_activo)),
        ("npx", Value::from(c.conserva("npx"))),
        ("shell", Value::from(shell)),
        ("pkg_manager", Value::from(c.conserva("pkg_manager"))),
        ("ca_certs", Value::from(c.conserva("ca_certs"))),
        ("corepack", Value::from(c.conserva("corepack"))),
        ("yarn", Value::from(c.conserva("yarn"))),
        ("pnpm", Value::from(c.conserva("pnpm"))),
        ("tools", Value::Array(herramientas)),
        ("build_tools", Value::from(c.conserva("build_tools"))),
        ("install_deps", Value::from(c.allow_install_deps && npm_activo)),
        ("network", Value::from(e.network_tests)),
        ("npm_registry", Value::from(e.npm_registry)),
        ("workdir", Value::from(c.workdir.clone())),
    ];
    let suite = NODE_SUITE_JS.replace("__EXPECT__", &py_objeto(&esperado));
    let pruebas = pruebas_esperadas(&esperado);
    let nonce: String = e.nonce.chars().filter(|ch| ch.is_ascii_alphanumeric()).take(32).collect();
    let nonce = if nonce.is_empty() { "0".to_string() } else { nonce };
    b.bloque(
        "Etapa de pruebas: ejecuta la suite funcional DENTRO de la imagen final. El valor de CHS_TEST_NONCE invalida la caché para \
         que las pruebas se ejecuten siempre de verdad.",
        vec![
            "FROM final AS test".into(),
            format!("ARG CHS_TEST_NONCE={}", nonce),
            "COPY tests/node_suite.js /opt/chs/node_suite.js".into(),
            "RUN [\"node\", \"/opt/chs/node_suite.js\"]".into(),
        ],
    );
    b.vacia();
    b.bloque("La etapa por defecto es la imagen final (las pruebas se construyen con --target test).", vec!["FROM final".into()]);

    let texto = format!("{}\n", b.lineas.join("\n"));
    let mut archivos: BTreeMap<String, String> = BTreeMap::new();
    archivos.insert("tests/node_suite.js".into(), suite);
    if usa_parche_npm {
        archivos.insert("patches/apply_patches.js".into(), APPLY_PATCHES_JS.to_string());
        #[derive(Serialize)]
        struct Parche<'a> {
            name: &'a str,
            fixes_by_major: &'a super::planificador::MapaOrdenado,
        }
        let lista: Vec<Parche> = e.plan.npm_patches.iter().map(|p| Parche { name: &p.name, fixes_by_major: &p.fixes_by_major }).collect();
        archivos.insert("patches/patches.json".into(), serde_json::to_string_pretty(&lista).expect("serializable"));
    }
    // Misma huella que Studio: texto + json.dumps(files, sort_keys=True)
    let json_archivos = format!(
        "{{{}}}",
        archivos.iter().map(|(k, val)| format!("{}: {}", py_cadena(k), py_cadena(val))).collect::<Vec<_>>().join(", ")
    );
    let huella = format!("{:x}", Sha256::digest(format!("{}{}", texto, json_archivos).as_bytes()));

    Ok(DocDockerfile {
        text: texto,
        explanations: b.explicaciones,
        files: archivos,
        expected_tests: pruebas,
        sha256: huella,
        meta: MetaDockerfile {
            src_ref: src_ref.to_string(),
            builder_ref: e.builder_ref.to_string(),
            alpine,
            node_version,
            effective_keep: c.keep.clone(),
            removed_by_authorization: e.plan.removed_components.clone(),
        },
    })
}

fn bandera(esperado: &[(&str, Value)], clave: &str) -> bool {
    esperado.iter().find(|(k, _)| *k == clave).and_then(|(_, val)| val.as_bool()).unwrap_or(false)
}

fn pruebas_esperadas(esperado: &[(&str, Value)]) -> Vec<PruebaEsperada> {
    let p = |id: &str, title: &str| PruebaEsperada { id: id.into(), title: title.into(), required: true };
    let mut t = vec![
        p("runtime_version", "Versión de Node.js exacta"),
        p("js_execution", "Ejecución de un script JavaScript"),
        p("workdir_writable", "Directorio de trabajo escribible"),
        p("no_setuid", "Sin binarios setuid/setgid"),
        p("shell_state", "Estado de la shell coincide con lo solicitado"),
        p("pkg_manager_state", "Estado de apk coincide con lo solicitado"),
        p("npm_state", "Estado de npm coincide con lo solicitado"),
    ];
    if bandera(esperado, "non_root") {
        t.push(p("non_root", "Ejecución con usuario no root"));
    }
    if bandera(esperado, "ca_certs") {
        t.push(p("ca_bundle", "Certificados CA cargados por Node.js"));
        if bandera(esperado, "network") {
            t.push(p("https_request", "Conexión HTTPS con verificación de certificados"));
        }
    }
    if bandera(esperado, "npm") {
        t.push(p("npm_version", "npm --version"));
        if bandera(esperado, "network") && bandera(esperado, "install_deps") {
            t.push(p("npm_install", "Instalación de dependencias con npm"));
        }
    }
    if let Some((_, Value::Array(herramientas))) = esperado.iter().find(|(k, _)| *k == "tools") {
        for tool in herramientas.iter().filter_map(|x| x.as_str()) {
            t.push(PruebaEsperada { id: format!("tool_{}", tool), title: format!("{} disponible", tool), required: true });
        }
    }
    if bandera(esperado, "build_tools") && bandera(esperado, "npm") {
        t.push(p("native_toolchain", "Toolchain para módulos nativos (gcc, python, node-gyp)"));
    }
    t
}
