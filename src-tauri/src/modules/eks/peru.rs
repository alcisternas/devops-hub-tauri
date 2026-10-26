use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, Semaphore};

use super::config::{load_or_create_accounts, RefreshError};
use super::discovery::{list_account_roles, AccountRoleInfo};
use super::full_run::{
    detect_revoked_access, read_previous_entries, write_eks_failures_csv, write_revoked_csv, InventoryClusterEntry,
};
use super::permissions::{
    build_context_alias, check_sts_identity, describe_cluster_status, emit_progress, get_eks_token,
    list_eks_clusters, read_server_and_ca, role_profile_name, run_permission_checks, update_kubeconfig,
    write_chile_profile_block,
};
use super::sso::{aws_config_path, aws_sso_cache_dir, find_cached_token};

const MAX_CONCURRENT_CLUSTERS: usize = 3;

// Intentos cuenta+rol de la fase A que corren a la vez. Medido con la CLI sobre 87
// combinaciones: secuencial 324 s, 3 a la vez 103 s, 6 a la vez 60 s.
const MAX_CONCURRENT_ACCOUNTS: usize = 6;

// ─────────────────────────────────────────────────────────────────────────
// Perfiles — desde que se retiró titan-pipeline, Perú entra igual que
// Chile: un perfil SSO por cuenta+rol (sso_role_name real), sin AssumeRole
// en cadena ni perfil base. Se reutilizan role_profile_name y
// write_chile_profile_block de permissions.rs (el bloque es el mismo, solo
// cambia la sso-session).
// ─────────────────────────────────────────────────────────────────────────

// Prefijo del perfil para una cuenta que NO está en accounts.json: se deriva
// del nombre de la cuenta ("RIPLEY BANCO  BI" → "ripley-banco-bi") para no
// dejar fuera cuentas nuevas del portal. role_profile_name le agrega luego
// "--<rol>", así que no choca con los perfiles viejos de titan.
fn profile_prefix_from_name(account_name: &str) -> String {
    let slug: String = account_name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    slug.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

// ─────────────────────────────────────────────────────────────────────────
// Descubrimiento — igual que Chile: list-accounts + list-account-roles.
// Los roles son los reales de la SSO. accounts.json solo aporta el prefijo
// del perfil; una cuenta fuera del mapeo no se omite, usa un prefijo
// derivado del nombre.
// ─────────────────────────────────────────────────────────────────────────

fn list_accounts(token: &str, region: &str) -> Result<Vec<(String, String)>, RefreshError> {
    let output = StdCommand::new("aws")
        .args(["sso", "list-accounts", "--access-token", token, "--region", region, "--output", "json"])
        .output()
        .map_err(|e| RefreshError::new("listar_cuentas", e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RefreshError::new("listar_cuentas", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value =
        serde_json::from_str(&stdout).map_err(|e| RefreshError::new("parsear_cuentas", e.to_string()))?;
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

#[derive(Debug, Serialize)]
pub struct PeruDiscoveryResult {
    pub total_cuentas_portal: usize,
    pub cuentas: Vec<AccountRoleInfo>,
    pub omitidas: Vec<String>,
    pub duracion_segundos: u64,
}

#[tauri::command]
pub async fn discover_peru_accounts(app: AppHandle) -> Result<PeruDiscoveryResult, RefreshError> {
    let started_at = std::time::Instant::now();
    emit_progress("eks-peru", &app, "descubriendo_cuentas", "Perú", 1, 1, "running");

    // Igual que en Chile — asegura la sesión en vez de exigir que ya
    // exista, para que el login deje de ser un paso manual obligatorio.
    if let Err(e) = super::sso::ensure_peru_sso(&app).await {
        emit_progress("eks-peru", &app, "descubriendo_cuentas", "Perú", 1, 1, "error");
        return Err(e);
    }

    let accounts_cfg = load_or_create_accounts(&app)?;
    let cache_dir = aws_sso_cache_dir(&app)?;

    let (token, _expiry) = match find_cached_token(&cache_dir, &accounts_cfg.peru.portal_url) {
        Some(t) => t,
        None => {
            emit_progress("eks-peru", &app, "descubriendo_cuentas", "Perú", 1, 1, "error");
            return Err(RefreshError::new(
                "sin_sesion",
                "La sesión se validó pero no se encontró el token en caché (inesperado).",
            ));
        }
    };

    let portal_accounts = match list_accounts(&token, &accounts_cfg.sso_region) {
        Ok(p) => p,
        Err(e) => {
            emit_progress("eks-peru", &app, "descubriendo_cuentas", "Perú", 1, 1, "error");
            return Err(e);
        }
    };
    let total_cuentas_portal = portal_accounts.len();

    let mapping: std::collections::HashMap<String, String> = accounts_cfg
        .peru
        .accounts
        .iter()
        .map(|a| (a.account_id.clone(), a.profile.clone()))
        .collect();

    let mut cuentas = Vec::new();
    let mut omitidas = Vec::new();

    for (account_id, account_name) in portal_accounts {
        // Mapeada en accounts.json → su prefijo; si no, uno derivado del nombre.
        let profile = mapping
            .get(&account_id)
            .cloned()
            .unwrap_or_else(|| profile_prefix_from_name(&account_name));

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

    emit_progress("eks-peru", &app, "descubriendo_cuentas", "Perú", 1, 1, "ok");

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
        emit_progress("eks-peru", &app, "cuenta_omitida", o, (i + 1) as u32, total_omitidas, "error");
    }

    Ok(PeruDiscoveryResult {
        total_cuentas_portal,
        cuentas,
        omitidas,
        duracion_segundos: started_at.elapsed().as_secs(),
    })
}

// ─────────────────────────────────────────────────────────────────────────
// Recorrido completo — misma arquitectura de 2 fases que Chile (secuencial
// para escribir perfiles/listar clusters, paralelo acotado jobs=3 para
// verificar permisos), y ahora también una verificación por cuenta+rol:
// cada rol de la SSO puede tener permisos distintos dentro del cluster
// (aws-view no puede borrar pods; aws-edit y aws-admin-e sí).
// ─────────────────────────────────────────────────────────────────────────

struct ClusterJob {
    account_id: String,
    account_name: String,
    profile: String,
    role: String,
    cluster_name: String,
    context_alias: String,
}

#[derive(Debug, Serialize)]
pub struct FullRunResult {
    pub total_intentos_rol: usize,
    pub total_clusters_probados: usize,
    pub ok: usize,
    pub insuficientes: usize,
    pub omitidos: Vec<String>,
    pub inventory_path: String,
    pub csv_path: String,
    pub shrink_warning: Option<String>,
    pub duracion_segundos: u64,
    pub failures_csv_path: String,
    pub accesos_revocados: usize,
    pub revoked_csv_path: Option<String>,
}

fn csv_quote(field: &str) -> String {
    format!("\"{}\"", field.replace('"', "\"\""))
}

fn csv_row(fields: &[String]) -> String {
    fields.iter().map(|f| csv_quote(f)).collect::<Vec<_>>().join(",")
}

// Un intento (cuenta+rol) de la fase A, con todo lo que necesita la tarea.
struct ScanAttempt {
    account_id: String,
    account_name: String,
    role: String,
    role_profile: String,
    item_label: String,
}

// Fase A1 de UN intento: sts, listar clusters y describir cada uno. Devuelve
// el motivo de omisión (si lo hubo) y los clusters ACTIVE encontrados. Emite
// un solo evento de progreso, al terminar, con un número único del contador.
fn scan_account_role(
    app: &AppHandle,
    att: &ScanAttempt,
    eks_region: &str,
    done: &AtomicU32,
    total: u32,
) -> (Option<String>, Vec<ClusterJob>) {
    let finish = |etapa: &str, item: &str, estado: &str| {
        let n = done.fetch_add(1, Ordering::SeqCst) + 1;
        emit_progress("eks-peru", app, etapa, item, n, total, estado);
    };

    if !check_sts_identity(&att.role_profile) {
        let msg = format!("{} — sin acceso con el profile", att.item_label);
        finish("verificando_acceso_sts", &msg, "error");
        return (Some(msg), Vec::new());
    }

    let clusters = match list_eks_clusters(&att.role_profile, eks_region) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("{} — error listando clusters: {}", att.item_label, e.mensaje);
            finish("listando_clusters", &msg, "error");
            return (Some(msg), Vec::new());
        }
    };

    if clusters.is_empty() {
        let msg = format!("{} — sin clusters EKS en la región {}", att.item_label, eks_region);
        finish("listando_clusters", &msg, "error");
        return (Some(msg), Vec::new());
    }

    let mut jobs = Vec::new();
    for cluster_name in &clusters {
        match describe_cluster_status(cluster_name, eks_region, &att.role_profile) {
            Ok(status) if status == "ACTIVE" => {
                // pais_cap "Peru" — el mismo formato [País] [AWS] / ... que Chile
                let context_alias = build_context_alias("Peru", &att.account_name, &att.role, cluster_name);
                jobs.push(ClusterJob {
                    account_id: att.account_id.clone(),
                    account_name: att.account_name.clone(),
                    profile: att.role_profile.clone(),
                    role: att.role.clone(),
                    cluster_name: cluster_name.clone(),
                    context_alias,
                });
            }
            // No ACTIVE (o error al describir) — se omite en silencio.
            _ => continue,
        }
    }

    finish("listando_clusters", &att.item_label, "ok");
    (None, jobs)
}

#[tauri::command]
pub async fn run_full_peru_scan(app: AppHandle, accounts: Vec<AccountRoleInfo>) -> Result<FullRunResult, RefreshError> {
    let started_at = std::time::Instant::now();

    // NOTA: igual que en Chile — ya no se llama a ensure_peru_sso() acá,
    // discover_peru_accounts siempre corre justo antes en el único flujo
    // actual y ya garantiza la sesión. Ver la nota equivalente en full_run.rs.
    let accounts_cfg = load_or_create_accounts(&app)?;
    let config_path = aws_config_path(&app)?;
    let eks_region = accounts_cfg.eks_region.clone();
    let sso_session = accounts_cfg.peru.sso_session.clone();

    // ── FASE A0 (secuencial) — escribir los perfiles ──────────────────────
    // Todos los perfiles van al mismo ~/.aws/config, así que se escriben de
    // a uno. Es rápido (solo archivo local); lo lento viene después.
    let total_intentos: u32 = accounts.iter().map(|a| a.roles.len() as u32).sum();
    let mut intento_idx: u32 = 0;
    // Una ranura por intento (cuenta+rol), en el orden original: así el
    // inventario y la lista de omitidos no dependen de quién termina primero.
    let mut slots: Vec<(Option<String>, Vec<ClusterJob>)> = Vec::with_capacity(total_intentos as usize);
    let mut pendientes: Vec<(usize, ScanAttempt)> = Vec::new();

    for account in &accounts {
        for role in &account.roles {
            intento_idx += 1;
            let slot_idx = slots.len();
            slots.push((None, Vec::new()));
            let role_profile = role_profile_name(&account.profile, role);
            let item_label = format!("{} / {}", account.profile, role);

            emit_progress("eks-peru", &app, "escribiendo_perfil", &item_label, intento_idx, total_intentos, "running");
            if let Err(e) = write_chile_profile_block(
                &config_path,
                &role_profile,
                &sso_session,
                &account.account_id,
                role,
                &eks_region,
            ) {
                let msg = format!("{} — error escribiendo perfil: {}", item_label, e.mensaje);
                slots[slot_idx].0 = Some(msg.clone());
                emit_progress("eks-peru", &app, "escribiendo_perfil", &msg, intento_idx, total_intentos, "error");
                continue;
            }

            pendientes.push((
                slot_idx,
                ScanAttempt {
                    account_id: account.account_id.clone(),
                    account_name: account.account_name.clone(),
                    role: role.clone(),
                    role_profile,
                    item_label,
                },
            ));
        }
    }

    // ── FASE A1 (paralelo acotado) — sts + listar + describir ─────────────
    // Cada intento ejecuta un solo proceso aws a la vez, así que la carga es
    // baja; las llamadas son bloqueantes (std::process), por eso van en
    // spawn_blocking. Cada tarea escribe solo en su ranura.
    let total_a1 = pendientes.len() as u32;
    let sem_a = Arc::new(Semaphore::new(MAX_CONCURRENT_ACCOUNTS));
    let done_a = Arc::new(AtomicU32::new(0));
    let mut handles_a = Vec::new();

    for (slot_idx, att) in pendientes {
        let sem_a = sem_a.clone();
        let done_a = done_a.clone();
        let app_h = app.clone();
        let region = eks_region.clone();

        handles_a.push((
            slot_idx,
            tokio::spawn(async move {
                let _permit = sem_a
                    .acquire_owned()
                    .await
                    .expect("el semáforo de concurrencia no debería cerrarse nunca en este flujo");
                tokio::task::spawn_blocking(move || scan_account_role(&app_h, &att, &region, &done_a, total_a1)).await
            }),
        ));
    }

    for (slot_idx, h) in handles_a {
        match h.await {
            Ok(Ok(result)) => slots[slot_idx] = result,
            Ok(Err(e)) | Err(e) => {
                slots[slot_idx].0 = Some(format!("tarea interna falló (join error): {}", e));
            }
        }
    }

    let mut omitidos: Vec<String> = Vec::new();
    let mut jobs: Vec<ClusterJob> = Vec::new();
    for (omitido, mut job_list) in slots {
        if let Some(msg) = omitido {
            omitidos.push(msg);
        }
        jobs.append(&mut job_list);
    }

    // ── FASE B (paralelo acotado, jobs=3) — idéntica a la de Chile ────────
    let total_jobs = jobs.len() as u32;
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CLUSTERS));
    let kube_mutex = Arc::new(Mutex::new(()));
    let completed = Arc::new(AtomicU32::new(0));
    // Mismo fix que en full_run.rs (Chile): contador atómico propio para
    // el evento "running", en vez de reutilizar completed.load() sin reservar.
    let started = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::new();

    for job in jobs {
        let semaphore = semaphore.clone();
        let kube_mutex = kube_mutex.clone();
        let completed = completed.clone();
        let started = started.clone();
        let app_h = app.clone();
        let region = eks_region.clone();

        handles.push(tokio::spawn(async move {
            let _permit = semaphore
                .acquire_owned()
                .await
                .expect("el semáforo de concurrencia no debería cerrarse nunca en este flujo");

            let item_label = format!("{} / {}", job.account_name, job.cluster_name);

            let workdir = std::env::temp_dir().join(format!(
                "eks-scan-pe-{}-{}",
                job.cluster_name.replace('/', "_"),
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ));

            let server_ca = {
                let _guard = kube_mutex.lock().await;
                if let Err(e) = update_kubeconfig(&job.cluster_name, &region, &job.profile, &job.context_alias) {
                    let detalle = format!("error actualizando kubeconfig: {}", e.mensaje);
                    let n = started.fetch_add(1, Ordering::SeqCst) + 1;
                    emit_progress(
                        "eks",
                        &app_h,
                        "verificando_permisos",
                        &format!("{} — {}", item_label, detalle),
                        n,
                        total_jobs,
                        "error",
                    );
                    return Err((job, detalle));
                }
                let _ = fs::create_dir_all(&workdir);
                read_server_and_ca(&job.context_alias, &workdir)
            };

            let server_token_ca = match server_ca {
                Some((server, ca_file)) => {
                    get_eks_token(&job.cluster_name, &region, &job.profile).map(|token| (server, token, ca_file))
                }
                None => None,
            };
            let used_token_optimization = server_token_ca.is_some();
            let kargs = match &server_token_ca {
                Some((server, token, ca_path)) => vec![
                    "--server".to_string(),
                    server.clone(),
                    "--token".to_string(),
                    token.clone(),
                    "--certificate-authority".to_string(),
                    ca_path.to_string_lossy().to_string(),
                ],
                None => vec!["--context".to_string(), job.context_alias.clone()],
            };

            let n_started = started.fetch_add(1, Ordering::SeqCst) + 1;
            emit_progress("eks-peru", &app_h, "verificando_permisos", &item_label, n_started, total_jobs, "running");

            let access = run_permission_checks(kargs, used_token_optimization).await;
            let _ = fs::remove_dir_all(&workdir);

            let n = completed.fetch_add(1, Ordering::SeqCst) + 1;
            let estado_final = if access.veredicto == "PERMISOS_OK" { "ok" } else { "error" };
            emit_progress("eks-peru", &app_h, "verificando_permisos", &item_label, n, total_jobs, estado_final);

            Ok((job, access))
        }));
    }

    let mut entries: Vec<InventoryClusterEntry> = Vec::new();
    let mut ok_count = 0usize;
    let mut insuficientes_count = 0usize;

    for h in handles {
        match h.await {
            Ok(Ok((job, access))) => {
                if access.veredicto == "PERMISOS_OK" {
                    ok_count += 1;
                } else {
                    insuficientes_count += 1;
                }
                entries.push(InventoryClusterEntry {
                    pais: "peru".to_string(),
                    account_id: job.account_id,
                    account_name: job.account_name,
                    role: job.role,
                    profile: job.profile,
                    cluster: job.cluster_name,
                    region: eks_region.clone(),
                    context_alias: job.context_alias,
                    permisos: access.permisos,
                });
            }
            Ok(Err((job, msg))) => {
                omitidos.push(format!("{} / {} — {}", job.account_name, job.cluster_name, msg));
            }
            Err(e) => {
                let msg = format!("tarea interna falló (join error): {}", e);
                let n = completed.fetch_add(1, Ordering::SeqCst) + 1;
                emit_progress("eks-peru", &app, "verificando_permisos", &msg, n, total_jobs, "error");
                omitidos.push(msg);
            }
        }
    }

    let home = app
        .path()
        .home_dir()
        .map_err(|e| RefreshError::new("resolver_home", e.to_string()))?;
    let inventory_path = home.join(".aws").join("eks-inventory-peru.json");
    let previous_entries = read_previous_entries(&inventory_path);
    let shrink_warning = super::full_run::save_inventory_for(&inventory_path, "peru", &eks_region, &entries)?;

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let csv_path = home.join(".aws").join(format!("eks-clusters-report-peru-{}.csv", ts));
    write_csv(&csv_path, &entries)?;

    let failures_csv_path = home.join(".aws").join(format!("eks-access-failures-peru-{}.csv", ts));
    write_eks_failures_csv(&failures_csv_path, &entries)?;

    let revoked = detect_revoked_access(&previous_entries, &entries);
    let revoked_csv_path = if revoked.is_empty() {
        None
    } else {
        let path = home.join(".aws").join(format!("eks-access-revoked-peru-{}.csv", ts));
        write_revoked_csv(&path, &revoked)?;
        Some(path.to_string_lossy().to_string())
    };

    Ok(FullRunResult {
        total_intentos_rol: total_intentos as usize,
        total_clusters_probados: entries.len(),
        ok: ok_count,
        insuficientes: insuficientes_count,
        omitidos,
        inventory_path: inventory_path.to_string_lossy().to_string(),
        csv_path: csv_path.to_string_lossy().to_string(),
        shrink_warning,
        duracion_segundos: started_at.elapsed().as_secs(),
        failures_csv_path: failures_csv_path.to_string_lossy().to_string(),
        accesos_revocados: revoked.len(),
        revoked_csv_path,
    })
}

fn write_csv(path: &PathBuf, entries: &[InventoryClusterEntry]) -> Result<(), RefreshError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RefreshError::new("crear_directorio_csv", e.to_string()))?;
    }

    let perm_cols = [
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

    let mut out = String::new();
    out.push_str("pais,account_id,cuenta,profile,rol,cluster,region");
    for c in perm_cols {
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
        for c in perm_cols {
            let val = e.permisos.get(c).copied().unwrap_or(false);
            fields.push(if val { "si".to_string() } else { "no".to_string() });
        }
        out.push_str(&csv_row(&fields));
        out.push('\n');
    }

    fs::write(path, out).map_err(|e| RefreshError::new("escribir_csv", e.to_string()))
}
