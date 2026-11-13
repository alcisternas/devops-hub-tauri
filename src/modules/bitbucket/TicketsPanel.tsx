import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, Dropdown, ErrorNotice, MetaLine } from "../../components/ui";
import ResultsPanel from "./ResultsPanel";
import { dibujarEvidenciaPng } from "./evidencia";
import {
  URL_JIRA,
  parseErr,
  parsearRepos,
  urlWidgetSonar,
  type CorreccionesTicket,
  type OpcionesProceso,
  type ResultadoTicket,
  type TicketPendiente,
  type ValidacionCorreccion,
} from "./shared";

interface Props {
  workspace: string;
  // Si el modo está a la vista. Mientras sea false no se consulta nada.
  activo: boolean;
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

const ESTILO_INPUT = {
  background: "var(--surface-raised)",
  border: "1px solid var(--border-strong)",
  color: "var(--text)",
};

// Datos del formulario de corrección, tal como los escribe el usuario.
interface FormCorreccion {
  projectKey: string;
  paisCodigo: string;
  // "" = sin elegir
  tipoRepo: "" | "codigo" | "bd";
  reposTexto: string;
}

// Parte de lo que trae el ticket, errores incluidos: el usuario corrige sobre
// lo que Jira dijo, no sobre un formulario en blanco.
function formInicial(t: TicketPendiente): FormCorreccion {
  return {
    projectKey: t.projectKey ?? "",
    paisCodigo: t.paisCodigo ?? "",
    tipoRepo: t.esBd ? "bd" : t.lenguaje ? "codigo" : "",
    reposTexto: t.repos.join("\n"),
  };
}

function aCorrecciones(f: FormCorreccion): CorreccionesTicket {
  return {
    projectKey: f.projectKey.trim(),
    paisCodigo: f.paisCodigo,
    esBd: f.tipoRepo === "" ? null : f.tipoRepo === "bd",
    repos: parsearRepos(f.reposTexto),
  };
}

function nombrePais(codigo: string): string {
  return codigo === "PE" ? "Perú" : codigo === "CL" ? "Chile" : "—";
}

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
  activo,
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
  // Con valor, el ticket se gestiona con datos corregidos a mano.
  const [corr, setCorr] = useState<FormCorreccion | null>(null);
  const [validacion, setValidacion] = useState<ValidacionCorreccion | null>(null);
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
    setCorr(null);
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

  // Al abrir el modo, con las credenciales ya validadas, se listan solos los
  // tickets asignados a la cuenta. Si falla, el error queda a la vista y no
  // se reintenta en bucle: hay un botón para volver a buscar.
  useEffect(() => {
    if (activo && jiraOk && bitbucketOk && tickets === null && !buscando && !errorLista) {
      buscar();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activo, jiraOk, bitbucketOk]);

  // Los datos con los que se va a procesar: los del ticket o, si se corrigió,
  // los del formulario. La revisión de abajo muestra siempre estos.
  const efectivo = elegido
    ? corr
      ? {
          projectKey: corr.projectKey.trim(),
          pais: nombrePais(corr.paisCodigo),
          repos: parsearRepos(corr.reposTexto),
          esBd: corr.tipoRepo === "bd",
        }
      : {
          projectKey: elegido.projectKey ?? "",
          pais: elegido.pais ?? "—",
          repos: elegido.repos,
          esBd: elegido.esBd,
        }
    : null;

  // Cada cambio del formulario se valida en Rust con las mismas reglas del
  // proceso (sin red); aquí no se copia ninguna regla.
  useEffect(() => {
    if (!elegido || !corr) {
      setValidacion(null);
      return;
    }
    let cancelado = false;
    invoke<ValidacionCorreccion>("jira_validar_correccion", {
      ticket: elegido,
      correcciones: aCorrecciones(corr),
    })
      .then((v) => {
        if (!cancelado) setValidacion(v);
      })
      .catch((e) => {
        if (!cancelado) setValidacion({ errores: [parseErr(e)], correcciones: [] });
      });
    return () => {
      cancelado = true;
    };
  }, [elegido, corr]);

  // Repos de Base de Datos: sin Pipelines ni SonarQube, sin excepción.
  const usaSonar = crearSonar && !!efectivo && !efectivo.esBd;
  const corregido = corr === null || (validacion !== null && validacion.errores.length === 0);
  const puedeProcesar = jiraOk && bitbucketOk && (!usaSonar || sonarOk) && corregido;

  async function procesar() {
    if (!elegido) return;
    setEjecutando(true);
    setErrorProceso("");
    setResultado(null);
    setEvidencia(null);
    onInicio();

    try {
      const opciones: OpcionesProceso = {
        workspace,
        sonar: usaSonar,
        etiquetas,
        cerrar,
        correcciones: corr ? aCorrecciones(corr) : undefined,
      };
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
      setCorr(null);

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
              correcciones: r.correcciones,
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
          Tickets de alta de repositorios asignados a tu cuenta de Jira. Con un botón se gestiona
          cada uno: crea los repositorios, activa Pipelines si corresponde, importa en SonarQube,
          etiqueta y cierra el ticket, y responde al solicitante.
        </div>
        <div className="flex items-center gap-3">
          <Button onClick={buscar} disabled={!listo || buscando || ejecutando}>
            {buscando ? "Buscando…" : tickets === null ? "Buscar mis tickets" : "Actualizar lista"}
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
                      disabled={(t.errores.length > 0 && !t.corregible) || ejecutando}
                      onClick={() => {
                        setElegido(t);
                        // Con errores, se abre el formulario con los datos tal
                        // como vienen para que el usuario los corrija.
                        setCorr(t.errores.length > 0 ? formInicial(t) : null);
                        setResultado(null);
                        setEvidencia(null);
                        setErrorProceso("");
                      }}
                    >
                      {t.errores.length > 0 && t.corregible ? "Corregir y gestionar" : "Gestionar ticket"}
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
                        {t.corregible ? "Requiere corrección" : "No se puede procesar"} — {e}
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
            {corr ? `Corrige los datos de ${elegido.clave}` : `Revisa antes de procesar ${elegido.clave}`}
          </div>

          {corr && (
            <div className="space-y-3">
              <div className="text-xs" style={{ color: "var(--text-muted)" }}>
                Los datos se cargaron tal como vienen en el ticket. El ticket en Jira no se modifica:
                lo que cambies solo vale para esta corrida y queda anotado en el comentario al
                solicitante.
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <label className="flex flex-col gap-1.5">
                  <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                    Proyecto — clave de Bitbucket, tal cual (respeta mayúsculas y minúsculas)
                  </span>
                  <input
                    type="text"
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                    value={corr.projectKey}
                    onChange={(e) => setCorr({ ...corr, projectKey: e.target.value.trim() })}
                    disabled={ejecutando}
                    className="px-3 py-2 rounded-md text-sm outline-hidden disabled:opacity-50"
                    style={{ ...ESTILO_INPUT, fontFamily: "var(--font-mono)" }}
                  />
                  <span className="text-xs" style={{ color: "var(--text-faint)" }}>
                    En el ticket: {elegido.proyecto ?? "vacío"}
                  </span>
                </label>

                <div className="flex flex-col gap-1.5">
                  <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                    País — define el nombre de la rama de desarrollo
                  </span>
                  <Dropdown
                    value={corr.paisCodigo}
                    onChange={(v) => setCorr({ ...corr, paisCodigo: v })}
                    options={[
                      { value: "", label: "Elegir…" },
                      { value: "CL", label: "Chile — rama development" },
                      { value: "PE", label: "Perú — rama develop" },
                    ]}
                  />
                  <span className="text-xs" style={{ color: "var(--text-faint)" }}>
                    En el ticket: {elegido.pais ?? "vacío"}
                  </span>
                </div>
              </div>

              <div className="flex flex-col gap-1.5">
                <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                  Tipo de repositorio — decide si llevan Pipelines y SonarQube
                </span>
                <Dropdown
                  value={corr.tipoRepo}
                  onChange={(v) => setCorr({ ...corr, tipoRepo: v as FormCorreccion["tipoRepo"] })}
                  options={[
                    { value: "", label: "Elegir…" },
                    { value: "codigo", label: "Código — con Pipelines y SonarQube" },
                    { value: "bd", label: "Base de Datos — sin Pipelines ni SonarQube" },
                  ]}
                />
                <span className="text-xs" style={{ color: "var(--text-faint)" }}>
                  Lenguaje / Tecnología en el ticket: {elegido.lenguaje ?? "vacío"}
                </span>
              </div>

              <div className="flex flex-col gap-1.5">
                <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                  Repositorios — uno por línea
                </span>
                <textarea
                  autoCapitalize="none"
                  autoCorrect="off"
                  spellCheck={false}
                  rows={Math.min(8, Math.max(3, corr.reposTexto.split("\n").length + 1))}
                  value={corr.reposTexto}
                  onChange={(e) => setCorr({ ...corr, reposTexto: e.target.value.toLowerCase() })}
                  disabled={ejecutando}
                  className="px-3 py-2 rounded-md text-sm outline-hidden resize-y disabled:opacity-50"
                  style={{ ...ESTILO_INPUT, fontFamily: "var(--font-mono)", lineHeight: 1.7 }}
                />
                {elegido.repos.length > 0 && (
                  <span className="text-xs" style={{ color: "var(--text-faint)" }}>
                    En el ticket: {elegido.repos.join(", ")}
                  </span>
                )}
              </div>

              {validacion && validacion.errores.length > 0 && (
                <div className="space-y-0.5">
                  {validacion.errores.map((e, i) => (
                    <div key={i} className="text-xs" style={{ color: "var(--fail)" }}>
                      {e}
                    </div>
                  ))}
                </div>
              )}
              {validacion && validacion.errores.length === 0 && (
                <div className="text-xs" style={{ color: "var(--ok)" }}>
                  Los datos están completos y cumplen las reglas del proceso.
                </div>
              )}
              {validacion && validacion.correcciones.length > 0 && (
                <div className="rounded-md px-3 py-2 space-y-0.5" style={{ background: "var(--surface-raised)" }}>
                  <div className="text-xs mb-1" style={{ color: "var(--text-muted)" }}>
                    Se anotará en el comentario al solicitante
                  </div>
                  {validacion.correcciones.map((c, i) => (
                    <div key={i} className="text-xs" style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>
                      {c}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          <div className="space-y-1">
            <div className="text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
              Se va a hacer
            </div>
            <div className="text-xs" style={{ color: "var(--text)" }}>
              Crear {efectivo!.repos.length} repositorio{efectivo!.repos.length === 1 ? "" : "s"} privado
              {efectivo!.repos.length === 1 ? "" : "s"}, sin forks, en el proyecto {efectivo!.projectKey}{" "}
              ({workspace})
            </div>
            <div className="text-xs" style={{ color: "var(--text)" }}>
              Commit inicial con .gitignore y README.md estándar, y las ramas de {efectivo!.pais} desde
              master
            </div>
            <div className="text-xs" style={{ color: efectivo!.esBd ? "var(--partial)" : "var(--text)" }}>
              {efectivo!.esBd
                ? "Repositorios de Base de Datos: se crean SIN Pipelines y SIN proyecto en SonarQube."
                : "Activar Pipelines en cada repositorio."}
            </div>
          </div>

          <div className="space-y-2.5">
            <div className="text-xs" style={{ color: "var(--text-muted)" }}>
              Después de crear
            </div>
            <Casilla
              marcada={crearSonar && !efectivo!.esBd}
              onCambio={onCrearSonar}
              deshabilitada={efectivo!.esBd}
              titulo="Crear los proyectos en SonarQube"
              detalle={
                efectivo!.esBd
                  ? "No aplica a repositorios de Base de Datos."
                  : `Con la key ${efectivo!.projectKey}_[repo], vinculados al repositorio. El widget en Bitbucket sigue siendo manual.`
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
            <Button
              variant="secondary"
              onClick={() => {
                setElegido(null);
                setCorr(null);
              }}
              disabled={ejecutando}
            >
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
          {/* El paso manual de Sonar lo cubre la tarjeta de abajo, con el enlace
              de cada repositorio. Sin esto el panel repetiría las instrucciones. */}
          <ResultsPanel resumen={resultado.resumen} sinSonar />

          {!resultado.esBd && resultado.resumen.resultados.some((r) => r.estado === "creado") && (
            <div className="p-4 rounded-lg border space-y-3" style={CAJA}>
              <div>
                <div className="text-sm font-semibold" style={{ color: "var(--text)" }}>
                  Pendiente manual — activar el widget de SonarQube
                </div>
                <div className="text-xs mt-0.5" style={{ color: "var(--text-muted)" }}>
                  Los proyectos ya están creados y vinculados. En cada repositorio hay que marcar
                  "Show repository overview widget" y guardar; esa opción no tiene API.
                </div>
              </div>
              <div className="space-y-1.5">
                {resultado.resumen.resultados
                  .filter((r) => r.estado === "creado")
                  .map((r) => {
                    const url = urlWidgetSonar(workspace, r.repo);
                    return (
                      <div
                        key={r.repo}
                        className="flex items-center gap-3 px-3 py-2 rounded-md"
                        style={{ background: "var(--surface-raised)" }}
                      >
                        <div className="min-w-0 flex-1">
                          <div className="text-sm" style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>
                            {r.repo}
                          </div>
                          <div
                            className="text-xs truncate"
                            style={{ color: "var(--text-faint)", fontFamily: "var(--font-mono)" }}
                          >
                            {url}
                          </div>
                          {!r.sonarBindeado && (
                            <div className="text-xs mt-0.5" style={{ color: "var(--partial)" }}>
                              Sonar no quedó vinculado: {r.sonarDetalle ?? "revisar el detalle de arriba"}
                            </div>
                          )}
                        </div>
                        <Button
                          variant="accent-outline"
                          className="text-xs px-2.5 py-1 shrink-0"
                          onClick={() => openUrl(url)}
                        >
                          Abrir
                        </Button>
                      </div>
                    );
                  })}
              </div>
            </div>
          )}

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
