import { Link } from "react-router-dom";

// Cada módulo migrado agrega su tarjeta acá. El mismo concepto visual
// del hub HTML anterior, ahora como componente React real.
interface Tool {
  to: string;
  icon: string;
  title: string;
  desc: string;
  // true: se ve como una tarjeta normal, clickeable — el hecho de que abra
  // algo al hacer clic ya comunica que está disponible, un badge que
  // dijera "Disponible" ahí sería redundante.
  // false: es donde el estado sí aporta algo — la tarjeta se ve apagada,
  // no es un link, y "note" explica por qué no se puede usar todavía.
  available: boolean;
  note?: string;
}

const TOOLS: Tool[] = [
  {
    to: "/clusters",
    icon: "☁️",
    title: "Clusters",
    desc: "Descubre cuentas EKS/GKE, verifica permisos y carga contextos a kubeconfig.",
    available: true,
  },
  {
    to: "/bitbucket",
    icon: "📦",
    title: "Bitbucket Repo Creator",
    desc: "Crea repositorios con el estándar del equipo: commit inicial, Pipelines y ramas base.",
    available: true,
  },
  // { to: "/cicd", icon: "🚀", title: "CI/CD Automation", desc: "...", available: false, note: "Pendiente de migrar" },
];

export default function Home() {
  return (
    <div className="p-10" style={{ color: "var(--text)" }}>
      <h1 className="text-2xl font-bold mb-1">DevOps Hub</h1>
      <p className="text-sm mb-8" style={{ color: "var(--text-muted)" }}>
        Selecciona la herramienta que necesitas usar
      </p>
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {TOOLS.map((tool) => {
          const content = (
            <>
              <div
                className="w-11 h-11 rounded-lg flex items-center justify-center text-xl mb-4"
                style={{ background: tool.available ? "var(--accent-dim)" : "var(--border)" }}
              >
                {tool.icon}
              </div>
              <div className="font-semibold mb-1.5">{tool.title}</div>
              <div className="text-xs leading-relaxed" style={{ color: "var(--text-muted)" }}>
                {tool.desc}
              </div>
              {!tool.available && tool.note && (
                <div className="text-xs mt-3" style={{ color: "var(--text-faint)" }}>
                  {tool.note}
                </div>
              )}
            </>
          );

          if (!tool.available) {
            return (
              <div
                key={tool.title}
                className="p-6 rounded-lg border opacity-60"
                style={{ borderColor: "var(--border)", background: "var(--surface)" }}
              >
                {content}
              </div>
            );
          }

          return (
            <Link
              key={tool.to}
              to={tool.to}
              className="block p-6 rounded-lg border transition-all hover:-translate-y-0.5 hover:border-[var(--accent)]"
              style={{ borderColor: "var(--border)", background: "var(--surface)" }}
            >
              {content}
            </Link>
          );
        })}
      </div>
    </div>
  );
}
