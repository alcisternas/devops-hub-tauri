// Tipos y helpers del módulo Bitbucket.
//
// Los tipos son el espejo exacto de lo que devuelve Rust. Ojo con la
// diferencia de estilo entre unos y otros: CredStatus llega en snake_case
// y ResumenRun en camelCase, porque en Rust solo el segundo lleva la
// anotación que renombra los campos. Están escritos tal como llegan, no
// como "deberían" verse.

import type { RefreshError } from "../clusters/shared";

// ─────────────────────────────────────────────────────────────────────────
// Lo que devuelve Rust
// ─────────────────────────────────────────────────────────────────────────

export interface CredStatus {
  almacen_disponible: boolean;
  guardadas: boolean;
  email: string | null;
  validas: boolean;
  detalle: string;
}

export interface SonarCredStatus {
  almacen_disponible: boolean;
  guardadas: boolean;
  validas: boolean;
  detalle: string;
  organizacion: string | null;
}

export interface ResultadoRepo {
  repo: string;
  estado: "creado" | "ya_existia" | "error";
  url: string | null;
  detalle: string;
  advertencias: string[];
  sonarKey: string;
  // null cuando no se pidió crear el proyecto en SonarQube.
  sonarEstado: "creado" | "ya_existia" | "error" | null;
  sonarDetalle: string | null;
  sonarBindeado: boolean;
}

export interface ResumenRun {
  total: number;
  creados: number;
  omitidos: number;
  conError: number;
  ramaDesarrollo: string;
  sonarOk: number;
  sonarPedido: boolean;
  resultados: ResultadoRepo[];
}

export interface ConfigRun {
  workspace: string;
  projectKey: string;
  repos: string[];
  pais: string;
  activarPipelines: boolean;
  crearRamas: boolean;
  crearSonar: boolean;
}

// ─────────────────────────────────────────────────────────────────────────
// Jira — modo "Desde ticket"
//
// JiraCredStatus llega en snake_case (igual que SonarCredStatus: en Rust no
// lleva rename_all). El resto lleva rename_all y llega en camelCase.
// ─────────────────────────────────────────────────────────────────────────

export interface JiraCredStatus {
  almacen_disponible: boolean;
  guardadas: boolean;
  validas: boolean;
  detalle: string;
}

export interface TicketPendiente {
  clave: string;
  resumen: string;
  estado: string;
  etiquetas: string[];
  proyecto: string | null;
  projectKey: string | null;
  projectName: string | null;
  repos: string[];
  tipo: string | null;
  pais: string | null;
  lenguaje: string | null;
  esBd: boolean;
  // Motivos por los que no se puede procesar. Vacío = válido.
  errores: string[];
}

export interface EvidenciaRepo {
  nombre: string;
  url: string;
  ramas: string[];
}

export interface OpcionesProceso {
  workspace: string;
  sonar: boolean;
  etiquetas: boolean;
  cerrar: boolean;
}

export interface ResultadoTicket {
  clave: string;
  esBd: boolean;
  resumen: ResumenRun;
  accionesJira: boolean;
  motivoSinAcciones: string | null;
  etiquetasAgregadas: string[];
  etiquetasError: string | null;
  cierreEstado: "no_pedido" | "cerrado" | "error" | "omitido";
  cierreDetalle: string;
  proyectoEtiqueta: string;
  ramas: string[];
  evidencia: EvidenciaRepo[];
}

export const URL_JIRA = "https://bancoripley1.atlassian.net";

// Mismo patrón de parseo que usa Clusters, para que un error de Rust se
// muestre igual en todo el hub.
export function parseErr(err: unknown): string {
  const e = err as RefreshError;
  return e && e.etapa && e.mensaje ? `[${e.etapa}] ${e.mensaje}` : String(err);
}

// ─────────────────────────────────────────────────────────────────────────
// Validación del nombre de repositorio
//
// Patrón estándar: br-[proyecto]-sd#####-oi#####
//   sd = número de iniciativa · oi = objeto de inversión del ITSM
//
// Las reglas vienen tal cual de la herramienta HTML anterior. Es una
// ADVERTENCIA, no un bloqueo: si algún día el estándar cambia o aparece una
// excepción legítima, el usuario puede crear igual. Por eso la confirmación
// avisa cuántos no cumplen en vez de impedir continuar.
// ─────────────────────────────────────────────────────────────────────────

export interface Validacion {
  ok: boolean;
  msg: string;
}

export function validarRepo(nombre: string): Validacion {
  if (!nombre) return { ok: false, msg: "" };
  if (nombre !== nombre.toLowerCase()) {
    return { ok: false, msg: "Debe estar en minúsculas" };
  }
  if (!nombre.startsWith("br-")) {
    return { ok: false, msg: 'Debe comenzar con "br-"' };
  }

  const partes = nombre.split("-");
  if (partes.length < 4) {
    return { ok: false, msg: "Formato incompleto — br-[proyecto]-sd#####-oi#####" };
  }
  if (!partes.some((p) => /^sd\d+$/.test(p))) {
    return { ok: false, msg: "Falta el código SD (ej: sd00295)" };
  }
  if (!partes.some((p) => /^oi\d+$/.test(p))) {
    return { ok: false, msg: "Falta el código OI (ej: oi00026)" };
  }

  return { ok: true, msg: "" };
}

// Una línea del textarea = un repositorio. Se normaliza a minúsculas y se
// descartan las líneas vacías, para que sobre una línea en blanco al final
// no se intente crear un repositorio sin nombre.
export function parsearRepos(texto: string): string[] {
  return texto
    .split("\n")
    .map((l) => l.trim().toLowerCase())
    .filter((l) => l.length > 0);
}

// Chile usa "development" (con T) y Perú usa "develop" (sin T). Esto es
// solo para MOSTRAR la rama antes de ejecutar: la que manda al crear es la
// que decide Rust, para que la regla tenga una sola fuente de verdad.
export function ramaDesarrolloPreview(pais: string): string {
  return pais.toUpperCase() === "PE" ? "develop" : "development";
}
