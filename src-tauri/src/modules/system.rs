use serde::Serialize;
use std::process::Command;

// Equivalente a check_dependencies() del bash — pero sin ofrecer instalación
// automática vía brew, porque eso es específico de macOS. En vez de eso,
// se detecta el sistema operativo real y se da la instrucción correcta
// para cada uno, ya que la app la va a usar todo el equipo en Windows,
// Linux y Mac indistintamente.

#[derive(Debug, Serialize)]
pub struct ToolStatus {
    pub name: String,
    pub found: bool,
    pub version: Option<String>,
    pub install_hint: String,
}

#[derive(Debug, Serialize)]
pub struct DependenciesCheckResult {
    pub os: String, // "macos" | "windows" | "linux" | otro
    pub tools: Vec<ToolStatus>,
    pub all_ok: bool,
}

fn check_tool(name: &str, version_args: &[&str]) -> (bool, Option<String>) {
    match Command::new(name).args(version_args).output() {
        Ok(o) if o.status.success() => {
            let out = String::from_utf8_lossy(&o.stdout);
            let err = String::from_utf8_lossy(&o.stderr);
            // Algunas herramientas (kubectl en ciertas versiones) imprimen
            // la versión en stderr en vez de stdout — se revisan ambos.
            let combined = if !out.trim().is_empty() { out.to_string() } else { err.to_string() };
            let first_line = combined.lines().next().unwrap_or("").trim().to_string();
            (true, if first_line.is_empty() { None } else { Some(first_line) })
        }
        _ => (false, None),
    }
}

fn install_hint(tool: &str, os: &str) -> String {
    match (tool, os) {
        ("aws", "macos") => "brew install awscli".to_string(),
        ("aws", "windows") => {
            "Instalador oficial: https://awscli.amazonaws.com/AWSCLIV2.msi".to_string()
        }
        ("aws", "linux") => {
            "Guía oficial: https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
                .to_string()
        }
        ("gcloud", "macos") => "brew install --cask google-cloud-sdk".to_string(),
        ("gcloud", "windows") => {
            "Instalador oficial: https://cloud.google.com/sdk/docs/install#windows".to_string()
        }
        ("gcloud", "linux") => {
            "Guía oficial: https://cloud.google.com/sdk/docs/install#linux".to_string()
        }
        ("kubectl", "macos") => "brew install kubectl".to_string(),
        ("kubectl", "windows") => {
            "Guía oficial: https://kubernetes.io/docs/tasks/tools/install-kubectl-windows/".to_string()
        }
        ("kubectl", "linux") => {
            "Guía oficial: https://kubernetes.io/docs/tasks/tools/install-kubectl-linux/".to_string()
        }
        _ => "Consulta la documentación oficial de la herramienta.".to_string(),
    }
}

#[tauri::command]
pub async fn check_dependencies() -> DependenciesCheckResult {
    // "macos" | "windows" | "linux" — valor de Rust std, no necesita ajuste.
    let os = std::env::consts::OS.to_string();

    let checks: [(&str, &[&str]); 3] = [
        ("aws", &["--version"]),
        ("gcloud", &["--version"]),
        ("kubectl", &["version", "--client"]),
    ];

    let mut tools = Vec::new();
    let mut all_ok = true;

    for (name, args) in checks {
        let (found, version) = check_tool(name, args);
        if !found {
            all_ok = false;
        }
        tools.push(ToolStatus {
            name: name.to_string(),
            found,
            version,
            install_hint: if found { String::new() } else { install_hint(name, &os) },
        });
    }

    DependenciesCheckResult { os, tools, all_ok }
}
