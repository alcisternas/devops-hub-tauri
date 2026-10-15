// Tipos y lógica pura del módulo CI/CD.
//
// Dos cosas viven acá y conviene no confundirlas:
//
//   CicdForm  — lo que el usuario escribe. Un solo objeto plano con todos
//               los campos del formulario, para que el estado de React sea
//               uno y no treinta.
//   CicdData  — lo que consumen los generadores de archivos. Se calcula a
//               partir del formulario con collectData(), resolviendo ahí
//               los valores derivados (ambientes activos, accounts, URIs).
//
// Los nombres de los campos de CicdData son los mismos que usaba el objeto
// "d" de la herramienta HTML. Es a propósito: los generadores se portaron
// línea por línea, y mantener los nombres permitió compararlos byte a byte
// contra la salida del HTML.

import type { RefreshError } from "../clusters/shared";
import {
  APIGEE_FIXED_CL,
  APIGEE_JENKINSFILE,
  BBPIPE_IMAGE,
  DEPS_FILE,
  GCR_HOST,
  GCR_ORG,
  JF_CR,
  JF_K8S,
  NETWORKING_PROJECTS,
} from "./data/catalogs";

// ─────────────────────────────────────────────────────────────────────────
// Ambientes
// ─────────────────────────────────────────────────────────────────────────
export type EnvKey = "dev" | "release" | "prod";

export interface EnvItem {
  key: EnvKey;
  label: string;
}

export const ENV_KEYS: EnvKey[] = ["dev", "release", "prod"];

export const ENV_LABEL: Record<EnvKey, string> = {
  dev: "DEV",
  release: "RELEASE",
  prod: "PROD",
};

// Sufijo de las variables del JenkinsParams: release se escribe PP.
export const ENV_SFX: Record<EnvKey, string> = {
  dev: "DEV",
  release: "PP",
  prod: "PROD",
};

// Subcarpeta del repositorio: release va en "pp".
export function envSub(k: EnvKey | string): string {
  const map: Record<string, string> = { dev: "dev", release: "pp", prod: "prod" };
  return map[k] || k;
}

// ─────────────────────────────────────────────────────────────────────────
// Subredes — tipos espejo de lo que devuelve Rust
// ─────────────────────────────────────────────────────────────────────────
export interface Subnet {
  n: string;
  r: string;
}

export interface CatalogoSubredes {
  dev: Subnet[];
  release: Subnet[];
  prod: Subnet[];
}

export interface EstadoSubredes {
  // "cache" | "sin_cache" | "consulta"
  origen: string;
  // Fecha ISO de la última consulta en vivo, si hubo alguna.
  actualizado: string | null;
  catalogo: CatalogoSubredes;
  detalle: string;
  // Nombre de los proyectos que fallaron. Vacío no significa que haya
  // datos: significa que ningún proyecto dio error.
  errores: string[];
  // Cuenta de gcloud con la que se consulta.
  cuenta: string | null;
}

// ─────────────────────────────────────────────────────────────────────────
// Formulario
// ─────────────────────────────────────────────────────────────────────────
export interface ApigeeEnvData {
  org: string;
  env: string;
  svc: string;
  sa: string;
}

// Campos que se calculan a partir de otro dato del formulario.
//
// Todos siguen la misma regla: mientras nadie los edite, el valor sigue a
// su origen y se recalcula cuando el origen cambia. Apenas el usuario
// escribe en el campo, deja de seguirlo para siempre.
//
// Esto corrige un problema concreto: antes la sugerencia se escribía
// apenas el campo estaba vacío, o sea con la primera letra tecleada en el
// proyecto. Si se escribía "T", quedaba "gcr-t" congelado, porque el campo
// ya tenía contenido y nunca se volvía a calcular.
export type CampoDerivado =
  // siguen al proyecto o cluster de DEV
  | "gcp.release"
  | "gcp.prod"
  | "gke.release"
  | "gke.prod"
  // siguen al proyecto GCP
  | "k8s.registryName"
  | "cr.registryName"
  | "cr.projId.dev"
  | "cr.projId.release"
  | "cr.projId.prod"
  | "cr.sa.dev"
  | "cr.sa.release"
  | "cr.sa.prod"
  // siguen a la tecnología o la infraestructura
  | "jfpath"
  | "depsFile"
  | "bbpipeImage"
  // siguen al país o a la organización de Apigee
  | "apigee.dev"
  | "apigee.release"
  | "apigee.prod";

// Devuelve el patch del formulario marcando el campo como editado a mano,
// para que deje de seguir a su origen.
export function patchTocado(
  f: CicdForm,
  patch: Partial<CicdForm>,
  clave: CampoDerivado
): Partial<CicdForm> {
  return { ...patch, tocados: { ...f.tocados, [clave]: true } };
}

export interface CicdForm {
  ticket: string;
  tech: string;
  country: string;
  destino: string;
  infra: string;
  ambientes: Record<EnvKey, boolean>;

  gcp: Record<EnvKey, string>;
  gke: Record<EnvKey, string>;
  aws: Record<EnvKey, string>;
  eks: Record<EnvKey, string>;

  reposText: string;
  apigeeSubtype: string;

  nsSame: boolean;
  nsAll: string;
  ns: Record<EnvKey, string>;

  bbKey: string;
  bbName: string;

  // Prefijo URI por repositorio. Solo guarda lo que el usuario cambió: si
  // un repo no está acá, se usa la sugerencia calculada de su nombre.
  uri: Record<string, string>;

  folder: string;
  jfpath: string;
  slack: string;

  k8sRegistryName: string;
  k8sRegistryUrl: string;
  k8sRegistryCredentials: string;
  k8sDockerfile: string;
  k8sScanCoverage: string;
  k8sScanExclude: string;
  k8sDepsFile: string;
  k8sCaDomain: string;
  k8sCaRepo: string;
  k8sAwsOwner: string;

  crRegistryName: string;
  crDockerfile: string;
  crRegion: string;
  crVersion: string;
  crScanExclude: string;
  crProjId: Record<EnvKey, string>;
  // ID de la credencial registrada en Jenkins. No se puede derivar del
  // proyecto: en Jenkins conviven credenciales con prefijo "gcp-" y sin
  // él para proyectos del mismo tipo, así que el valor se toma de ahí.
  crServiceId: Record<EnvKey, string>;
  // Cuenta de servicio de DevOps: la que está dentro del JSON guardado en
  // Jenkins, y siempre lleva "devops" en el nombre. Es distinta de la
  // cuenta de servicio del proyecto que usa el service.yaml de Kubernetes.
  crSA: Record<EnvKey, string>;

  bbpipeImage: string;

  apigeeProxyName: string;
  apigeeAppIntegration: boolean;
  apigeeBasepath: Record<EnvKey, string>;
  apigeeEnv: Record<EnvKey, ApigeeEnvData>;

  // Campos derivados que el usuario editó a mano.
  tocados: Partial<Record<CampoDerivado, boolean>>;
}

const envRecord = (v: string): Record<EnvKey, string> => ({ dev: v, release: v, prod: v });

export function formVacio(): CicdForm {
  return {
    ticket: "",
    tech: "",
    country: "CL",
    destino: "GCP",
    infra: "kubernetes",
    ambientes: { dev: true, release: true, prod: true },

    gcp: envRecord(""),
    gke: envRecord(""),
    aws: envRecord(""),
    eks: envRecord(""),

    reposText: "",
    apigeeSubtype: "",

    nsSame: true,
    nsAll: "customer-service",
    ns: envRecord("customer-service"),

    bbKey: "",
    bbName: "",
    uri: {},

    folder: "",
    jfpath: "",
    slack: "",

    k8sRegistryName: "",
    k8sRegistryUrl: "",
    k8sRegistryCredentials: "",
    k8sDockerfile: "",
    k8sScanCoverage: "",
    k8sScanExclude: "",
    k8sDepsFile: "",
    k8sCaDomain: "",
    k8sCaRepo: "",
    k8sAwsOwner: "",

    crRegistryName: "",
    crDockerfile: "",
    crRegion: "",
    crVersion: "",
    crScanExclude: "",
    crProjId: envRecord(""),
    crServiceId: envRecord(""),
    crSA: envRecord(""),

    bbpipeImage: "",

    apigeeProxyName: "",
    apigeeAppIntegration: false,
    apigeeBasepath: envRecord(""),
    apigeeEnv: {
      dev: { org: "", env: "", svc: "", sa: "" },
      release: { org: "", env: "", svc: "", sa: "" },
      prod: { org: "", env: "", svc: "", sa: "" },
    },

    tocados: {},
  };
}

// ─────────────────────────────────────────────────────────────────────────
// Datos derivados que consumen los generadores
// ─────────────────────────────────────────────────────────────────────────
export interface CicdData {
  ticket: string;
  tech: string;
  destino: string;
  infra: string;
  isK8s: boolean;
  isCR: boolean;
  isGCP: boolean;
  isAWS: boolean;
  isApigee: boolean;
  country: string;

  apigeeSubtype: string;
  apigeeProxyName: string;
  apigeeAppIntegration: boolean;
  apigeeBasepath: Record<EnvKey, string>;
  apigeeEnvData: Record<EnvKey, ApigeeEnvData>;

  envs: EnvItem[];

  gcpDev: string;
  gcpRelease: string;
  gcpProd: string;
  gkeDev: string;
  gkeRelease: string;
  gkeProd: string;
  awsDev: string;
  awsRelease: string;
  awsProd: string;
  eksDev: string;
  eksRelease: string;
  eksProd: string;

  uriPrefixes: Record<string, string>;
  repos: { name: string }[];
  ns: Record<string, string>;
  // Account de Spinnaker calculado por ambiente. Solo se llena en K8S+GCP.
  accounts: Record<string, string>;

  k8sRegistryName: string;
  k8sRegistryUrl: string;
  k8sRegistryCredentials: string;
  k8sDockerfile: string;
  k8sScanCoverage: string;
  k8sScanExclude: string;
  k8sDepsFile: string;
  k8sCodeArtifactDomain: string;
  k8sCodeArtifactRepo: string;
  k8sAwsOwner: string;

  crRegion: string;
  crVersion: string;
  crScanExclude: string;
  crRegistryName: string;
  crDockerfile: string;
  crProjId: Record<EnvKey, string>;
  crServiceId: Record<EnvKey, string>;
  crSA: Record<EnvKey, string>;

  bbpipeImage: string;
  bbKey: string;
  bbName: string;
  folder: string;
  jfpath: string;
  slack: string;
}

// ─────────────────────────────────────────────────────────────────────────
// Helpers de estado
// ─────────────────────────────────────────────────────────────────────────
export const esK8s = (f: CicdForm) => f.infra === "kubernetes";
export const esCR = (f: CicdForm) => f.infra === "cloudrun";
export const esGCP = (f: CicdForm) => f.destino === "GCP";
export const esAWS = (f: CicdForm) => f.destino === "AWS";
export const esApigee = (f: CicdForm) => f.tech === "apigeex";

// Spinnaker solo entra en Kubernetes sobre GCP. Apigee queda afuera incluso
// si alguien elige Kubernetes: su despliegue lo hace el pipeline de Jenkins.
export const necesitaSpinnaker = (f: CicdForm) => esK8s(f) && esGCP(f) && !esApigee(f);

export function ambientesActivos(f: CicdForm): EnvItem[] {
  return ENV_KEYS.filter((k) => f.ambientes[k]).map((k) => ({ key: k, label: ENV_LABEL[k] }));
}

// Una línea del textarea = un repositorio. Sin límite de cantidad: el tope
// de 5 que traía la herramienta anterior se quitó.
//
// Los nombres repetidos se eliminan. Dos líneas iguales no tienen ningún
// sentido (serían los mismos archivos escritos dos veces en el mismo
// repositorio) y además rompían la lista de prefijos URI: mientras se
// escribe hay momentos en que varias líneas coinciden, y con nombres
// repetidos la lista no podía emparejar las filas viejas con las nuevas.
export function parsearRepos(texto: string): string[] {
  const vistos = new Set<string>();
  const out: string[] = [];
  for (const linea of texto.split("\n")) {
    const nombre = linea.trim();
    if (!nombre || vistos.has(nombre)) continue;
    vistos.add(nombre);
    out.push(nombre);
  }
  return out;
}

// Cuenta solo las líneas con contenido, repetidas incluidas: sirve para
// avisar que se pasó del máximo o que hay duplicados.
export function lineasRepetidas(texto: string): string[] {
  const cuenta = new Map<string, number>();
  texto
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
    .forEach((n) => cuenta.set(n, (cuenta.get(n) || 0) + 1));
  return [...cuenta.entries()].filter(([, n]) => n > 1).map(([nombre]) => nombre);
}

// BR-GCP-VENTAS-PPFF-DEV → ventas-ppff
export function extractProjectKey(gcpProject: string): string {
  return (gcpProject || "")
    .toLowerCase()
    .replace(/^br-gcp-/, "")
    .replace(/-(dev|pp|prod|release|preprod)$/, "");
}

// br-ms-sd00130-oi00002-curse-av-sav-v4 → sd00130-curse-av-sav-v4
// Se queda con el primer código (sd/oi/b/cr/ms) y le pega el nombre sin
// ningún otro código.
export function calcURIPrefix(repoName: string): string {
  if (!repoName) return "";
  const clean = repoName.replace(/^br-[a-z]+-/, "");
  const parts = clean.split("-");
  const codePat = /^(sd|oi|b|cr|ms)\d+$/i;
  const firstCode = parts.find((p) => codePat.test(p)) || "";
  const nameParts = parts.filter((p) => !codePat.test(p));
  return firstCode
    ? firstCode + (nameParts.length ? "-" + nameParts.join("-") : "")
    : nameParts.join("-");
}

// Carpeta de los manifiestos dentro del repositorio.
export const getFolder = (d: CicdData) => (!d.isK8s ? "cloud-run" : d.isGCP ? "k8s/gcp" : "k8s/aws");

// Rama de desarrollo: Perú usa "develop", sin "ment".
export const ramaDesarrollo = (country: string) => (country === "PE" ? "develop" : "development");

export function calcApigeeJenkinsfile(subtype: string, country: string): string {
  return (APIGEE_JENKINSFILE[subtype] || {})[country] || "";
}

// Account de Spinnaker calculado. Es solo un punto de partida: la región
// varía por proyecto, no por ambiente, así que el valor real se confirma
// contra Spinnaker (eso llega en la fase de Spinnaker).
export function calcAccount(proj: string, cluster: string, env: EnvKey): string {
  const region = env === "prod" ? "southamerica-west1" : "us-east4";
  return `gke_${(proj || "").toLowerCase()}_${region}_${cluster}`;
}

// ─────────────────────────────────────────────────────────────────────────
// La imagen del registro: un solo dato, tres formas de escribirlo
//
// REGISTRY_NAME del JenkinsParams define dónde Jenkins deja la imagen, y es
// el mismo lugar donde Spinnaker la busca. De ese valor salen las tres
// formas, así que no hay manera de que queden desalineadas.
// ─────────────────────────────────────────────────────────────────────────

// Valor efectivo de REGISTRY_NAME: lo que escribió el usuario, o la
// sugerencia gcr-{proyecto} cuando el destino es GCP.
export function registryNameK8s(d: CicdData): string {
  const projKey = extractProjectKey(d.gcpDev || d.gcpRelease || d.gcpProd || "");
  return d.k8sRegistryName || (d.isGCP ? `gcr-${projKey}` : "");
}

// Como se escribe en el service.yaml de Kubernetes: gcr.io/organizacion/imagen
export function imagenServiceYaml(d: CicdData): string {
  const name = registryNameK8s(d);
  return `${GCR_HOST}/${GCR_ORG}/${name || "NOMBRE-IMAGEN"}`;
}

// Como la pide el trigger de Spinnaker: organizacion/imagen
export function imagenSpinnaker(d: CicdData): string {
  const name = registryNameK8s(d);
  return name ? `${GCR_ORG}/${name}` : "";
}

// ─────────────────────────────────────────────────────────────────────────
// Subredes
//
// Se prueban dos formas de nombre, en este orden: la estándar
// subnet1-{proyecto}-{sufijo} y la irregular br-gcp-{proyecto}-{sufijo},
// que usan unos pocos proyectos. En producción además hay dos convenciones
// de sufijo ("prod" y "prd"), y está verificado que ningún proyecto tiene
// las dos, así que probar ambas no genera ambigüedad.
// ─────────────────────────────────────────────────────────────────────────
export interface SubnetMatch {
  name: string;
  region: string;
  networkProject: string;
  network: string;
}

export function findSubnet(
  catalogo: CatalogoSubredes,
  projectKey: string,
  envKey: EnvKey
): SubnetMatch | null {
  const netEnv = envSub(envKey);
  const list = catalogo[envKey] || [];
  const suffixes = envKey === "prod" ? ["prod", "prd"] : [netEnv];
  for (const suf of suffixes) {
    const candidates = [`subnet1-${projectKey}-${suf}`, `br-gcp-${projectKey}-${suf}`];
    for (const cand of candidates) {
      const found = list.find((s) => s.n === cand);
      if (found) {
        return {
          name: found.n,
          region: found.r,
          networkProject: NETWORKING_PROJECTS[netEnv] || `br-gcp-networking-${netEnv}`,
          network: `br-shared-net-${netEnv}`,
        };
      }
    }
  }
  return null;
}

// ─────────────────────────────────────────────────────────────────────────
// Formulario → datos
// ─────────────────────────────────────────────────────────────────────────
export function collectData(f: CicdForm): CicdData {
  const envs = ambientesActivos(f);
  const repos = parsearRepos(f.reposText);

  const ns: Record<string, string> = {};
  envs.forEach((env) => {
    ns[env.key] = f.nsSame
      ? f.nsAll || "customer-service"
      : f.ns[env.key] || f.nsAll || "customer-service";
  });

  const uriPrefixes: Record<string, string> = {};
  repos.forEach((name) => {
    uriPrefixes[name] = f.uri[name] || calcURIPrefix(name) || name;
  });

  const d: CicdData = {
    ticket: f.ticket,
    tech: f.tech,
    destino: f.destino,
    infra: f.infra,
    isK8s: esK8s(f),
    isCR: esCR(f),
    isGCP: esGCP(f),
    isAWS: esAWS(f),
    isApigee: esApigee(f),
    country: f.country || "CL",

    apigeeSubtype: f.apigeeSubtype,
    apigeeProxyName: f.apigeeProxyName,
    apigeeAppIntegration: f.apigeeAppIntegration,
    apigeeBasepath: f.apigeeBasepath,
    apigeeEnvData: f.apigeeEnv,

    envs,

    gcpDev: f.gcp.dev,
    gcpRelease: f.gcp.release,
    gcpProd: f.gcp.prod,
    gkeDev: f.gke.dev,
    gkeRelease: f.gke.release,
    gkeProd: f.gke.prod,
    awsDev: f.aws.dev,
    awsRelease: f.aws.release,
    awsProd: f.aws.prod,
    eksDev: f.eks.dev,
    eksRelease: f.eks.release,
    eksProd: f.eks.prod,

    uriPrefixes,
    repos: repos.map((name) => ({ name })),
    ns,
    accounts: {},

    k8sRegistryName: f.k8sRegistryName,
    k8sRegistryUrl: f.k8sRegistryUrl,
    k8sRegistryCredentials: f.k8sRegistryCredentials,
    k8sDockerfile: f.k8sDockerfile || "Dockerfile",
    k8sScanCoverage: f.k8sScanCoverage || "coverage/**/",
    k8sScanExclude: f.k8sScanExclude,
    // Sin valor escrito se usa el archivo típico de la tecnología. .NET
    // queda vacío porque el nombre del .csproj cambia en cada proyecto.
    k8sDepsFile: f.k8sDepsFile || DEPS_FILE[f.tech] || "",
    k8sCodeArtifactDomain: f.k8sCaDomain || "br",
    k8sCodeArtifactRepo: f.k8sCaRepo || "br-devops-codeartifact-01",
    k8sAwsOwner: f.k8sAwsOwner,

    crRegion: f.crRegion || "southamerica-west1",
    crVersion: f.crVersion,
    crScanExclude: f.crScanExclude,
    crRegistryName: f.crRegistryName,
    crDockerfile: f.crDockerfile || "Dockerfile",
    crProjId: f.crProjId,
    crServiceId: f.crServiceId,
    crSA: f.crSA,

    bbpipeImage: f.bbpipeImage,
    bbKey: f.bbKey,
    bbName: f.bbName,
    folder: f.folder,
    jfpath: f.jfpath,
    slack: f.slack,
  };

  if (d.isGCP && d.isK8s) {
    envs.forEach((env) => {
      const proj = { dev: d.gcpDev, release: d.gcpRelease, prod: d.gcpProd }[env.key];
      const cluster = { dev: d.gkeDev, release: d.gkeRelease, prod: d.gkeProd }[env.key];
      d.accounts[env.key] = calcAccount(proj, cluster, env.key);
    });
  }

  return d;
}

// ─────────────────────────────────────────────────────────────────────────
// Validación por sección. Devuelve la lista de lo que falta, en el orden en
// que aparece en pantalla. Vacío = se puede generar.
// ─────────────────────────────────────────────────────────────────────────
export function validar(f: CicdForm): string[] {
  const faltas: string[] = [];
  const envs = ambientesActivos(f);
  const apigee = esApigee(f);

  if (!f.ticket.trim()) faltas.push("Ticket / Iniciativa");
  if (!f.tech) faltas.push("Lenguaje / Tecnología");
  if (!envs.length) faltas.push("Al menos un ambiente");

  if (esGCP(f)) {
    envs.forEach((e) => {
      if (!f.gcp[e.key].trim()) faltas.push(`Proyecto GCP ${e.label}`);
    });
  }
  if (esAWS(f)) {
    envs.forEach((e) => {
      if (!f.aws[e.key].trim()) faltas.push(`Proyecto AWS ${e.label}`);
    });
  }
  if (esK8s(f) && esGCP(f) && !apigee) {
    envs.forEach((e) => {
      if (!f.gke[e.key].trim()) faltas.push(`Cluster GKE ${e.label}`);
    });
  }
  if (esK8s(f) && esAWS(f) && !apigee) {
    envs.forEach((e) => {
      if (!f.eks[e.key].trim()) faltas.push(`Cluster EKS ${e.label}`);
    });
  }

  const repos = parsearRepos(f.reposText);
  if (!repos.length) faltas.push("Al menos un repositorio");
  const mayusculas = repos.filter((r) => r !== r.toLowerCase());
  if (mayusculas.length) faltas.push(`Repositorios en minúsculas: ${mayusculas.join(", ")}`);

  if (apigee) {
    if (!f.apigeeSubtype) faltas.push("Subtipo Apigee");
    if (!f.apigeeProxyName.trim()) faltas.push("APIGEE_PROXY_NAME");
    return faltas;
  }

  if (!f.bbKey.trim()) faltas.push("Project Key Bitbucket");
  if (!f.bbName.trim()) faltas.push("Project Name Bitbucket");
  if (!f.folder.trim()) faltas.push("Foldername en Jenkins");
  if (!f.jfpath.trim()) faltas.push("Jenkinsfile path");

  return faltas;
}

// Mismo patrón de parseo de errores que usan Clusters y Bitbucket, para que
// un error de Rust se lea igual en todo el hub.
export function parseErr(err: unknown): string {
  const e = err as RefreshError;
  return e && e.etapa && e.mensaje ? `[${e.etapa}] ${e.mensaje}` : String(err);
}

// ─────────────────────────────────────────────────────────────────────────
// Derivación del valor de los otros ambientes a partir de DEV
//
// El proyecto y el cluster suelen ser el mismo nombre con el ambiente
// cambiado, así que escribir DEV alcanza para los tres. Las dos formas son
// distintas y por eso hay dos funciones:
//
//   proyecto → el ambiente está al final:  BR-GCP-VENTAS-PPFF-DEV
//   cluster  → el ambiente va en el medio: gke-br-dev-core-ventas-ppff-01
//
// Si el valor de DEV no tiene la forma esperada se devuelve cadena vacía y
// no se completa nada: preferible dejar el campo en blanco a inventar un
// nombre que no existe.
// ─────────────────────────────────────────────────────────────────────────
const SUFIJO_ENV: Record<EnvKey, string> = { dev: "dev", release: "pp", prod: "prod" };

// Conserva el formato del original: si venía en mayúsculas, el reemplazo
// también va en mayúsculas.
function igualarCaja(modelo: string, texto: string): string {
  return modelo === modelo.toUpperCase() ? texto.toUpperCase() : texto;
}

export function derivarProyecto(proyectoDev: string, env: EnvKey): string {
  const valor = proyectoDev.trim();
  if (!valor || env === "dev") return "";
  const m = valor.match(/^(.*)-(dev)$/i);
  if (!m) return "";
  return `${m[1]}-${igualarCaja(m[2], SUFIJO_ENV[env])}`;
}

export function derivarCluster(clusterDev: string, env: EnvKey): string {
  const valor = clusterDev.trim();
  if (!valor || env === "dev") return "";
  // Solo el primer segmento que sea exactamente "dev" entre guiones: así
  // un nombre que además contenga "dev" en otra parte (por ejemplo
  // "devops") no se toca.
  const m = valor.match(/^(.*?-)(dev)(-.*)$/i);
  if (!m) return "";
  return `${m[1]}${igualarCaja(m[2], SUFIJO_ENV[env])}${m[3]}`;
}

// ─────────────────────────────────────────────────────────────────────────
// Sugerencias automáticas
//
// Equivale a lo que hacía la herramienta HTML cuando entrabas a cada paso:
// precompletar los campos que se pueden deducir del proyecto, la tecnología
// o el país. La regla es la misma: SOLO se escribe en campos vacíos, nunca
// se pisa algo escrito a mano.
//
// Devuelve el MISMO objeto cuando no hay nada que sugerir. Eso importa:
// se llama desde un efecto en cada cambio del formulario, y devolver una
// copia nueva siempre dejaría a React renderizando sin parar.
// ─────────────────────────────────────────────────────────────────────────
export function aplicarSugerencias(f: CicdForm): CicdForm {
  const borrador: CicdForm = {
    ...f,
    gcp: { ...f.gcp },
    gke: { ...f.gke },
    crProjId: { ...f.crProjId },
    crSA: { ...f.crSA },
    apigeeEnv: {
      dev: { ...f.apigeeEnv.dev },
      release: { ...f.apigeeEnv.release },
      prod: { ...f.apigeeEnv.prod },
    },
  };
  let cambio = false;

  // Escribe la sugerencia solo si el campo no fue editado a mano. Si el
  // origen cambia, el valor se recalcula: terminar de escribir el nombre
  // del proyecto corrige todos los derivados de una vez.
  const seguir = (clave: CampoDerivado, actual: string, sugerido: string, aplicar: (v: string) => void) => {
    if (f.tocados[clave]) return;
    if (!sugerido || sugerido === actual) return;
    aplicar(sugerido);
    cambio = true;
  };

  // Campos sin origen variable: valor por defecto una sola vez, cuando
  // están vacíos. No hace falta que sigan a nada.
  const porDefecto = (actual: string, valor: string, aplicar: (v: string) => void) => {
    if (actual || !valor) return;
    aplicar(valor);
    cambio = true;
  };

  const apigee = esApigee(f);
  const projKey = extractProjectKey(f.gcp.dev || f.gcp.release || f.gcp.prod || "");

  // ── Proyecto y cluster de RELEASE y PROD siguen a DEV ──
  (["release", "prod"] as EnvKey[]).forEach((env) => {
    seguir(`gcp.${env}` as CampoDerivado, f.gcp[env], derivarProyecto(f.gcp.dev, env), (v) => {
      borrador.gcp[env] = v;
    });
    seguir(`gke.${env}` as CampoDerivado, f.gke[env], derivarCluster(f.gke.dev, env), (v) => {
      borrador.gke[env] = v;
    });
  });

  // ── Jenkinsfile: Cloud Run usa siempre el mismo; en Kubernetes solo se
  //    sugiere cuando la tecnología tiene una única plantilla ──
  if (!apigee) {
    const opts = JF_K8S[f.tech] || [];
    const validos = esCR(f) ? [JF_CR] : opts.map((o) => o.p);
    const sugerido = esCR(f) ? JF_CR : esK8s(f) && opts.length === 1 ? opts[0].p : "";
    seguir("jfpath", f.jfpath, sugerido, (v) => {
      borrador.jfpath = v;
    });
    // Node.js tiene seis plantillas, así que no hay sugerencia. Pero si
    // venía elegida la de otra tecnología, dejarla sería peor que vaciar:
    // el pipeline se crearía con la plantilla equivocada.
    if (!f.tocados.jfpath && !sugerido && f.jfpath && !validos.includes(f.jfpath)) {
      borrador.jfpath = "";
      cambio = true;
    }
  }

  seguir("bbpipeImage", f.bbpipeImage, BBPIPE_IMAGE[f.tech] || "", (v) => {
    borrador.bbpipeImage = v;
  });

  if (esK8s(f) && !apigee) {
    seguir("k8s.registryName", f.k8sRegistryName, esGCP(f) && projKey ? `gcr-${projKey}` : "", (v) => {
      borrador.k8sRegistryName = v;
    });
    seguir("depsFile", f.k8sDepsFile, DEPS_FILE[f.tech] || "", (v) => {
      borrador.k8sDepsFile = v;
    });
    porDefecto(f.k8sDockerfile, "Dockerfile", (v) => {
      borrador.k8sDockerfile = v;
    });
    porDefecto(f.k8sScanCoverage, "coverage/**/", (v) => {
      borrador.k8sScanCoverage = v;
    });
    porDefecto(f.k8sCaDomain, "br", (v) => {
      borrador.k8sCaDomain = v;
    });
    porDefecto(f.k8sCaRepo, "br-devops-codeartifact-01", (v) => {
      borrador.k8sCaRepo = v;
    });
    if (esAWS(f)) {
      porDefecto(f.k8sAwsOwner, "063003577365", (v) => {
        borrador.k8sAwsOwner = v;
      });
    }
  }

  if (esCR(f)) {
    seguir("cr.registryName", f.crRegistryName, projKey ? `gcr-${projKey}` : "", (v) => {
      borrador.crRegistryName = v;
    });
    porDefecto(f.crDockerfile, "Dockerfile", (v) => {
      borrador.crDockerfile = v;
    });
    porDefecto(f.crRegion, "southamerica-west1", (v) => {
      borrador.crRegion = v;
    });
    porDefecto(f.crVersion, "1.0.0", (v) => {
      borrador.crVersion = v;
    });

    // El Project ID y el Service ID son el mismo valor: el proyecto GCP en
    // minúsculas. La cuenta de servicio sigue el patrón del equipo.
    ambientesActivos(f).forEach((env) => {
      const proyecto = (borrador.gcp[env.key] || "").toLowerCase();
      if (!proyecto) return;
      seguir(`cr.projId.${env.key}` as CampoDerivado, f.crProjId[env.key], proyecto, (v) => {
        borrador.crProjId[env.key] = v;
      });
      // SERVICE_ACCOUNT_NAME y SERVICE_ID no se sugieren: ambos salen de
      // la credencial registrada en Jenkins y no siguen ningún patrón
      // derivable del proyecto. El patrón que había antes generaba un
      // correo inexistente y el pipeline fallaba al autenticar.
    });
  }

  if (apigee) {
    // Chile tiene valores fijos confirmados y siguen al país: cambiar de
    // país recalcula los tres. Perú todavía no tiene datos, así que sus
    // campos quedan vacíos y se completan a mano.
    ENV_KEYS.forEach((k) => {
      const fijo = f.country === "CL" ? APIGEE_FIXED_CL[k] : undefined;
      const actual = f.apigeeEnv[k];
      const sa =
        f.apigeeAppIntegration && (fijo?.org || actual.org)
          ? `sa-application-integration-inv@${fijo?.org || actual.org}.iam.gserviceaccount.com`
          : "";
      const sugerido = JSON.stringify({
        org: fijo?.org || actual.org,
        env: fijo?.env || actual.env,
        svc: fijo?.svc || actual.svc,
        sa: sa || actual.sa,
      });
      seguir(`apigee.${k}` as CampoDerivado, JSON.stringify(actual), sugerido, (v) => {
        borrador.apigeeEnv[k] = JSON.parse(v);
      });
    });
  }

  return cambio ? borrador : f;
}

// ─────────────────────────────────────────────────────────────────────────
// Subida a Bitbucket — tipos espejo de lo que define Rust
// ─────────────────────────────────────────────────────────────────────────

// El workspace no cambia: el equipo trabaja siempre sobre el mismo. Es el
// mismo valor que usa el módulo de creación de repositorios.
export const WORKSPACE = "banco_ripley";

export type Accion = "sobrescribir" | "renombrar" | "omitir";

export interface ArchivoRepo {
  ruta: string;
  contenido: string;
  accion: Accion;
}

export interface RepoArchivos {
  repo: string;
  archivos: ArchivoRepo[];
}

export interface VerificacionRepo {
  repo: string;
  repoExiste: boolean;
  ramaExiste: boolean;
  conflictos: string[];
  error: string | null;
}

export interface ResultadoRepo {
  repo: string;
  ok: boolean;
  url: string | null;
  subidos: string[];
  // Pares (ruta pedida, ruta efectiva) de los archivos renombrados.
  renombrados: [string, string][];
  omitidos: string[];
  error: string | null;
}

// Mensaje de commit, sin tildes: es el mismo texto que usaba el flujo
// anterior, y así el historial de los repositorios queda consistente.
export function mensajeCommit(ticket: string): string {
  return `feat: agregar configuracion CI/CD [${ticket.trim() || "SIN-TICKET"}]`;
}

// Cuántos campos trae completados la herramienta sin intervención del
// usuario. Sirve para el rótulo de la sección: informar que hay valores
// sugeridos es honesto, decir "completa" sin que nadie los haya mirado no.
export function contarSugeridos(f: CicdForm): number {
  const derivados: [CampoDerivado, string][] = [
    ["gcp.release", f.gcp.release],
    ["gcp.prod", f.gcp.prod],
    ["gke.release", f.gke.release],
    ["gke.prod", f.gke.prod],
    ["jfpath", f.jfpath],
    ["depsFile", f.k8sDepsFile],
    ["bbpipeImage", f.bbpipeImage],
    ["k8s.registryName", f.k8sRegistryName],
    ["cr.registryName", f.crRegistryName],
    ["cr.projId.dev", f.crProjId.dev],
    ["cr.projId.release", f.crProjId.release],
    ["cr.projId.prod", f.crProjId.prod],
    ["apigee.dev", f.apigeeEnv.dev.org],
    ["apigee.release", f.apigeeEnv.release.org],
    ["apigee.prod", f.apigeeEnv.prod.org],
  ];
  return derivados.filter(([clave, valor]) => valor && !f.tocados[clave]).length;
}
