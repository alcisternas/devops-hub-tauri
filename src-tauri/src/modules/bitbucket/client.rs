// ─────────────────────────────────────────────────────────────────────────
// Cliente HTTP para la API de Bitbucket
//
// Centraliza lo que antes estaba suelto en credentials.rs: el header de
// autenticacion, el tiempo maximo de espera y la lectura de respuestas.
// Todas las operaciones de repos.rs pasan por aca.
// ─────────────────────────────────────────────────────────────────────────

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::Value;

use super::super::eks::config::RefreshError;

pub const API_BASE: &str = "https://api.bitbucket.org/2.0";
const TIMEOUT_SEGUNDOS: u64 = 30;

pub struct BBClient {
    http: reqwest::Client,
    auth: String,
}

impl BBClient {
    pub fn new(email: &str, token: &str) -> Result<Self, RefreshError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SEGUNDOS))
            .build()
            .map_err(|e| RefreshError::new("crear_cliente_http", e.to_string()))?;

        // Basic Auth armado a mano. Viene de un bug real de la herramienta
        // anterior: el mecanismo automatico de autenticacion esperaba un
        // desafio-respuesta del servidor que Bitbucket no entrega igual en
        // todos los endpoints, y los POST fallaban.
        let auth = format!("Basic {}", BASE64.encode(format!("{}:{}", email, token)));

        Ok(Self { http, auth })
    }

    // Todas las peticiones devuelven (codigo, cuerpo) en vez de fallar ante
    // un codigo de error HTTP. El que llama decide que significa cada codigo:
    // un 404 al verificar un repo es una respuesta util, no un problema.
    // Solo se devuelve Err cuando la peticion no llego a destino.
    async fn enviar(&self, req: reqwest::RequestBuilder) -> Result<(u16, String), RefreshError> {
        let resp = req
            .header("Authorization", &self.auth)
            .header("Accept", "application/json")
            .header("User-Agent", "devops-hub-bitbucket")
            .send()
            .await
            .map_err(|e| RefreshError::new("conectar_bitbucket", e.to_string()))?;

        let codigo = resp.status().as_u16();
        let cuerpo = resp.text().await.unwrap_or_default();

        Ok((codigo, cuerpo))
    }

    pub async fn get(&self, path: &str) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.get(format!("{}{}", API_BASE, path))).await
    }

    pub async fn post_json(&self, path: &str, payload: Value) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.post(format!("{}{}", API_BASE, path)).json(&payload))
            .await
    }

    pub async fn put_json(&self, path: &str, payload: Value) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.put(format!("{}{}", API_BASE, path)).json(&payload))
            .await
    }

    // Envio multiparte para el commit inicial. Bitbucket recibe los archivos
    // de un commit como campos de formulario, donde el NOMBRE del campo es la
    // ruta del archivo en el repo y el valor es su contenido.
    //
    // El cuerpo se arma a mano en vez de usar el constructor de formularios
    // de la libreria. Dos razones: es exactamente el mismo formato que ya
    // funciono en la herramienta anterior, y evita sumar una caracteristica
    // extra a la dependencia solo para esto.
    pub async fn post_multipart(
        &self,
        path: &str,
        campos: &[(&str, &str)],
    ) -> Result<(u16, String), RefreshError> {
        let boundary = "----DevOpsHubBitbucket";
        let mut cuerpo = String::new();

        for (nombre, valor) in campos {
            cuerpo.push_str(&format!("--{}\r\n", boundary));
            cuerpo.push_str(&format!(
                "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                nombre
            ));
            cuerpo.push_str(valor);
            cuerpo.push_str("\r\n");
        }
        cuerpo.push_str(&format!("--{}--\r\n", boundary));

        let req = self
            .http
            .post(format!("{}{}", API_BASE, path))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(cuerpo.into_bytes());

        self.enviar(req).await
    }
}

// Extrae el mensaje de error que devuelve Bitbucket. Si el cuerpo no es el
// JSON esperado, se devuelve un recorte del texto crudo: es preferible a
// perder la unica pista de que fallo.
pub fn mensaje_error(cuerpo: &str) -> String {
    serde_json::from_str::<Value>(cuerpo)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| cuerpo.chars().take(200).collect())
}

// Saca la URL web del repositorio de la respuesta de Bitbucket.
pub fn url_repo(cuerpo: &str) -> Option<String> {
    serde_json::from_str::<Value>(cuerpo)
        .ok()
        .and_then(|v| {
            v.get("links")
                .and_then(|l| l.get("html"))
                .and_then(|h| h.get("href"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string())
        })
}
