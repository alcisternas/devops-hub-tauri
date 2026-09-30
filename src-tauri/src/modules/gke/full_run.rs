use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, Semaphore};

use crate::modules::eks::config::{load_or_create_accounts, RefreshError};
use crate::modules::eks::full_run::{
    detect_revoked_access, read_previous_entries, write_revoked_csv, InventoryClusterEntry,
};
use crate::modules::eks::permissions::{build_kargs, emit_progress, read_server_and_ca, run_permission_checks};

use super::discovery::GcpProject;
use super::test_cluster::{
    get_gcp_token, gke_get_credentials, list_clusters_for_project, rename_context_to_alias, set_active_project,
    GkeClusterInfo,
};

const MAX_CONCURRENT_CLUSTERS: usize = 3;

struct ClusterJob {
    project_id: String,
    project_name: String,
    cluster_name: String,
    location: String,
}

#[derive(Debug, Serialize)]
struct GkeInventoryFile {
    generated_at: String,
    provider: String,
    country: String,
    total: usize,
    clusters: Vec<InventoryClusterEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct GkeInventoryFileHeader {
    #[serde(default)]
    total: usize,
}

#[derive(Debug, Serialize)]
pub struct GkeFullRunResult {
    pub total_proyectos_intentados: usize,
    pub total_clusters_probados: usize,
    pub ok: usize,
    pub insuficientes: usize,
    pub omitidos: Vec<String>,
    pub inventory_path: String,
    pub csv_path: String,
    pub failures_csv_path: String,
    pub shrink_warning: Option<String>,
    pub duracion_segundos: u64,
    pub accesos_revocados: usize,
    pub revoked_csv_path: Option<String>,
}

fn csv_quote(field: &str) -> String {
    format!("\"{}\"", field.replace('"', "\"\""))
}

fn csv_row(fields: &[String]) -> String {
    fields.iter().map(|f| csv_quote(f)).collect::<Vec<_>>().join(",")
}

fn gcp_dir(app: &AppHandle) -> Result<PathBuf, RefreshError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| RefreshError::new("resolver_home", e.to_string()))?;
    Ok(home.join(".gcp"))
}

#[tauri::command]
pub async fn run_full_gke_scan(app: AppHandle, projects: Vec<GcpProject>) -> Result<GkeFullRunResult, RefreshError> {
    let started_at = std::time::Instant::now();

    let accounts_cfg = load_or_create_accounts(&app)?;
    let account = accounts_cfg.gcp.chile.account.clone();

    // ── FASE A (secuencial) — por proyecto, no por rol (GCP no tiene) ──────
    let total_proyectos = projects.len() as u32;
    let mut idx: u32 = 0;
    let mut omitidos: Vec<String> = Vec::new();
    let mut jobs: Vec<ClusterJob> = Vec::new();

    for project in &projects {
        idx += 1;
        let item_label = format!("{} ({})", project.name, project.project_id);

        emit_progress("gke", &app, "fijando_proyecto", &item_label, idx, total_proyectos, "running");
        if let Err(e) = set_active_project(&project.project_id, &account) {
            omitidos.push(format!("{} — sin acceso al proyecto: {}", item_label, e.mensaje));
            emit_progress("gke", &app, "fijando_proyecto", &item_label, idx, total_proyectos, "error");
            continue;
        }

        let clusters: Vec<GkeClusterInfo> = match list_clusters_for_project(&project.project_id, &account) {
            Ok(c) => c,
            Err(e) => {
                omitidos.push(format!("{} — error listando clusters: {}", item_label, e.mensaje));
                emit_progress("gke", &app, "listando_clusters", &item_label, idx, total_proyectos, "error");
                continue;
            }
        };

        if clusters.is_empty() {
            omitidos.push(format!("{} — sin clusters GKE", item_label));
            emit_progress("gke", &app, "listando_clusters", &item_label, idx, total_proyectos, "error");
            continue;
        }

        emit_progress("gke", &app, "listando_clusters", &item_label, idx, total_proyectos, "ok");

        for c in &clusters {
            if c.status != "RUNNING" {
                continue; // mismo criterio silencioso que el bash para clusters no RUNNING
            }
            jobs.push(ClusterJob {
                project_id: project.project_id.clone(),
                project_name: project.name.clone(),
                cluster_name: c.name.clone(),
                location: c.location.clone(),
            });
        }
    }

    // Token OAuth pedido UNA sola vez — optimización pedida explícitamente
    // para ahorrar tiempo frente a ~130+ proyectos. Si falla, cada job cae
    // solo a modo --context (build_kargs ya maneja ese fallback).
    let shared_token = get_gcp_token();

    // ── FASE B (paralelo acotado, jobs=3) ─────────────────────────────────
    // Igual que en EKS: solo get-credentials + rename + lectura de
    // server/CA quedan serializados bajo un Mutex, porque todo eso toca
    // el mismo ~/.kube/config compartido. Las 17 verificaciones por
    // cluster corren libres.
    let total_jobs = jobs.len() as u32;
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CLUSTERS));
    let kube_mutex = Arc::new(Mutex::new(()));
    let completed = Arc::new(AtomicU32::new(0));
    // Mismo fix que en EKS: contador atómico propio para el evento "running".
    let started = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::new();

    for job in jobs {
        let semaphore = semaphore.clone();
        let kube_mutex = kube_mutex.clone();
        let completed = completed.clone();
        let started = started.clone();
        let app_h = app.clone();
        let account_h = account.clone();
        let token_h = shared_token.clone();

        handles.push(tokio::spawn(async move {
            let _permit = semaphore
                .acquire_owned()
                .await
                .expect("el semáforo de concurrencia no debería cerrarse nunca en este flujo");

            let item_label = format!("{} / {}", job.project_name, job.cluster_name);

            let desired_alias = format!(
                "[Chile] [GCP] / {} / {}",
                job.project_id.to_lowercase(),
                job.cluster_name.to_lowercase()
            );

            let workdir = std::env::temp_dir().join(format!(
                "gke-scan-{}-{}",
                job.cluster_name.replace('/', "_"),
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ));

            let server_ca = {
                let _guard = kube_mutex.lock().await;
                if let Err(e) = gke_get_credentials(&job.cluster_name, &job.location, &job.project_id, &account_h) {
                    return Err((job, format!("error en get-credentials: {}", e.mensaje)));
                }
                let _ = fs::create_dir_all(&workdir);
                (
                    rename_context_to_alias(&job.project_id, &job.location, &job.cluster_name, &desired_alias),
                    read_server_and_ca(&desired_alias, &workdir),
                )
            };
            let (context_alias, server_ca_opt) = server_ca;
            // read_server_and_ca se llamó con desired_alias — si el rename
            // falló y quedó el nombre de gcloud, hay que reintentar la
            // lectura con el alias real para no perder la optimización.
            let server_ca_opt = if context_alias != desired_alias {
                let _guard = kube_mutex.lock().await;
                read_server_and_ca(&context_alias, &workdir)
            } else {
                server_ca_opt
            };

            let server_token_ca = match (server_ca_opt, &token_h) {
                (Some((server, ca_file)), Some(token)) => Some((server, token.clone(), ca_file)),
                _ => None,
            };
            let used_token_optimization = server_token_ca.is_some();
            let kargs = build_kargs(&server_token_ca, &context_alias);

            let n_started = started.fetch_add(1, Ordering::SeqCst) + 1;
            emit_progress("gke", &app_h, "verificando_permisos", &item_label, n_started, total_jobs, "running");

            let access = run_permission_checks(kargs, used_token_optimization).await;
            let _ = fs::remove_dir_all(&workdir);

            let n = completed.fetch_add(1, Ordering::SeqCst) + 1;
            let estado_final = if access.veredicto == "PERMISOS_OK" { "ok" } else { "error" };
            emit_progress("gke", &app_h, "verificando_permisos", &item_label, n, total_jobs, estado_final);

            Ok((job, context_alias, access))
        }));
    }

    let mut entries: Vec<InventoryClusterEntry> = Vec::new();
    let mut ok_count = 0usize;
    let mut insuficientes_count = 0usize;

    for h in handles {
        match h.await {
            Ok(Ok((job, context_alias, access))) => {
                if access.veredicto == "PERMISOS_OK" {
                    ok_count += 1;
                } else {
                    insuficientes_count += 1;
                }
                entries.push(InventoryClusterEntry {
                    pais: "chile".to_string(),
                    account_id: job.project_id.clone(),
                    account_name: job.project_name,
                    role: "gcp-iam".to_string(),
                    profile: job.project_id,
                    cluster: job.cluster_name,
                    region: job.location,
                    context_alias,
                    permisos: access.permisos,
                });
            }
            Ok(Err((job, msg))) => {
                omitidos.push(format!("{} / {} — {}", job.project_name, job.cluster_name, msg));
            }
            Err(e) => {
                omitidos.push(format!("tarea interna falló (join error): {}", e));
            }
        }
    }

    let dir = gcp_dir(&app)?;
    let inventory_path = dir.join("gke-inventory-chile.json");
    let previous_entries = read_previous_entries(&inventory_path);
    let shrink_warning = save_gke_inventory(&inventory_path, &entries)?;

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let csv_path = dir.join(format!("gke-clusters-report-{}.csv", ts));
    write_csv(&csv_path, &entries)?;

    let failures_csv_path = dir.join(format!("gke-access-failures-{}.csv", ts));
    write_failures_csv(&failures_csv_path, &entries)?;

    let revoked = detect_revoked_access(&previous_entries, &entries);
    let revoked_csv_path = if revoked.is_empty() {
        None
    } else {
        let path = dir.join(format!("gke-access-revoked-{}.csv", ts));
        write_revoked_csv(&path, &revoked)?;
        Some(path.to_string_lossy().to_string())
    };

    Ok(GkeFullRunResult {
        total_proyectos_intentados: total_proyectos as usize,
        total_clusters_probados: entries.len(),
        ok: ok_count,
        insuficientes: insuficientes_count,
        omitidos,
        inventory_path: inventory_path.to_string_lossy().to_string(),
        csv_path: csv_path.to_string_lossy().to_string(),
        failures_csv_path: failures_csv_path.to_string_lossy().to_string(),
        shrink_warning,
        duracion_segundos: started_at.elapsed().as_secs(),
        accesos_revocados: revoked.len(),
        revoked_csv_path,
    })
}

fn save_gke_inventory(path: &PathBuf, entries: &[InventoryClusterEntry]) -> Result<Option<String>, RefreshError> {
    let new_count = entries.len();
    let mut shrink_warning = None;

    if path.exists() {
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(old) = serde_json::from_str::<GkeInventoryFileHeader>(&raw) {
                if old.total > 0 && new_count < old.total {
                    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
                    let original_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    let backup_path = path.with_file_name(format!("{}.{}", original_name, ts));
                    let _ = fs::copy(path, &backup_path);
                    let drop_pct = 100.0 * (1.0 - (new_count as f64 / old.total as f64));
                    shrink_warning = Some(format!(
                        "El inventario nuevo tiene menos clusters que el anterior ({} → {}, -{:.0}%). Se respaldó el anterior en {}.",
                        old.total,
                        new_count,
                        drop_pct,
                        backup_path.to_string_lossy()
                    ));
                }
            }
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RefreshError::new("crear_directorio_gcp", e.to_string()))?;
    }

    let file = GkeInventoryFile {
        generated_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        provider: "gcp".to_string(),
        country: "chile".to_string(),
        total: new_count,
        clusters: entries.to_vec(),
    };

    let json = serde_json::to_string_pretty(&file)
        .map_err(|e| RefreshError::new("serializar_inventario_gke", e.to_string()))?;
    fs::write(path, json).map_err(|e| RefreshError::new("escribir_inventario_gke", e.to_string()))?;

    Ok(shrink_warning)
}

const PERM_COLS: [&str; 14] = [
    "ver_pods",
    "listar_pods",
    "eliminar_pods",
    "ver_logs",
    "exec_pods",
    "ver_deployments",
    "rollout_restart",
    "ver_services",
    "ver_secrets",
    "ver_configmaps",
    "ver_nodos",
    "ver_eventos",
    "ver_namespaces",
    "listar_namespaces",
];

fn write_csv(path: &PathBuf, entries: &[InventoryClusterEntry]) -> Result<(), RefreshError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RefreshError::new("crear_directorio_csv", e.to_string()))?;
    }

    let mut out = String::new();
    out.push_str("pais,account_id,cuenta,profile,rol,cluster,region");
    for c in PERM_COLS {
        out.push(',');
        out.push_str(c);
    }
    out.push('\n');

    for e in entries {
        let mut fields = vec![
            e.pais.clone(),
            e.account_id.clone(),
            e.account_name.clone(),
            e.profile.clone(),
            e.role.clone(),
            e.cluster.clone(),
            e.region.clone(),
        ];
        for c in PERM_COLS {
            let val = e.permisos.get(c).copied().unwrap_or(false);
            fields.push(if val { "si".to_string() } else { "no".to_string() });
        }
        out.push_str(&csv_row(&fields));
        out.push('\n');
    }

    fs::write(path, out).map_err(|e| RefreshError::new("escribir_csv", e.to_string()))
}

// Reporte separado — solo clusters SIN acceso completo, agrupados por
// (pais, proyecto, cluster). En GKE hay una sola entrada por cluster (no
// hay roles que agrupar), así que es más simple que el equivalente EKS.
fn write_failures_csv(path: &PathBuf, entries: &[InventoryClusterEntry]) -> Result<(), RefreshError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RefreshError::new("crear_directorio_failures", e.to_string()))?;
    }

    let mut out = String::new();
    out.push_str("pais,project_id,proyecto,cluster,region,estado,permisos\n");

    for e in entries {
        let total_p = e.permisos.len();
        let ok_p = e.permisos.values().filter(|v| **v).count();
        if total_p > 0 && ok_p == total_p {
            continue; // solo se reporta si NO tiene los permisos completos
        }
        let estado = if ok_p == 0 { "FAIL" } else { "PARCIAL" };
        let pais_cap = {
            let mut c = e.pais.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => e.pais.clone(),
            }
        };
        let fields = vec![
            pais_cap,
            e.account_id.clone(),
            e.account_name.clone(),
            e.cluster.clone(),
            e.region.clone(),
            estado.to_string(),
            format!("{}/{}", ok_p, total_p),
        ];
        out.push_str(&csv_row(&fields));
        out.push('\n');
    }

    fs::write(path, out).map_err(|e| RefreshError::new("escribir_failures_csv", e.to_string()))
}
