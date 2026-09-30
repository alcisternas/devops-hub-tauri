pub mod config;
pub mod discovery;
pub mod full_run;
pub mod permissions;
pub mod peru;
pub mod sso;

use config::{load_or_create_accounts, AccountsConfig, RefreshError};
use tauri::AppHandle;

// Comando expuesto a React: invoke("get_eks_accounts")
// Lee accounts.json desde app_data_dir; si no existe, lo crea con los
// valores migrados del bash (ver default_config() en config.rs).
#[tauri::command]
pub async fn get_eks_accounts(app: AppHandle) -> Result<AccountsConfig, RefreshError> {
    load_or_create_accounts(&app)
}
