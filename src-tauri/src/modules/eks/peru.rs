use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, Semaphore};

use super::config::{load_or_create_accounts, write_ini_block, RefreshError};
use super::discovery::AccountRoleInfo;
use super::full_run::{
    detect_revoked_access, read_previous_entries, write_eks_failures_csv, write_revoked_csv, InventoryClusterEntry,
};
use super::permissions::{
    build_context_alias, check_sts_identity, describe_cluster_status, emit_progress, get_eks_token,
    list_eks_clusters, read_server_and_ca, run_permission_checks, update_kubeconfig,
};
use super::sso::{aws_config_path, aws_sso_cache_dir, find_cached_token};

const MAX_CONCURRENT_CLUSTERS: usize = 3;

// ─────────────────────────────────────────────────────────────────────────
// Escritura de perfiles — ver el detalle de por qué Perú NO usa force en
// la conversación del proyecto: el perfil titan-pipeline es idéntico sin
// importar qué rol de la SSO se esté probando, así que forzar reescribirlo
// sería trabajo inútil (y riesgo) sin ningún beneficio real.
// ─────────────────────────────────────────────────────────────────────────

// Perfil base (devops-base) — SÍ usa force, igual que setup_config_peru()
// del bash. Es el único punto de entrada SSO real; si cambia algo en
// accounts.json (el rol base, por ejemplo), debe reflejarse siempre.
// Nótese que, a diferencia del perfil de Chile, este NO lleva "output=json"
// — así está en el bash original, y no afecta a esta herramienta porque
// todos nuestros comandos ya piden --output json de forma explícita.
pub(crate) fn write_peru_base_profile(
    config_path: &PathBuf,
    base_profile: &str,
    sso_session: &str,
    base_account_id: &str,
    base_role: &str,
    region: &str,
) -> Result<(), RefreshError> {
    let header = format!("[profile {}]", base_profile);
    let body = format!(
        "sso_session = {}\nsso_account_id = {}\nsso_role_name = {}\nregion = {}",
        sso_session, base_account_id, base_role, region
    );
    write_ini_block(config_path, &header, &body, true)
}

// Perfil por cuenta — SIN force. role_session_name incluido, sin
// "output=json", tal como add_profile_peru() en el bash.
fn write_peru_account_profile(
    config_path: &PathBuf,
    profile_name: &str,
    account_id: &str,
    role_arn_name: &str,
    base_profile: &str,
    region: &str,
) -> Result<(), RefreshError> {
    let header = format!("[profile {}]", profile_name);
    let body = format!(
        "role_arn = arn:aws:iam::{}:role/{}\nsource_profile = {}\nrole_session_name = {}\nregion = {}",
        account_id, role_arn_name, base_profile, role_arn_name, region
    );
    write_ini_block(config_path, &header, &body, false)
}

// ─────────────────────────────────────────────────────────────────────────
// Descubrimiento — más simple que Chile: solo list-accounts, sin
// list-account-roles, porque el rol SSO no determina el acceso real.
// Cada cuenta mapeada se representa con un único "rol" sintético
// (role_arn_name, ej. "titan-pipeline") para reutilizar el mismo tipo
// AccountRoleInfo que ya usa la tabla de Chile en la UI.
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
        match mapping.get(&account_id) {
            Some(profile) => cuentas.push(AccountRoleInfo {
                account_id,
                account_name,
                profile: profile.clone(),
                // Rol sintético — el acceso real siempre es vía role_arn_name,
                // no vía este valor. Se guarda así para reusar el mismo tipo
                // que la tabla de Chile.
                roles: vec![accounts_cfg.peru.role_arn_name.clone()],
            }),
            None => omitidas.push(format!("{} ({}) — sin mapeo en accounts.json", account_id, account_name)),
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
// verificar permisos), pero UNA verificación por cuenta, no por rol —
// decisión tomada explícitamente para evitar probar 2-3 veces el mismo
// cluster con resultados idénticos.
// ─────────────────────────────────────────────────────────────────────────

struct ClusterJob {
    account_id: String,
    account_name: String,
    profile: String,
    cluster_name: String,
    context_alias: String,
}

#[derive(Debug, Serialize)]
pub struct FullRunResult {
    pub total_cuentas_intentadas: usize,
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

#[tauri::command]
pub async fn run_full_peru_scan(app: AppHandle, accounts: Vec<AccountRoleInfo>) -> Result<FullRunResult, RefreshError> {
    let started_at = std::time::Instant::now();

    // NOTA: igual que en Chile — ya no se llama a ensure_peru_sso() acá,
    // discover_peru_accounts siempre corre justo antes en el único flujo
    // actual y ya garantiza la sesión. Ver la nota equivalente en full_run.rs.
    let accounts_cfg = load_or_create_accounts(&app)?;
    let config_path = aws_config_path(&app)?;
    let eks_region = accounts_cfg.eks_region.clone();
    let role_arn_name = accounts_cfg.peru.role_arn_name.clone();
    let base_profile = accounts_cfg.peru.base_profile.clone();

    // ── FASE A (secuencial) — una por cuenta, no por rol ──────────────────
    let total_cuentas = accounts.len() as u32;
    let mut idx: u32 = 0;
    let mut omitidos: Vec<String> = Vec::new();
    let mut jobs: Vec<ClusterJob> = Vec::new();

    for account in &accounts {
        idx += 1;
        let item_label = format!("{} ({})", account.profile, account.account_name);

        emit_progress("eks-peru", &app, "escribiendo_perfil", &item_label, idx, total_cuentas, "running");
        if let Err(e) = write_peru_account_profile(
            &config_path,
            &account.profile,
            &account.account_id,
            &role_arn_name,
            &base_profile,
            &eks_region,
        ) {
            let msg = format!("{} — error escribiendo perfil: {}", item_label, e.mensaje);
            omitidos.push(msg.clone());
            emit_progress("eks-peru", &app, "escribiendo_perfil", &msg, idx, total_cuentas, "error");
            continue;
        }

        if !check_sts_identity(&account.profile) {
            let msg = format!("{} — sin acceso con el profile", item_label);
            omitidos.push(msg.clone());
            emit_progress("eks-peru", &app, "verificando_acceso_sts", &msg, idx, total_cuentas, "error");
            continue;
        }

        let clusters = match list_eks_clusters(&account.profile, &eks_region) {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("{} — error listando clusters: {}", item_label, e.mensaje);
                omitidos.push(msg.clone());
                emit_progress("eks-peru", &app, "listando_clusters", &msg, idx, total_cuentas, "error");
                continue;
            }
        };

        if clusters.is_empty() {
            let msg = format!("{} — sin clusters EKS en la región {}", item_label, eks_region);
            omitidos.push(msg.clone());
            emit_progress("eks-peru", &app, "listando_clusters", &msg, idx, total_cuentas, "error");
            continue;
        }

        emit_progress("eks-peru", &app, "listando_clusters", &item_label, idx, total_cuentas, "ok");

        for cluster_name in &clusters {
            match describe_cluster_status(cluster_name, &eks_region, &account.profile) {
                Ok(status) if status == "ACTIVE" => {
                    // pais_cap "Peru" — el mismo formato [País] [AWS] / ... que Chile
                    let context_alias = build_context_alias("Peru", &account.account_name, &role_arn_name, cluster_name);
                    jobs.push(ClusterJob {
                        account_id: account.account_id.clone(),
                        account_name: account.account_name.clone(),
                        profile: account.profile.clone(),
                        cluster_name: cluster_name.clone(),
                        context_alias,
                    });
                }
                _ => continue,
            }
        }
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
                    role: role_arn_name.clone(),
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
        total_cuentas_intentadas: total_cuentas as usize,
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
