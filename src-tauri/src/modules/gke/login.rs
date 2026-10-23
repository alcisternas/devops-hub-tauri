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

#[derive(Debug, Serialize)]
pub struct GcpLoginResult {
    pub reused_session: bool,
    pub account: String,
}

// Garantiza una sesión GCP válida — reutiliza el token si lo hay y abre el
// login en el navegador solo si de verdad hace falta (sin cuenta guardada,
// token vencido o cuenta heredada sin sesión). Mismo patrón que
// ensure_chile_sso / ensure_peru_sso para AWS: el flujo que la necesita se
// recupera solo, sin mandar al usuario a buscar un botón en otra pantalla.
//
// No fuerza ninguna cuenta: el usuario elige la suya en el navegador y la
// que quedó activa se guarda en accounts.json. Devuelve la cuenta efectiva,
// que puede diferir de la recibida; quien la use después debe tomar esa.
//
// Bloqueante (el login espera al navegador): llamar desde spawn_blocking.
// Usado por discovery.rs, load/loader.rs y cicd/subnets.rs.
pub(crate) fn ensure_gcp_session(
    app: &AppHandle,
    account: &str,
) -> Result<GcpLoginResult, RefreshError> {
    if !account.is_empty() && check_gcloud_token(account).is_some() {
        return Ok(GcpLoginResult { reused_session: true, account: account.to_string() });
    }

    run_gcloud_login_any().map_err(|e| {
        RefreshError::new(
            "sesion_gcp_vencida",
            format!(
                "No se pudo iniciar sesión en GCP: {} Cierra la ventana del navegador si quedó abierta y vuelve a intentar.",
                e.mensaje
            ),
        )
    })?;
    let detected = get_active_gcloud_account()?;

    if check_gcloud_token(&detected).is_none() {
        return Err(RefreshError::new(
            "validar_token_gcp",
            "El login se completó pero no se pudo obtener un token de acceso válido.",
        ));
    }

    if detected != account {
        let mut accounts_cfg = load_or_create_accounts(app)?;
        accounts_cfg.gcp.chile.account = detected.clone();
        save_accounts(app, &accounts_cfg)?;
    }

    Ok(GcpLoginResult { reused_session: false, account: detected })
}

#[tauri::command]
pub async fn setup_gcloud_login(app: AppHandle) -> Result<GcpLoginResult, RefreshError> {
    let account = load_or_create_accounts(&app)?.gcp.chile.account;
    tokio::task::spawn_blocking(move || ensure_gcp_session(&app, &account))
        .await
        .map_err(|e| RefreshError::new("sesion_gcp", e.to_string()))?
}
