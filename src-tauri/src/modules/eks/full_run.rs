use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, Semaphore};

use super::config::{load_or_create_accounts, RefreshError};
use super::discovery::AccountRoleInfo;
use super::permissions::{
    build_context_alias, check_sts_identity, describe_cluster_status, emit_progress, get_eks_token,
    list_eks_clusters, read_server_and_ca, role_profile_name, run_permission_checks, update_kubeconfig,
    write_chile_profile_block,
};
use super::sso::aws_config_path;

// Cuántos clusters se verifican en paralelo como máximo — valor medido en
// una corrida anterior del bash (jobs=6 degradaba por contención de CPU;
// jobs=3 fue el punto óptimo). No está revalidado con este código Rust,
// así que puede ajustarse si en la práctica no es el número ideal acá.
const MAX_CONCURRENT_CLUSTERS: usize = 3;

// ─────────────────────────────────────────────────────────────────────────
// Tipos del inventario — mismo formato EXACTO que ya usa load-cluster.sh,
// para no romper la herramienta bash mientras dure la migración completa.
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InventoryClusterEntry {
    pub pais: String,
    pub account_id: String,
    pub account_name: String,
    pub role: String,
    pub profile: String,
    pub cluster: String,
    pub region: String,
    pub context_alias: String,
    pub permisos: HashMap<String, bool>,
}

#[derive(Debug, Serialize)]
struct InventoryFile {
    generated_at: String,
    eks_region: String,
    portal: String,
    total: usize,
    clusters: Vec<InventoryClusterEntry>,
}

// Solo para leer el "total" del inventario anterior — no nos importa el
// resto del contenido para la protección contra achicamiento.
#[derive(Debug, serde::Deserialize)]
struct InventoryFileHeader {
    #[serde(default)]
    total: usize,
}

// ─────────────────────────────────────────────────────────────────────────
// Resultado expuesto a React
// ─────────────────────────────────────────────────────────────────────────

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
    // Equivalente a START_EPOCH/END_EPOCH/ELAPSED del bash — el bash sí
    // medía esto y nunca lo trasladamos a Rust hasta ahora.
    pub duracion_segundos: u64,
    pub failures_csv_path: String,
    pub accesos_revocados: usize,
    pub revoked_csv_path: Option<String>,
}

// Un cluster ACTIVE ya localizado, listo para pasar a la fase paralela.
struct ClusterJob {
    account_id: String,
    account_name: String,
    profile: String,
    role: String,
    cluster_name: String,
    context_alias: String,
}

fn csv_quote(field: &str) -> String {
    format!("\"{}\"", field.replace('"', "\"\""))
}

fn csv_row(fields: &[String]) -> String {
    fields.iter().map(|f| csv_quote(f)).collect::<Vec<_>>().join(",")
}

#[tauri::command]
pub async fn run_full_chile_scan(app: AppHandle, accounts: Vec<AccountRoleInfo>) -> Result<FullRunResult, RefreshError> {
    let started_at = std::time::Instant::now();

    // NOTA: ya no se llama a ensure_chile_sso() acá — discover_chile_accounts
    // siempre corre justo antes en el único flujo que existe hoy (el botón
    // "Descubrir clusters" encadena descubrir → recorrer), y ese ya
    // garantiza la sesión. Llamarlo de nuevo acá era 100% redundante
    // (repetía una validación completa contra AWS milisegundos después) y
    // posible causa de un segundo login. Si en el futuro este comando se
    // vuelve a invocar de forma independiente, hay que restaurar esto.
    let accounts_cfg = load_or_create_accounts(&app)?;
    let config_path = aws_config_path(&app)?;
    let eks_region = accounts_cfg.eks_region.clone();
    let sso_session = accounts_cfg.chile.sso_session.clone();

    // ── FASE A (secuencial) ───────────────────────────────────────────────
    // Escribir perfil + sts + listar clusters + elegir ACTIVE, cuenta por
    // cuenta, rol por rol. Secuencial a propósito: todos escriben en el
    // mismo ~/.aws/config, y turnarse evita cualquier condición de carrera
    // en esa fase (que es rápida de por sí — no hay 17 llamadas acá).
    let total_intentos: u32 = accounts.iter().map(|a| a.roles.len() as u32).sum();
    let mut intento_idx: u32 = 0;
    let mut omitidos: Vec<String> = Vec::new();
    let mut jobs: Vec<ClusterJob> = Vec::new();

    for account in &accounts {
        for role in &account.roles {
            intento_idx += 1;
            let role_profile = role_profile_name(&account.profile, role);
            let item_label = format!("{} / {}", account.profile, role);

            emit_progress("eks-chile", &app, "escribiendo_perfil", &item_label, intento_idx, total_intentos, "running");
            if let Err(e) = write_chile_profile_block(
                &config_path,
                &role_profile,
                &sso_session,
                &account.account_id,
                role,
                &eks_region,
            ) {
                let msg = format!("{} — error escribiendo perfil: {}", item_label, e.mensaje);
                omitidos.push(msg.clone());
                emit_progress("eks-chile", &app, "escribiendo_perfil", &msg, intento_idx, total_intentos, "error");
                continue;
            }

            if !check_sts_identity(&role_profile) {
                let msg = format!("{} — sin acceso con el profile", item_label);
                omitidos.push(msg.clone());
                emit_progress("eks-chile", &app, "verificando_acceso_sts", &msg, intento_idx, total_intentos, "error");
                continue;
            }

            let clusters = match list_eks_clusters(&role_profile, &eks_region) {
                Ok(c) => c,
                Err(e) => {
                    let msg = format!("{} — error listando clusters: {}", item_label, e.mensaje);
                    omitidos.push(msg.clone());
                    emit_progress("eks-chile", &app, "listando_clusters", &msg, intento_idx, total_intentos, "error");
                    continue;
                }
            };

            if clusters.is_empty() {
                let msg = format!("{} — sin clusters EKS en la región {}", item_label, eks_region);
                omitidos.push(msg.clone());
                emit_progress("eks-chile", &app, "listando_clusters", &msg, intento_idx, total_intentos, "error");
                continue;
            }

            emit_progress("eks-chile", &app, "listando_clusters", &item_label, intento_idx, total_intentos, "ok");

            for cluster_name in &clusters {
                match describe_cluster_status(cluster_name, &eks_region, &role_profile) {
                    Ok(status) if status == "ACTIVE" => {
                        let context_alias = build_context_alias("Chile", &account.account_name, role, cluster_name);
                        jobs.push(ClusterJob {
                            account_id: account.account_id.clone(),
                            account_name: account.account_name.clone(),
                            profile: role_profile.clone(),
                            role: role.clone(),
                            cluster_name: cluster_name.clone(),
                            context_alias,
                        });
                    }
                    // No ACTIVE (o error al describir) — se omite en silencio,
                    // igual que el "continue" del bash para este caso puntual.
                    _ => continue,
                }
            }
        }
    }

    // ── FASE B (paralelo acotado, jobs=3) ─────────────────────────────────
    // Solo la escritura/lectura de ~/.kube/config queda serializada con un
    // mutex — las 17 llamadas de verificación por cluster corren libres,
    // así que hasta 3 clusters pueden estar en su fase de 17-en-paralelo
    // al mismo tiempo (hasta 51 procesos kubectl concurrentes en el pico).
    let total_jobs = jobs.len() as u32;
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CLUSTERS));
    let kube_mutex = Arc::new(Mutex::new(()));
    let completed = Arc::new(AtomicU32::new(0));
    // Contador separado y también atómico para el evento "running" — antes
    // ese evento usaba completed.load()+1 (una simple LECTURA, sin
    // reservar nada), así que si 2-3 clusters arrancaban casi al mismo
    // tiempo, todos leían el mismo valor y mostraban el mismo número.
    // Con su propio fetch_add, cada arranque obtiene un número único,
    // igual que ya pasaba con los que terminan.
    let started = Arc::new(AtomicU32::new(0));

    let mut handles = Vec::new();

    for job in jobs {
        let semaphore = semaphore.clone();
        let kube_mutex = kube_mutex.clone();
        let completed = completed.clone();
        let started = started.clone();
        // AppHandle es barato de clonar (patrón estándar de Tauri para
        // moverlo dentro de tareas async); la región es un string corto.
        let app_h = app.clone();
        let region = eks_region.clone();

        handles.push(tokio::spawn(async move {
            let _permit = semaphore
                .acquire_owned()
                .await
                .expect("el semáforo de concurrencia no debería cerrarse nunca en este flujo");

            let item_label = format!("{} / {}", job.account_name, job.cluster_name);

            let workdir = std::env::temp_dir().join(format!(
                "eks-scan-{}-{}",
                job.cluster_name.replace('/', "_"),
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ));

            // Serializado: solo esto toca ~/.kube/config (update_kubeconfig
            // escribe, read_server_and_ca lee) — el resto corre libre.
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

            // Sin lock: obtener token y correr las 17 llamadas
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
            emit_progress("eks-chile", &app_h, "verificando_permisos", &item_label, n_started, total_jobs, "running");

            let access = run_permission_checks(kargs, used_token_optimization).await;
            let _ = fs::remove_dir_all(&workdir);

            let n = completed.fetch_add(1, Ordering::SeqCst) + 1;
            let estado_final = if access.veredicto == "PERMISOS_OK" { "ok" } else { "error" };
            emit_progress("eks-chile", &app_h, "verificando_permisos", &item_label, n, total_jobs, estado_final);

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
                    pais: "chile".to_string(),
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
                emit_progress("eks-chile", &app, "verificando_permisos", &msg, n, total_jobs, "error");
                omitidos.push(msg);
            }
        }
    }

    // ── Guardar inventario JSON ────────────────────────────────────────────
    let home = app
        .path()
        .home_dir()
        .map_err(|e| RefreshError::new("resolver_home", e.to_string()))?;
    let inventory_path = home.join(".aws").join("eks-inventory-chile.json");

    // Leer el inventario anterior COMPLETO antes de que save_inventory_for
    // lo sobreescriba — es la única oportunidad de tenerlo.
    let previous_entries = read_previous_entries(&inventory_path);

    let shrink_warning = save_inventory_for(&inventory_path, "chile", &eks_region, &entries)?;

    // ── Generar CSV (todas las filas probadas, no solo las exitosas) ──────
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let csv_path = home.join(".aws").join(format!("eks-clusters-report-{}.csv", ts));
    write_csv(&csv_path, &entries)?;

    // ── CSV de fallos — separado por país, no combinado ────────────────────
    let failures_csv_path = home.join(".aws").join(format!("eks-access-failures-chile-{}.csv", ts));
    write_eks_failures_csv(&failures_csv_path, &entries)?;

    // ── CSV de accesos revocados — solo si hay algún caso ──────────────────
    let revoked = detect_revoked_access(&previous_entries, &entries);
    let revoked_csv_path = if revoked.is_empty() {
        None
    } else {
        let path = home.join(".aws").join(format!("eks-access-revoked-chile-{}.csv", ts));
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

// ─────────────────────────────────────────────────────────────────────────
// Detección de accesos revocados — funcionalidad NUEVA, no existe en el
// bash original. Compara la corrida de hoy contra el inventario que
// existía en disco justo antes de sobreescribirlo. Solo detecta cambios
// entre dos corridas consecutivas — no hay histórico más allá de eso.
//
// Clave de comparación: cuenta + cluster + rol (completa, sin ignorar
// nada) — así detecta revocaciones específicas de un rol aunque otro rol
// en el mismo cluster siga funcionando (importante para Chile). Para
// Perú, donde el rol siempre es "titan-pipeline", esto se comporta igual
// que comparar solo cuenta+cluster porque ahí nunca hay otro rol.
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct InventoryFileFull {
    #[serde(default)]
    clusters: Vec<InventoryClusterEntry>,
}

// Debe llamarse ANTES de sobreescribir el archivo (save_inventory_for lo
// sobreescribe). Si el archivo no existe o no se puede parsear, retorna
// vacío — no hay nada contra qué comparar, no es un error.
pub(crate) fn read_previous_entries(path: &PathBuf) -> Vec<InventoryClusterEntry> {
    let raw = match fs::read_to_string(path) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str::<InventoryFileFull>(&raw)
        .map(|f| f.clusters)
        .unwrap_or_default()
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct RevokedAccessRow {
    pub pais: String,
    pub account_id: String,
    pub account_name: String,
    pub cluster: String,
    pub role: String,
    pub permisos_antes: String,
    pub permisos_ahora: String,
}

fn perm_ratio(e: &InventoryClusterEntry) -> (usize, usize) {
    let total = e.permisos.len();
    let ok = e.permisos.values().filter(|v| **v).count();
    (ok, total)
}

pub(crate) fn detect_revoked_access(
    old_entries: &[InventoryClusterEntry],
    new_entries: &[InventoryClusterEntry],
) -> Vec<RevokedAccessRow> {
    // Mapa (account_id, cluster, role) -> entrada, de la corrida ACTUAL.
    let new_map: HashMap<(String, String, String), &InventoryClusterEntry> = new_entries
        .iter()
        .map(|e| ((e.account_id.clone(), e.cluster.clone(), e.role.clone()), e))
        .collect();

    let mut rows = Vec::new();

    for old_e in old_entries {
        let (old_ok, old_total) = perm_ratio(old_e);
        let old_was_full = old_total > 0 && old_ok == old_total;
        if !old_was_full {
            continue; // solo interesa lo que ANTES tenía acceso completo
        }

        let key = (old_e.account_id.clone(), old_e.cluster.clone(), old_e.role.clone());
        // Solo se compara si el mismo cluster+rol se volvió a probar hoy —
        // si no aparece en la corrida actual, no sabemos si fue omitido
        // a propósito o si de verdad perdió acceso, así que no se reporta.
        if let Some(new_e) = new_map.get(&key) {
            let (new_ok, new_total) = perm_ratio(new_e);
            let new_is_full = new_total > 0 && new_ok == new_total;
            if !new_is_full {
                rows.push(RevokedAccessRow {
                    pais: old_e.pais.clone(),
                    account_id: old_e.account_id.clone(),
                    account_name: old_e.account_name.clone(),
                    cluster: old_e.cluster.clone(),
                    role: old_e.role.clone(),
                    permisos_antes: format!("{}/{}", old_ok, old_total),
                    permisos_ahora: format!("{}/{}", new_ok, new_total),
                });
            }
        }
    }

    rows
}

pub(crate) fn write_revoked_csv(path: &PathBuf, rows: &[RevokedAccessRow]) -> Result<(), RefreshError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RefreshError::new("crear_directorio_revoked", e.to_string()))?;
    }

    let mut out = String::new();
    out.push_str("pais,account_id,cuenta,cluster,rol,permisos_antes,permisos_ahora\n");
    for r in rows {
        let fields = vec![
            r.pais.clone(),
            r.account_id.clone(),
            r.account_name.clone(),
            r.cluster.clone(),
            r.role.clone(),
            r.permisos_antes.clone(),
            r.permisos_ahora.clone(),
        ];
        out.push_str(&csv_row(&fields));
        out.push('\n');
    }

    fs::write(path, out).map_err(|e| RefreshError::new("escribir_revoked_csv", e.to_string()))
}

pub(crate) fn save_inventory_for(
    path: &PathBuf,
    portal: &str,
    eks_region: &str,
    entries: &[InventoryClusterEntry],
) -> Result<Option<String>, RefreshError> {
    let new_count = entries.len();
    let mut shrink_warning = None;

    if path.exists() {
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(old) = serde_json::from_str::<InventoryFileHeader>(&raw) {
                if old.total > 0 && new_count < old.total {
                    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
                    // Derivado del nombre real del archivo (antes quedaba
                    // hardcodeado a "chile" sin importar cuál se respaldara).
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
        fs::create_dir_all(parent).map_err(|e| RefreshError::new("crear_directorio_inventario", e.to_string()))?;
    }

    let file = InventoryFile {
        generated_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        eks_region: eks_region.to_string(),
        portal: portal.to_string(),
        total: new_count,
        clusters: entries.to_vec(),
    };

    let json = serde_json::to_string_pretty(&file)
        .map_err(|e| RefreshError::new("serializar_inventario", e.to_string()))?;
    fs::write(path, json).map_err(|e| RefreshError::new("escribir_inventario", e.to_string()))?;

    Ok(shrink_warning)
}

fn write_csv(path: &PathBuf, entries: &[InventoryClusterEntry]) -> Result<(), RefreshError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RefreshError::new("crear_directorio_csv", e.to_string()))?;
    }

    // Mismas 14 columnas de permisos, mismo orden que PERMISSION_CHECKS.
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
    // El header va SIN comillas — igual que el bash, que lo escribe con
    // echo plano en vez de pasarlo por el mismo csv.writer que las filas.
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

// ─────────────────────────────────────────────────────────────────────────
// CSV de fallos — equivalente a generate_failures_csv() del bash. Antes se
// dejó pendiente (Sub-paso 6); ahora se construye para Chile y Perú por
// separado (no combinado), a pedido explícito.
//
// Agrupa por (pais, account_id, account_name, cluster) — un cluster puede
// tener varias entradas si se probaron varios roles (Chile). Solo se
// reporta si NINGÚN rol logró acceso completo en ese cluster. Como el
// campo "pais" ya viene en los datos, esta misma función sirve para
// ambos países sin distinguir — cada llamada recibe solo las entradas
// de UN país (Chile o Perú), así que nunca se mezclan en la salida.
// ─────────────────────────────────────────────────────────────────────────
pub(crate) fn write_eks_failures_csv(path: &PathBuf, entries: &[InventoryClusterEntry]) -> Result<(), RefreshError> {
    #[derive(Clone)]
    struct GroupKey {
        pais: String,
        account_id: String,
        account_name: String,
        cluster: String,
    }

    let mut groups: HashMap<(String, String, String, String), Vec<&InventoryClusterEntry>> = HashMap::new();
    for e in entries {
        let key = (e.pais.clone(), e.account_id.clone(), e.account_name.clone(), e.cluster.clone());
        groups.entry(key).or_default().push(e);
    }

    let mut rows: Vec<(GroupKey, usize, usize, String, String)> = Vec::new();

    for ((pais, account_id, account_name, cluster), group_entries) in &groups {
        let total_roles = group_entries.len();
        let mut roles_completos = 0usize;
        let mut algun_permiso = false;
        let mut detalles: Vec<String> = Vec::new();

        for e in group_entries {
            let total_p = e.permisos.len();
            let ok_p = e.permisos.values().filter(|v| **v).count();
            detalles.push(format!("{}:{}/{}", e.role, ok_p, total_p));
            if total_p > 0 && ok_p == total_p {
                roles_completos += 1;
            }
            if ok_p > 0 {
                algun_permiso = true;
            }
        }

        // Solo se reporta si NINGÚN rol logró permisos completos en este cluster
        if roles_completos > 0 {
            continue;
        }

        let estado = if algun_permiso { "PARCIAL" } else { "FAIL" };
        let pais_cap = {
            let mut c = pais.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => pais.clone(),
            }
        };

        rows.push((
            GroupKey {
                pais: pais_cap,
                account_id: account_id.clone(),
                account_name: account_name.clone(),
                cluster: cluster.clone(),
            },
            total_roles,
            roles_completos,
            estado.to_string(),
            detalles.join("; "),
        ));
    }

    // Orden determinista — el bash usa sorted(groups.keys())
    rows.sort_by(|a, b| {
        (&a.0.pais, &a.0.account_id, &a.0.account_name, &a.0.cluster).cmp(&(
            &b.0.pais,
            &b.0.account_id,
            &b.0.account_name,
            &b.0.cluster,
        ))
    });

    let mut out = String::new();
    out.push_str("pais,account_id,cuenta,cluster,roles_totales,roles_con_acceso_total,estado,detalle_roles\n");
    for (key, total_roles, roles_completos, estado, detalle) in rows {
        let fields = vec![
            key.pais,
            key.account_id,
            key.account_name,
            key.cluster,
            total_roles.to_string(),
            roles_completos.to_string(),
            estado,
            detalle,
        ];
        out.push_str(&csv_row(&fields));
        out.push('\n');
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| RefreshError::new("crear_directorio_failures", e.to_string()))?;
    }
    fs::write(path, out).map_err(|e| RefreshError::new("escribir_failures_csv", e.to_string()))
}
