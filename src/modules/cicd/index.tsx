import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useLocation } from "react-router-dom";
import { ConsoleProvider } from "../clusters/ConsoleContext";
import GlobalConsole from "../clusters/GlobalConsole";
import { useConsolePanel } from "../../hooks/useConsolePanel";
import FormTab from "./FormTab";
import FilesTab from "./FilesTab";
import JenkinsTab from "./JenkinsTab";
import { SUBNETS_SEED } from "./data/subnets-seed";
import { buscarHuecos, collectArchivos } from "./generators";
import type { CatalogoSubredes, CicdForm, EstadoSubredes } from "./shared";
import { aplicarSugerencias, collectData, formVacio, parseErr, ramaDesarrollo } from "./shared";

type TabId = "formulario" | "archivos" | "jenkins";

export default function Cicd() {
  const [f, setF] = useState<CicdForm>(formVacio);
  const [tab, setTab] = useState<TabId>("formulario");
  const consolePanel = useConsolePanel(280);
  // El módulo queda montado aunque no esté a la vista (App.tsx lo oculta
  // con CSS), así que la pestaña visible no alcanza para saber si el
  // usuario está acá.
  const visible = useLocation().pathname === "/cicd";

  const [subnets, setSubnets] = useState<EstadoSubredes | null>(null);
  const [subnetsCargando, setSubnetsCargando] = useState(false);
  const [subnetsError, setSubnetsError] = useState("");

  const set = (patch: Partial<CicdForm>) => setF((prev) => ({ ...prev, ...patch }));

  // Precompletar lo deducible en cada cambio. aplicarSugerencias devuelve
  // el mismo objeto cuando no hay nada que sugerir, así que esto no genera
  // renders en cadena.
  useEffect(() => {
    setF((prev) => aplicarSugerencias(prev));
  }, [f]);

  // Al abrir el módulo se lee la caché. Nunca se consulta en vivo sin que
  // el usuario lo pida: la consulta tarda y puede pedir renovar la sesión
  // de gcloud justo cuando alguien venía a llenar un formulario.
  useEffect(() => {
    invoke<EstadoSubredes>("cicd_subnets_leer")
      .then(setSubnets)
      .catch((err) => setSubnetsError(parseErr(err)));
  }, []);

  async function actualizarSubnets() {
    setSubnetsCargando(true);
    setSubnetsError("");
    try {
      setSubnets(await invoke<EstadoSubredes>("cicd_subnets_actualizar"));
    } catch (err) {
      setSubnetsError(parseErr(err));
    } finally {
      setSubnetsCargando(false);
    }
  }

  // Catálogo efectivo: lo consultado si hay algo, el incluido en la
  // aplicación si no. Un ambiente vacío cae al semilla por separado, para
  // que una consulta donde solo falló producción no deje prod sin datos.
  const catalogo: CatalogoSubredes = useMemo(() => {
    const c = subnets?.catalogo;
    return {
      dev: c?.dev?.length ? c.dev : SUBNETS_SEED.dev,
      release: c?.release?.length ? c.release : SUBNETS_SEED.release,
      prod: c?.prod?.length ? c.prod : SUBNETS_SEED.prod,
    };
  }, [subnets]);

  const subnetsTotal = catalogo.dev.length + catalogo.release.length + catalogo.prod.length;

  const d = useMemo(() => collectData(f), [f]);
  const archivos = useMemo(() => collectArchivos(d, catalogo), [d, catalogo]);
  // Marcadores PENDIENTE más variables que quedarían escritas vacías.
  const huecos = useMemo(() => buscarHuecos(archivos), [archivos]);

  const tabs: { id: TabId; label: string }[] = [
    { id: "formulario", label: "Formulario" },
    { id: "archivos", label: "Archivos" },
    { id: "jenkins", label: "Jenkins" },
  ];

  return (
    <ConsoleProvider>
      <div className="h-full flex flex-col" style={{ color: "var(--text)" }}>
        {/* Encabezado fijo: nunca scrollea, igual que en Clusters. */}
        <div className="shrink-0 px-8 pt-8">
          <h1 className="text-xl font-semibold mb-1">CI/CD Automation</h1>
          <p className="text-sm mb-5" style={{ color: "var(--text-muted)" }}>
            Genera la configuración de Jenkins, Spinnaker y Kubernetes, Cloud Run o Apigee para los
            repositorios de una solicitud.
          </p>

          <div
            className="flex mb-5 p-1 rounded-lg"
            style={{ border: "1px solid var(--border-strong)", background: "var(--surface)" }}
          >
            {tabs.map((t) => (
              <button
                key={t.id}
                onClick={() => setTab(t.id)}
                className="flex-1 px-4 py-2 rounded-md text-sm font-medium transition-colors"
                style={{
                  color: tab === t.id ? "white" : "var(--text-muted)",
                  background: tab === t.id ? "var(--accent)" : "transparent",
                }}
              >
                {t.label}
              </button>
            ))}
          </div>
        </div>

        {/* Las tres pestañas quedan montadas y se alternan con CSS: cambiar
            de pestaña no debe perder lo escrito ni el scroll. */}
        <div className="flex-1 min-h-0 px-8 pb-4 overflow-y-auto">
          <div style={{ display: tab === "formulario" ? "block" : "none" }}>
            <FormTab
              f={f}
              set={set}
              onReiniciar={() => setF(formVacio())}
              catalogo={catalogo}
              subnetsEstado={subnets}
              subnetsCargando={subnetsCargando}
              subnetsError={subnetsError}
              subnetsTotal={subnetsTotal}
              onActualizarSubnets={actualizarSubnets}
              huecos={huecos}
            />
          </div>
          <div style={{ display: tab === "archivos" ? "block" : "none" }}>
            <FilesTab
              archivos={archivos}
              hallazgos={huecos}
              ramaDestino={ramaDesarrollo(f.country)}
              ticket={f.ticket}
              activo={visible && tab === "archivos"}
            />
          </div>
          <div style={{ display: tab === "jenkins" ? "block" : "none" }}>
            <JenkinsTab f={f} />
          </div>
        </div>

        {/* Consola al fondo. Arranca contraída si así quedó la última vez:
            en este módulo solo informa la actualización de subredes, así
            que lo habitual es quererla fuera del camino. */}
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
    </ConsoleProvider>
  );
}
