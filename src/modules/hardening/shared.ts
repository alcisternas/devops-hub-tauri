// Tipos y helpers del módulo Hardening.
//
// Los tipos son el espejo de lo que devuelve Rust. Todos los structs de este
// módulo llegan en snake_case (llevan los mismos nombres que el motor de
// referencia, Container Hardening Studio); solo los argumentos de los comandos
// van en camelCase, como en el resto del hub.

import type { RefreshError } from "../clusters/shared";

// ─────────────────────────────────────────────────────────────────────────
// Herramientas
// ─────────────────────────────────────────────────────────────────────────

export interface Herramienta {
  instalada: boolean;
  version: string | null;
}

export interface Herramientas {
  trivy: Herramienta;
  scout: Herramienta;
  docker: Herramienta;
}

// ─────────────────────────────────────────────────────────────────────────
// Configuración (lo que elige el usuario)
// ─────────────────────────────────────────────────────────────────────────

export interface ConfigHardening {
  ecosystem: string;
  profile: string;
  // id de componente -> true: se conserva (o instala) en la imagen final
  keep: Record<string, boolean>;
  runtime_only: boolean;
  non_root: boolean;
  allow_install_deps: boolean;
  run_user: string;
  run_uid: number;
  workdir: string;
  max_attempts: number;
  extra_apk: string[];
  preserve_paths: string[];
  authorize_remove_components: string[];
  allow_npm_upgrade: boolean;
  authorize_base_change: boolean;
  authorize_runtime_change: boolean;
  builder_base_override: string | null;
  network_tests: boolean | null;
}

export interface Perfil {
  nombre: string;
  descripcion: string;
  config: ConfigHardening;
}

// ─────────────────────────────────────────────────────────────────────────
// Análisis
// ─────────────────────────────────────────────────────────────────────────

export type Severidad = "CRITICAL" | "HIGH" | "MEDIUM" | "LOW" | "UNKNOWN";

export interface ImagenResuelta {
  reference: string;
  registry: string;
  repository: string;
  tag: string | null;
  platform: string;
  available_platforms: string[];
  index_digest: string | null;
  manifest_digest: string;
  pinned_ref: string;
  created: string | null;
  os: string | null;
  architecture: string | null;
  size_bytes: number;
  layers: number;
  env: Record<string, string>;
  user: string;
  entrypoint: string[] | null;
  cmd: string[] | null;
  workdir: string;
  labels: Record<string, string>;
}

export interface InfoEscaner {
  name: string;
  version: string | null;
  vulnerability_db: Record<string, unknown> | null;
  args: string[];
  ignore_unfixed: boolean;
}

export interface SistemaOperativo {
  family: string | null;
  name: string | null;
  eosl: boolean;
}

export interface HallazgoVista {
  id: string;
  severity: Severidad;
  pkg_name: string;
  installed_version: string;
  fixed_version: string | null;
  class: string;
  pkg_path: string;
  // componente del catálogo al que pertenece (npm, shell, os-base...)
  owner: string;
  title: string;
}

export interface ComponenteDetectado {
  id: string;
  label: string;
  description: string;
  group: "node" | "system" | "security" | string;
  present: boolean;
  installable: boolean;
  version: string | null;
  packages: number;
  high_critical: number;
  unfixed_high_critical: number;
  vulnerabilities: number;
  requires: string[];
}

export interface Detectados {
  components: ComponenteDetectado[];
  other_os_packages: { name: string; version: string }[];
  other_language_locations: { path: string; packages: number }[];
  derived_image_warning: boolean;
}

export interface Soporte {
  generador: boolean;
  motivo: string | null;
  componentes_tipicos: string[];
}

// ─────────────────────────────────────────────────────────────────────────
// Evaluación (impacto + plan)
// ─────────────────────────────────────────────────────────────────────────

export interface Problema {
  level: "error" | "warning";
  component: string;
  message: string;
}

export interface Efecto {
  component: string;
  label: string;
  kept: boolean;
  messages: string[];
}

export interface Corregible {
  package: string;
  version: string;
  cves: string[];
  owner: string;
}

export interface SinCorreccion extends Corregible {
  path: string;
}

export type Veredicto = "invalid" | "likely_blocked" | "needs_remediation" | "candidate";

export interface Analisis {
  verdict: Veredicto;
  message: string;
  issues: Problema[];
  effects: Efecto[];
  vulnerabilities_removed: { id: string; severity: string; package: string }[];
  remaining_high_critical: number;
  fixable: Corregible[];
  unfixable: SinCorreccion[];
  normalized: ConfigHardening;
}

export interface Alternativa {
  kind: string;
  description: string;
  requires_authorization: boolean;
}

export interface Bloqueo {
  package: string;
  version: string;
  path: string;
  owner: string;
  cves: string[];
  severity: string;
  reason: string;
  alternatives: Alternativa[];
}

export interface Estrategia {
  kind: string;
  title: string;
  detail: string;
  confidence: "alta" | "media" | "baja" | string;
  addresses: string[];
  keeps_required_versions: boolean;
  requires_authorization: boolean;
}

export interface Plan {
  attempt: number;
  apk_packages: string[];
  apk_pins: Record<string, string>;
  npm_patches: { name: string; fixes_by_major: Record<string, string>; cves: string[]; cross_major: boolean }[];
  npm_upgrade: string | null;
  removed_components: string[];
  strategies: Estrategia[];
  blockers: Bloqueo[];
}

export interface Recomendacion {
  id: string;
  kind: string;
  title: string;
  confidence: string;
  resolves: string[];
  changes: string[];
  affected: string[];
  pros: string[];
  cons: string[];
  incompatibilities: string[];
  keeps_required_versions: boolean;
  requires_authorization: boolean;
}

export interface Evaluacion {
  adaptada: ConfigHardening;
  // componentes pedidos que no existen en la imagen origen y se descartaron
  descartados: string[];
  analisis: Analisis;
  plan: Plan;
  recomendaciones: Recomendacion[];
}

export interface ResultadoAnalisis {
  id_sesion: string;
  imagen: ImagenResuelta;
  scanner: InfoEscaner;
  os: SistemaOperativo;
  counts: Record<string, number>;
  ecosistema: string;
  hallazgos: HallazgoVista[];
  detectados: Detectados;
  soporte: Soporte;
  perfil_inicial: string;
  config_inicial: ConfigHardening;
  evaluacion: Evaluacion;
}

// ─────────────────────────────────────────────────────────────────────────
// Generación
// ─────────────────────────────────────────────────────────────────────────

export interface PasoComando {
  titulo: string;
  comando: string;
}

export interface DocDockerfile {
  text: string;
  explanations: { start: number; end: number; text: string }[];
  files: Record<string, string>;
  expected_tests: { id: string; title: string; required: boolean }[];
  sha256: string;
  meta: {
    src_ref: string;
    builder_ref: string;
    alpine: string;
    node_version: string;
    effective_keep: Record<string, boolean>;
    removed_by_authorization: string[];
  };
}

export interface DestinoEcr {
  cuenta: string;
  region: string;
  repositorio: string;
  // vacío: se propone <nombre>-<tag original>-sec
  tag: string;
}

export interface ResultadoGeneracion {
  doc: DocDockerfile;
  builder_ref: string;
  etiqueta_sugerida: string;
  // destino ya resuelto, con el tag completado y validado
  destino_ecr: DestinoEcr;
  pasos: PasoComando[];
  evaluacion: Evaluacion;
}

// ─────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────

export const PLATAFORMAS = ["linux/amd64", "linux/arm64"] as const;

export const URL_REGISTRO_NPM = "https://registry.npmjs.org/";

// Valores del procedimiento del equipo para subir imágenes securitizadas
// (cuenta CORE-AWS-SHARED-SERVICES-CL); editables en pantalla.
export const DESTINO_ECR_INICIAL: DestinoEcr = {
  cuenta: "841964127262",
  region: "us-east-1",
  repositorio: "ecr-base-image",
  tag: "",
};

export const SEVERIDADES: Severidad[] = ["CRITICAL", "HIGH", "MEDIUM", "LOW", "UNKNOWN"];

export const NOMBRE_SEVERIDAD: Record<Severidad, string> = {
  CRITICAL: "Críticas",
  HIGH: "Altas",
  MEDIUM: "Medias",
  LOW: "Bajas",
  UNKNOWN: "Desconocidas",
};

// Mismo patrón de parseo que usan Clusters y Bitbucket.
export function parseErr(err: unknown): string {
  const e = err as RefreshError;
  return e && e.etapa && e.mensaje ? `[${e.etapa}] ${e.mensaje}` : String(err);
}

export function tamano(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

// Un dígito corto para mostrar el digest sin ocupar la línea entera.
export function digestCorto(digest: string): string {
  return digest.startsWith("sha256:") ? `sha256:${digest.slice(7, 19)}…` : digest;
}

// Perfil mostrado cuando la selección ya no coincide con ninguno de los incluidos.
export const PERFIL_PERSONALIZADO = "Personalizado";

export const ESTADO_VEREDICTO: Record<Veredicto, { texto: string; color: string }> = {
  invalid: { texto: "Configuración con conflictos", color: "var(--fail)" },
  likely_blocked: { texto: "Probable bloqueo por la política", color: "var(--fail)" },
  needs_remediation: { texto: "Requiere corrección y verificación", color: "var(--partial)" },
  candidate: { texto: "Candidata a cumplir la política", color: "var(--ok)" },
};

// «Imagen solo runtime» no admite estos componentes: si se activa uno, se
// desactiva esa opción en vez de dejar al usuario con un conflicto.
const NO_RUNTIME = ["build_tools", "python", "git", "pkg_manager", "yarn", "pnpm"];

// Cambia un componente respetando sus dependencias: activar uno activa lo que
// necesita; desactivar uno desactiva lo que depende de él.
export function alternarComponente(config: ConfigHardening, comps: ComponenteDetectado[], id: string, valor: boolean): ConfigHardening {
  const keep = { ...config.keep };
  const requiere = (cid: string) => comps.find((c) => c.id === cid)?.requires ?? [];
  const tocados = new Set<string>();

  if (valor) {
    const pila = [id];
    while (pila.length) {
      const c = pila.pop()!;
      if (tocados.has(c)) continue;
      tocados.add(c);
      keep[c] = true;
      pila.push(...requiere(c));
    }
  } else {
    const pila = [id];
    while (pila.length) {
      const c = pila.pop()!;
      if (tocados.has(c)) continue;
      tocados.add(c);
      keep[c] = false;
      pila.push(...comps.filter((x) => x.requires.includes(c)).map((x) => x.id));
    }
  }

  const next: ConfigHardening = { ...config, keep, profile: PERFIL_PERSONALIZADO };
  if (valor && [...tocados].some((c) => NO_RUNTIME.includes(c))) next.runtime_only = false;
  if (!keep.npm) next.allow_install_deps = false;
  return next;
}
