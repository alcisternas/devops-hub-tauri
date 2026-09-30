use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

// ─────────────────────────────────────────────────────────────────────────
// Tipos de datos — reflejan 1:1 lo que hoy vive hardcodeado en refresh-eks.sh:
// setup_config_chile(), setup_config_peru(), get_chile_profile_name(),
// get_peru_profile_name(), y las constantes PERU_BASE_*.
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountEntry {
    pub account_id: String,
    pub profile: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChileConfig {
    pub sso_session: String,
    pub portal_url: String,
    pub accounts: Vec<AccountEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PeruConfig {
    pub sso_session: String,
    pub portal_url: String,
    // Profile base usado como source_profile para el AssumeRole en cadena
    pub base_profile: String,
    pub base_account_id: String,
    pub base_role: String,
    // Nombre del rol que se asume en cada cuenta (titan-pipeline)
    pub role_arn_name: String,
    pub accounts: Vec<AccountEntry>,
}

// GCP no usa tabla de cuentas — GKE descubre los proyectos dinámicamente
// vía "gcloud projects list" en la organización, así que solo hace falta
// guardar la cuenta y el ID de organización. Perú queda como Option
// porque el bash original ni siquiera lo tiene implementado todavía
// ("preparado para el futuro", literalmente el comentario del script).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GcpCountryConfig {
    pub account: String,
    pub org_id: String,
}

fn default_gcp_chile() -> GcpCountryConfig {
    GcpCountryConfig {
        account: "eb_acisternasg@bancoripley.cl".to_string(),
        org_id: "662536622727".to_string(),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GcpConfig {
    #[serde(default = "default_gcp_chile")]
    pub chile: GcpCountryConfig,
    #[serde(default)]
    pub peru: Option<GcpCountryConfig>,
}

impl Default for GcpConfig {
    fn default() -> Self {
        Self {
            chile: default_gcp_chile(),
            peru: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountsConfig {
    pub version: u32,
    // us-east-1 en ambos portales hoy — con default para que archivos
    // ya existentes (creados antes de este campo) sigan leyéndose sin error.
    #[serde(default = "default_sso_region")]
    pub sso_region: String,
    // Región donde viven los clusters EKS — conceptualmente distinta de
    // sso_region (identidad) aunque hoy compartan el mismo valor.
    #[serde(default = "default_eks_region")]
    pub eks_region: String,
    #[serde(default)]
    pub gcp: GcpConfig,
    pub chile: ChileConfig,
    pub peru: PeruConfig,
}

fn default_sso_region() -> String {
    "us-east-1".to_string()
}

fn default_eks_region() -> String {
    "us-east-1".to_string()
}

// Error estructurado — igual al que ya usan Clusters y los módulos futuros.
// React recibe {etapa, mensaje} en vez de un string plano.
#[derive(Debug, Serialize)]
pub struct RefreshError {
    pub etapa: String,
    pub mensaje: String,
}

impl RefreshError {
    pub fn new(etapa: &str, mensaje: impl Into<String>) -> Self {
        Self {
            etapa: etapa.to_string(),
            mensaje: mensaje.into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Valores por defecto — migrados EXACTAMENTE de las tablas case() del bash.
// Esto solo se usa la primera vez que corre la app (si accounts.json no
// existe todavía). Después, lo que manda es el archivo en disco, editable
// a mano por cualquiera del equipo.
// ─────────────────────────────────────────────────────────────────────────
fn default_config() -> AccountsConfig {
    let chile_accounts = vec![
        ("847309888826", "cl-app-dev"),
        ("659018920863", "cl-app-pp"),
        ("198306748277", "cl-app-prd"),
        ("638635262599", "cl-car-dev"),
        ("998809683714", "cl-car-pp"),
        ("313947525243", "cl-car-prd"),
        ("479382773437", "cl-pyb-dev"),
        ("141987615906", "cl-pyb-pp"),
        ("864968012687", "cl-pyb-prd"),
        ("912271875241", "cl-core-data-pp"),
        ("837538682169", "cl-core-data-prd"),
        ("063003577365", "cl-core-devops"),
        ("841964127262", "cl-core-shared-services"),
        ("045312750961", "cl-corp-dev"),
        ("275003359076", "cl-corp-pp"),
    ]
    .into_iter()
    .map(|(id, profile)| AccountEntry {
        account_id: id.to_string(),
        profile: profile.to_string(),
    })
    .collect();

    let peru_accounts = vec![
        ("147882410889", "core-dev"),
        ("805170969621", "core-prd"),
        ("433525774989", "core-qa"),
        ("290955649831", "digital-dev"),
        ("097123220056", "digital-prd"),
        ("782658102466", "digital-qa"),
        ("392514017703", "bi-dev"),
        ("418956650366", "bi-qa"),
        ("531488432257", "bi-prd"),
        ("503133918962", "corp-rpass-prd"),
        ("594671381337", "devops-eks"),
        ("738234143984", "integrations-qa"),
        ("964724138803", "ripley-qa"),
        ("230622975316", "banco-security"),
        ("571062953933", "corp-pe"),
        ("954937221722", "cobranzas-ics"),
        ("661407848026", "security-prd"),
        ("625556741936", "security-dev"),
        ("203722324772", "shared-service-qa"),
        ("891907582249", "banco-shared"),
        ("789040743053", "banco-network"),
        ("008699018419", "corp-networking"),
        ("480179662672", "insurance-dev"),
        ("283694444684", "insurance-qa"),
        ("815421581184", "insurance-prd"),
        ("435160705078", "integrations-dev"),
        ("945826603128", "integrations-prd"),
    ]
    .into_iter()
    .map(|(id, profile)| AccountEntry {
        account_id: id.to_string(),
        profile: profile.to_string(),
    })
    .collect();

    AccountsConfig {
        version: 1,
        sso_region: default_sso_region(),
        eks_region: default_eks_region(),
        gcp: GcpConfig::default(),
        chile: ChileConfig {
            sso_session: "BR_CHILE".to_string(),
            portal_url: "https://d-90677ee34c.awsapps.com/start".to_string(),
            accounts: chile_accounts,
        },
        peru: PeruConfig {
            sso_session: "BR_PERU".to_string(),
            portal_url: "https://ssocloud.awsapps.com/start".to_string(),
            base_profile: "devops-base".to_string(),
            base_account_id: "594671381337".to_string(),
            base_role: "aws-edit".to_string(),
            role_arn_name: "titan-pipeline".to_string(),
            accounts: peru_accounts,
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Resolución de ruta y carga/creación del archivo
// ─────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────
// Escritura genérica de bloques estilo INI (usada por sso.rs para
// [sso-session ...] y por permissions.rs para [profile ...]).
// Comportamiento "force": si el header ya existe, reemplaza todo el bloque
// hasta el próximo "[" o el final del archivo. Si no existe, lo agrega.
// ─────────────────────────────────────────────────────────────────────────
// force=true: si el header ya existe, se reemplaza el bloque completo
//   (igual al bash cuando pasa "force" — usado por Chile, donde el rol
//   vive DENTRO del perfil y cambia entre corridas).
// force=false: si el header ya existe, NO SE TOCA — se deja intacto
//   (igual al bash sin "force" — usado por Perú, donde el perfil nunca
//   cambia entre roles, así que reescribir sería trabajo inútil y un
//   riesgo de corrupción sin ningún beneficio).
pub(crate) fn write_ini_block(path: &PathBuf, header: &str, body: &str, force: bool) -> Result<(), RefreshError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| RefreshError::new("crear_directorio_aws", e.to_string()))?;
    }

    let existing = if path.exists() {
        fs::read_to_string(path).map_err(|e| RefreshError::new("leer_config", e.to_string()))?
    } else {
        String::new()
    };

    if existing.contains(header) && !force {
        // Ya existe y no se pidió force — se conserva tal cual, sin escribir nada.
        return Ok(());
    }

    let new_content = if let Some(start) = existing.find(header) {
        let after_header = start + header.len();
        let rest = &existing[after_header..];
        let end_offset = rest.find('[').map(|i| after_header + i).unwrap_or(existing.len());

        let mut result = String::new();
        result.push_str(&existing[..start]);
        result.push_str(header);
        result.push('\n');
        result.push_str(body);
        result.push('\n');
        result.push_str(&existing[end_offset..]);
        result
    } else {
        let mut result = existing.clone();
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push('\n');
        result.push_str(header);
        result.push('\n');
        result.push_str(body);
        result.push('\n');
        result
    };

    fs::write(path, new_content).map_err(|e| RefreshError::new("escribir_config", e.to_string()))
}

pub fn accounts_file_path(app: &AppHandle) -> Result<PathBuf, RefreshError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| RefreshError::new("resolver_ruta", e.to_string()))?;
    Ok(dir.join("accounts.json"))
}

pub fn load_or_create_accounts(app: &AppHandle) -> Result<AccountsConfig, RefreshError> {
    let path = accounts_file_path(app)?;

    // Asegurar que la carpeta exista antes de leer o escribir
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| RefreshError::new("crear_directorio", e.to_string()))?;
    }

    let config: AccountsConfig = if path.exists() {
        let raw = fs::read_to_string(&path)
            .map_err(|e| RefreshError::new("leer_archivo", e.to_string()))?;
        serde_json::from_str(&raw).map_err(|e| RefreshError::new("parsear_json", e.to_string()))?
    } else {
        // Primera ejecución: crear el archivo con los datos migrados del bash
        default_config()
    };

    // Reescribe siempre con la versión resuelta: si el archivo venía de una
    // versión anterior sin "sso_region" (u otro campo agregado a futuro),
    // queda completado en disco con su valor por defecto, visible y editable.
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| RefreshError::new("serializar_json", e.to_string()))?;
    fs::write(&path, json).map_err(|e| RefreshError::new("escribir_archivo", e.to_string()))?;

    Ok(config)
}
