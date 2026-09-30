use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;
use tauri::AppHandle;

use super::config::{load_or_create_accounts, RefreshError};
use super::permissions::emit_progress;
use super::sso::{aws_sso_cache_dir, find_cached_token};

// ─────────────────────────────────────────────────────────────────────────
// Tipos expuestos a React
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountRoleInfo {
    pub account_id: String,
    pub account_name: String,
    pub profile: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DiscoveryResult {
    pub total_cuentas_portal: usize,
    pub cuentas: Vec<AccountRoleInfo>,
    // Cuentas del portal que no están mapeadas en accounts.json, o que
    // fallaron al pedir sus roles — mismo criterio que log_skip/log_warn
    // en process_portals() del bash: se registran, no rompen el resto.
    pub omitidas: Vec<String>,
    // Duración de ESTA fase únicamente (descubrimiento) — el frontend la
    // suma a la del recorrido completo para mostrar un total combinado.
    pub duracion_segundos: u64,
}

// ─────────────────────────────────────────────────────────────────────────
// Llamadas a la API SSO — equivalentes a "aws sso list-accounts" y
// "aws sso list-account-roles" del bash, parseadas con serde_json en vez
// de jq.
// ─────────────────────────────────────────────────────────────────────────

fn list_accounts(token: &str, region: &str) -> Result<Vec<(String, String)>, RefreshError> {
    let output = Command::new("aws")
        .args([
            "sso",
            "list-accounts",
            "--access-token",
            token,
            "--region",
            region,
            "--output",
            "json",
        ])
        .output()
        .map_err(|e| RefreshError::new("listar_cuentas", e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RefreshError::new("listar_cuentas", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(&stdout)
        .map_err(|e| RefreshError::new("parsear_cuentas", e.to_string()))?;

    let list = parsed
        .get("accountList")
        .and_then(|v| v.as_array())
        .ok_or_else(|| RefreshError::new("parsear_cuentas", "Respuesta sin accountList"))?;

    let mut result = Vec::new();
    for acc in list {
        let id = acc.get("accountId").and_then(|v| v.as_str()).unwrap_or("");
        let name = acc.get("accountName").and_then(|v| v.as_str()).unwrap_or("");
        if !id.is_empty() {
            result.push((id.to_string(), name.to_string()));
        }
    }
    Ok(result)
}

fn list_account_roles(token: &str, region: &str, account_id: &str) -> Result<Vec<String>, RefreshError> {
    let output = Command::new("aws")
        .args([
            "sso",
            "list-account-roles",
            "--account-id",
            account_id,
            "--access-token",
            token,
            "--region",
            region,
            "--output",
            "json",
        ])
        .output()
        .map_err(|e| RefreshError::new("listar_roles", e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RefreshError::new("listar_roles", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(&stdout)
        .map_err(|e| RefreshError::new("parsear_roles", e.to_string()))?;

    let list = parsed
        .get("roleList")
        .and_then(|v| v.as_array())
        .ok_or_else(|| RefreshError::new("parsear_roles", "Respuesta sin roleList"))?;

    let roles = list
        .iter()
        .filter_map(|r| r.get("roleName").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();

    Ok(roles)
}

// ─────────────────────────────────────────────────────────────────────────
// Comando expuesto a React
// ─────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn discover_chile_accounts(app: AppHandle) -> Result<DiscoveryResult, RefreshError> {
    let started_at = std::time::Instant::now();
    emit_progress("eks-chile", &app, "descubriendo_cuentas", "Chile", 1, 1, "running");

    // Asegura la sesión (reutiliza caché si es válida, login solo si de
    // verdad hace falta) — ya no es requisito haber apretado el botón de
    // login por separado antes. Mismo comportamiento que discover_gcp_projects.
    if let Err(e) = super::sso::ensure_chile_sso(&app).await {
        emit_progress("eks-chile", &app, "descubriendo_cuentas", "Chile", 1, 1, "error");
        return Err(e);
    }

    let accounts_cfg = load_or_create_accounts(&app)?;
    let cache_dir = aws_sso_cache_dir(&app)?;

    let (token, _expiry) = match find_cached_token(&cache_dir, &accounts_cfg.chile.portal_url) {
        Some(t) => t,
        None => {
            emit_progress("eks-chile", &app, "descubriendo_cuentas", "Chile", 1, 1, "error");
            return Err(RefreshError::new(
                "sin_sesion",
                "La sesión se validó pero no se encontró el token en caché (inesperado).",
            ));
        }
    };

    let portal_accounts = match list_accounts(&token, &accounts_cfg.sso_region) {
        Ok(p) => p,
        Err(e) => {
            emit_progress("eks-chile", &app, "descubriendo_cuentas", "Chile", 1, 1, "error");
            return Err(e);
        }
    };
    let total_cuentas_portal = portal_accounts.len();

    // Mapa account_id -> profile, tomado directamente de accounts.json
    let mapping: HashMap<String, String> = accounts_cfg
        .chile
        .accounts
        .iter()
        .map(|a| (a.account_id.clone(), a.profile.clone()))
        .collect();

    let mut cuentas = Vec::new();
    let mut omitidas = Vec::new();

    for (account_id, account_name) in portal_accounts {
        let profile = match mapping.get(&account_id) {
            Some(p) => p.clone(),
            None => {
                omitidas.push(format!("{} ({}) — sin mapeo en accounts.json", account_id, account_name));
                continue;
            }
        };

        match list_account_roles(&token, &accounts_cfg.sso_region, &account_id) {
            Ok(roles) => cuentas.push(AccountRoleInfo {
                account_id,
                account_name,
                profile,
                roles,
            }),
            Err(e) => omitidas.push(format!(
                "{} ({}) — error obteniendo roles: {}",
                account_id, account_name, e.mensaje
            )),
        }
    }

    emit_progress("eks-chile", &app, "descubriendo_cuentas", "Chile", 1, 1, "ok");

    // Detalle cuenta por cuenta en la consola — no solo el resumen. Reusa
    // el mismo canal de module_progress; total/progreso acá describen la
    // posición dentro de esta lista, no una unidad de tiempo real.
    let total_cuentas = cuentas.len() as u32;
    for (i, c) in cuentas.iter().enumerate() {
        emit_progress(
            "eks",
            &app,
            "cuenta_descubierta",
            &format!("{} — {} ({}) — roles: {}", c.profile, c.account_name, c.account_id, c.roles.join(", ")),
            (i + 1) as u32,
            total_cuentas,
            "ok",
        );
    }
    let total_omitidas = omitidas.len() as u32;
    for (i, o) in omitidas.iter().enumerate() {
        emit_progress("eks-chile", &app, "cuenta_omitida", o, (i + 1) as u32, total_omitidas, "error");
    }

    Ok(DiscoveryResult {
        total_cuentas_portal,
        cuentas,
        omitidas,
        duracion_segundos: started_at.elapsed().as_secs(),
    })
}
