// ─────────────────────────────────────────────────────────────────────────
// Cliente HTTP para Jira Cloud
//
// El token es un token de API con scopes (read:jira-work y write:jira-work).
// Los tokens con scopes NO funcionan contra https://<sitio>.atlassian.net:
// hay que llamar a api.atlassian.com/ex/jira/<cloudId>/..., con Bearer. El
// cloudId se obtiene de un endpoint publico del sitio, sin autenticacion.
//
// La API de formularios (donde viven los nombres de repositorio del ticket)
// tiene otra base: api.atlassian.com/jira/forms/cloud/<cloudId>/...
// ─────────────────────────────────────────────────────────────────────────

use serde_json::Value;

use super::super::eks::config::RefreshError;

pub const SITE: &str = "bancoripley1.atlassian.net";
// Proyecto de Jira donde llegan las solicitudes. Tambien sirve para validar el
// token: la cuenta debe ver sus tickets (ver credentials::validar).
pub const PROYECTO_JIRA: &str = "SCDO";
const TIMEOUT_SEGUNDOS: u64 = 30;

pub struct JiraClient {
    http: reqwest::Client,
    auth: String,
    api: String,
    forms: String,
}

impl JiraClient {
    pub async fn new(token: &str) -> Result<Self, RefreshError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SEGUNDOS))
            .build()
            .map_err(|e| RefreshError::new("crear_cliente_http", e.to_string()))?;

        let cloud_id = Self::cloud_id(&http).await?;

        Ok(Self {
            http,
            auth: format!("Bearer {}", token),
            api: format!("https://api.atlassian.com/ex/jira/{}", cloud_id),
            forms: format!("https://api.atlassian.com/jira/forms/cloud/{}", cloud_id),
        })
    }

    async fn cloud_id(http: &reqwest::Client) -> Result<String, RefreshError> {
        let resp = http
            .get(format!("https://{}/_edge/tenant_info", SITE))
            .send()
            .await
            .map_err(|e| RefreshError::new("conectar_jira", e.to_string()))?;
        let cuerpo = resp.text().await.unwrap_or_default();

        serde_json::from_str::<Value>(&cuerpo)
            .ok()
            .and_then(|v| v.get("cloudId").and_then(|c| c.as_str()).map(String::from))
            .ok_or_else(|| {
                RefreshError::new(
                    "conectar_jira",
                    "No se pudo obtener el identificador del sitio de Jira.",
                )
            })
    }

    // Igual que en Bitbucket: devuelve (codigo, cuerpo) sin fallar ante un
    // codigo de error HTTP; quien llama decide que significa cada uno.
    async fn enviar(&self, req: reqwest::RequestBuilder) -> Result<(u16, String), RefreshError> {
        let resp = req
            .header("Authorization", &self.auth)
            .header("Accept", "application/json")
            .header("User-Agent", "devops-hub-jira")
            .send()
            .await
            .map_err(|e| RefreshError::new("conectar_jira", e.to_string()))?;

        let codigo = resp.status().as_u16();
        let cuerpo = resp.text().await.unwrap_or_default();
        Ok((codigo, cuerpo))
    }

    pub async fn get(&self, ruta: &str, query: &[(&str, &str)]) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.get(format!("{}{}", self.api, ruta)).query(query))
            .await
    }

    pub async fn get_forms(&self, ruta: &str) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.get(format!("{}{}", self.forms, ruta))).await
    }

    pub async fn post_json(&self, ruta: &str, payload: Value) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.post(format!("{}{}", self.api, ruta)).json(&payload))
            .await
    }

    pub async fn put_json(&self, ruta: &str, payload: Value) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.put(format!("{}{}", self.api, ruta)).json(&payload))
            .await
    }

    // Adjunta un archivo a un ticket. El cuerpo multiparte se arma a mano,
    // igual que en el cliente de Bitbucket, pero con bytes: una imagen no es
    // texto. X-Atlassian-Token: no-check es obligatorio en este endpoint.
    pub async fn adjuntar(
        &self,
        ruta: &str,
        nombre: &str,
        tipo: &str,
        bytes: &[u8],
    ) -> Result<(u16, String), RefreshError> {
        let boundary = "----DevOpsHubJira";
        let mut cuerpo: Vec<u8> = Vec::with_capacity(bytes.len() + 256);
        cuerpo.extend_from_slice(
            format!(
                "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
                boundary, nombre, tipo
            )
            .as_bytes(),
        );
        cuerpo.extend_from_slice(bytes);
        cuerpo.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

        let req = self
            .http
            .post(format!("{}{}", self.api, ruta))
            .header("X-Atlassian-Token", "no-check")
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={}", boundary),
            )
            .body(cuerpo);

        self.enviar(req).await
    }
}

// Jira devuelve los errores de tres formas segun el endpoint: una lista
// errorMessages, un objeto errors campo -> mensaje, o un campo message.
// Se prueba en ese orden y, si nada coincide, se devuelve un recorte del
// texto crudo: es preferible a perder la unica pista de que fallo.
pub fn mensaje_error(cuerpo: &str) -> String {
    let v = match serde_json::from_str::<Value>(cuerpo) {
        Ok(v) => v,
        Err(_) => return cuerpo.chars().take(200).collect(),
    };

    if let Some(m) = v
        .get("errorMessages")
        .and_then(|e| e.as_array())
        .and_then(|a| a.first())
        .and_then(|m| m.as_str())
    {
        return m.to_string();
    }
    if let Some(m) = v
        .get("errors")
        .and_then(|e| e.as_object())
        .and_then(|o| o.values().next())
        .and_then(|m| m.as_str())
    {
        return m.to_string();
    }
    if let Some(m) = v.get("message").and_then(|m| m.as_str()) {
        return m.to_string();
    }
    cuerpo.chars().take(200).collect()
}
