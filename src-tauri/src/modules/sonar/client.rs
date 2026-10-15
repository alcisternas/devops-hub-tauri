// ─────────────────────────────────────────────────────────────────────────
// Cliente HTTP para SonarQube Cloud
//
// OJO: SonarQube Cloud expone DOS bases distintas y hay que usar la correcta
// segun el endpoint. No es un detalle cosmetico, fue verificado contra la
// instancia real:
//
//   https://sonarcloud.io      → API v1. Form-urlencoded.
//                                (projects/create, navigation/component)
//   https://api.sonarcloud.io  → API v2. JSON.
//                                (dop-translation/project-bindings)
//
// Ademas, la API de Cloud NO es la misma que la de SonarQube Server: el
// binding en Server es 'api/v2/dop-translation/bound-projects' y en Cloud es
// 'dop-translation/project-bindings'. La documentacion de Server no aplica.
// ─────────────────────────────────────────────────────────────────────────

use serde_json::Value;

use super::super::eks::config::RefreshError;

const BASE_V1: &str = "https://sonarcloud.io";
const BASE_V2: &str = "https://api.sonarcloud.io";

// Organizacion fija: el equipo trabaja siempre sobre la misma.
pub const ORG: &str = "banco-ripley";

const TIMEOUT_SEGUNDOS: u64 = 30;

pub struct SonarClient {
    http: reqwest::Client,
    auth: String,
}

impl SonarClient {
    pub fn new(token: &str) -> Result<Self, RefreshError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SEGUNDOS))
            .build()
            .map_err(|e| RefreshError::new("crear_cliente_http", e.to_string()))?;

        // SonarQube Cloud usa Bearer, no Basic. Es la unica diferencia de
        // autenticacion respecto al cliente de Bitbucket.
        Ok(Self {
            http,
            auth: format!("Bearer {}", token),
        })
    }

    async fn enviar(&self, req: reqwest::RequestBuilder) -> Result<(u16, String), RefreshError> {
        let resp = req
            .header("Authorization", &self.auth)
            .header("Accept", "application/json")
            .header("User-Agent", "devops-hub-sonar")
            .send()
            .await
            .map_err(|e| RefreshError::new("conectar_sonarqube", e.to_string()))?;

        let codigo = resp.status().as_u16();
        let cuerpo = resp.text().await.unwrap_or_default();
        Ok((codigo, cuerpo))
    }

    pub async fn get_v1(&self, path: &str) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.get(format!("{}{}", BASE_V1, path))).await
    }

    // La v1 espera formulario, no JSON.
    pub async fn post_v1_form(
        &self,
        path: &str,
        campos: &[(&str, &str)],
    ) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.post(format!("{}{}", BASE_V1, path)).form(campos))
            .await
    }

    pub async fn get_v2(&self, path: &str) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.get(format!("{}{}", BASE_V2, path))).await
    }

    // La v2 espera JSON.
    pub async fn post_v2_json(&self, path: &str, payload: Value) -> Result<(u16, String), RefreshError> {
        self.enviar(self.http.post(format!("{}{}", BASE_V2, path)).json(&payload))
            .await
    }
}

// SonarQube v1 devuelve los errores como {"errors":[{"msg":"..."}]} y la v2
// como {"message":"..."}. Se contemplan ambos para no perder el motivo.
pub fn mensaje_error(cuerpo: &str) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(cuerpo) {
        if let Some(errores) = v.get("errors").and_then(|e| e.as_array()) {
            let msgs: Vec<String> = errores
                .iter()
                .filter_map(|e| e.get("msg").and_then(|m| m.as_str()).map(String::from))
                .collect();
            if !msgs.is_empty() {
                return msgs.join("; ");
            }
        }
        if let Some(m) = v.get("message").and_then(|m| m.as_str()) {
            return m.to_string();
        }
    }
    cuerpo.chars().take(200).collect()
}

pub fn campo_str(cuerpo: &str, ruta: &[&str]) -> Option<String> {
    let mut actual: Value = serde_json::from_str(cuerpo).ok()?;
    for clave in ruta {
        actual = actual.get(clave)?.clone();
    }
    actual.as_str().map(String::from)
}
