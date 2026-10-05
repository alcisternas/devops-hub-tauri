import { NavLink } from "react-router-dom";
import type { ReactNode } from "react";
import { useFontScale } from "../hooks/useFontScale";
import { APP_VERSION } from "../version";

// Cada entrada del menú apunta a la ruta de un módulo.
// Para agregar una herramienta nueva: se agrega una línea acá + su <Route> en App.tsx.
const NAV_ITEMS = [
  { to: "/", label: "Inicio", icon: "🏠", end: true },
  { to: "/clusters", label: "Clusters", icon: "☁️" },
  { to: "/bitbucket", label: "Bitbucket", icon: "📦" },
  // { to: "/cicd", label: "CI/CD", icon: "🚀" },
];

export default function Layout({ children }: { children: ReactNode }) {
  const font = useFontScale();

  return (
    <div className="flex h-screen bg-[var(--bg)] text-[var(--text)]">
      <aside className="w-56 shrink-0 border-r border-[var(--border)] bg-[var(--surface)] flex flex-col">
        <div className="px-4 py-4 border-b border-[var(--border)] flex items-baseline justify-between">
          <span className="font-bold text-sm tracking-wide">
            DevOps <span className="text-[var(--accent)]">Hub</span>
          </span>
          <span className="text-[10px]" style={{ color: "var(--text-faint)" }}>
            v{APP_VERSION}
          </span>
        </div>
        <nav className="flex-1 px-2 py-3 space-y-1">
          {NAV_ITEMS.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
              className={({ isActive }) =>
                `flex items-center gap-2 px-3 py-2 rounded-md text-sm font-medium transition-colors ${
                  isActive
                    ? "bg-[var(--accent-dim)] text-[var(--accent)]"
                    : "text-[var(--text-muted)] hover:bg-[var(--surface-raised)] hover:text-[var(--text)]"
                }`
              }
            >
              <span>{item.icon}</span>
              {item.label}
            </NavLink>
          ))}
        </nav>
        <div className="px-3 py-3 border-t border-[var(--border)] flex items-center justify-between">
          <span className="text-[11px]" style={{ color: "var(--text-faint)" }}>
            Tamaño de texto
          </span>
          <div className="flex items-center gap-1">
            <button
              onClick={font.decrease}
              disabled={!font.canDecrease}
              className="w-6 h-6 rounded bg-[var(--surface-raised)] hover:bg-[var(--border-strong)] disabled:opacity-30 text-xs"
              style={{ color: "var(--text-muted)" }}
              title="Reducir texto"
            >
              A-
            </button>
            <button
              onClick={font.reset}
              className="px-1.5 h-6 rounded bg-[var(--surface-raised)] hover:bg-[var(--border-strong)] text-[10px]"
              style={{ color: "var(--text-muted)" }}
              title="Restablecer"
            >
              {Math.round(font.percent)}%
            </button>
            <button
              onClick={font.increase}
              disabled={!font.canIncrease}
              className="w-6 h-6 rounded bg-[var(--surface-raised)] hover:bg-[var(--border-strong)] disabled:opacity-30 text-xs"
              style={{ color: "var(--text-muted)" }}
              title="Aumentar texto"
            >
              A+
            </button>
          </div>
        </div>
      </aside>
      <main className="flex-1 overflow-auto">{children}</main>
    </div>
  );
}
