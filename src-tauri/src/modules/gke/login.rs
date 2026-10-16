use serde::Serialize;
use std::process::Command as StdCommand;
use tauri::AppHandle;

use crate::modules::eks::config::{load_or_create_accounts, save_accounts, RefreshError};

// A diferencia de AWS SSO, gcloud guarda las credenciales en
// ~/.config/gcloud/credentials.db y las renueva automáticamente — no hay
// ningún archivo que nosotros necesitemos escribir a mano (nada de bloques
// [sso-session] ni [profile]). Solo verificar/disparar el login.

fn check_gcloud_token(account: &str) -> Option<String> {
    let output = StdCommand::new(super::GCLOUD_BIN)
        .args(["auth", "print-access-token", "--account", account])
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
    let output = StdCommand::new(super::GCLOUD_BIN)
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

// Primera instalación o cuenta vencida: abre el navegador sin restringir la
// cuenta — el usuario elige la suya — y devuelve la que quedó activa para
// guardarla en accounts.json.
fn run_gcloud_login_any() -> Result<(), RefreshError> {
    let output = StdCommand::new(super::GCLOUD_BIN)
        .args(["auth", "login"])
        .output()
        .map_err(|e| RefreshError::new("gcloud_auth_login", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(RefreshError::new("gcloud_auth_login", stderr))
    }
}

fn get_active_gcloud_account() -> Result<String, RefreshError> {
    let output = StdCommand::new(super::GCLOUD_BIN)
        .args(["config", "get-value", "account"])
        .output()
        .map_err(|e| RefreshError::new("leer_cuenta_gcloud", e.to_string()))?;

    let account = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if account.is_empty() || account == "(unset)" {
        return Err(RefreshError::new(
            "leer_cuenta_gcloud",
            "No se pudo determinar la cuenta activa después del login.",
        ));
    }
    Ok(account)
}

// Verifica si hay un token válido para la cuenta guardada — sin abrir browser
// ni modificar la config de gcloud. El login interactivo solo ocurre en
// setup_gcloud_login. Usado por discovery.rs, load/loader.rs y cicd/subnets.rs
// para confirmar que la sesión sigue vigente antes de invocar gcloud.
pub(crate) fn ensure_gcp_session(account: &str) -> Result<bool, RefreshError> {
    if account.is_empty() {
        return Err(RefreshError::new(
            "cuenta_gcp_no_configurada",
            "No hay cuenta GCP configurada. Ve a Clusters → GKE Chile para iniciar sesión.",
        ));
    }

    if check_gcloud_token(account).is_some() {
        return Ok(true);
    }

    Err(RefreshError::new(
        "sesion_gcp_vencida",
        "La sesión de GCP venció. Ve a Clusters → GKE Chile para renovar el acceso.",
    ))
}

#[derive(Debug, Serialize)]
pub struct GcpLoginResult {
    pub reused_session: bool,
    pub account: String,
}

#[tauri::command]
pub async fn setup_gcloud_login(app: AppHandle) -> Result<GcpLoginResult, RefreshError> {
    let mut accounts_cfg = load_or_create_accounts(&app)?;
    let account = accounts_cfg.gcp.chile.account.clone();

    // Si hay cuenta guardada y el token sigue vigente, reutilizar sesión.
    if !account.is_empty() {
        if check_gcloud_token(&account).is_some() {
            return Ok(GcpLoginResult { reused_session: true, account });
        }
    }

    // Sin cuenta O token vencido/inválido → login libre en el navegador.
    // No se fuerza una cuenta específica: el usuario elige la suya.
    // Esto también corrige cuentas heredadas de instalaciones anteriores.
    run_gcloud_login_any()?;
    let detected = get_active_gcloud_account()?;
    accounts_cfg.gcp.chile.account = detected.clone();
    save_accounts(&app, &accounts_cfg)?;
    Ok(GcpLoginResult { reused_session: false, account: detected })
}
