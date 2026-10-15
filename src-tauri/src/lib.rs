use std::process::Command;

mod modules;

// Las apps lanzadas desde Finder/GUI en macOS y Linux no heredan el PATH del
// shell del usuario — solo reciben el PATH mínimo del sistema. Esta función
// prepende las rutas comunes de Homebrew e instaladores oficiales para que
// aws, gcloud y kubectl sean encontrables igual que en modo dev.
#[cfg(not(target_os = "windows"))]
fn ampliar_path() {
    let actual = std::env::var("PATH").unwrap_or_default();
    let mut dirs: Vec<String> = actual.split(':').map(str::to_string).collect();

    if let Ok(home) = std::env::var("HOME") {
        let local_bin = format!("{}/.local/bin", home);
        if !dirs.contains(&local_bin) {
            dirs.insert(0, local_bin);
        }
    }

    for p in [
        "/opt/homebrew/sbin",
        "/opt/homebrew/bin",
        "/usr/local/sbin",
        "/usr/local/bin",
    ] {
        let s = p.to_string();
        if !dirs.contains(&s) {
            dirs.insert(0, s);
        }
    }

    std::env::set_var("PATH", dirs.join(":"));
}

#[cfg(target_os = "windows")]
fn ampliar_path() {}
use modules::bitbucket;
use modules::cicd;
use modules::eks;
use modules::gke;
use modules::load;
use modules::sonar;
use modules::system;

// Comando genérico para ejecutar programas del sistema operativo (aws, gcloud, kubectl, etc.)
// Este patrón viene validado del prototipo de clusters: cualquier módulo que necesite
// correr un comando del SO usa esta misma función desde React con invoke("run_command", {...}).
#[tauri::command]
fn run_command(program: String, args: Vec<String>) -> Result<String, String> {
    let output = Command::new(&program)
        .args(&args)
        .output()
        .map_err(|e| format!("No se pudo ejecutar '{}': {}", program, e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    ampliar_path();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(eks::sso::ConfigBackupState(std::sync::Mutex::new(false)))
        .invoke_handler(tauri::generate_handler![
            run_command,
            eks::get_eks_accounts,
            eks::sso::setup_sso_chile,
            eks::sso::setup_sso_peru,
            eks::discovery::discover_chile_accounts,
            eks::permissions::test_single_cluster_permissions,
            eks::full_run::run_full_chile_scan,
            eks::peru::discover_peru_accounts,
            eks::peru::run_full_peru_scan,
            gke::login::setup_gcloud_login,
            gke::discovery::discover_gcp_projects,
            gke::test_cluster::test_single_gke_cluster,
            gke::full_run::run_full_gke_scan,
            system::check_dependencies,
            load::discovery::read_combined_inventory,
            load::loader::load_selected_clusters,
            // ── Bitbucket Repo Creator (v0.3.0) ──
            bitbucket::credentials::bitbucket_cred_status,
            bitbucket::credentials::bitbucket_cred_save,
            bitbucket::credentials::bitbucket_cred_delete,
            bitbucket::full_run::bitbucket_crear_repos,
            // ── CI/CD Automation (v0.4.0) ──
            cicd::subnets::cicd_subnets_leer,
            cicd::subnets::cicd_subnets_actualizar,
            cicd::bitbucket::cicd_bb_verificar,
            cicd::bitbucket::cicd_bb_subir,
            // ── SonarQube Cloud (v0.6.0) ──
            sonar::credentials::sonar_cred_status,
            sonar::credentials::sonar_cred_save,
            sonar::credentials::sonar_cred_delete
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación Tauri");
}
