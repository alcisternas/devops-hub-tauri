// Validaciones de texto que se interpolan en comandos y en el Dockerfile.
//
// Todo valor que termina dentro de un script de shell o de un Dockerfile pasa
// por una de estas funciones antes. Equivalen una a una a las expresiones
// regulares de security.py en Container Hardening Studio, escritas a mano
// para no sumar la dependencia `regex`.

fn alnum_minuscula(c: char) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit()
}

pub fn es_digest(s: &str) -> bool {
    match s.strip_prefix("sha256:") {
        Some(h) => h.len() == 64 && h.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
        None => false,
    }
}

pub fn es_tag(s: &str) -> bool {
    let mut it = s.chars();
    match it.next() {
        Some(c) if c.is_ascii_alphanumeric() || c == '_' => {}
        _ => return false,
    }
    s.len() <= 128 && it.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

// Un componente de repositorio: tramos [a-z0-9]+ separados por ".", "_", "__" o
// una o mas "-". No puede empezar ni terminar con separador.
fn es_componente_repo(c: &str) -> bool {
    if c.is_empty() {
        return false;
    }
    let mut tramos: Vec<(bool, String)> = Vec::new(); // (es_alnum, texto)
    for ch in c.chars() {
        let alnum = alnum_minuscula(ch);
        match tramos.last_mut() {
            Some((a, t)) if *a == alnum => t.push(ch),
            _ => tramos.push((alnum, ch.to_string())),
        }
    }
    if !tramos.first().map(|t| t.0).unwrap_or(false) || !tramos.last().map(|t| t.0).unwrap_or(false) {
        return false;
    }
    tramos.iter().filter(|t| !t.0).all(|(_, sep)| {
        sep == "." || sep == "_" || sep == "__" || sep.chars().all(|c| c == '-')
    })
}

pub fn es_repo(s: &str) -> bool {
    !s.is_empty() && s.split('/').all(es_componente_repo)
}

pub fn es_plataforma(s: &str) -> bool {
    matches!(
        s,
        "linux/amd64" | "linux/arm64" | "linux/arm/v7" | "linux/arm/v6" | "linux/386" | "linux/ppc64le" | "linux/s390x" | "linux/riscv64"
    )
}

pub fn es_nombre_apk(s: &str) -> bool {
    let mut it = s.chars();
    match it.next() {
        Some(c) if alnum_minuscula(c) => {}
        _ => return false,
    }
    s.len() <= 101 && it.all(|c| alnum_minuscula(c) || c == '+' || c == '.' || c == '_' || c == '-')
}

pub fn es_version_pkg(s: &str) -> bool {
    let mut it = s.chars();
    match it.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    s.len() <= 61 && it.all(|c| c.is_ascii_alphanumeric() || "+._~:-".contains(c))
}

pub fn es_nombre_npm(s: &str) -> bool {
    let cuerpo = |t: &str| {
        let mut it = t.chars();
        match it.next() {
            Some(c) if alnum_minuscula(c) => {}
            _ => return false,
        }
        it.all(|c| alnum_minuscula(c) || "._~-".contains(c))
    };
    match s.strip_prefix('@') {
        Some(resto) => match resto.split_once('/') {
            Some((scope, nombre)) => cuerpo(scope) && cuerpo(nombre),
            None => false,
        },
        None => cuerpo(s),
    }
}

// MAYOR.MENOR.PARCHE[-prerelease][+build]
pub fn es_semver(s: &str) -> bool {
    let (sin_build, build) = match s.split_once('+') {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    };
    if let Some(b) = build {
        if b.is_empty() || !b.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
            return false;
        }
    }
    let (nucleo, pre) = match sin_build.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (sin_build, None),
    };
    if let Some(p) = pre {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
            return false;
        }
    }
    let partes: Vec<&str> = nucleo.split('.').collect();
    partes.len() == 3 && partes.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

// nombre@sha256:<64 hex>, con el nombre en el alfabeto que usan las referencias.
pub fn es_referencia_fijada(s: &str) -> bool {
    match s.split_once("@sha256:") {
        Some((nombre, hash)) => {
            !nombre.is_empty()
                && nombre.chars().all(|c| alnum_minuscula(c) || ".-:/_".contains(c))
                && hash.len() == 64
                && hash.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        }
        None => false,
    }
}

// https://host[:puerto][/ruta]
pub fn es_url_registro_npm(s: &str) -> bool {
    let resto = match s.strip_prefix("https://") {
        Some(r) => r,
        None => return false,
    };
    let (autoridad, ruta) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, ""),
    };
    let (host, puerto) = match autoridad.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (autoridad, None),
    };
    if host.is_empty() || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return false;
    }
    if let Some(p) = puerto {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }
    ruta.chars().all(|c| c.is_ascii_alphanumeric() || "._~/-".contains(c))
}

pub fn es_usuario(s: &str) -> bool {
    let mut it = s.chars();
    match it.next() {
        Some(c) if c.is_ascii_lowercase() || c == '_' => {}
        _ => return false,
    }
    s.len() <= 32 && it.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

pub fn es_ruta_absoluta(s: &str) -> bool {
    match s.strip_prefix('/') {
        Some(r) => (1..=200).contains(&r.len()) && r.chars().all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c)),
        None => false,
    }
}

// Equivalente a shlex.quote de Python: deja pasar el texto seguro y, si no,
// lo envuelve en comillas simples escapando las que traiga.
pub fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn digest_y_referencia_fijada() {
        let h = "a".repeat(64);
        assert!(es_digest(&format!("sha256:{}", h)));
        assert!(!es_digest(&format!("sha256:{}", "A".repeat(64))));
        assert!(!es_digest("sha256:abc"));
        assert!(es_referencia_fijada(&format!("docker.io/library/node@sha256:{}", h)));
        assert!(!es_referencia_fijada(&format!("Docker.io/node@sha256:{}", h)));
        assert!(!es_referencia_fijada("docker.io/library/node:latest"));
    }

    #[test]
    fn repositorios() {
        for ok in ["library/node", "node", "my-org/my_app.v2", "a__b", "a--b/c"] {
            assert!(es_repo(ok), "{}", ok);
        }
        for mal in ["", "/node", "node/", "Node", "a._b", "a..b", "-a", "a-", "a/b//c", "a b"] {
            assert!(!es_repo(mal), "{}", mal);
        }
    }

    #[test]
    fn tags_y_plataformas() {
        assert!(es_tag("26.12.0-alpine3.24"));
        assert!(es_tag("_x"));
        assert!(!es_tag(".x"));
        assert!(!es_tag(&"a".repeat(129)));
        assert!(es_plataforma("linux/arm/v7"));
        assert!(!es_plataforma("windows/amd64"));
    }

    #[test]
    fn nombres_de_paquetes() {
        assert!(es_nombre_apk("ca-certificates-bundle"));
        assert!(es_nombre_apk("g++")); // el catalogo incluye g++: '+' es valido tras el primer caracter
        assert!(es_nombre_apk("libstdc++"));
        assert!(!es_nombre_apk("Bad"));
        assert!(es_nombre_npm("undici"));
        assert!(es_nombre_npm("@scope/pkg"));
        assert!(!es_nombre_npm("@scope"));
        assert!(!es_nombre_npm("UPPER"));
        assert!(es_version_pkg("1.2.3-r0"));
        assert!(!es_version_pkg("-1"));
    }

    #[test]
    fn semver_y_urls() {
        assert!(es_semver("26.12.0"));
        assert!(es_semver("1.0.0-rc.1+build.5"));
        assert!(!es_semver("1.0"));
        assert!(!es_semver("v1.0.0"));
        assert!(es_url_registro_npm("https://registry.npmjs.org/"));
        assert!(es_url_registro_npm("https://npm.interno.cl:8443/repo"));
        assert!(!es_url_registro_npm("http://registry.npmjs.org/"));
        assert!(!es_url_registro_npm("https://exa mple.com"));
    }

    #[test]
    fn usuarios_y_rutas() {
        assert!(es_usuario("node"));
        assert!(!es_usuario("Node"));
        assert!(es_ruta_absoluta("/usr/local/share/foo"));
        assert!(!es_ruta_absoluta("usr/local"));
        assert!(!es_ruta_absoluta("/a b"));
    }

    #[test]
    fn shell_quote_coincide_con_python() {
        assert_eq!(shell_quote("abc-1.2/x"), "abc-1.2/x");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\"'\"'s'");
        assert_eq!(shell_quote("https://registry.npmjs.org/"), "https://registry.npmjs.org/");
    }
}
