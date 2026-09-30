import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import RefreshTab from "./RefreshTab";
import LoadTab from "./LoadTab";
import { ConsoleProvider } from "./ConsoleContext";
import GlobalConsole from "./GlobalConsole";
import { useResizablePanel } from "../../hooks/useResizablePanel";

interface ToolStatus {
  name: string;
  found: boolean;
  version: string | null;
  install_hint: string;
}
interface DependenciesCheckResult {
  os: string;
  tools: ToolStatus[];
  all_ok: boolean;
}

type TabId = "actualizar" | "cargar";

export default function Clusters() {
  const [tab, setTab] = useState<TabId>("actualizar");
  const [deps, setDeps] = useState<DependenciesCheckResult | null>(null);
  const consolePanel = useResizablePanel("devops-hub-console-height", 320);

  useEffect(() => {
    invoke<DependenciesCheckResult>("check_dependencies")
      .then(setDeps)
      .catch(() => {
        // Si el chequeo mismo falla, no bloqueamos el resto de la página.
      });
  }, []);

  const tabs: { id: TabId; label: string }[] = [
    { id: "actualizar", label: "Actualizar" },
    { id: "cargar", label: "Cargar clusters" },
  ];

  return (
    <ConsoleProvider>
      <div className="h-full flex flex-col" style={{ color: "var(--text)" }}>
        {/* ── Encabezado fijo — título, banner, pestañas. Nunca scrollea,
            siempre visible sin importar qué tan largo sea el contenido
            de la pestaña activa ni el tamaño de la consola. ── */}
        <div className="shrink-0 px-8 pt-8">
          <h1 className="text-xl font-semibold mb-1">Clusters</h1>
          <p className="text-sm mb-5" style={{ color: "var(--text-muted)" }}>
            EKS (Chile, Perú) y GKE — descubrimiento, verificación de permisos y carga a kubeconfig.
          </p>

          {deps && !deps.all_ok && (
            <div
              className="mb-5 p-3 rounded-md text-xs"
              style={{ background: "var(--partial-dim)", border: "1px solid #4a3a10", color: "#fde68a" }}
            >
              <div className="font-semibold mb-2">⚠ Faltan herramientas del sistema (detectado: {deps.os})</div>
              <ul className="space-y-1">
                {deps.tools
                  .filter((t) => !t.found)
                  .map((t) => (
                    <li key={t.name}>
                      <span style={{ fontFamily: "var(--font-mono)" }}>{t.name}</span> no encontrado —{" "}
                      {t.install_hint}
                    </li>
                  ))}
              </ul>
            </div>
          )}

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

        {/* ── Contenido de la pestaña activa — ocupa el resto del espacio
            disponible ARRIBA de la consola. Cada pestaña administra su
            propio scroll interno (RefreshTab scrollea entero si hace
            falta; LoadTab reserva su barra de acción pegada al fondo). ── */}
        <div className="flex-1 min-h-0 px-8 pb-4">
          <div className="h-full" style={{ display: tab === "actualizar" ? "block" : "none" }}>
            <div className="h-full overflow-y-auto">
              <RefreshTab />
            </div>
          </div>
          <div className="h-full" style={{ display: tab === "cargar" ? "block" : "none" }}>
            <LoadTab />
          </div>
        </div>

        {/* ── Consola — siempre al fondo, redimensionable arrastrando la
            barra. Solo cambia el espacio disponible arriba (la franja de
            contenido), nunca el encabezado. ── */}
        <div
          onMouseDown={consolePanel.onDragStart}
          className="h-1.5 shrink-0 cursor-row-resize resize-handle"
          title="Arrastrar para cambiar el tamaño de la consola"
        />
        <GlobalConsole height={consolePanel.height} />
      </div>
    </ConsoleProvider>
  );
}
