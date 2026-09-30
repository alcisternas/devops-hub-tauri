use serde::Serialize;
use std::process::Command as StdCommand;
use tauri::AppHandle;

use crate::modules::eks::config::{load_or_create_accounts, RefreshError};
use crate::modules::eks::permissions::update_kubeconfig;
use crate::modules::eks::sso::{ensure_chile_sso, ensure_peru_sso};
use crate::modules::gke::login::ensure_gcp_session;
use crate::modules::gke::test_cluster::{gke_get_credentials, rename_context_to_alias};

use super::discovery::CombinedClusterEntry;

// ─────────────────────────────────────────────────────────────────────────
// Helpers de kubectl — equivalentes a los bloques de load_clusters() del
// bash que listan/eliminan contextos.
// ─────────────────────────────────────────────────────────────────────────

fn list_existing_contexts() -> Vec<String> {
    let output = StdCommand::new("kubectl")
        .args(["config", "get-contexts", "--no-headers", "-o", "name"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn context_exists(alias: &str) -> bool {
    StdCommand::new("kubectl")
        .args(["config", "get-contexts", alias])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn delete_context(alias: &str) {
    let _ = StdCommand::new("kubectl").args(["config", "delete-context", alias]).output();
}

// ─────────────────────────────────────────────────────────────────────────
// Resultado expuesto a React
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LoadResultItem {
    pub cluster: String,
    pub context_alias: String,
    pub estado: String, // "cargado" | "omitido" | "error"
    pub detalle: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoadSummary {
    pub cargados: usize,
    pub omitidos: usize,
    pub con_errores: usize,
    pub items: Vec<LoadResultItem>,
    pub contextos_eliminados: usize,
}

#[tauri::command]
pub async fn load_selected_clusters(
    app: AppHandle,
    selections: Vec<CombinedClusterEntry>,
    mode: String,
) -> Result<LoadSummary, RefreshError> {
    let accounts_cfg = load_or_create_accounts(&app)?;

    // ── Modo reemplazar: limpiar todo antes de cargar ──────────────────────
    let mut contextos_eliminados = 0usize;
    if mode == "reemplazar" {
        let existing = list_existing_contexts();
        for ctx in &existing {
            delete_context(ctx);
        }
        contextos_eliminados = existing.len();
    }

    // ── Asegurar sesión — solo para los sistemas realmente presentes ───────
    // en la selección, no todos por defecto.
    let needs_chile = selections.iter().any(|s| s.proveedor == "aws" && s.pais == "chile");
    let needs_peru = selections.iter().any(|s| s.proveedor == "aws" && s.pais == "peru");
    let needs_gcp = selections.iter().any(|s| s.proveedor == "gcp");

    if needs_chile {
        ensure_chile_sso(&app).await?;
    }
    if needs_peru {
        ensure_peru_sso(&app).await?;
    }
    if needs_gcp {
        ensure_gcp_session(&accounts_cfg.gcp.chile.account)?;
    }

    // ── Cargar cada selección ───────────────────────────────────────────────
    let mut items = Vec::new();
    let mut cargados = 0usize;
    let mut omitidos = 0usize;
    let mut con_errores = 0usize;

    for e in &selections {
        // Modo agregar: no recargar lo que ya está — igual al bash
        // (recargar un contexto existente no renueva credenciales).
        if mode == "agregar" && context_exists(&e.context_alias) {
            items.push(LoadResultItem {
                cluster: e.cluster.clone(),
                context_alias: e.context_alias.clone(),
                estado: "omitido".to_string(),
                detalle: Some("ya está cargado".to_string()),
            });
            omitidos += 1;
            continue;
        }

        let result: Result<(), RefreshError> = if e.proveedor == "aws" {
            update_kubeconfig(&e.cluster, &e.region, &e.profile, &e.context_alias)
        } else {
            // GCP — get-credentials + rename al alias del inventario.
            // Si el rename falla, rename_context_to_alias cae al nombre
            // de gcloud (igual que el bash), y aun así se cuenta como
            // cargado — el cluster sí quedó accesible, solo con otro nombre.
            gke_get_credentials(&e.cluster, &e.region, &e.account_id, &accounts_cfg.gcp.chile.account).map(|_| {
                let _ = rename_context_to_alias(&e.account_id, &e.region, &e.cluster, &e.context_alias);
            })
        };

        match result {
            Ok(_) => {
                items.push(LoadResultItem {
                    cluster: e.cluster.clone(),
                    context_alias: e.context_alias.clone(),
                    estado: "cargado".to_string(),
                    detalle: None,
                });
                cargados += 1;
            }
            Err(err) => {
                items.push(LoadResultItem {
                    cluster: e.cluster.clone(),
                    context_alias: e.context_alias.clone(),
                    estado: "error".to_string(),
                    detalle: Some(err.mensaje),
                });
                con_errores += 1;
            }
        }
    }

    Ok(LoadSummary {
        cargados,
        omitidos,
        con_errores,
        items,
        contextos_eliminados,
    })
}
