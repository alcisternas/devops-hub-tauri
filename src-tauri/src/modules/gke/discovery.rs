use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command as StdCommand;
use tauri::AppHandle;

use crate::modules::eks::config::{load_or_create_accounts, RefreshError};
use crate::modules::eks::permissions::emit_progress;
use super::login::run_gcloud_login;

// Mismas señales que el bash busca en stderr para distinguir "sesión
// expirada, recuperable" de "la cuenta realmente no tiene permisos".
// Antes de este chequeo, ambos casos se confundían y terminaban guardando
// un inventario vacío por un problema de credenciales, no de permisos.
fn contains_reauth_signal(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["reauth", "refreshing your current auth", "does not have any valid credentials", "invalid_grant"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn run_list_projects(account: &str) -> (bool, String, String) {
    match StdCommand::new("gcloud")
        .args([
            "projects",
            "list",
            "--filter=lifecycleState=ACTIVE",
            "--format=json(projectId,name,projectNumber)",
            "--account",
            account,
        ])
        .output()
    {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
        ),
        Err(e) => (false, String::new(), e.to_string()),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GcpProject {
    pub project_id: String,
    pub name: String,
    pub project_number: String,
}

#[derive(Debug, Serialize)]
pub struct GcpDiscoveryResult {
    pub total_proyectos: usize,
    pub proyectos: Vec<GcpProject>,
    pub reauth_detectado: bool,
}

#[tauri::command]
pub async fn discover_gcp_projects(app: AppHandle) -> Result<GcpDiscoveryResult, RefreshError> {
    emit_progress("gke", &app, "descubriendo_proyectos", "GCP", 1, 1, "running");

    let accounts_cfg = load_or_create_accounts(&app)?;
    let account = accounts_cfg.gcp.chile.account.clone();
    let org_id = accounts_cfg.gcp.chile.org_id.clone();

    // Asegura sesión ANTES de listar — si la cuenta nunca hizo login,
    // no hay garantía de que el error de gcloud coincida con alguno de
    // los patrones de reautenticación que se detectan más abajo (esos
    // cubren token vencido a mitad de operación, no "nunca hizo login").
    if let Err(e) = super::login::ensure_gcp_session(&account) {
        emit_progress("gke", &app, "descubriendo_proyectos", "GCP", 1, 1, "error");
        return Err(e);
    }

    let (mut ok, mut stdout, stderr) = run_list_projects(&account);
    let mut reauth_detectado = false;

    // ¿El fallo fue por credenciales? Reintentar UNA vez tras relogin,
    // igual que process_country() en el bash.
    if contains_reauth_signal(&stderr) {
        reauth_detectado = true;
        if let Err(e) = run_gcloud_login(&account) {
            emit_progress("gke", &app, "descubriendo_proyectos", "GCP", 1, 1, "error");
            return Err(e);
        }
        let retry = run_list_projects(&account);
        ok = retry.0;
        stdout = retry.1;
    }

    let parsed: Value = serde_json::from_str(stdout.trim()).unwrap_or(Value::Array(vec![]));
    let list = parsed.as_array().cloned().unwrap_or_default();

    if !ok && list.is_empty() {
        emit_progress("gke", &app, "descubriendo_proyectos", "GCP", 1, 1, "error");
        return Err(RefreshError::new(
            "listar_proyectos_gcp",
            format!(
                "No se encontraron proyectos activos para la organización {}. Verifica que la cuenta {} tenga permisos de listado.",
                org_id, account
            ),
        ));
    }

    let proyectos: Vec<GcpProject> = list
        .iter()
        .filter_map(|p| {
            let project_id = p.get("projectId").and_then(|v| v.as_str())?.to_string();
            let name = p
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&project_id)
                .to_string();
            let project_number = match p.get("projectNumber") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            };
            Some(GcpProject {
                project_id,
                name,
                project_number,
            })
        })
        .collect();

    emit_progress("gke", &app, "descubriendo_proyectos", "GCP", 1, 1, "ok");

    let total_proyectos = proyectos.len() as u32;
    for (i, p) in proyectos.iter().enumerate() {
        emit_progress(
            "gke",
            &app,
            "proyecto_descubierto",
            &format!("{} ({}) — número: {}", p.name, p.project_id, p.project_number),
            (i + 1) as u32,
            total_proyectos,
            "ok",
        );
    }

    Ok(GcpDiscoveryResult {
        total_proyectos: proyectos.len(),
        proyectos,
        reauth_detectado,
    })
}
