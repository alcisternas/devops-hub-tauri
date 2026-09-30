use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::process::Command as StdCommand;
use tauri::AppHandle;

use crate::modules::eks::config::{load_or_create_accounts, RefreshError};
use crate::modules::eks::permissions::{build_kargs, read_server_and_ca, run_permission_checks, ClusterAccessResult};

// ─────────────────────────────────────────────────────────────────────────
// Descubrimiento de clusters dentro de UN proyecto — equivalente al bucle
// interno de process_country() del bash, pero para un solo proyecto (el
// recorrido de todos los proyectos es el Sub-paso 4).
// ─────────────────────────────────────────────────────────────────────────

pub(crate) fn set_active_project(project_id: &str, account: &str) -> Result<(), RefreshError> {
    let output = StdCommand::new("gcloud")
        .args(["config", "set", "project", project_id, "--account", account])
        .output()
        .map_err(|e| RefreshError::new("set_active_project", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(RefreshError::new("set_active_project", stderr))
    }
}

pub(crate) struct GkeClusterInfo {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) location: String,
}

pub(crate) fn list_clusters_for_project(project_id: &str, account: &str) -> Result<Vec<GkeClusterInfo>, RefreshError> {
    let output = StdCommand::new("gcloud")
        .args([
            "container",
            "clusters",
            "list",
            "--project",
            project_id,
            "--account",
            account,
            "--format",
            "json(name,status,location,currentMasterVersion)",
        ])
        .output()
        .map_err(|e| RefreshError::new("listar_clusters_gke", e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RefreshError::new("listar_clusters_gke", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(stdout.trim()).unwrap_or(Value::Array(vec![]));
    let list = parsed.as_array().cloned().unwrap_or_default();

    let clusters = list
        .iter()
        .filter_map(|c| {
            let name = c.get("name").and_then(|v| v.as_str())?.to_string();
            let status = c.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let location = c.get("location").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Some(GkeClusterInfo { name, status, location })
        })
        .collect();

    Ok(clusters)
}

// ─────────────────────────────────────────────────────────────────────────
// get-credentials + rename de contexto — mecanismo NUEVO que no existía en
// EKS (aws eks update-kubeconfig sí soporta --alias directo; gcloud no).
// ─────────────────────────────────────────────────────────────────────────

pub(crate) fn gke_get_credentials(cluster_name: &str, location: &str, project_id: &str, account: &str) -> Result<(), RefreshError> {
    let output = StdCommand::new("gcloud")
        .args([
            "container",
            "clusters",
            "get-credentials",
            cluster_name,
            "--region",
            location,
            "--project",
            project_id,
            "--account",
            account,
        ])
        .output()
        .map_err(|e| RefreshError::new("gke_get_credentials", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(RefreshError::new("gke_get_credentials", stderr))
    }
}

// gcloud nombra el contexto automáticamente como gke_<project>_<location>_<cluster>.
// Lo renombramos al alias legible; si el alias ya existía de una corrida
// anterior, se elimina primero (si no, el rename falla). Si el rename
// falla por cualquier otro motivo, se conserva el nombre de gcloud tal
// cual, en vez de fallar todo el flujo.
pub(crate) fn rename_context_to_alias(project_id: &str, location: &str, cluster_name: &str, desired_alias: &str) -> String {
    let gcloud_context = format!("gke_{}_{}_{}", project_id, location, cluster_name);

    let exists = StdCommand::new("kubectl")
        .args(["config", "get-contexts", desired_alias])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if exists {
        let _ = StdCommand::new("kubectl").args(["config", "delete-context", desired_alias]).output();
    }

    let renamed = StdCommand::new("kubectl")
        .args(["config", "rename-context", &gcloud_context, desired_alias])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if renamed {
        desired_alias.to_string()
    } else {
        gcloud_context
    }
}

// Token OAuth de la cuenta activa — a diferencia de AWS, sirve para
// cualquier cluster GKE, no hay que pedirlo por-cluster.
pub(crate) fn get_gcp_token() -> Option<String> {
    let output = StdCommand::new("gcloud")
        .args(["auth", "print-access-token", "--format", "value(token)"])
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

// ─────────────────────────────────────────────────────────────────────────
// Comando expuesto a React
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SingleGkeTestResult {
    pub cluster_name: String,
    pub context_alias: String,
    pub total_clusters_encontrados: usize,
    pub access: ClusterAccessResult,
}

#[tauri::command]
pub async fn test_single_gke_cluster(
    app: AppHandle,
    project_id: String,
    project_name: String,
) -> Result<SingleGkeTestResult, RefreshError> {
    let accounts_cfg = load_or_create_accounts(&app)?;
    let account = accounts_cfg.gcp.chile.account.clone();

    set_active_project(&project_id, &account)?;

    let clusters = list_clusters_for_project(&project_id, &account)?;
    if clusters.is_empty() {
        return Err(RefreshError::new(
            "sin_clusters",
            format!("El proyecto '{}' no tiene clusters GKE.", project_name),
        ));
    }

    let chosen = clusters
        .iter()
        .find(|c| c.status == "RUNNING")
        .ok_or_else(|| {
            RefreshError::new(
                "sin_cluster_running",
                format!("Ninguno de los {} cluster(s) encontrados está RUNNING.", clusters.len()),
            )
        })?;

    let desired_alias = format!(
        "[Chile] [GCP] / {} / {}",
        project_id.to_lowercase(),
        chosen.name.to_lowercase()
    );

    gke_get_credentials(&chosen.name, &chosen.location, &project_id, &account)?;
    let context_alias = rename_context_to_alias(&project_id, &chosen.location, &chosen.name, &desired_alias);

    let workdir = std::env::temp_dir().join(format!(
        "gke-vca-{}-{}",
        chosen.name.replace('/', "_"),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let _ = fs::create_dir_all(&workdir);

    let server_ca = read_server_and_ca(&context_alias, &workdir);
    let server_token_ca = match (server_ca, get_gcp_token()) {
        (Some((server, ca_file)), Some(token)) => Some((server, token, ca_file)),
        _ => None,
    };
    let used_token_optimization = server_token_ca.is_some();
    let kargs = build_kargs(&server_token_ca, &context_alias);

    let access = run_permission_checks(kargs, used_token_optimization).await;
    let _ = fs::remove_dir_all(&workdir);

    Ok(SingleGkeTestResult {
        cluster_name: chosen.name.clone(),
        context_alias,
        total_clusters_encontrados: clusters.len(),
        access,
    })
}
