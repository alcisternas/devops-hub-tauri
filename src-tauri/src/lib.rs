use std::process::Command;

mod modules;
use modules::eks;
use modules::gke;
use modules::load;
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
            load::loader::load_selected_clusters
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación Tauri");
}
