import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useConsole } from "./ConsoleContext";

interface ModuleProgressEvent {
  modulo: string;
  etapa: string;
  item: string;
  progreso: number;
  total: number;
  estado: "running" | "ok" | "error" | string;
}

// Con varias operaciones corriendo a la vez (Chile, Perú, GKE, Carga) sin
// que ninguna se borre entre sí, hace falta poder distinguir de un
// vistazo de dónde viene cada línea entrelazada.
const MODULE_LABELS: Record<string, string> = {
  "eks-chile": "[EKS-Chile]",
  "eks-peru": "[EKS-Perú]",
  "gke-chile": "[GKE-Chile]",
  load: "[Carga]",
  bitbucket: "[Bitbucket]",
  cicd: "[CI/CD]",
};
function modulePrefix(modulo: string): string {
  return MODULE_LABELS[modulo] ?? `[${modulo}]`;
}

interface GlobalConsoleProps {
  height: number;
  // Contraída se muestra solo la barra de título. Las dos props van juntas:
  // el estado vive en el contenedor (useConsolePanel) porque también decide
  // si mostrar la barra de arrastre.
  collapsed?: boolean;
  onToggleCollapsed?: () => void;
}

export default function GlobalConsole({ height, collapsed = false, onToggleCollapsed }: GlobalConsoleProps) {
  const { resetSignal, bump } = useConsole();
  const [events, setEvents] = useState<ModuleProgressEvent[]>([]);
  const [listenError, setListenError] = useState<string>("");
  const logRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setEvents([]);
  }, [resetSignal]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ModuleProgressEvent>("module_progress", (event) => {
      setEvents((prev) => [...prev, event.payload]);
    })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch((err) => {
        if (!cancelled) setListenError(String(err));
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (collapsed) return;
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [events, collapsed]);

  const last = events[events.length - 1];
  const isRunning = last?.estado === "running";
  const pct = last && last.total > 0 ? Math.round((last.progreso / last.total) * 100) : 0;

  return (
    <div
      className="border-t flex flex-col shrink-0"
      style={{ borderColor: "var(--border)", background: "#080b10", height }}
    >
      <div className="px-4 py-2 border-b flex items-center justify-between" style={{ borderColor: "var(--border)" }}>
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium" style={{ color: "var(--text-muted)" }}>
            Consola
          </span>
          {isRunning && (
            <span
              className="w-1.5 h-1.5 rounded-full"
              style={{ background: "var(--accent)", animation: "pulse-dot 1s ease-in-out infinite" }}
            />
          )}
        </div>
        <div className="flex items-center gap-3">
          {last && (
            <span className="text-xs" style={{ color: "var(--text-faint)" }}>
              {modulePrefix(last.modulo)} {last.etapa} · {last.progreso}/{last.total} ·{" "}
              {isRunning ? "en curso" : last.estado === "ok" ? "listo" : "error"}
            </span>
          )}
          <button
            onClick={bump}
            disabled={events.length === 0}
            className="text-xs px-2 py-0.5 rounded disabled:opacity-30"
            style={{ color: "var(--text-muted)", border: "1px solid var(--border-strong)" }}
          >
            Limpiar
          </button>
          {onToggleCollapsed && (
            <button
              onClick={onToggleCollapsed}
              className="text-xs px-2 py-0.5 rounded"
              style={{ color: "var(--text-muted)", border: "1px solid var(--border-strong)" }}
              title={collapsed ? "Mostrar la consola" : "Contraer la consola"}
            >
              {collapsed ? "Mostrar" : "Contraer"}
            </button>
          )}
        </div>
      </div>

      {/* Barra indeterminada (animada, sin ancho fijo) mientras corre un
          paso de un solo tramo (total<=1) — de lo contrario un 1/1 se veía
          lleno al 100% desde el instante en que arrancaba, sin transmitir
          que seguía en curso. Con total>1 sí se usa el ancho real. */}
      {!collapsed && last && (
        <div className="h-1 overflow-hidden" style={{ background: "var(--border)" }}>
          {isRunning && last.total <= 1 ? (
            <div
              className="h-full"
              style={{ width: "30%", background: "var(--accent)", animation: "indeterminate-bar 1.2s ease-in-out infinite" }}
            />
          ) : (
            <div className="h-full transition-all" style={{ width: `${pct}%`, background: "var(--accent)" }} />
          )}
        </div>
      )}

      {/* Contraída se oculta con CSS, no se desmonta: el historial de
          eventos vive en el estado de este componente y hay que conservarlo
          para que al volver a abrirla esté todo lo que pasó mientras tanto. */}
      <div
        ref={logRef}
        className="flex-1 overflow-y-auto px-4 py-2 text-xs"
        style={{ fontFamily: "var(--font-mono)", display: collapsed ? "none" : "block" }}
      >
        {listenError ? (
          <div style={{ color: "var(--fail)" }}>No se pudo suscribir a module_progress: {listenError}</div>
        ) : events.length === 0 ? (
          <div style={{ color: "var(--text-faint)" }}>Sin actividad.</div>
        ) : (
          events.map((e, i) => (
            <div
              key={i}
              style={{
                color: e.estado === "error" ? "var(--fail)" : e.estado === "ok" ? "var(--ok)" : "var(--text-muted)",
              }}
            >
              {modulePrefix(e.modulo)} [{e.progreso}/{e.total}] {e.etapa} — {e.item}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
