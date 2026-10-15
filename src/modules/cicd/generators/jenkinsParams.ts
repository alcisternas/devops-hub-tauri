// JenkinsParams — el archivo que el pipeline de Jenkins lee con "source"
// al arrancar. Va en la raíz del repositorio.
//
// Kubernetes y Cloud Run tienen estructuras distintas, confirmadas cada una
// con un archivo real en producción. No es el mismo archivo con campos de
// más: cambian los nombres de las variables (REGISTRY_URL contra
// GCP_REGISTRY_URL), el orden y qué es fijo.
//
// Detalle que importa: AWS_OWNER solo existe cuando el destino es AWS. En
// GCP no va ni como línea vacía.

import type { CicdData } from "../shared";
import { ENV_SFX, extractProjectKey, registryNameK8s } from "../shared";
import { GCR_HOST, GCR_ORG, TOOL_VERSION_CR, TOOL_VERSION_K8S } from "../data/catalogs";

export function buildJenkinsParams(d: CicdData): string {
  // Los tres valores de relleno se ven en la vista previa cuando el campo
  // todavía está vacío, para que quede claro qué falta completar.
  const slack = d.slack || "ci-cd-TU-PROYECTO";
  const bbName = d.bbName || "TU-PROJECT-NAME";
  const bbKey = d.bbKey || "KEY";
  const projKey = extractProjectKey(d.gcpDev || d.gcpRelease || d.gcpProd || "");

  if (d.isCR) {
    const techLine = TOOL_VERSION_CR[d.tech] || "";
    const gcp: Record<string, string> = {
      dev: d.gcpDev,
      release: d.gcpRelease,
      prod: d.gcpProd,
    };

    // Un bloque de tres variables por ambiente, y las tres salen de
    // lugares distintos. Esto costó dos fallos de pipeline, así que vale
    // tenerlo escrito:
    //
    //   SERVICE_ACCOUNT_NAME  el "client_email" del JSON de la credencial
    //                         guardada en Jenkins. Es una cuenta de
    //                         DevOps y lleva "devops" en el nombre. No es
    //                         la cuenta de servicio del proyecto.
    //   SERVICE_ID            el ID de esa credencial en Jenkins, tal cual
    //                         está registrado. Algunos llevan el prefijo
    //                         "gcp-" y otros no, así que no se deriva.
    //   PROJECT_ID            el proyecto de GCP en minúsculas. Este sí
    //                         sale del formulario.
    //
    // Los dos primeros quedan vacíos si no se completaron: un valor
    // inventado que parece correcto es peor que uno ausente, porque el
    // pipeline falla recién al autenticar.
    const crLines = d.envs
      .map((env) => {
        const fallback = (gcp[env.key] || "").toLowerCase();
        const s = ENV_SFX[env.key] || env.key.toUpperCase();
        const p = (d.crProjId && d.crProjId[env.key]) || fallback;
        const sa = (d.crSA && d.crSA[env.key]) || "";
        const svc = (d.crServiceId && d.crServiceId[env.key]) || "";
        return `\nSERVICE_ACCOUNT_NAME_${s}=${sa}\nSERVICE_ID_${s}=${svc}\nPROJECT_ID_${s}=${p}`;
      })
      .join("");

    const scanExclude = d.crScanExclude ? `\nSCAN_EXCLUDE_FILES=${d.crScanExclude}` : "";
    const version = d.crVersion ? `\nVERSION=${d.crVersion}` : "";
    const registryName = d.crRegistryName || `gcr-${projKey}`;
    const dockerfile = d.crDockerfile || "Dockerfile";

    return `GCP_REGISTRY_URL=${GCR_HOST}/${GCR_ORG}
GCP_REGISTRY_NAME=${registryName}
SLACK_CHANNEL=${slack}
DOCKERFILE_PATH=${dockerfile}
REGION=${d.crRegion || "southamerica-west1"}
GCR_DEVOPS=gcr:${GCR_ORG}
FORTIFY_ENABLE_GATE=true
ENABLE_DEBRICKED=true
${crLines}
TYPE_CR=SERVICE
APPLY_POLICY_AFTER_DEPLOY=false

SCAN_SOURCE=src${scanExclude}
${techLine}${version}`;
  }

  // ── Kubernetes ──
  const techLine = TOOL_VERSION_K8S[d.tech] || "";
  const registryName = registryNameK8s(d);
  const dockerfile = d.k8sDockerfile || "Dockerfile";
  const scanCoverage = d.k8sScanCoverage || "coverage/**/";
  const scanExcludeLine = d.k8sScanExclude ? `\nSCAN_EXCLUDE_FILES=${d.k8sScanExclude}` : "";
  const depsFile = d.k8sDepsFile !== undefined ? d.k8sDepsFile : "";
  const caDomain = d.k8sCodeArtifactDomain || "br";
  const caRepo = d.k8sCodeArtifactRepo || "br-devops-codeartifact-01";
  const awsOwnerLine = d.isGCP ? "" : `\nAWS_OWNER=${d.k8sAwsOwner || "063003577365"}`;

  // En GCP las credenciales del registro son fijas y van entre comillas.
  // En AWS las tres son editables porque cambian por cuenta.
  const registryBlock = d.isGCP
    ? `REGISTRY_URL=${GCR_HOST}/${GCR_ORG}
REGISTRY_CREDENTIALS="gcr:${GCR_ORG}"
REGISTRY_NAME=${registryName}`
    : `REGISTRY_URL=${d.k8sRegistryUrl || ""}
REGISTRY_CREDENTIALS=${d.k8sRegistryCredentials || ""}
REGISTRY_NAME=${registryName}`;

  return `CODE_ARTIFACT_DOMAIN=${caDomain}
CODE_ARTIFACT_REPOSITORY=${caRepo}${awsOwnerLine}
${registryBlock}
${techLine}
DOCKERFILE_PATH=${dockerfile}
SLACK_CHANNEL=${slack}
PROJECT_KEY=${bbKey}
PROJECT_NAME="${bbName}"
SCAN_COVERAGE_REPORT=${scanCoverage}
SCAN_SOURCE=src${scanExcludeLine}
DEPENDENCIES_FILE_PATH=${depsFile}
ENABLE_FORTIFY_QUICK=false
FORTIFY_ENABLE_GATE=true
ENABLE_DEBRICKED=true`;
}
