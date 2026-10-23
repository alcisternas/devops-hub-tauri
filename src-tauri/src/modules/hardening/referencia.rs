// Analisis y normalizacion de referencias de imagen OCI.
// Port de registry/reference.py de Container Hardening Studio.

use super::validar::{es_digest, es_repo, es_tag};

const ALIAS_DOCKER_HUB: [&str; 3] = ["docker.io", "index.docker.io", "registry-1.docker.io"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagenRef {
    // host[:puerto] canonico (docker.io para Docker Hub)
    pub registry: String,
    // incluye "library/" en las imagenes oficiales de Docker Hub
    pub repository: String,
    pub tag: Option<String>,
    pub digest: Option<String>,
}

impl ImagenRef {
    pub fn api_host(&self) -> String {
        if self.registry == "docker.io" {
            "registry-1.docker.io".to_string()
        } else {
            self.registry.clone()
        }
    }

    // Nombre sin tag ni digest, utilizable en un Dockerfile o en Trivy.
    pub fn nombre(&self) -> String {
        format!("{}/{}", self.registry, self.repository)
    }

    // Forma corta para mostrar al usuario.
    pub fn mostrar(&self) -> String {
        let repo = if self.registry == "docker.io" {
            self.repository.strip_prefix("library/").unwrap_or(&self.repository).to_string()
        } else {
            self.repository.clone()
        };
        let mut base = if self.registry == "docker.io" { repo } else { format!("{}/{}", self.registry, repo) };
        if let Some(t) = &self.tag {
            base.push(':');
            base.push_str(t);
        }
        if let Some(d) = &self.digest {
            base.push('@');
            base.push_str(d);
        }
        base
    }

    pub fn fijada(&self, digest: &str) -> Result<String, String> {
        if !es_digest(digest) {
            return Err(format!("digest invalido: {:?}", digest));
        }
        Ok(format!("{}@{}", self.nombre(), digest))
    }
}

pub fn parsear_referencia(raw: &str) -> Result<ImagenRef, String> {
    let raw = raw.trim();
    let malo = |m: &str| Err(format!("Referencia de imagen invalida: {}", m));
    if raw.is_empty() || raw.chars().count() > 400 || raw.chars().any(|c| c.is_whitespace() || "\"'`$;&|<>\\".contains(c)) {
        return malo(&format!("{:?}", raw));
    }
    if raw.contains("://") {
        return Err("La referencia no debe incluir esquema (http/https).".to_string());
    }

    let (resto, digest) = match raw.split_once('@') {
        Some((r, d)) => {
            if !es_digest(d) {
                return Err(format!("digest invalido: {:?}", d));
            }
            (r, Some(d.to_string()))
        }
        None => (raw, None),
    };

    let (primero, restante) = match resto.split_once('/') {
        Some((p, r)) => (p, Some(r)),
        None => (resto, None),
    };
    let (mut registry, mut path) = match restante {
        Some(r) if primero.contains('.') || primero.contains(':') || primero == "localhost" => (primero.to_lowercase(), r.to_string()),
        _ => ("docker.io".to_string(), resto.to_string()),
    };

    let mut tag: Option<String> = None;
    let ultimo = path.rsplit('/').next().unwrap_or("").to_string();
    if ultimo.contains(':') {
        let (p, t) = path.rsplit_once(':').map(|(a, b)| (a.to_string(), b.to_string())).unwrap();
        if !es_tag(&t) {
            return Err(format!("tag invalido: {:?}", t));
        }
        path = p;
        tag = Some(t);
    }
    if ALIAS_DOCKER_HUB.contains(&registry.as_str()) {
        registry = "docker.io".to_string();
    }
    if registry == "docker.io" && !path.contains('/') {
        path = format!("library/{}", path);
    }
    if !es_repo(&path) {
        return Err(format!("repositorio invalido: {:?}", path));
    }
    if tag.is_none() && digest.is_none() {
        tag = Some("latest".to_string());
    }
    Ok(ImagenRef { registry, repository: path, tag, digest })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn oficial_de_docker_hub() {
        let r = parsear_referencia("node:26.12.0-alpine3.24").unwrap();
        assert_eq!(r.registry, "docker.io");
        assert_eq!(r.repository, "library/node");
        assert_eq!(r.tag.as_deref(), Some("26.12.0-alpine3.24"));
        assert_eq!(r.nombre(), "docker.io/library/node");
        assert_eq!(r.mostrar(), "node:26.12.0-alpine3.24");
        assert_eq!(r.api_host(), "registry-1.docker.io");
    }

    #[test]
    fn sin_tag_es_latest_y_con_organizacion_no_agrega_library() {
        assert_eq!(parsear_referencia("node").unwrap().tag.as_deref(), Some("latest"));
        let r = parsear_referencia("bitnami/redis:7").unwrap();
        assert_eq!(r.repository, "bitnami/redis");
    }

    #[test]
    fn registro_propio_y_digest() {
        let h = format!("sha256:{}", "c".repeat(64));
        let r = parsear_referencia(&format!("123456789.dkr.ecr.us-east-1.amazonaws.com/mi/app@{}", h)).unwrap();
        assert_eq!(r.registry, "123456789.dkr.ecr.us-east-1.amazonaws.com");
        assert_eq!(r.repository, "mi/app");
        assert_eq!(r.digest.as_deref(), Some(h.as_str()));
        assert_eq!(r.tag, None);
        assert_eq!(r.fijada(&h).unwrap(), format!("123456789.dkr.ecr.us-east-1.amazonaws.com/mi/app@{}", h));
    }

    #[test]
    fn alias_de_docker_hub() {
        let r = parsear_referencia("index.docker.io/library/alpine:3.24").unwrap();
        assert_eq!(r.registry, "docker.io");
    }

    #[test]
    fn rechaza_lo_peligroso() {
        for mal in ["", "node; rm -rf /", "node:$(id)", "https://docker.io/node", "a b", "node`x`", "NODE", "node:", "node@sha256:abc"] {
            assert!(parsear_referencia(mal).is_err(), "{:?}", mal);
        }
    }
}
