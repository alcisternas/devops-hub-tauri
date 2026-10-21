import { useEffect, useRef, useState } from "react";
import { useLocation } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { ErrorNotice } from "../../components/ui";
import { ConsoleProvider, useConsole } from "../clusters/ConsoleContext";
import GlobalConsole from "../clusters/GlobalConsole";
import { useConsolePanel } from "../../hooks/useConsolePanel";
import ImagenForm from "./ImagenForm";
import ResumenPanel from "./ResumenPanel";
import OpcionesPanel from "./OpcionesPanel";
import ImpactoPanel from "./ImpactoPanel";
import DockerfilePanel from "./DockerfilePanel";
import HallazgosTabla from "./HallazgosTabla";
import {
  URL_REGISTRO_NPM,
  parseErr,
  type ConfigHardening,
  type Evaluacion,
  type Herramientas,
  type Perfil,
  type ResultadoAnalisis,
  type ResultadoGeneracion,
} from "./shared";

// Espera entre el último cambio de una casilla y la reevaluación: evaluar es
// local y rápido, pero no hace falta lanzar una por cada clic seguido.
const ESPERA_EVALUAR_MS = 200;

// El contenido vive en un componente aparte porque necesita useConsole, y
// ese hook solo funciona dentro del ConsoleProvider que monta el exterior.
function Contenido() {
  const { bump } = useConsole();
  const consolePanel = useConsolePanel(320);
  const visible = useLocation().pathname === "/hardening";

  const [herramientas, setHerramientas] = useState<Herramientas | null>(null);
  const [perfiles, setPerfiles] = useState<Perfil[]>([]);
  const iniciado = useRef(false);

  const [referencia, setReferencia] = useState("");
  const [plataforma, setPlataforma] = useState("linux/amd64");
  const [escaner, setEscaner] = useState("trivy");
  const [analizando, setAnalizando] = useState(false);
  const [error, setError] = useState("");

  const [analisis, setAnalisis] = useState<ResultadoAnalisis | null>(null);
  const [config, setConfig] = useState<ConfigHardening | null>(null);
  const [evaluacion, setEvaluacion] = useState<Evaluacion | null>(null);
  const [evaluando, setEvaluando] = useState(false);
  const [errorEvaluar, setErrorEvaluar] = useState("");
  // Para ignorar la respuesta de una evaluación si ya hay otra más reciente.
  const secuencia = useRef(0);

  const [npmRegistry, setNpmRegistry] = useState(URL_REGISTRO_NPM);
  const [pruebasRed, setPruebasRed] = useState(true);
  const [generando, setGenerando] = useState(false);
  const [generado, setGenerado] = useState<ResultadoGeneracion | null>(null);
  const [errorGenerar, setErrorGenerar] = useState("");
  const [ruta, setRuta] = useState("");
  const [guardando, setGuardando] = useState(false);
  const [errorGuardar, setErrorGuardar] = useState("");

  // Herramientas y perfiles se consultan al abrir el módulo por primera vez.
  useEffect(() => {
    if (!visible || iniciado.current) return;
    iniciado.current = true;
    invoke<Herramientas>("hardening_herramientas").then(setHerramientas).catch(() => {});
    invoke<Perfil[]>("hardening_perfiles").then(setPerfiles).catch(() => {});
  }, [visible]);

  // Cada cambio de la selección recalcula impacto y plan en Rust.
  useEffect(() => {
    if (!analisis || !config) return;
    const mia = ++secuencia.current;
    setEvaluando(true);
    const t = setTimeout(async () => {
      try {
        const ev = await invoke<Evaluacion>("hardening_evaluar", { idSesion: analisis.id_sesion, config });
        if (mia !== secuencia.current) return;
        setEvaluacion(ev);
        setErrorEvaluar("");
        // Si se descartó algo que no existe en la imagen, la selección visible se alinea con la efectiva.
        // La adaptación es idempotente: la segunda pasada ya no descarta nada y no hay bucle.
        if (ev.descartados.length > 0) setConfig(ev.adaptada);
      } catch (e) {
        if (mia === secuencia.current) setErrorEvaluar(parseErr(e));
      } finally {
        if (mia === secuencia.current) setEvaluando(false);
      }
    }, ESPERA_EVALUAR_MS);
    return () => clearTimeout(t);
  }, [analisis, config]);

  // Cualquier cambio de selección invalida el Dockerfile ya generado.
  function cambiarConfig(c: ConfigHardening) {
    setConfig(c);
    setGenerado(null);
    setRuta("");
    setErrorGenerar("");
  }

  async function analizar() {
    setAnalizando(true);
    setError("");
    setAnalisis(null);
    setConfig(null);
    setEvaluacion(null);
    setErrorEvaluar("");
    setGenerado(null);
    setRuta("");
    setErrorGenerar("");
    setErrorGuardar("");
    // Limpia la consola para que solo se vea la corrida actual.
    bump();
    try {
      const r = await invoke<ResultadoAnalisis>("hardening_analizar", { referencia: referencia.trim(), plataforma, escaner });
      setAnalisis(r);
      setConfig(r.config_inicial);
      setEvaluacion(r.evaluacion);
    } catch (e) {
      setError(parseErr(e));
    } finally {
      setAnalizando(false);
    }
  }

  async function generar() {
    if (!analisis || !config) return;
    setGenerando(true);
    setErrorGenerar("");
    setRuta("");
    try {
      const g = await invoke<ResultadoGeneracion>("hardening_generar", {
        idSesion: analisis.id_sesion,
        config,
        npmRegistry: npmRegistry.trim() || null,
        pruebasRed,
      });
      setGenerado(g);
    } catch (e) {
      setGenerado(null);
      setErrorGenerar(parseErr(e));
    } finally {
      setGenerando(false);
    }
  }

  async function guardar() {
    if (!analisis) return;
    setGuardando(true);
    setErrorGuardar("");
    try {
      setRuta(await invoke<string>("hardening_guardar", { idSesion: analisis.id_sesion, directorio: null }));
    } catch (e) {
      setErrorGuardar(parseErr(e));
    } finally {
      setGuardando(false);
    }
  }

  // Razón por la que todavía no se puede generar, para mostrarla junto al botón.
  let motivoBloqueo: string | null = null;
  if (analisis && !analisis.soporte.generador) motivoBloqueo = analisis.soporte.motivo;
  else if (errorEvaluar) motivoBloqueo = "La selección tiene un valor no válido (revisa las opciones avanzadas).";
  else if (!evaluacion) motivoBloqueo = "Calculando el impacto de la selección…";
  else if (evaluacion.analisis.issues.some((i) => i.level === "error")) motivoBloqueo = "Resuelve los conflictos de la selección antes de generar.";
  else if (evaluacion.plan.blockers.length > 0) motivoBloqueo = "Hay bloqueos sin resolver: elige una alternativa en el análisis de impacto.";

  return (
    <div className="h-full flex flex-col" style={{ color: "var(--text)" }}>
      {/* Encabezado fijo — nunca scrollea */}
      <div className="shrink-0 px-8 pt-8">
        <h1 className="text-xl font-semibold mb-1">Hardening de imágenes</h1>
        <p className="text-sm mb-5" style={{ color: "var(--text-muted)" }}>
          Analiza una imagen, elige qué conservar según su tipo y genera el Dockerfile endurecido para publicarla en ECR.
        </p>
      </div>

      {/* Contenido con scroll propio */}
      <div className="flex-1 min-h-0 px-8 pb-4 overflow-y-auto">
        <div className="space-y-3 pb-2">
          <ImagenForm
            referencia={referencia}
            onReferencia={setReferencia}
            plataforma={plataforma}
            onPlataforma={setPlataforma}
            escaner={escaner}
            onEscaner={setEscaner}
            herramientas={herramientas}
            analizando={analizando}
            onAnalizar={analizar}
          />

          {error && <ErrorNotice>{error}</ErrorNotice>}

          {analisis && config && (
            <>
              <ResumenPanel analisis={analisis} />
              <OpcionesPanel
                analisis={analisis}
                config={config}
                onConfig={cambiarConfig}
                perfiles={perfiles}
                evaluacion={evaluacion}
                deshabilitado={generando}
              />
              {errorEvaluar && <ErrorNotice>{errorEvaluar}</ErrorNotice>}
              {evaluacion && analisis.soporte.generador && (
                <ImpactoPanel analisis={analisis} evaluacion={evaluacion} config={config} onConfig={cambiarConfig} evaluando={evaluando} />
              )}
              <DockerfilePanel
                generado={generado}
                motivoBloqueo={motivoBloqueo}
                generando={generando}
                onGenerar={generar}
                error={errorGenerar}
                npmRegistry={npmRegistry}
                onNpmRegistry={setNpmRegistry}
                pruebasRed={pruebasRed}
                onPruebasRed={setPruebasRed}
                ruta={ruta}
                guardando={guardando}
                onGuardar={guardar}
                errorGuardar={errorGuardar}
                herramientas={herramientas}
              />
              <HallazgosTabla hallazgos={analisis.hallazgos} />
            </>
          )}
        </div>
      </div>

      {/* Consola al fondo, redimensionable arrastrando la barra */}
      {!consolePanel.collapsed && (
        <div
          onMouseDown={consolePanel.onDragStart}
          className="h-1.5 shrink-0 cursor-row-resize resize-handle"
          title="Arrastrar para cambiar el tamaño de la consola"
        />
      )}
      <GlobalConsole
        height={consolePanel.height}
        collapsed={consolePanel.collapsed}
        onToggleCollapsed={consolePanel.toggleCollapsed}
      />
    </div>
  );
}

export default function Hardening() {
  return (
    <ConsoleProvider>
      <Contenido />
    </ConsoleProvider>
  );
}
