use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use tauri::{AppHandle, Emitter};
use tokio::process::Command as AsyncCommand;

use super::config::{load_or_create_accounts, write_ini_block, RefreshError};
use super::sso::aws_config_path;

// ─────────────────────────────────────────────────────────────────────────
// Evento de progreso — esquema acordado para los tres módulos (eks, gke,
// load), no solo este comando de prueba. React lo escucha con
// listen("module_progress", ...).
// ─────────────────────────────────────────────────────────────────────────
#[derive(Debug, Serialize, Clone)]
pub struct ModuleProgress {
    pub modulo: String,
    pub etapa: String,
    pub item: String,
    pub progreso: u32,
    pub total: u32,
    pub estado: String, // "running" | "ok" | "error"
}

pub(crate) fn emit_progress(modulo: &str, app: &AppHandle, etapa: &str, item: &str, progreso: u32, total: u32, estado: &str) {
    let payload = ModuleProgress {
        modulo: modulo.to_string(),
        etapa: etapa.to_string(),
        item: item.to_string(),
        progreso,
        total,
        estado: estado.to_string(),
    };
    // Si emitir falla (ventana cerrada, etc.) no debe romper el flujo real.
    let _ = app.emit("module_progress", payload);
}

// ─────────────────────────────────────────────────────────────────────────
// Los 14 permisos a verificar — migrados EXACTOS del array PERMISSION_CHECKS
// de refresh-eks.sh. Nota: los comentarios del bash dicen "13 permisos" en
// varios lugares, pero el array real tiene 14 entradas — se porta el array
// real, no el número que dicen los comentarios.
// ─────────────────────────────────────────────────────────────────────────
const PERMISSION_CHECKS: [(&str, &str, &str); 14] = [
    ("get", "pods", "ver_pods"),
    ("list", "pods", "listar_pods"),
    ("delete", "pods", "eliminar_pods"),
    ("get", "pods/log", "ver_logs"),
    ("create", "pods/exec", "exec_pods"),
    ("get", "deployments", "ver_deployments"),
    ("patch", "deployments", "rollout_restart"),
    ("get", "services", "ver_services"),
    ("get", "secrets", "ver_secrets"),
    ("get", "configmaps", "ver_configmaps"),
    ("get", "nodes", "ver_nodos"),
    ("get", "events", "ver_eventos"),
    ("get", "namespaces", "ver_namespaces"),
    ("list", "namespaces", "listar_namespaces"),
];

// ─────────────────────────────────────────────────────────────────────────
// Escritura del perfil de cuenta — equivalente a add_profile_chile() del
// bash. Distinto de Perú (que siempre fuerza titan-pipeline vía
// devops-base, sin usar el rol real) — esto es SOLO para Chile.
// ─────────────────────────────────────────────────────────────────────────
pub(crate) fn write_chile_profile_block(
    config_path: &PathBuf,
    profile_name: &str,
    sso_session: &str,
    account_id: &str,
    role_name: &str,
    region: &str,
) -> Result<(), RefreshError> {
    let header = format!("[profile {}]", profile_name);
    let body = format!(
        "sso_session = {}\nsso_account_id = {}\nsso_role_name = {}\nregion = {}\noutput = json",
        sso_session, account_id, role_name, region
    );
    write_ini_block(config_path, &header, &body, true)
}

// ─────────────────────────────────────────────────────────────────────────
// Pasos previos a la verificación de permisos — equivalentes a las
// llamadas síncronas del bash (sts, list-clusters, describe-cluster,
// update-kubeconfig). Se quedan en std::process porque son pasos
// secuenciales únicos, no algo que valga la pena paralelizar.
// ─────────────────────────────────────────────────────────────────────────

pub(crate) fn check_sts_identity(profile_name: &str) -> bool {
    StdCommand::new("aws")
        .args(["sts", "get-caller-identity", "--profile", profile_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub(crate) fn list_eks_clusters(profile_name: &str, region: &str) -> Result<Vec<String>, RefreshError> {
    let output = StdCommand::new("aws")
        .args(["eks", "list-clusters", "--profile", profile_name, "--region", region, "--output", "json"])
        .output()
        .map_err(|e| RefreshError::new("listar_clusters_eks", e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RefreshError::new("listar_clusters_eks", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Value = serde_json::from_str(&stdout)
        .map_err(|e| RefreshError::new("parsear_clusters_eks", e.to_string()))?;

    let list = parsed.get("clusters").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    Ok(list.into_iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
}

pub(crate) fn describe_cluster_status(cluster_name: &str, region: &str, profile_name: &str) -> Result<String, RefreshError> {
    let output = StdCommand::new("aws")
        .args([
            "eks", "describe-cluster",
            "--name", cluster_name,
            "--region", region,
            "--profile", profile_name,
            "--query", "cluster.status",
            "--output", "text",
        ])
        .output()
        .map_err(|e| RefreshError::new("describir_cluster", e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RefreshError::new("describir_cluster", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn update_kubeconfig(cluster_name: &str, region: &str, profile_name: &str, alias: &str) -> Result<(), RefreshError> {
    let output = StdCommand::new("aws")
        .args([
            "eks", "update-kubeconfig",
            "--name", cluster_name,
            "--region", region,
            "--profile", profile_name,
            "--alias", alias,
        ])
        .output()
        .map_err(|e| RefreshError::new("update_kubeconfig", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(RefreshError::new("update_kubeconfig", stderr))
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Optimización de token — equivalente al bloque de verify_cluster_access()
// que obtiene server+CA+token UNA vez y los reutiliza en las 17 llamadas,
// en vez de que cada kubectl dispare el plugin de credenciales de aws.
// Si algo falla acá, se cae al modo compatible (--context) sin error fatal.
// ─────────────────────────────────────────────────────────────────────────
fn kubectl_jsonpath(jsonpath: &str, raw: bool) -> Option<String> {
    let mut args = vec!["config", "view"];
    if raw {
        args.push("--raw");
    }
    args.push("-o");
    args.push(jsonpath);

    let output = StdCommand::new("kubectl").args(&args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

// Parte que LEE el kubeconfig — cuando el Sub-paso 6 corra varios clusters
// en paralelo, esta es la única porción que debe quedar serializada con un
// mutex (junto con update_kubeconfig), porque ambas tocan el mismo archivo
// compartido ~/.kube/config. El resto (get-token, las 17 llamadas) no toca
// ese archivo y puede correr sin protección.
pub(crate) fn read_server_and_ca(context_alias: &str, workdir: &PathBuf) -> Option<(String, PathBuf)> {
    let cluster_ref_jsonpath = format!(
        "jsonpath={{.contexts[?(@.name=='{}')].context.cluster}}",
        context_alias
    );
    let cluster_ref = kubectl_jsonpath(&cluster_ref_jsonpath, false)?;

    let server_jsonpath = format!("jsonpath={{.clusters[?(@.name=='{}')].cluster.server}}", cluster_ref);
    let server = kubectl_jsonpath(&server_jsonpath, false)?;

    let ca_jsonpath = format!(
        "jsonpath={{.clusters[?(@.name=='{}')].cluster.certificate-authority-data}}",
        cluster_ref
    );
    let ca_data_b64 = kubectl_jsonpath(&ca_jsonpath, true)?;

    let decoded = general_purpose::STANDARD.decode(ca_data_b64.trim()).ok()?;
    let ca_file = workdir.join("ca.crt");
    fs::write(&ca_file, decoded).ok()?;

    Some((server, ca_file))
}

// Parte que SOLO pide un token — no toca ~/.kube/config, puede correr
// libremente en paralelo entre varios clusters.
pub(crate) fn get_eks_token(cluster_name: &str, region: &str, profile_name: &str) -> Option<String> {
    let token_output = StdCommand::new("aws")
        .args([
            "eks", "get-token",
            "--cluster-name", cluster_name,
            "--region", region,
            "--profile", profile_name,
            "--output", "json",
        ])
        .output()
        .ok()?;
    if !token_output.status.success() {
        return None;
    }
    let token_json: Value = serde_json::from_slice(&token_output.stdout).ok()?;
    token_json.get("status")?.get("token")?.as_str().map(|s| s.to_string())
}

// Combina ambas partes — usado tal cual por el test de un solo cluster
// (Sub-paso 4), donde no hay concurrencia y no hace falta separar el lock.
fn prepare_token_and_ca(
    context_alias: &str,
    cluster_name: &str,
    region: &str,
    profile_name: &str,
    workdir: &PathBuf,
) -> Option<(String, String, PathBuf)> {
    let (server, ca_file) = read_server_and_ca(context_alias, workdir)?;
    let token = get_eks_token(cluster_name, region, profile_name)?;
    Some((server, token, ca_file))
}

pub(crate) fn build_kargs(server_token_ca: &Option<(String, String, PathBuf)>, context_alias: &str) -> Vec<String> {
    match server_token_ca {
        Some((server, token, ca_path)) => vec![
            "--server".to_string(),
            server.clone(),
            "--token".to_string(),
            token.clone(),
            "--certificate-authority".to_string(),
            ca_path.to_string_lossy().to_string(),
        ],
        None => vec!["--context".to_string(), context_alias.to_string()],
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Las 17 llamadas en paralelo — 14 "kubectl auth can-i" + 3 "kubectl get"
// informativos (namespaces / pods / nodes). Cada una corre en su propia
// tarea de Tokio; tokio::spawn ya las deja corriendo en background antes
// de que awaitemos los resultados, así que sí son concurrentes de verdad.
// ─────────────────────────────────────────────────────────────────────────

fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).lines().next().unwrap_or("").trim().to_string()
}

async fn run_can_i(kargs: Vec<String>, verb: String, resource: String, col_name: String) -> (String, bool) {
    let mut args = vec!["auth".to_string(), "can-i".to_string(), verb, resource, "--all-namespaces".to_string()];
    args.extend(kargs);
    args.push("--request-timeout=15s".to_string());

    let ok = AsyncCommand::new("kubectl")
        .args(&args)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    (col_name, ok)
}

async fn run_get_ok(kargs: Vec<String>, resource: String) -> (bool, String) {
    let mut args = vec!["get".to_string(), resource];
    args.extend(kargs);
    args.push("--request-timeout=15s".to_string());

    match AsyncCommand::new("kubectl").args(&args).output().await {
        Ok(o) if o.status.success() => (true, String::new()),
        Ok(o) => (false, first_line(&o.stderr)),
        Err(e) => (false, e.to_string()),
    }
}

async fn run_get_count(kargs: Vec<String>, resource: String, extra_args: Vec<String>) -> (bool, usize, String) {
    let mut args = vec!["get".to_string(), resource];
    args.extend(extra_args);
    args.extend(kargs);
    args.push("--request-timeout=15s".to_string());

    match AsyncCommand::new("kubectl").args(&args).output().await {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let count = stdout.lines().filter(|l| !l.trim().is_empty()).count();
            (true, count, String::new())
        }
        Ok(o) => (false, 0, first_line(&o.stderr)),
        Err(e) => (false, 0, e.to_string()),
    }
}

#[derive(Debug, Serialize)]
pub struct ClusterAccessResult {
    pub permisos: HashMap<String, bool>,
    pub permisos_ok: usize,
    pub permisos_total: usize,
    pub veredicto: String, // "PERMISOS_OK" | "PERMISOS_INSUFICIENTES"
    pub cluster_healthy: bool,
    pub pods_count: Option<usize>,
    pub nodes_count: Option<usize>,
    pub pods_error: Option<String>,
    pub nodes_error: Option<String>,
    pub used_token_optimization: bool,
}

// Las 17 llamadas en paralelo, ya con kargs resuelto (server+token+ca, o
// --context como fallback). No toca ~/.kube/config para nada — por eso
// full_run.rs puede llamarla fuera de cualquier lock, incluso con varios
// clusters corriendo esto al mismo tiempo.
pub(crate) async fn run_permission_checks(kargs: Vec<String>, used_token_optimization: bool) -> ClusterAccessResult {
    // 14 can-i en paralelo
    let mut handles = Vec::new();
    for (verb, resource, col) in PERMISSION_CHECKS.iter() {
        handles.push(tokio::spawn(run_can_i(
            kargs.clone(),
            verb.to_string(),
            resource.to_string(),
            col.to_string(),
        )));
    }

    // 3 "get" informativos en paralelo, junto con los 14 anteriores
    let ns_handle = tokio::spawn(run_get_ok(kargs.clone(), "namespaces".to_string()));
    let pods_handle = tokio::spawn(run_get_count(
        kargs.clone(),
        "pods".to_string(),
        vec!["--all-namespaces".to_string(), "--no-headers".to_string()],
    ));
    let nodes_handle = tokio::spawn(run_get_count(kargs.clone(), "nodes".to_string(), vec!["--no-headers".to_string()]));

    let mut permisos: HashMap<String, bool> = HashMap::new();
    for h in handles {
        if let Ok((col, ok)) = h.await {
            permisos.insert(col, ok);
        }
    }

    let (_ns_ok, _ns_err) = ns_handle.await.unwrap_or((false, "error_join".to_string()));
    let (pods_ok, pods_count, pods_err) = pods_handle.await.unwrap_or((false, 0, "error_join".to_string()));
    let (nodes_ok, nodes_count, nodes_err) = nodes_handle.await.unwrap_or((false, 0, "error_join".to_string()));

    let permisos_ok = permisos.values().filter(|v| **v).count();
    let permisos_total = permisos.len();
    let all_ok = permisos_total > 0 && permisos_ok == permisos_total;
    let cluster_healthy = pods_ok && nodes_ok;

    ClusterAccessResult {
        permisos,
        permisos_ok,
        permisos_total,
        veredicto: if all_ok { "PERMISOS_OK".to_string() } else { "PERMISOS_INSUFICIENTES".to_string() },
        cluster_healthy,
        pods_count: if pods_ok { Some(pods_count) } else { None },
        nodes_count: if nodes_ok { Some(nodes_count) } else { None },
        pods_error: if pods_ok { None } else { Some(pods_err) },
        nodes_error: if nodes_ok { None } else { Some(nodes_err) },
        used_token_optimization,
    }
}

async fn verify_cluster_access(
    context_alias: &str,
    cluster_name: &str,
    region: &str,
    profile_name: &str,
) -> ClusterAccessResult {
    let workdir = std::env::temp_dir().join(format!(
        "eks-vca-{}-{}",
        cluster_name.replace('/', "_"),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let _ = fs::create_dir_all(&workdir);

    let server_token_ca = prepare_token_and_ca(context_alias, cluster_name, region, profile_name, &workdir);
    let used_token_optimization = server_token_ca.is_some();
    let kargs = build_kargs(&server_token_ca, context_alias);

    let result = run_permission_checks(kargs, used_token_optimization).await;

    let _ = fs::remove_dir_all(&workdir);

    result
}

// ─────────────────────────────────────────────────────────────────────────
// Comando expuesto a React — orquesta todo el flujo para UN account+role,
// eligiendo el primer cluster ACTIVE que encuentre. El recorrido de TODOS
// los roles/cuentas/clusters es el Sub-paso 6, no este.
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SingleClusterTestResult {
    pub cluster_name: String,
    pub context_alias: String,
    pub total_clusters_encontrados: usize,
    pub access: ClusterAccessResult,
}

// Formato de alias: [País] [AWS] / cuenta / rol / cluster — todo en minúsculas
// salvo el país. Extraído a función para que full_run.rs use exactamente
// la misma lógica, sin arriesgarse a que diverja.
pub(crate) fn build_context_alias(pais_cap: &str, account_name: &str, role: &str, cluster_name: &str) -> String {
    format!(
        "[{}] [AWS] / {} / {} / {}",
        pais_cap,
        account_name.to_lowercase(),
        role.to_lowercase(),
        cluster_name.to_lowercase()
    )
}

#[tauri::command]
pub async fn test_single_cluster_permissions(
    app: AppHandle,
    account_id: String,
    account_name: String,
    profile: String,
    role: String,
) -> Result<SingleClusterTestResult, RefreshError> {
    const TOTAL: u32 = 6;
    let accounts_cfg = load_or_create_accounts(&app)?;
    let config_path = aws_config_path(&app)?;

    emit_progress("eks", &app, "escribiendo_perfil", &profile, 1, TOTAL, "running");
    if let Err(e) = write_chile_profile_block(
        &config_path,
        &profile,
        &accounts_cfg.chile.sso_session,
        &account_id,
        &role,
        &accounts_cfg.eks_region,
    ) {
        emit_progress("eks", &app, "escribiendo_perfil", &profile, 1, TOTAL, "error");
        return Err(e);
    }

    emit_progress("eks", &app, "verificando_acceso_sts", &profile, 2, TOTAL, "running");
    if !check_sts_identity(&profile) {
        emit_progress("eks", &app, "verificando_acceso_sts", &profile, 2, TOTAL, "error");
        return Err(RefreshError::new(
            "sin_acceso_profile",
            format!("Sin acceso con el profile '{}' — revisa el rol o la sesión SSO.", profile),
        ));
    }

    emit_progress("eks", &app, "listando_clusters", &profile, 3, TOTAL, "running");
    let clusters = match list_eks_clusters(&profile, &accounts_cfg.eks_region) {
        Ok(c) => c,
        Err(e) => {
            emit_progress("eks", &app, "listando_clusters", &profile, 3, TOTAL, "error");
            return Err(e);
        }
    };
    if clusters.is_empty() {
        emit_progress("eks", &app, "listando_clusters", &profile, 3, TOTAL, "error");
        return Err(RefreshError::new(
            "sin_clusters",
            format!("El rol '{}' no tiene clusters EKS en la región {}.", role, accounts_cfg.eks_region),
        ));
    }

    emit_progress(
        "eks",
        &app,
        "buscando_cluster_activo",
        &format!("{} cluster(s) encontrados", clusters.len()),
        4,
        TOTAL,
        "running",
    );

    // Elegir el primer cluster ACTIVE
    let mut chosen: Option<String> = None;
    for c in &clusters {
        match describe_cluster_status(c, &accounts_cfg.eks_region, &profile) {
            Ok(status) if status == "ACTIVE" => {
                chosen = Some(c.clone());
                break;
            }
            _ => continue,
        }
    }

    let cluster_name = chosen.ok_or_else(|| {
        emit_progress("eks", &app, "buscando_cluster_activo", "sin ACTIVE", 4, TOTAL, "error");
        RefreshError::new(
            "sin_cluster_activo",
            format!("Ninguno de los {} cluster(s) encontrados está ACTIVE.", clusters.len()),
        )
    })?;

    let context_alias = build_context_alias("Chile", &account_name, &role, &cluster_name);

    emit_progress("eks", &app, "actualizando_kubeconfig", &cluster_name, 5, TOTAL, "running");
    if let Err(e) = update_kubeconfig(&cluster_name, &accounts_cfg.eks_region, &profile, &context_alias) {
        emit_progress("eks", &app, "actualizando_kubeconfig", &cluster_name, 5, TOTAL, "error");
        return Err(e);
    }

    emit_progress("eks", &app, "verificando_permisos", &cluster_name, 6, TOTAL, "running");
    let access = verify_cluster_access(&context_alias, &cluster_name, &accounts_cfg.eks_region, &profile).await;
    let estado_final = if access.veredicto == "PERMISOS_OK" { "ok" } else { "error" };
    emit_progress("eks", &app, "verificando_permisos", &cluster_name, 6, TOTAL, estado_final);

    Ok(SingleClusterTestResult {
        cluster_name,
        context_alias,
        total_clusters_encontrados: clusters.len(),
        access,
    })
}
