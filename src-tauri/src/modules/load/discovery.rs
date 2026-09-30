use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Manager};

use crate::modules::eks::config::RefreshError;
use crate::modules::eks::full_run::{read_previous_entries, InventoryClusterEntry};

// ─────────────────────────────────────────────────────────────────────────
// Entrada combinada — mismos campos que InventoryClusterEntry + el
// "proveedor" que el bash inyecta al combinar (build_combined_list), más
// el conteo de permisos ya calculado para no repetir esa cuenta en el
// frontend.
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CombinedClusterEntry {
    pub pais: String,
    pub proveedor: String, // "aws" | "gcp"
    pub account_id: String,
    pub account_name: String,
    pub role: String,
    pub profile: String,
    pub cluster: String,
    pub region: String,
    pub context_alias: String,
    pub permisos: HashMap<String, bool>,
    pub permisos_ok: usize,
    pub permisos_total: usize,
    // 0 = acceso completo, 1 = parcial, 2 = sin acceso — mismo criterio
    // de orden que select_clusters() en el bash.
    pub sort_key: u8,
}

fn to_combined(e: InventoryClusterEntry, proveedor: &str) -> CombinedClusterEntry {
    let total = e.permisos.len();
    let ok = e.permisos.values().filter(|v| **v).count();
    let sort_key = if total > 0 && ok == total {
        0
    } else if ok > 0 {
        1
    } else {
        2
    };

    CombinedClusterEntry {
        pais: e.pais,
        proveedor: proveedor.to_string(),
        account_id: e.account_id,
        account_name: e.account_name,
        role: e.role,
        profile: e.profile,
        cluster: e.cluster,
        region: e.region,
        context_alias: e.context_alias,
        permisos: e.permisos,
        permisos_ok: ok,
        permisos_total: total,
        sort_key,
    }
}

#[derive(Debug, Serialize)]
pub struct CombinedInventoryResult {
    pub country: String,
    pub has_aws: bool,
    pub has_gcp: bool,
    pub entries: Vec<CombinedClusterEntry>,
}

#[tauri::command]
pub async fn read_combined_inventory(app: AppHandle, country: String) -> Result<CombinedInventoryResult, RefreshError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| RefreshError::new("resolver_home", e.to_string()))?;

    let aws_path = home.join(".aws").join(format!("eks-inventory-{}.json", country));
    // GKE hoy solo existe para Chile — si no existe el archivo (ej. Perú),
    // read_previous_entries ya devuelve vacío sin error, así que esto
    // funciona igual sin necesitar un caso especial.
    let gcp_path = home.join(".gcp").join(format!("gke-inventory-{}.json", country));

    let has_aws = aws_path.exists();
    let has_gcp = gcp_path.exists();

    let mut entries: Vec<CombinedClusterEntry> = Vec::new();

    if has_aws {
        for e in read_previous_entries(&aws_path) {
            entries.push(to_combined(e, "aws"));
        }
    }
    if has_gcp {
        for e in read_previous_entries(&gcp_path) {
            entries.push(to_combined(e, "gcp"));
        }
    }

    // Orden: por estado (completo/parcial/sin acceso), luego país,
    // proveedor y cuenta — igual que el "sort -t$'\t' -k15,15 -k2,2..."
    // del bash.
    entries.sort_by(|a, b| {
        (a.sort_key, &a.pais, &a.proveedor, &a.account_name).cmp(&(
            b.sort_key,
            &b.pais,
            &b.proveedor,
            &b.account_name,
        ))
    });

    if entries.is_empty() {
        return Err(RefreshError::new(
            "sin_inventario",
            format!(
                "No hay clusters disponibles para {}. Ejecuta primero el recorrido completo (EKS y/o GKE).",
                country
            ),
        ));
    }

    Ok(CombinedInventoryResult {
        country,
        has_aws,
        has_gcp,
        entries,
    })
}
