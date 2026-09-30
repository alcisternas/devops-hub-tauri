use serde::Serialize;
use std::process::Command as StdCommand;
use tauri::AppHandle;

use crate::modules::eks::config::{load_or_create_accounts, RefreshError};

// A diferencia de AWS SSO, gcloud guarda las credenciales en
// ~/.config/gcloud/credentials.db y las renueva automáticamente — no hay
// ningún archivo que nosotros necesitemos escribir a mano (nada de bloques
// [sso-session] ni [profile]). Solo verificar/disparar el login.

fn check_gcloud_token(account: &str) -> Option<String> {
    let output = StdCommand::new("gcloud")
        .args(["auth", "print-access-token", "--account", account, "--format", "value(token)"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

pub(crate) fn run_gcloud_login(account: &str) -> Result<(), RefreshError> {
    let output = StdCommand::new("gcloud")
        .args(["auth", "login", account])
        .output()
        .map_err(|e| RefreshError::new("gcloud_auth_login", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(RefreshError::new("gcloud_auth_login", stderr))
    }
}

fn set_active_account(account: &str) -> Result<(), RefreshError> {
    let output = StdCommand::new("gcloud")
        .args(["config", "set", "account", account])
        .output()
        .map_err(|e| RefreshError::new("gcloud_config_set_account", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(RefreshError::new("gcloud_config_set_account", stderr))
    }
}

// Garantiza una sesión GCP válida — reutiliza token si es válido, login
// solo si de verdad hace falta. Extraída para que el módulo load también
// la use antes de cargar clusters GCP, igual que ensure_chile_sso /
// ensure_peru_sso para AWS.
pub(crate) fn ensure_gcp_session(account: &str) -> Result<bool, RefreshError> {
    if check_gcloud_token(account).is_some() {
        set_active_account(account)?;
        return Ok(true); // reused_session
    }

    run_gcloud_login(account)?;
    set_active_account(account)?;

    if check_gcloud_token(account).is_none() {
        return Err(RefreshError::new(
            "validar_token_gcp",
            "El login se completó pero no se pudo obtener un token de acceso válido.",
        ));
    }

    Ok(false) // no se reutilizó, se hizo login recién
}

#[derive(Debug, Serialize)]
pub struct GcpLoginResult {
    pub reused_session: bool,
    pub account: String,
}

#[tauri::command]
pub async fn setup_gcloud_login(app: AppHandle) -> Result<GcpLoginResult, RefreshError> {
    let accounts_cfg = load_or_create_accounts(&app)?;
    let account = accounts_cfg.gcp.chile.account.clone();

    let reused_session = ensure_gcp_session(&account)?;

    Ok(GcpLoginResult { reused_session, account })
}
