import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, ErrorNotice, MetaLine } from "../../components/ui";
import ResultsPanel from "./ResultsPanel";
import { dibujarEvidenciaPng } from "./evidencia";
import {
  URL_JIRA,
  parseErr,
  type OpcionesProceso,
  type ResultadoTicket,
  type TicketPendiente,
} from "./shared";

interface Props {
  workspace: string;
  jiraOk: boolean;
  bitbucketOk: boolean;
  // La casilla de SonarQube es la misma del modo manual: así el panel del
  // token de Sonar se monta o no con una sola regla.
  crearSonar: boolean;
  onCrearSonar: (valor: boolean) => void;
  sonarOk: boolean;
  // Limpia la consola global antes de cada corrida.
  onInicio: () => void;
}

interface EstadoEvidencia {
  estado: "publicada" | "error" | "omitida" | "no_pedida";
  detalle: string;
}

const CAJA = { borderColor: "var(--border)", background: "var(--surface)" };

function Casilla({
  marcada,
  onCambio,
  deshabilitada,
  titulo,
  detalle,
}: {
  marcada: boolean;
  onCambio: (v: boolean) => void;
  deshabilitada?: boolean;
  titulo: string;
  detalle: string;
}) {
  return (
    <label className="flex items-start gap-2 cursor-pointer">
      <input
        type="checkbox"
        checked={marcada}
        onChange={(e) => onCambio(e.target.checked)}
        disabled={deshabilitada}
        className="w-4 h-4 mt-0.5 cursor-pointer"
        style={{ accentColor: "var(--accent)" }}
      />
      <span>
        <span className="text-sm" style={{ color: "var(--text)" }}>
          {titulo}
        </span>
        <span className="block text-xs" style={{ color: "var(--text-faint)" }}>
          {detalle}
        </span>
      </span>
    </label>
  );
}

export default function TicketsPanel({
  workspace,
  jiraOk,
  bitbucketOk,
  crearSonar,
  onCrearSonar,
  sonarOk,
  onInicio,
}: Props) {
  const [tickets, setTickets] = useState<TicketPendiente[] | null>(null);
  const [buscando, setBuscando] = useState(false);
  const [errorLista, setErrorLista] = useState("");

  const [elegido, setElegido] = useState<TicketPendiente | null>(null);
  const [etiquetas, setEtiquetas] = useState(true);
  const [cerrar, setCerrar] = useState(true);
  const [comentar, setComentar] = useState(true);

  const [ejecutando, setEjecutando] = useState(false);
  const [resultado, setResultado] = useState<ResultadoTicket | null>(null);
  const [evidencia, setEvidencia] = useState<EstadoEvidencia | null>(null);
  const [errorProceso, setErrorProceso] = useState("");

  async function buscar() {
    setBuscando(true);
    setErrorLista("");
    setElegido(null);
    setResultado(null);
    setEvidencia(null);
    setErrorProceso("");
    try {
      setTickets(await invoke<TicketPendiente[]>("jira_tickets_pendientes"));
    } catch (e) {
      setTickets(null);
      setErrorLista(parseErr(e));
    } finally {
      setBuscando(false);
    }
  }

  // Repos de Base de Datos: sin Pipelines ni SonarQube, sin excepción.
  const usaSonar = crearSonar && !!elegido && !elegido.esBd;
  const puedeProcesar = jiraOk && bitbucketOk && (!usaSonar || sonarOk);

  async function procesar() {
    if (!elegido) return;
    setEjecutando(true);
    setErrorProceso("");
    setResultado(null);
    setEvidencia(null);
    onInicio();

    try {
      const opciones: OpcionesProceso = { workspace, sonar: usaSonar, etiquetas, cerrar };
      const r = await invoke<ResultadoTicket>("jira_procesar_ticket", {
        clave: elegido.clave,
        opciones,
      });
      setResultado(r);

      // El ticket ya no está pendiente en cuanto se crearon sus repos: se
      // saca de la lista para no procesarlo dos veces.
      if (r.resumen.creados > 0) {
        setTickets((prev) => (prev ? prev.filter((t) => t.clave !== r.clave) : prev));
      }
      setElegido(null);

      if (!comentar) {
        setEvidencia({ estado: "no_pedida", detalle: "No se pidió publicar el comentario." });
      } else if (!r.accionesJira || r.evidencia.length === 0) {
        setEvidencia({
          estado: "omitida",
          detalle: "No se publicó: el ticket no quedó procesado por completo.",
        });
      } else {
        try {
          // Si la imagen no se puede dibujar se publica igual el texto con las
          // URLs, que es lo que el solicitante necesita; se avisa en pantalla.
          let imagen: string | null = null;
          try {
            imagen = dibujarEvidenciaPng(r.evidencia);
          } catch {
            imagen = null;
          }
          await invoke("jira_publicar_evidencia", {
            peticion: {
              clave: r.clave,
              proyectoEtiqueta: r.proyectoEtiqueta,
              ramas: r.ramas,
              evidencia: r.evidencia,
              imagenPngBase64: imagen,
            },
          });
          setEvidencia({
            estado: "publicada",
            detalle: imagen
              ? "Respuesta publicada al solicitante, con la imagen de repositorios y ramas adjunta."
              : "Respuesta publicada al solicitante, pero sin imagen: no se pudo dibujar.",
          });
        } catch (e) {
          setEvidencia({ estado: "error", detalle: parseErr(e) });
        }
      }
    } catch (e) {
      setErrorProceso(parseErr(e));
    } finally {
      setEjecutando(false);
    }
  }

  const listo = jiraOk && bitbucketOk;

  return (
    <div className="space-y-3">
      <div
        className="p-4 rounded-lg border space-y-3"
        style={CAJA}
      >
        <div className="text-sm" style={{ color: "var(--text)" }}>
          Procesa los tickets de alta de repositorios asignados a la cuenta del token: lee el
          formulario, crea los repositorios y deja el ticket listo.
        </div>
        <div className="flex items-center gap-3">
          <Button onClick={buscar} disabled={!listo || buscando || ejecutando}>
            {buscando ? "Buscando…" : "Buscar tickets pendientes"}
          </Button>
          {!listo && (
            <span className="text-xs" style={{ color: "var(--text-muted)" }}>
              {!bitbucketOk && !jiraOk
                ? "Primero hay que validar las credenciales de Bitbucket y de Jira."
                : !bitbucketOk
                  ? "Primero hay que validar las credenciales de Bitbucket."
                  : "Primero hay que validar el token de Jira."}
            </span>
          )}
        </div>
        {errorLista && <ErrorNotice>{errorLista}</ErrorNotice>}
      </div>

      {tickets && tickets.length === 0 && !resultado && (
        <div className="p-4 rounded-lg border" style={CAJA}>
          <MetaLine>
            No hay tickets pendientes: ninguno asignado a esta cuenta en estado Asignado o Trabajo
            en curso.
          </MetaLine>
        </div>
      )}

      {tickets && tickets.length > 0 && (
        <div className="space-y-2">
          {tickets.map((t) => {
            const seleccionado = elegido?.clave === t.clave;
            return (
              <div
                key={t.clave}
                className="p-3 rounded-lg border space-y-2"
                style={{
                  borderColor: seleccionado ? "var(--accent)" : "var(--border)",
                  background: "var(--surface)",
                }}
              >
                <div className="flex items-center gap-2 flex-wrap">
                  <button
                    onClick={() => openUrl(`${URL_JIRA}/browse/${t.clave}`)}
                    className="text-sm underline"
                    style={{ color: "var(--accent)", fontFamily: "var(--font-mono)" }}
                    title="Abrir el ticket en Jira"
                  >
                    {t.clave}
                  </button>
                  <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                    {t.estado}
                  </span>
                  {t.esBd && (
                    <span
                      className="text-xs px-2 py-0.5 rounded-sm"
                      style={{ background: "var(--partial-dim)", color: "var(--partial)" }}
                    >
                      Base de Datos — sin Pipelines ni SonarQube
                    </span>
                  )}
                  <div className="ml-auto">
                    <Button
                      variant="accent-outline"
                      className="text-xs px-2.5 py-1"
                      disabled={t.errores.length > 0 || ejecutando}
                      onClick={() => {
                        setElegido(t);
                        setResultado(null);
                        setEvidencia(null);
                        setErrorProceso("");
                      }}
                    >
                      Revisar y procesar
                    </Button>
                  </div>
                </div>

                <div className="text-xs" style={{ color: "var(--text-muted)" }}>
                  {t.resumen}
                </div>

                <div className="flex items-center gap-4 flex-wrap text-xs" style={{ color: "var(--text)" }}>
                  <span>
                    <span style={{ color: "var(--text-faint)" }}>Proyecto </span>
                    <span style={{ fontFamily: "var(--font-mono)" }}>{t.proyecto ?? "—"}</span>
                  </span>
                  <span>
                    <span style={{ color: "var(--text-faint)" }}>País </span>
                    {t.pais ?? "—"}
                  </span>
                  <span>
                    <span style={{ color: "var(--text-faint)" }}>Tipo </span>
                    {t.tipo ?? "—"}
                  </span>
                  <span>
                    <span style={{ color: "var(--text-faint)" }}>Tecnología </span>
                    {t.lenguaje ?? "—"}
                  </span>
                </div>

                <div className="rounded-md px-3 py-2" style={{ background: "var(--surface-raised)" }}>
                  {t.repos.length === 0 ? (
                    <div className="text-xs" style={{ color: "var(--text-faint)" }}>
                      Sin repositorios en el formulario.
                    </div>
                  ) : (
                    t.repos.map((r) => (
                      <div
                        key={r}
                        className="text-xs py-0.5"
                        style={{ fontFamily: "var(--font-mono)", color: "var(--text)" }}
                      >
                        {r}
                      </div>
                    ))
                  )}
                </div>

                {t.errores.length > 0 && (
                  <div className="space-y-0.5">
                    {t.errores.map((e, i) => (
                      <div key={i} className="text-xs" style={{ color: "var(--fail)" }}>
                        No se puede procesar — {e}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {elegido && (
        <div
          className="p-4 rounded-lg border space-y-4"
          style={{ borderColor: "var(--accent)", background: "var(--surface)" }}
        >
          <div className="text-sm font-semibold" style={{ color: "var(--text)" }}>
            Revisa antes de procesar {elegido.clave}
          </div>

          <div className="space-y-1">
            <div className="text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
              Se va a hacer
            </div>
            <div className="text-xs" style={{ color: "var(--text)" }}>
              Crear {elegido.repos.length} repositorio{elegido.repos.length === 1 ? "" : "s"} privado
              {elegido.repos.length === 1 ? "" : "s"}, sin forks, en el proyecto {elegido.projectKey}{" "}
              ({workspace})
            </div>
            <div className="text-xs" style={{ color: "var(--text)" }}>
              Commit inicial con .gitignore y README.md estándar, y las ramas de {elegido.pais} desde
              master
            </div>
            <div className="text-xs" style={{ color: elegido.esBd ? "var(--partial)" : "var(--text)" }}>
              {elegido.esBd
                ? "Repositorios de Base de Datos: se crean SIN Pipelines y SIN proyecto en SonarQube."
                : "Activar Pipelines en cada repositorio."}
            </div>
          </div>

          <div className="space-y-2.5">
            <div className="text-xs" style={{ color: "var(--text-muted)" }}>
              Después de crear
            </div>
            <Casilla
              marcada={crearSonar && !elegido.esBd}
              onCambio={onCrearSonar}
              deshabilitada={elegido.esBd}
              titulo="Crear los proyectos en SonarQube"
              detalle={
                elegido.esBd
                  ? "No aplica a repositorios de Base de Datos."
                  : `Con la key ${elegido.projectKey}_[repo], vinculados al repositorio. El widget en Bitbucket sigue siendo manual.`
              }
            />
            <Casilla
              marcada={etiquetas}
              onCambio={setEtiquetas}
              titulo="Agregar las etiquetas que falten al ticket"
              detalle="ACN, DEVOPS-CHAPTER y CHILE o PERU. Solo se usan etiquetas que ya existen en Jira."
            />
            <Casilla
              marcada={cerrar}
              onCambio={setCerrar}
              titulo="Cerrar el ticket"
              detalle="Comenzar progreso (si hace falta) y Marcar como hecho, con resolución Listo."
            />
            <Casilla
              marcada={comentar}
              onCambio={setComentar}
              titulo="Responder al solicitante con la evidencia"
              detalle="Las URLs de los repositorios y una imagen con sus ramas. Es visible para el solicitante."
            />
          </div>

          {usaSonar && !sonarOk && (
            <div
              className="p-3 rounded-md text-xs"
              style={{ background: "var(--partial-dim)", border: "1px solid #4a3a10", color: "#fde68a" }}
            >
              Falta validar el token de SonarQube, o desmarcar la creación de proyectos.
            </div>
          )}

          <div className="flex items-center gap-2 pt-1">
            <Button onClick={procesar} disabled={ejecutando || !puedeProcesar}>
              {ejecutando ? "Procesando…" : `Procesar ${elegido.clave}`}
            </Button>
            <Button variant="secondary" onClick={() => setElegido(null)} disabled={ejecutando}>
              Cancelar
            </Button>
            {ejecutando && (
              <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                El detalle va apareciendo en la consola.
              </span>
            )}
          </div>
        </div>
      )}

      {errorProceso && <ErrorNotice>{errorProceso}</ErrorNotice>}

      {resultado && (
        <div className="space-y-3">
          <ResultsPanel resumen={resultado.resumen} sinSonar={resultado.esBd} />

          <div className="p-4 rounded-lg border space-y-2" style={CAJA}>
            <div className="text-sm font-semibold" style={{ color: "var(--text)" }}>
              Ticket {resultado.clave}
            </div>

            {!resultado.accionesJira ? (
              <div className="text-xs" style={{ color: "var(--partial)" }}>
                {resultado.motivoSinAcciones}
              </div>
            ) : (
              <>
                <div
                  className="text-xs"
                  style={{ color: resultado.etiquetasError ? "var(--fail)" : "var(--text)" }}
                >
                  Etiquetas —{" "}
                  {resultado.etiquetasError
                    ? resultado.etiquetasError
                    : resultado.etiquetasAgregadas.length > 0
                      ? `agregadas: ${resultado.etiquetasAgregadas.join(", ")}`
                      : "el ticket ya las tenía o no se pidieron"}
                </div>
                <div
                  className="text-xs"
                  style={{
                    color:
                      resultado.cierreEstado === "error"
                        ? "var(--fail)"
                        : resultado.cierreEstado === "omitido"
                          ? "var(--partial)"
                          : "var(--text)",
                  }}
                >
                  Cierre — {resultado.cierreDetalle}
                </div>
                {evidencia && (
                  <div
                    className="text-xs"
                    style={{
                      color:
                        evidencia.estado === "error"
                          ? "var(--fail)"
                          : evidencia.estado === "omitida"
                            ? "var(--partial)"
                            : "var(--text)",
                    }}
                  >
                    Evidencia — {evidencia.detalle}
                  </div>
                )}
              </>
            )}
          </div>

          <Button variant="secondary" onClick={buscar} disabled={buscando || !listo}>
            Buscar otros tickets
          </Button>
        </div>
      )}
    </div>
  );
}
