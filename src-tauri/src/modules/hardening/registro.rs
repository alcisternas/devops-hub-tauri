// Cliente minimo de la API OCI Distribution (Docker Hub, GHCR, ECR publico y
// registros compatibles) para resolver una referencia a su digest y leer la
// configuracion de la imagen sin descargarla.
// Port de registry/client.py de Container Hardening Studio.
//
// Seguridad (igual que en Studio):
//   * las redirecciones se siguen a mano, maximo 4, y nunca a un esquema no
//     seguro; si cambian de host, no llevan credenciales y el destino debe ser
//     publico;
//   * el servicio de tokens debe usar HTTPS y resolver a una direccion publica;
//   * el digest devuelto debe coincidir con el pedido.
// Por ahora solo imagenes publicas: sin credenciales de registro.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::referencia::{parsear_referencia, ImagenRef};
use super::validar::es_plataforma;

const ACEPTAR_MANIFIESTO: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, \
     application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";
const TIPOS_INDICE: [&str; 2] = ["application/vnd.oci.image.index.v1+json", "application/vnd.docker.distribution.manifest.list.v2+json"];
const MAX_REDIRECCIONES: usize = 4;

#[derive(Debug, Clone, Serialize)]
pub struct ImagenResuelta {
    pub reference: String,
    pub registry: String,
    pub repository: String,
    pub tag: Option<String>,
    pub platform: String,
    pub available_platforms: Vec<String>,
    pub index_digest: Option<String>,
    pub manifest_digest: String,
    pub pinned_ref: String,
    pub created: Option<String>,
    pub os: Option<String>,
    pub architecture: Option<String>,
    pub size_bytes: u64,
    pub layers: usize,
    pub env: BTreeMap<String, String>,
    pub user: String,
    pub entrypoint: Option<Value>,
    pub cmd: Option<Value>,
    pub workdir: String,
    pub labels: BTreeMap<String, String>,
}

// ─────────────────────────────────────────────────────────────────────────
// Funciones puras (sin red)
// ─────────────────────────────────────────────────────────────────────────

pub fn es_ip_publica(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            !(a.is_private()
                || a.is_loopback()
                || a.is_link_local()
                || a.is_unspecified()
                || a.is_broadcast()
                || a.is_multicast()
                || o[0] == 0
                || (o[0] == 100 && (64..=127).contains(&o[1]))
                || o[0] >= 240)
        }
        IpAddr::V6(a) => {
            if let Some(v4) = a.to_ipv4_mapped() {
                return es_ip_publica(IpAddr::V4(v4));
            }
            let s = a.segments();
            !(a.is_loopback() || a.is_unspecified() || a.is_multicast() || (s[0] & 0xfe00) == 0xfc00 || (s[0] & 0xffc0) == 0xfe80)
        }
    }
}

// Lee los pares clave="valor" de un desafio WWW-Authenticate.
pub fn parsear_desafio(cabecera: &str) -> BTreeMap<String, String> {
    let mut pares = BTreeMap::new();
    let b: Vec<char> = cabecera.chars().collect();
    let mut i = 0;
    while i < b.len() {
        // palabra
        let ini = i;
        while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
            i += 1;
        }
        if i > ini && i + 1 < b.len() && b[i] == '=' && b[i + 1] == '"' {
            let clave: String = b[ini..i].iter().collect();
            let mut j = i + 2;
            let mut valor = String::new();
            while j < b.len() && b[j] != '"' {
                valor.push(b[j]);
                j += 1;
            }
            if j < b.len() {
                pares.insert(clave, valor);
                i = j + 1;
                continue;
            }
        }
        if i == ini {
            i += 1;
        }
    }
    pares
}

fn es_host(h: &str) -> bool {
    let (nombre, puerto) = match h.split_once(':') {
        Some((n, p)) => (n, Some(p)),
        None => (h, None),
    };
    if let Some(p) = puerto {
        if p.is_empty() || p.len() > 5 || !p.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }
    !nombre.is_empty()
        && nombre.split('.').all(|e| {
            !e.is_empty() && e.len() <= 63 && e.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !e.starts_with('-') && !e.ends_with('-')
        })
}

fn es_local(registro: &str) -> bool {
    registro.starts_with("localhost") || registro.starts_with("127.")
}

// Entrada de un indice que corresponde a la plataforma pedida, y todas las
// plataformas publicadas (sin atestaciones).
pub fn elegir_plataforma(indice: &Value, os: &str, arch: &str, variante: Option<&str>) -> (Option<Value>, Vec<String>) {
    let mut plataformas: Vec<String> = Vec::new();
    let mut elegido: Option<Value> = None;
    for m in indice.get("manifests").and_then(|x| x.as_array()).map(|x| x.as_slice()).unwrap_or(&[]) {
        let p = m.get("platform").cloned().unwrap_or(Value::Null);
        let s = |k: &str| p.get(k).and_then(|x| x.as_str()).map(String::from);
        let (p_os, p_arch, p_var) = (s("os"), s("architecture"), s("variant"));
        // atestaciones y provenance
        if p_os.is_none() || p_os.as_deref() == Some("unknown") || p_arch.as_deref() == Some("unknown") {
            continue;
        }
        let por_defecto = matches!((p_arch.as_deref(), p_var.as_deref()), (Some("arm64"), Some("v8")) | (Some("amd64"), Some("v1")));
        let mut nombre = format!("{}/{}", p_os.clone().unwrap_or_default(), p_arch.clone().unwrap_or_default());
        if let (Some(v), false) = (&p_var, por_defecto) {
            nombre.push('/');
            nombre.push_str(v);
        }
        plataformas.push(nombre);
        if p_os.as_deref() == Some(os) && p_arch.as_deref() == Some(arch) && (variante.is_none() || p_var.as_deref() == variante) && elegido.is_none() {
            elegido = Some(m.clone());
        }
    }
    (elegido, plataformas)
}

pub fn variables_de_entorno(config: &Value) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    if let Some(items) = config.pointer("/config/Env").and_then(|e| e.as_array()) {
        for item in items.iter().filter_map(|x| x.as_str()) {
            let (k, v) = item.split_once('=').unwrap_or((item, ""));
            env.insert(k.to_string(), v.to_string());
        }
    }
    env
}

// ─────────────────────────────────────────────────────────────────────────
// Cliente
// ─────────────────────────────────────────────────────────────────────────

struct Cliente {
    http: reqwest::Client,
    referencia: ImagenRef,
    esquema: &'static str,
    token: Option<String>,
}

impl Cliente {
    fn nuevo(referencia: ImagenRef) -> Result<Self, String> {
        let host = referencia.api_host();
        if !es_host(&host) {
            return Err(format!("Host de registro inválido: {:?}", host));
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("devops-hub-hardening/1.0")
            .build()
            .map_err(|e| format!("No se pudo crear el cliente HTTP: {}", e))?;
        let esquema = if es_local(&referencia.registry) { "http" } else { "https" };
        Ok(Self { http, referencia, esquema, token: None })
    }

    fn url(&self, ruta: &str) -> String {
        format!("{}://{}{}", self.esquema, self.referencia.api_host(), ruta)
    }

    async fn exigir_publico(&self, host_con_puerto: &str) -> Result<(), String> {
        let host = host_con_puerto.split(':').next().unwrap_or("");
        if es_local(host) {
            return Ok(());
        }
        let direcciones = tokio::net::lookup_host((host, 443)).await.map_err(|e| format!("No se pudo resolver {:?}: {}", host, e))?;
        for d in direcciones {
            if !es_ip_publica(d.ip()) {
                return Err(format!("{:?} resuelve a una dirección no pública ({}); bloqueado (SSRF).", host, d.ip()));
            }
        }
        Ok(())
    }

    async fn peticion(&mut self, url: &str, aceptar: Option<&str>) -> Result<(u16, reqwest::header::HeaderMap, Vec<u8>), String> {
        let mut reintento = true;
        loop {
            let mut destino = url.to_string();
            let mut con_credencial = true;
            let mut respuesta = None;
            for _ in 0..=MAX_REDIRECCIONES {
                let mut req = self.http.get(&destino);
                if let Some(a) = aceptar {
                    req = req.header("Accept", a);
                }
                if let (Some(t), true) = (&self.token, con_credencial) {
                    req = req.header("Authorization", format!("Bearer {}", t));
                }
                let r = req.send().await.map_err(|e| format!("No se pudo conectar con el registro: {}", e))?;
                if matches!(r.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
                    let loc = r.headers().get("location").and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
                    let absoluta = if loc.contains("://") { loc } else { self.url(&loc) };
                    let (esquema, resto) = absoluta.split_once("://").ok_or("Redirección inválida")?;
                    if esquema != "https" && self.esquema != "http" {
                        return Err("Redirección a esquema no seguro bloqueada".into());
                    }
                    let host = resto.split('/').next().unwrap_or("");
                    if host != self.referencia.api_host() {
                        // CDN de blobs: debe ser publico y no recibe credenciales
                        self.exigir_publico(host).await?;
                        con_credencial = false;
                    }
                    destino = absoluta;
                    continue;
                }
                respuesta = Some(r);
                break;
            }
            let r = respuesta.ok_or("Demasiadas redirecciones")?;
            let codigo = r.status().as_u16();
            let cabeceras = r.headers().clone();
            if codigo == 401 && reintento {
                let desafio = cabeceras.get("www-authenticate").and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
                self.autenticar(&desafio).await?;
                reintento = false;
                continue;
            }
            let cuerpo = r.bytes().await.map_err(|e| format!("No se pudo leer la respuesta del registro: {}", e))?.to_vec();
            return Ok((codigo, cabeceras, cuerpo));
        }
    }

    async fn autenticar(&mut self, desafio: &str) -> Result<(), String> {
        if !desafio.to_lowercase().starts_with("bearer") {
            return Ok(());
        }
        let p = parsear_desafio(desafio);
        let realm = p.get("realm").ok_or("Desafío de autenticación sin realm")?;
        let (esquema, resto) = realm.split_once("://").ok_or("realm inválido")?;
        if esquema != "https" && self.esquema != "http" {
            return Err("El servicio de tokens debe usar HTTPS".into());
        }
        let host = resto.split('/').next().unwrap_or("");
        if host != self.referencia.api_host() {
            self.exigir_publico(host).await?;
        }
        let scope = p.get("scope").cloned().unwrap_or_else(|| format!("repository:{}:pull", self.referencia.repository));
        let mut consulta: Vec<(&str, String)> = vec![("scope", scope)];
        if let Some(s) = p.get("service") {
            consulta.push(("service", s.clone()));
        }
        let r = self.http.get(realm).query(&consulta).send().await.map_err(|e| format!("No se pudo obtener el token del registro: {}", e))?;
        if r.status().as_u16() != 200 {
            return Err(format!("Autenticación rechazada por el registro ({})", r.status().as_u16()));
        }
        let datos: Value = r.json().await.map_err(|e| format!("Respuesta de token inválida: {}", e))?;
        self.token = datos.get("token").or_else(|| datos.get("access_token")).and_then(|t| t.as_str()).map(String::from);
        Ok(())
    }

    async fn manifiesto(&mut self, referencia: &str) -> Result<(Value, String, String), String> {
        let url = self.url(&format!("/v2/{}/manifests/{}", self.referencia.repository, referencia));
        let (codigo, cab, cuerpo) = self.peticion(&url, Some(ACEPTAR_MANIFIESTO)).await?;
        match codigo {
            404 => return Err(format!("Imagen no encontrada: {}", self.referencia.mostrar())),
            401 | 403 => return Err("Acceso denegado al registro; la imagen no es pública o requiere credenciales (aún no soportadas).".into()),
            200 => {}
            otro => return Err(format!("El registro respondió {}", otro)),
        }
        let digest = cab
            .get("docker-content-digest")
            .and_then(|h| h.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| format!("sha256:{:x}", Sha256::digest(&cuerpo)));
        let tipo = cab.get("content-type").and_then(|h| h.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").to_string();
        let json: Value = serde_json::from_slice(&cuerpo).map_err(|e| format!("Manifiesto inválido: {}", e))?;
        Ok((json, digest, tipo))
    }

    async fn blob_json(&mut self, digest: &str) -> Result<Value, String> {
        let url = self.url(&format!("/v2/{}/blobs/{}", self.referencia.repository, digest));
        let (codigo, _, cuerpo) = self.peticion(&url, None).await?;
        if codigo != 200 {
            return Err(format!("No se pudo leer el blob de configuración ({})", codigo));
        }
        serde_json::from_slice(&cuerpo).map_err(|e| format!("Configuración de imagen inválida: {}", e))
    }
}

pub async fn resolver(referencia: &ImagenRef, plataforma: &str) -> Result<ImagenResuelta, String> {
    if !es_plataforma(plataforma) {
        return Err(format!("Plataforma no válida: {:?}", plataforma));
    }
    let (os, arch_completa) = plataforma.split_once('/').unwrap_or((plataforma, ""));
    let (arch, variante) = match arch_completa.split_once('/') {
        Some((a, v)) => (a, Some(v)),
        None => (arch_completa, None),
    };

    let mut cli = Cliente::nuevo(referencia.clone())?;
    let pedido = referencia.digest.clone().or_else(|| referencia.tag.clone()).unwrap_or_else(|| "latest".into());
    let (mut manifiesto, mut digest, mut tipo) = cli.manifiesto(&pedido).await?;
    if let Some(d) = &referencia.digest {
        if *d != digest {
            return Err("El digest devuelto por el registro no coincide con el solicitado".into());
        }
    }

    let mut digest_indice: Option<String> = None;
    let plataformas: Vec<String>;
    if TIPOS_INDICE.contains(&tipo.as_str()) || manifiesto.get("manifests").is_some() {
        digest_indice = Some(digest.clone());
        let (elegido, todas) = elegir_plataforma(&manifiesto, os, arch, variante);
        let elegido = elegido.ok_or_else(|| format!("La imagen no publica la plataforma {}. Disponibles: {}", plataforma, todas.join(", ")))?;
        plataformas = todas;
        let digest_elegido = elegido.get("digest").and_then(|d| d.as_str()).ok_or("Entrada de índice sin digest")?.to_string();
        let (m, d, t) = cli.manifiesto(&digest_elegido).await?;
        if d != digest_elegido {
            return Err("Digest del manifiesto de plataforma inconsistente".into());
        }
        manifiesto = m;
        digest = d;
        tipo = t;
    } else {
        plataformas = vec![plataforma.to_string()];
    }
    let _ = tipo;

    let digest_config = manifiesto.pointer("/config/digest").and_then(|d| d.as_str()).ok_or("El manifiesto no declara su configuración")?.to_string();
    let config = cli.blob_json(&digest_config).await?;
    let capas = manifiesto.get("layers").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let cfg = config.get("config").cloned().unwrap_or(Value::Null);
    let texto = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from);

    Ok(ImagenResuelta {
        reference: referencia.mostrar(),
        registry: referencia.registry.clone(),
        repository: referencia.repository.clone(),
        tag: referencia.tag.clone(),
        platform: plataforma.to_string(),
        available_platforms: plataformas,
        index_digest: digest_indice,
        pinned_ref: referencia.fijada(&digest)?,
        manifest_digest: digest,
        created: texto(&config, "created"),
        os: texto(&config, "os"),
        architecture: texto(&config, "architecture"),
        size_bytes: capas.iter().map(|c| c.get("size").and_then(|s| s.as_u64()).unwrap_or(0)).sum(),
        layers: capas.len(),
        env: variables_de_entorno(&config),
        user: texto(&cfg, "User").unwrap_or_default(),
        entrypoint: cfg.get("Entrypoint").filter(|v| !v.is_null()).cloned(),
        cmd: cfg.get("Cmd").filter(|v| !v.is_null()).cloned(),
        workdir: texto(&cfg, "WorkingDir").unwrap_or_default(),
        labels: cfg
            .get("Labels")
            .and_then(|l| l.as_object())
            .map(|o| o.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
            .unwrap_or_default(),
    })
}

// Referencia de alpine fijada por digest que se usa como constructor de la
// imagen: la misma version menor de Alpine que la imagen origen, o la que
// el usuario autorizo expresamente.
pub async fn resolver_constructor(nombre_os: Option<&str>, sobrescribir: Option<&str>, plataforma: &str) -> Result<String, String> {
    let etiqueta = match sobrescribir {
        Some(t) => t.to_string(),
        None => {
            let n = nombre_os.unwrap_or("");
            let mayor: String = n.chars().take_while(|c| c.is_ascii_digit()).collect();
            let resto = n[mayor.len()..].strip_prefix('.').unwrap_or("");
            let menor: String = resto.chars().take_while(|c| c.is_ascii_digit()).collect();
            if mayor.is_empty() || menor.is_empty() {
                return Err("No se pudo determinar la versión de Alpine".into());
            }
            format!("alpine:{}.{}", mayor, menor)
        }
    };
    let r = parsear_referencia(&etiqueta)?;
    let res = resolver(&r, plataforma).await?;
    Ok(res.pinned_ref)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    #[test]
    fn solo_direcciones_publicas() {
        for malo in ["10.0.0.1", "192.168.1.5", "172.16.0.1", "127.0.0.1", "169.254.169.254", "0.0.0.0", "100.64.0.1", "::1", "fd00::1", "fe80::1", "::ffff:10.0.0.1"] {
            assert!(!es_ip_publica(malo.parse().unwrap()), "{}", malo);
        }
        for bueno in ["8.8.8.8", "104.18.121.25", "2606:4700::6810:7919"] {
            assert!(es_ip_publica(bueno.parse().unwrap()), "{}", bueno);
        }
    }

    #[test]
    fn lee_el_desafio_de_docker_hub() {
        let p = parsear_desafio(r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/node:pull""#);
        assert_eq!(p["realm"], "https://auth.docker.io/token");
        assert_eq!(p["service"], "registry.docker.io");
        assert_eq!(p["scope"], "repository:library/node:pull");
        assert!(parsear_desafio("Basic realm=x").is_empty());
    }

    #[test]
    fn hosts_validos() {
        assert!(es_host("registry-1.docker.io"));
        assert!(es_host("123456789.dkr.ecr.us-east-1.amazonaws.com"));
        assert!(es_host("localhost:5000"));
        assert!(!es_host("evil.com/x"));
        assert!(!es_host("a b"));
        assert!(!es_host("host:99999999"));
        assert!(!es_host("-malo.com"));
    }

    #[test]
    fn elige_la_plataforma_y_descarta_atestaciones() {
        let indice = json!({"manifests": [
            {"digest": "sha256:aaa", "platform": {"os": "linux", "architecture": "amd64"}},
            {"digest": "sha256:bbb", "platform": {"os": "linux", "architecture": "arm64", "variant": "v8"}},
            {"digest": "sha256:ccc", "platform": {"os": "linux", "architecture": "arm", "variant": "v7"}},
            {"digest": "sha256:ddd", "platform": {"os": "unknown", "architecture": "unknown"}},
        ]});
        let (e, todas) = elegir_plataforma(&indice, "linux", "arm64", None);
        assert_eq!(e.unwrap()["digest"], "sha256:bbb");
        assert_eq!(todas, vec!["linux/amd64", "linux/arm64", "linux/arm/v7"]);
        let (e, _) = elegir_plataforma(&indice, "linux", "arm", Some("v7"));
        assert_eq!(e.unwrap()["digest"], "sha256:ccc");
        let (e, _) = elegir_plataforma(&indice, "linux", "s390x", None);
        assert!(e.is_none());
    }

    #[test]
    fn variables_de_entorno_de_la_imagen() {
        let c = json!({"config": {"Env": ["PATH=/usr/bin", "NODE_VERSION=26.12.0", "VACIA="]}});
        let env = variables_de_entorno(&c);
        assert_eq!(env["NODE_VERSION"], "26.12.0");
        assert_eq!(env["VACIA"], "");
        assert_eq!(env.len(), 3);
    }

    // Requiere red: se ejecuta a mano con `cargo test -- --ignored registro`.
    #[tokio::test]
    #[ignore]
    async fn resuelve_una_imagen_real_de_docker_hub() {
        let r = parsear_referencia("node:26.12.0-alpine3.24").unwrap();
        let img = resolver(&r, "linux/amd64").await.unwrap();
        println!("{:#?}", img);
        assert!(img.pinned_ref.contains("@sha256:"));
        assert_eq!(img.env.get("NODE_VERSION").map(String::as_str), Some("26.12.0"));
    }
}
