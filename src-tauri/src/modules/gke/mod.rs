pub mod discovery;
pub mod full_run;
pub mod login;
pub mod test_cluster;

// En Windows el binario ejecutable es gcloud.cmd; "gcloud" a secas es un
// script de shell Unix que CreateProcess no puede correr.
#[cfg(target_os = "windows")]
pub const GCLOUD_BIN: &str = "gcloud.cmd";
#[cfg(not(target_os = "windows"))]
pub const GCLOUD_BIN: &str = "gcloud";
