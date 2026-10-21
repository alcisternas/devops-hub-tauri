// Publicacion en ECR: el script que se ejecuta DESPUES de generar el Dockerfile,
// en cualquier equipo con Docker (BuildKit), AWS CLI y Docker Scout; por ejemplo
// el Cloud Shell de GCP. Sigue el procedimiento manual del equipo para
// securitizar y subir imagenes (credenciales temporales del access portal,
// Docker Scout como filtro, etiqueta con sufijo -sec) y suma lo que ese
// procedimiento no tenia: pruebas funcionales y comprobar que la imagen no
// exista ya en ECR.
//
// Esta fase no ejecuta nada: solo genera el texto. Sin Docker ni acceso a AWS no
// se pudo probar de punta a punta; el script se valida en sintaxis (bash -n).

use serde::{Deserialize, Serialize};

use super::validar::{es_tag, shell_quote};

// Datos de la cuenta compartida del banco; salen del procedimiento del equipo
// y son editables en la pantalla.
pub const CUENTA_POR_DEFECTO: &str = "841964127262";
pub const REGION_POR_DEFECTO: &str = "us-east-1";
pub const REPOSITORIO_POR_DEFECTO: &str = "ecr-base-image";
pub const SUFIJO_SEGURO: &str = "sec";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DestinoEcr {
    #[serde(default)]
    pub cuenta: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub repositorio: String,
    // Vacio: se propone <nombre>-<tag original>-sec
    #[serde(default)]
    pub tag: String,
}

impl Default for DestinoEcr {
    fn default() -> Self {
        DestinoEcr {
            cuenta: CUENTA_POR_DEFECTO.into(),
            region: REGION_POR_DEFECTO.into(),
            repositorio: REPOSITORIO_POR_DEFECTO.into(),
            tag: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PasoPublicacion {
    pub titulo: String,
    pub comando: String,
}

fn es_region(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 25
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && s.contains('-')
        && s.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false)
}

// Reglas de nombre de repositorio de ECR: minusculas, digitos, . _ - y / entre niveles.
fn es_repositorio_ecr(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.split('/').all(|nivel| {
            !nivel.is_empty()
                && nivel.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
                && nivel.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
                && nivel.ends_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

impl DestinoEcr {
    // Completa el tag si vino vacio y valida todo antes de armar comandos.
    pub fn resuelto(&self, nombre: &str, tag_origen: &str) -> Result<DestinoEcr, String> {
        let mut d = self.clone();
        d.cuenta = d.cuenta.trim().to_string();
        d.region = d.region.trim().to_string();
        d.repositorio = d.repositorio.trim().to_string();
        d.tag = d.tag.trim().to_string();
        if d.tag.is_empty() {
            d.tag = format!("{}-{}-{}", nombre, tag_origen, SUFIJO_SEGURO);
        }
        if d.cuenta.len() != 12 || !d.cuenta.chars().all(|c| c.is_ascii_digit()) {
            return Err("La cuenta de AWS debe tener 12 dígitos.".into());
        }
        if !es_region(&d.region) {
            return Err(format!("Región de AWS no válida: {:?}", d.region));
        }
        if !es_repositorio_ecr(&d.repositorio) {
            return Err(format!("Nombre de repositorio de ECR no válido: {:?}", d.repositorio));
        }
        if !es_tag(&d.tag) {
            return Err(format!("Tag no válido: {:?}", d.tag));
        }
        Ok(d)
    }

    pub fn registro(&self) -> String {
        format!("{}.dkr.ecr.{}.amazonaws.com", self.cuenta, self.region)
    }

    pub fn imagen(&self) -> String {
        format!("{}/{}:{}", self.registro(), self.repositorio, self.tag)
    }
}

// Los mismos pasos que hace el script, para mostrarlos y copiarlos de a uno.
pub fn pasos(etiqueta_local: &str, d: &DestinoEcr) -> Vec<PasoPublicacion> {
    let p = |t: &str, c: String| PasoPublicacion { titulo: t.into(), comando: c };
    let local = shell_quote(etiqueta_local);
    let destino = shell_quote(&d.imagen());
    vec![
        p(
            "1. Comprobar que la imagen no existe ya en ECR",
            format!(
                "aws ecr describe-images --region {} --repository-name {} --image-ids imageTag={}",
                shell_quote(&d.region),
                shell_quote(&d.repositorio),
                shell_quote(&d.tag)
            ),
        ),
        p(
            "2. Pruebas funcionales dentro de la imagen",
            "docker buildx build --target test --no-cache --build-arg CHS_TEST_NONCE=$(date +%s) .".into(),
        ),
        p("3. Construir la imagen endurecida", format!("docker buildx build --target final --load -t {} .", local)),
        p("4. Analizar con Docker Scout (resumen)", format!("docker scout quickview {}", local)),
        p(
            "5. Filtro: no debe haber vulnerabilidades críticas ni altas",
            format!("docker scout cves --exit-code --only-severity critical,high {}", local),
        ),
        p(
            "6. Iniciar sesión en ECR",
            format!(
                "aws ecr get-login-password --region {} | docker login --username AWS --password-stdin {}",
                shell_quote(&d.region),
                shell_quote(&d.registro())
            ),
        ),
        p("7. Etiquetar para ECR", format!("docker tag {} {}", local, destino)),
        p("8. Subir a ECR", format!("docker push {}", destino)),
    ]
}

// Script completo. Todos los valores ya pasaron por `resuelto`, y aun asi se
// entrecomillan.
pub fn script(etiqueta_local: &str, d: &DestinoEcr, imagen_origen: &str, huella_dockerfile: &str) -> String {
    let q = shell_quote;
    format!(
        r#"#!/usr/bin/env bash
# Publicar la imagen endurecida en ECR.
# Generado por DevOps Hub. Dockerfile sha256: {huella}
# Imagen de origen: {origen}
#
# Requisitos: docker con BuildKit (buildx), aws cli y docker scout.
# Credenciales: en el access portal de AWS, cuenta CORE-AWS-SHARED-SERVICES-CL ({cuenta}),
# "Claves de acceso" -> Opcion 1, y pegar las variables en esta terminal antes de ejecutar.
# Ejecutar desde esta carpeta:  ./publicar.sh
set -euo pipefail

CUENTA={cuenta_q}
REGION={region_q}
REPOSITORIO={repo_q}
TAG={tag_q}
LOCAL={local_q}
REGISTRO="${{CUENTA}}.dkr.ecr.${{REGION}}.amazonaws.com"
DESTINO="${{REGISTRO}}/${{REPOSITORIO}}:${{TAG}}"

for herramienta in docker aws; do
  command -v "$herramienta" >/dev/null 2>&1 || {{ echo "Falta '$herramienta' en el PATH." >&2; exit 1; }}
done
docker buildx version >/dev/null 2>&1 || {{ echo "Falta docker buildx (BuildKit)." >&2; exit 1; }}
docker scout version >/dev/null 2>&1 || {{ echo "Falta docker scout." >&2; exit 1; }}

echo "== 0. Credenciales de AWS"
cuenta_actual="$(aws sts get-caller-identity --query Account --output text 2>/dev/null)" || {{
  echo "No hay credenciales de AWS validas. Copie las variables de 'Claves de acceso' (Opcion 1) del access portal." >&2
  exit 1
}}
if [ "$cuenta_actual" != "$CUENTA" ]; then
  echo "Las credenciales son de la cuenta $cuenta_actual y se esperaba $CUENTA." >&2
  exit 1
fi

echo "== 1. Comprobando que $DESTINO no exista"
if salida="$(aws ecr describe-images --region "$REGION" --repository-name "$REPOSITORIO" --image-ids "imageTag=$TAG" 2>&1)"; then
  echo "La imagen ya existe en ECR. No se sube nada."
  exit 0
fi
case "$salida" in
  *ImageNotFoundException*) ;;
  *) echo "No se pudo consultar ECR:" >&2; echo "$salida" >&2; exit 1 ;;
esac

echo "== 2. Pruebas funcionales dentro de la imagen"
docker buildx build --target test --no-cache --build-arg CHS_TEST_NONCE="$(date +%s)" .

echo "== 3. Construyendo la imagen endurecida"
docker buildx build --target final --load -t "$LOCAL" .

echo "== 4. Docker Scout"
docker scout quickview "$LOCAL" || true
if ! docker scout cves --exit-code --only-severity critical,high "$LOCAL"; then
  echo "Hay vulnerabilidades criticas o altas: la imagen NO se sube." >&2
  exit 1
fi

echo "== 5. Subiendo a ECR"
aws ecr get-login-password --region "$REGION" | docker login --username AWS --password-stdin "$REGISTRO"
docker tag "$LOCAL" "$DESTINO"
docker push "$DESTINO"
echo "Listo: $DESTINO"
"#,
        huella = huella_dockerfile,
        origen = imagen_origen,
        cuenta = d.cuenta,
        cuenta_q = q(&d.cuenta),
        region_q = q(&d.region),
        repo_q = q(&d.repositorio),
        tag_q = q(&d.tag),
        local_q = q(etiqueta_local),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_destino_por_defecto_sigue_el_procedimiento_del_equipo() {
        let d = DestinoEcr::default().resuelto("node", "26.12.0-alpine3.24").unwrap();
        assert_eq!(d.tag, "node-26.12.0-alpine3.24-sec");
        assert_eq!(d.registro(), "841964127262.dkr.ecr.us-east-1.amazonaws.com");
        assert_eq!(d.imagen(), "841964127262.dkr.ecr.us-east-1.amazonaws.com/ecr-base-image:node-26.12.0-alpine3.24-sec");
    }

    #[test]
    fn rechaza_valores_que_romperian_el_script() {
        let base = DestinoEcr::default();
        let con = |f: &dyn Fn(&mut DestinoEcr)| {
            let mut d = base.clone();
            f(&mut d);
            d.resuelto("node", "1").is_err()
        };
        assert!(con(&|d| d.cuenta = "123".into()));
        assert!(con(&|d| d.cuenta = "84196412726a".into()));
        assert!(con(&|d| d.region = "us-east-1; rm -rf /".into()));
        assert!(con(&|d| d.region = "useast1".into()));
        assert!(con(&|d| d.repositorio = "Mayusculas".into()));
        assert!(con(&|d| d.repositorio = "a b".into()));
        assert!(con(&|d| d.repositorio = "a/../b".into()));
        assert!(con(&|d| d.tag = "$(id)".into()));
        assert!(con(&|d| d.tag = "con espacio".into()));
        let mut ok = base.clone();
        ok.repositorio = "equipo/base-image".into();
        ok.tag = "buster-golang1.16.3-sec".into();
        assert!(ok.resuelto("x", "y").is_ok());
    }

    #[test]
    fn el_script_tiene_sintaxis_valida_y_los_pasos_clave() {
        let d = DestinoEcr::default().resuelto("node", "26.12.0-alpine3.24").unwrap();
        let s = script("node:26.12.0-alpine3.24-hardened", &d, "docker.io/library/node@sha256:abc", "deadbeef");
        for esperado in [
            "set -euo pipefail",
            "aws sts get-caller-identity",
            "ImageNotFoundException",
            "docker buildx build --target test --no-cache",
            "docker scout cves --exit-code --only-severity critical,high",
            "docker push \"$DESTINO\"",
            "CORE-AWS-SHARED-SERVICES-CL",
        ] {
            assert!(s.contains(esperado), "falta: {}", esperado);
        }
        // El push va despues de la comprobacion de vulnerabilidades.
        assert!(s.find("only-severity").unwrap() < s.find("docker push").unwrap());

        // bash -n solo parsea: no ejecuta nada.
        let ruta = std::env::temp_dir().join(format!("publicar-prueba-{}.sh", std::process::id()));
        std::fs::write(&ruta, &s).unwrap();
        let r = std::process::Command::new("bash").arg("-n").arg(&ruta).output();
        let _ = std::fs::remove_file(&ruta);
        if let Ok(out) = r {
            assert!(out.status.success(), "bash -n: {}", String::from_utf8_lossy(&out.stderr));
        }
    }

    #[test]
    fn los_pasos_coinciden_con_el_orden_del_script() {
        let d = DestinoEcr::default().resuelto("node", "26").unwrap();
        let p = pasos("node:26-hardened", &d);
        assert_eq!(p.len(), 8);
        assert!(p[0].comando.contains("describe-images"));
        assert!(p[4].comando.contains("--only-severity critical,high"));
        assert!(p[7].comando.starts_with("docker push "));
    }
}
