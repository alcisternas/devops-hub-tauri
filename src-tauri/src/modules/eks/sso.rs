use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

use super::config::{load_or_create_accounts, RefreshError};

// Umbral mínimo de vigencia del token — igual a SSO_MIN_MINUTES en el bash.
// Si el token cacheado expira antes de este umbral, se fuerza login nuevo.
const SSO_MIN_MINUTES: i64 = 60;

// Equivalente a la variable global CONFIG_BACKED_UP del bash: ahí solo hacía
// falta durar mientras corría el script; para nosotros, la unidad de tiempo
// equivalente es "mientras la app está abierta". Se registra como estado
// gestionado de Tauri en lib.rs (.manage(...)) y así sobrevive entre clics
// de distintos botones (Login, Descubrir, Recorrido completo), evitando un
// respaldo nuevo por cada uno.
pub(crate) struct ConfigBackupState(pub(crate) Mutex<bool>);

// ─────────────────────────────────────────────────────────────────────────
// Rutas — usa el resolver de Tauri, no variables de entorno manuales,
// para que funcione igual en Windows/Mac/Linux.
// ─────────────────────────────────────────────────────────────────────────

pub(crate) fn aws_config_path(app: &AppHandle) -> Result<PathBuf, RefreshError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| RefreshError::new("resolver_home", e.to_string()))?;
    Ok(home.join(".aws").join("config"))
}

pub(crate) fn aws_sso_cache_dir(app: &AppHandle) -> Result<PathBuf, RefreshError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| RefreshError::new("resolver_home", e.to_string()))?;
    Ok(home.join(".aws").join("sso").join("cache"))
}

// ─────────────────────────────────────────────────────────────────────────
// Respaldo de ~/.aws/config — equivalente a backup_aws_config_once() del bash,
// ahora "una vez por sesión de la app" en vez de "una vez por ejecución del
// script". Si ya se respaldó en esta sesión, no hace nada — igual al bash
// cuando CONFIG_BACKED_UP ya era true.
// ─────────────────────────────────────────────────────────────────────────
fn backup_aws_config(app: &AppHandle, path: &PathBuf) -> Result<Option<PathBuf>, RefreshError> {
    let state = app.state::<ConfigBackupState>();

    {
        let already_done = state.0.lock().unwrap();
        if *already_done {
            return Ok(None);
        }
    }

    if !path.exists() {
        *state.0.lock().unwrap() = true;
        return Ok(None);
    }
    let metadata = fs::metadata(path)
        .map_err(|e| RefreshError::new("leer_metadata_config", e.to_string()))?;
    if metadata.len() == 0 {
        *state.0.lock().unwrap() = true;
        return Ok(None);
    }

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let backup_name = format!("config.{}", ts);
    let backup_path = path.with_file_name(backup_name);

    // Si la copia falla, NO se marca como hecho — a diferencia del bash
    // (que aborta el script entero y nunca vuelve a intentar), nosotros
    // preferimos permitir que un intento posterior lo vuelva a intentar,
    // ya que la app sigue abierta y no hay un "abortar todo" equivalente.
    fs::copy(path, &backup_path)
        .map_err(|e| RefreshError::new("respaldar_config", e.to_string()))?;

    *state.0.lock().unwrap() = true;
    Ok(Some(backup_path))
}

// ─────────────────────────────────────────────────────────────────────────
// Escritura del bloque [sso-session ...] — equivalente a write_config_block()
// con force=true, que es como setup_config_chile() siempre lo llama.
// Reutiliza write_ini_block() de config.rs (misma lógica que usará
// permissions.rs para escribir bloques [profile ...]).
// ─────────────────────────────────────────────────────────────────────────
fn write_sso_session_block(
    config_path: &PathBuf,
    sso_session: &str,
    portal_url: &str,
    sso_region: &str,
) -> Result<(), RefreshError> {
    let header = format!("[sso-session {}]", sso_session);
    let body = format!(
        "sso_start_url = {}\nsso_region = {}\nsso_registration_scopes = sso:account:access",
        portal_url, sso_region
    );
    super::config::write_ini_block(config_path, &header, &body, true)
}

// ─────────────────────────────────────────────────────────────────────────
// Login SSO — equivalente al bloque "aws sso login" de do_sso_login().
// Bloquea hasta que el usuario complete el login en el browser (o falle).
// ─────────────────────────────────────────────────────────────────────────
fn run_sso_login(sso_session: &str) -> Result<(), RefreshError> {
    let output = Command::new("aws")
        .args(["sso", "login", "--sso-session", sso_session])
        .output()
        .map_err(|e| RefreshError::new("ejecutar_aws_sso_login", e.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let mensaje = if !stderr.is_empty() { stderr } else { stdout };
        Err(RefreshError::new("aws_sso_login", mensaje))
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Búsqueda de token en caché — equivalente al bucle de do_sso_login() que
// filtra por startUrl e ignora archivos oidc-client.
// Retorna el token con expiración más lejana que coincida con el portal.
// ─────────────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
struct SsoCacheFile {
    #[serde(rename = "startUrl")]
    start_url: Option<String>,
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<String>,
}

pub(crate) fn find_cached_token(cache_dir: &PathBuf, portal_url: &str) -> Option<(String, DateTime<Utc>)> {
    let entries = fs::read_dir(cache_dir).ok()?;
    let mut best: Option<(String, DateTime<Utc>)> = None;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let is_oidc_client = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.contains("oidc-client"))
            .unwrap_or(false);
        if is_oidc_client {
            continue;
        }

        let raw = match fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let parsed: SsoCacheFile = match serde_json::from_str(&raw) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let (Some(url), Some(token), Some(expires_raw)) =
            (parsed.start_url, parsed.access_token, parsed.expires_at)
        else {
            continue;
        };

        if url != portal_url {
            continue;
        }

        let Ok(expiry) = DateTime::parse_from_rfc3339(&expires_raw) else {
            continue;
        };
        let expiry_utc = expiry.with_timezone(&Utc);

        let is_better = match &best {
            Some((_, best_expiry)) => expiry_utc > *best_expiry,
            None => true,
        };
        if is_better {
            best = Some((token, expiry_utc));
        }
    }

    best
}

// ─────────────────────────────────────────────────────────────────────────
// Validación real contra AWS — equivalente a la llamada
// "aws sso list-accounts --access-token ... --max-results 1" del bash.
// Un token puede estar vigente por fecha y aun así ser rechazado por AWS.
// ─────────────────────────────────────────────────────────────────────────
fn validate_token(token: &str, sso_region: &str) -> bool {
    let output = Command::new("aws")
        .args([
            "sso",
            "list-accounts",
            "--access-token",
            token,
            "--region",
            sso_region,
            "--max-results",
            "1",
            "--output",
            "json",
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            serde_json::from_str::<Value>(&stdout)
                .map(|v| v.get("accountList").is_some())
                .unwrap_or(false)
        }
        _ => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Comando expuesto a React
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SsoLoginResult {
    pub reused_cache: bool,
    pub expires_at: String,
    pub config_backup: Option<String>,
}

// Lógica de login compartida por Chile y Perú: busca token cacheado válido
// (evita repetir login), y si no hay, corre "aws sso login" y valida el
// resultado. La única diferencia entre países está en QUÉ se escribe en
// ~/.aws/config antes de llamar a esto — eso lo decide cada comando público.
async fn login_sso_session(
    app: &AppHandle,
    sso_session: &str,
    portal_url: &str,
    sso_region: &str,
    config_backup: Option<String>,
) -> Result<SsoLoginResult, RefreshError> {
    let cache_dir = aws_sso_cache_dir(app)?;

    // 1. ¿Hay un token cacheado todavía vigente y válido? Si sí, no repetir login.
    if let Some((token, expiry)) = find_cached_token(&cache_dir, portal_url) {
        let mins_left = (expiry - Utc::now()).num_minutes();
        if mins_left >= SSO_MIN_MINUTES && validate_token(&token, sso_region) {
            return Ok(SsoLoginResult {
                reused_cache: true,
                expires_at: expiry.to_rfc3339(),
                config_backup,
            });
        }
    }

    // 2. No hay token válido — hacer login (abre el browser, espera a que termine)
    run_sso_login(sso_session)?;

    // 3. Buscar el token recién generado y validarlo de verdad contra AWS
    match find_cached_token(&cache_dir, portal_url) {
        Some((token, expiry)) => {
            if !validate_token(&token, sso_region) {
                return Err(RefreshError::new(
                    "validar_token",
                    "El login se completó pero AWS rechazó el token al validarlo.",
                ));
            }
            Ok(SsoLoginResult {
                reused_cache: false,
                expires_at: expiry.to_rfc3339(),
                config_backup,
            })
        }
        None => Err(RefreshError::new(
            "buscar_token_cache",
            "El login se completó pero no se encontró el token en la caché SSO.",
        )),
    }
}

// Garantiza una sesión SSO Chile válida: escribe el bloque sso-session
// (idempotente) y asegura login (reutiliza caché si es válida, abre
// browser solo si de verdad hace falta). Antes esto vivía SOLO en
// setup_sso_chile — ahora también lo llaman discover_chile_accounts y
// run_full_chile_scan, para que el login deje de ser un botón manual
// obligatorio y sea, como en GCP, algo que se garantiza solo.
pub(crate) async fn ensure_chile_sso(app: &AppHandle) -> Result<SsoLoginResult, RefreshError> {
    let accounts = load_or_create_accounts(app)?;
    let config_path = aws_config_path(app)?;

    let backup = backup_aws_config(app, &config_path)?;
    write_sso_session_block(
        &config_path,
        &accounts.chile.sso_session,
        &accounts.chile.portal_url,
        &accounts.sso_region,
    )?;

    login_sso_session(
        app,
        &accounts.chile.sso_session,
        &accounts.chile.portal_url,
        &accounts.sso_region,
        backup.map(|p| p.to_string_lossy().to_string()),
    )
    .await
}

// Mismo concepto para Perú, incluyendo el profile base (devops-base) que
// Chile no necesita.
pub(crate) async fn ensure_peru_sso(app: &AppHandle) -> Result<SsoLoginResult, RefreshError> {
    let accounts = load_or_create_accounts(app)?;
    let config_path = aws_config_path(app)?;

    let backup = backup_aws_config(app, &config_path)?;
    write_sso_session_block(
        &config_path,
        &accounts.peru.sso_session,
        &accounts.peru.portal_url,
        &accounts.sso_region,
    )?;

    super::peru::write_peru_base_profile(
        &config_path,
        &accounts.peru.base_profile,
        &accounts.peru.sso_session,
        &accounts.peru.base_account_id,
        &accounts.peru.base_role,
        &accounts.eks_region,
    )?;

    login_sso_session(
        app,
        &accounts.peru.sso_session,
        &accounts.peru.portal_url,
        &accounts.sso_region,
        backup.map(|p| p.to_string_lossy().to_string()),
    )
    .await
}

// ─────────────────────────────────────────────────────────────────────────
// Comandos expuestos a React — ahora son wrappers delgados sobre las
// funciones ensure_*_sso, que también reutilizan discovery.rs y full_run.rs.
// ─────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn setup_sso_chile(app: AppHandle) -> Result<SsoLoginResult, RefreshError> {
    ensure_chile_sso(&app).await
}

#[tauri::command]
pub async fn setup_sso_peru(app: AppHandle) -> Result<SsoLoginResult, RefreshError> {
    ensure_peru_sso(&app).await
}
