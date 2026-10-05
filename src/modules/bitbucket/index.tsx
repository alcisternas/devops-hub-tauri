import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ErrorNotice } from "../../components/ui";
import { ConsoleProvider, useConsole } from "../clusters/ConsoleContext";
import GlobalConsole from "../clusters/GlobalConsole";
import { useResizablePanel } from "../../hooks/useResizablePanel";
import CredentialsPanel from "./CredentialsPanel";
import RepoForm, { type DatosForm } from "./RepoForm";
import ConfirmPanel from "./ConfirmPanel";
import ResultsPanel from "./ResultsPanel";
import { parseErr, parsearRepos, type ConfigRun, type ResumenRun } from "./shared";

// El workspace viene precargado porque no va a cambiar: el equipo trabaja
// siempre sobre el mismo. Queda editable por si algún día hace falta.
const DATOS_INICIALES: DatosForm = {
  workspace: "banco_ripley",
  projectKey: "",
  reposTexto: "",
  pais: "CL",
  activarPipelines: true,
  crearRamas: true,
};

// El contenido vive en un componente aparte porque necesita useConsole, y
// ese hook solo funciona dentro del ConsoleProvider que monta el exterior.
function Contenido() {
  const { bump } = useConsole();
  const consolePanel = useResizablePanel("devops-hub-console-height", 320);

  const [datos, setDatos] = useState<DatosForm>(DATOS_INICIALES);
  const [credOk, setCredOk] = useState(false);
  const [confirmando, setConfirmando] = useState(false);
  const [ejecutando, setEjecutando] = useState(false);
  const [resumen, setResumen] = useState<ResumenRun | null>(null);
  const [error, setError] = useState("");

  const repos = parsearRepos(datos.reposTexto);
  const puedeCrear =
    credOk && repos.length > 0 && datos.projectKey.trim() !== "" && datos.workspace.trim() !== "";

  const config: ConfigRun = {
    workspace: datos.workspace,
    projectKey: datos.projectKey,
    repos,
    pais: datos.pais,
    activarPipelines: datos.activarPipelines,
    crearRamas: datos.crearRamas,
  };

  async function ejecutar() {
    setEjecutando(true);
    setError("");
    setResumen(null);
    // Limpia la consola para que solo se vea la corrida actual.
    bump();

    try {
      const r = await invoke<ResumenRun>("bitbucket_crear_repos", { config });
      setResumen(r);
      setConfirmando(false);
    } catch (e) {
      setError(parseErr(e));
    } finally {
      setEjecutando(false);
    }
  }

  function nuevaCorrida() {
    setResumen(null);
    setError("");
    setConfirmando(false);
    // Se conservan workspace, proyecto, país y opciones: lo habitual es
    // crear otro lote del mismo proyecto. Solo se vacían los nombres.
    setDatos({ ...datos, reposTexto: "" });
  }

  return (
    <div className="h-full flex flex-col" style={{ color: "var(--text)" }}>
      {/* Encabezado fijo — nunca scrollea */}
      <div className="shrink-0 px-8 pt-8">
        <h1 className="text-xl font-semibold mb-1">Bitbucket Repo Creator</h1>
        <p className="text-sm mb-5" style={{ color: "var(--text-muted)" }}>
          Crea repositorios con el estándar del equipo: commit inicial, Pipelines y ramas base.
        </p>
      </div>

      {/* Contenido con scroll propio */}
      <div className="flex-1 min-h-0 px-8 pb-4 overflow-y-auto">
        <div className="space-y-3 pb-2">
          <CredentialsPanel workspace={datos.workspace} onEstadoCambia={setCredOk} />

          {/* Mientras hay resultados, el formulario se oculta para no
              competir con ellos. No se desmonta: el estado sigue vivo si
              el usuario vuelve a editar. */}
          <div style={{ display: resumen ? "none" : "block" }} className="space-y-3">
            <RepoForm datos={datos} onCambio={setDatos} deshabilitado={confirmando || ejecutando} />

            {!confirmando && (
              <div className="flex items-center gap-3">
                <Button onClick={() => setConfirmando(true)} disabled={!puedeCrear}>
                  Continuar
                </Button>
                {!credOk && (
                  <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                    Primero hay que validar las credenciales.
                  </span>
                )}
                {credOk && !puedeCrear && (
                  <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                    Falta el proyecto o los nombres de repositorio.
                  </span>
                )}
              </div>
            )}

            {confirmando && (
              <ConfirmPanel
                config={config}
                ejecutando={ejecutando}
                onConfirmar={ejecutar}
                onCancelar={() => setConfirmando(false)}
              />
            )}
          </div>

          {error && <ErrorNotice>{error}</ErrorNotice>}

          {resumen && (
            <>
              <ResultsPanel resumen={resumen} />
              <Button variant="secondary" onClick={nuevaCorrida}>
                Crear otros repositorios
              </Button>
            </>
          )}
        </div>
      </div>

      {/* Consola al fondo, redimensionable arrastrando la barra */}
      <div
        onMouseDown={consolePanel.onDragStart}
        className="h-1.5 shrink-0 cursor-row-resize resize-handle"
        title="Arrastrar para cambiar el tamaño de la consola"
      />
      <GlobalConsole height={consolePanel.height} />
    </div>
  );
}

export default function Bitbucket() {
  return (
    <ConsoleProvider>
      <Contenido />
    </ConsoleProvider>
  );
}
