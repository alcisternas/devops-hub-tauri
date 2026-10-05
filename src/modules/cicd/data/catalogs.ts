// Valores fijos del CI/CD, portados tal cual de la herramienta HTML v4.28.0.
//
// Todo lo que está acá es una tabla de verdades del equipo, no una decisión
// de diseño: rutas de Jenkinsfile reales, nombres de organización de Apigee
// confirmados con ejemplos en producción, versiones de herramientas. Si algo
// cambia en la plataforma, se cambia acá y queda cambiado en todo el módulo.

import type { EnvKey } from "../shared";

// ─────────────────────────────────────────────────────────────────────────
// Jenkinsfiles por tecnología (Kubernetes)
//
// Cuando una tecnología tiene una sola opción, el formulario la completa
// sola. Node.js es la única con varias, así que ahí hay que elegir.
// ─────────────────────────────────────────────────────────────────────────
export interface JenkinsfileOpcion {
  l: string;
  p: string;
}

export const JF_K8S: Record<string, JenkinsfileOpcion[]> = {
  nodejs: [
    { l: "Microservicio", p: "node/ms/Jenkinsfile.template" },
    { l: "Cloud Function", p: "node/cloud-function/Jenkinsfile.template" },
    { l: "Lambda", p: "node/lambda/no-build/Jenkinsfile.template" },
    { l: "Microfront", p: "node/microfront-cloud-storage/Jenkinsfile.template" },
    { l: "Firebase", p: "node/firebase/build/Jenkinsfile.template" },
    { l: "Code Artifact", p: "node/code-artifact-library/Jenkinsfile.template" },
  ],
  golang: [{ l: "Microservicio", p: "golang/lib/Jenkinsfile.template" }],
  java: [{ l: "Microservicio", p: "java/ms/Jenkinsfile.template" }],
  python: [{ l: "Microservicio", p: "python/ms/Jenkinsfile.template" }],
  dotnet: [{ l: "Microservicio", p: "dotnet/ms/Jenkinsfile.template" }],
  angular: [{ l: "Angular", p: "angular/Jenkinsfile.template" }],
  cobol: [{ l: "COBOL", p: "cobol/Jenkinsfile.template" }],
  database: [{ l: "Database", p: "database/Jenkinsfile.template" }],
  apigeex: [{ l: "Apigee X", p: "apigeex/Jenkinsfile.template" }],
  api: [{ l: "API", p: "api/Jenkinsfile.template" }],
  bus: [{ l: "Bus", p: "bus/Jenkinsfile.template" }],
  "cloud-scheduler": [{ l: "Cloud Scheduler", p: "cloud-scheduler/Jenkinsfile.template" }],
};

// Cloud Run usa siempre el mismo Jenkinsfile, sin importar la tecnología.
export const JF_CR = "cloud-run/unificado/Jenkinsfile.template";

// ─────────────────────────────────────────────────────────────────────────
// Opciones de los desplegables del paso 1
// ─────────────────────────────────────────────────────────────────────────
export const TECNOLOGIAS: { v: string; l: string }[] = [
  { v: "nodejs", l: "Node.js" },
  { v: "golang", l: "Golang" },
  { v: "java", l: "Java" },
  { v: "python", l: "Python" },
  { v: "dotnet", l: ".NET" },
  { v: "angular", l: "Angular" },
  { v: "cobol", l: "COBOL" },
  { v: "database", l: "Base de Datos" },
  { v: "apigeex", l: "Apigee X" },
  { v: "api", l: "API" },
  { v: "bus", l: "Bus" },
  { v: "cloud-scheduler", l: "Cloud Scheduler" },
];

export const DESTINOS: { v: string; l: string }[] = [
  { v: "GCP", l: "GCP" },
  { v: "AWS", l: "AWS" },
  { v: "ON-PREMISE", l: "On-Premise" },
  { v: "NUTANIX", l: "Nutanix" },
  { v: "BASE DE DATOS", l: "Base de Datos" },
];

// AWS no ofrece Cloud Run. El resto de las opciones son iguales.
export const INFRAS_GCP: { v: string; l: string }[] = [
  { v: "kubernetes", l: "Kubernetes" },
  { v: "cloudrun", l: "Cloud Run" },
  { v: "bucket", l: "Bucket (front)" },
  { v: "lambda", l: "Lambda" },
  { v: "otros", l: "Otros" },
];

export const INFRAS_AWS: { v: string; l: string }[] = [
  { v: "kubernetes", l: "Kubernetes" },
  { v: "lambda", l: "Lambda" },
  { v: "bucket", l: "Bucket (front)" },
  { v: "otros", l: "Otros" },
];

export const SUBTIPOS_APIGEE: { v: string; l: string }[] = [
  { v: "proxy", l: "Proxy" },
  { v: "facade", l: "Facade" },
  { v: "sharedflow", l: "Shared-Flow" },
];

export const INFRA_LABELS: Record<string, string> = {
  kubernetes: "Kubernetes",
  cloudrun: "Cloud Run",
  bucket: "Bucket",
  lambda: "Lambda",
  otros: "Otros",
};

// ─────────────────────────────────────────────────────────────────────────
// Sugerencias por tecnología
// ─────────────────────────────────────────────────────────────────────────

// Archivo de dependencias que lee el escaneo de seguridad. .NET queda vacío
// a propósito: el nombre del .csproj cambia en cada proyecto.
export const DEPS_FILE: Record<string, string> = {
  nodejs: "package.json",
  java: "pom.xml",
  python: "requirements.txt",
  golang: "go.mod",
  dotnet: "",
};

// Versión de la herramienta de build. Las dos tablas difieren a propósito:
// Cloud Run usa GO_TOOL_VERSION=1.25 y Kubernetes GOLANG_TOOL_VERSION=1.21.
export const TOOL_VERSION_K8S: Record<string, string> = {
  nodejs: "NODEJS_TOOL_VERSION=22",
  java: "JAVA_TOOL_VERSION=17",
  python: "PYTHON_TOOL_VERSION=3.11",
  golang: "GOLANG_TOOL_VERSION=1.21",
  dotnet: "DOTNET_TOOL_VERSION=8",
};

export const TOOL_VERSION_CR: Record<string, string> = {
  nodejs: "NODEJS_TOOL_VERSION=22",
  java: "JAVA_TOOL_VERSION=17",
  python: "PYTHON_TOOL_VERSION=3.11",
  golang: "GO_TOOL_VERSION=1.25",
  dotnet: "DOTNET_TOOL_VERSION=8",
};

// Imagen base del bitbucket-pipelines.yml. Solo hay valor sugerido para las
// dos tecnologías donde está confirmado; el resto se completa a mano.
export const BBPIPE_IMAGE: Record<string, string> = {
  nodejs: "841964127262.dkr.ecr.us-east-1.amazonaws.com/ecr-base-image:alpine3.21-node_22.0.0-sec",
  golang: "841964127262.dkr.ecr.us-east-1.amazonaws.com/ecr-base-image:alpine3.23-golang1.25-sec",
};

// ─────────────────────────────────────────────────────────────────────────
// Apigee
// ─────────────────────────────────────────────────────────────────────────

// Valores fijos de Chile, confirmados con un JenkinsParams real.
// Perú todavía no tiene datos: sus campos quedan vacíos y editables.
export const APIGEE_FIXED_CL: Record<EnvKey, { org: string; env: string; svc: string }> = {
  dev: { org: "br-gcp-apigee-services-cl-dev", env: "dev-internal", svc: "br-gcp-apigee-services-cl-dev" },
  release: { org: "br-gcp-apigee-services-cl-pp", env: "pp-internal", svc: "br-gcp-apigee-cl-pp" },
  prod: { org: "br-gcp-apigee-services-cl-prod", env: "prod-external-1", svc: "br-gcp-apigee-cl-prod" },
};

// Los 6 Jenkinsfiles de Apigee: subtipo x país. Nombres reales del repo de
// templates. Ojo con la asimetría del último: shared-flow de Perú usa
// "-pe" con guion, no ".pe" con punto como los otros dos.
export const APIGEE_JENKINSFILE: Record<string, Record<string, string>> = {
  proxy: { CL: "Jenkinsfile.template", PE: "Jenkinsfile.pe.template" },
  facade: { CL: "Jenkinsfile-facade.template", PE: "Jenkinsfile-facade.pe.template" },
  sharedflow: { CL: "Jenkinsfile-shared-flow.template", PE: "Jenkinsfile-shared-flow-pe.template" },
};

// Folder fijo de Apigee en Jenkins, y el job distinto al del resto.
export const APIGEE_FOLDER = "VST-OPEN-BANKING/CL_APIGEE";

// ─────────────────────────────────────────────────────────────────────────
// Jenkins — jobs y parámetros fijos
// ─────────────────────────────────────────────────────────────────────────
export const JENKINS_BASE = "https://jenkins-devops.bancoripley.cl";
export const JENKINS_JOB_PATH = "/job/COE-DEVOPS/job/UTILITY-PIPELINES/job/jenkinsctl";
export const JENKINS_JOB_K8S = "create_pipelines";
export const JENKINS_JOB_APIGEE = "create_pipeline_v2";

// Registro de imágenes de GCP. Un solo lugar del que salen las tres formas
// en que se escribe la imagen (JenkinsParams, service.yaml y Spinnaker).
export const GCR_HOST = "gcr.io";
export const GCR_ORG = "br-gcp-cicd-devops";

// Proyectos de networking donde viven las subredes compartidas.
export const NETWORKING_PROJECTS: Record<string, string> = {
  dev: "br-gcp-networking-dev",
  pp: "br-gcp-networking-pp",
  prod: "br-gcp-networking-prod",
};
