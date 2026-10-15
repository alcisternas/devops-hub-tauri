import { openUrl } from "@tauri-apps/plugin-opener";
import CollapsibleSection from "../../components/CollapsibleSection";
import type { ResultadoRepo, ResumenRun } from "./shared";

// Color por estado. Son los tres tokens de estado del hub, usados para
// codificar información real y no para decorar.
function colorEstado(estado: ResultadoRepo["estado"]): string {
  if (estado === "creado") return "var(--ok)";
  if (estado === "ya_existia") return "var(--partial)";
  return "var(--fail)";
}

function etiquetaEstado(estado: ResultadoRepo["estado"]): string {
  if (estado === "creado") return "creado";
  if (estado === "ya_existia") return "ya existía";
  return "error";
}

export default function ResultsPanel({ resumen }: { resumen: ResumenRun }) {
  const creados = resumen.resultados.filter((r) => r.estado === "creado");

  return (
    <div className="space-y-3">
      {/* ── Totales ── */}
      <div
        className="p-4 rounded-lg border"
        style={{ borderColor: "var(--border)", background: "var(--surface)" }}
      >
        <div className="flex items-baseline gap-6 mb-4">
          <div>
            <span className="text-2xl font-semibold" style={{ color: "var(--ok)" }}>
              {resumen.creados}
            </span>
            <span className="text-sm ml-1.5" style={{ color: "var(--text-muted)" }}>
              de {resumen.total} creados
            </span>
          </div>
          {resumen.omitidos > 0 && (
            <div className="text-sm" style={{ color: "var(--partial)" }}>
              {resumen.omitidos} ya existían
            </div>
          )}
          {resumen.conError > 0 && (
            <div className="text-sm" style={{ color: "var(--fail)" }}>
              {resumen.conError} con error
            </div>
          )}
        </div>

        <div className="space-y-2">
          {resumen.resultados.map((r) => (
            <div
              key={r.repo}
              className="px-3 py-2 rounded-md"
              style={{ background: "var(--surface-raised)" }}
            >
              <div className="flex items-center gap-2 flex-wrap">
                <span
                  className="w-1.5 h-1.5 rounded-full shrink-0"
                  style={{ background: colorEstado(r.estado) }}
                />
                <span className="text-sm" style={{ fontFamily: "var(--font-mono)", color: "var(--text)" }}>
                  {r.repo}
                </span>
                <span className="text-xs" style={{ color: colorEstado(r.estado) }}>
                  {etiquetaEstado(r.estado)}
                </span>
                {r.url && (
                  // openUrl abre en el navegador del sistema. Un <a> normal
                  // intentaría navegar dentro de la ventana de la app.
                  <button
                    onClick={() => openUrl(r.url!)}
                    className="text-xs ml-auto underline"
                    style={{ color: "var(--accent)" }}
                  >
                    Abrir en Bitbucket
                  </button>
                )}
              </div>

              {r.estado !== "creado" && (
                <div className="text-xs mt-1 ml-3.5" style={{ color: "var(--text-muted)" }}>
                  {r.detalle}
                </div>
              )}

              {/* Advertencias: el repositorio existe y sirve, pero algún
                  paso posterior quedó pendiente y hay que completarlo a mano. */}
              {r.advertencias.length > 0 && (
                <div className="mt-1 ml-3.5 space-y-0.5">
                  {r.advertencias.map((a, i) => (
                    <div key={i} className="text-xs" style={{ color: "var(--partial)" }}>
                      Pendiente — {a}
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* ── SonarQube: el procedimiento es idéntico para todos, así que se
          explica UNA vez y después se lista la key de cada uno. Con muchos
          repositorios, repetirlo entero por cada uno era inservible. ── */}
      {creados.length > 0 && (
        <CollapsibleSection
          title="Paso siguiente — SonarQube Cloud"
          subtitle="Manual: el widget de Bitbucket no tiene API"
          defaultOpen
        >
          <div className="space-y-4">
            <div className="space-y-2 text-xs" style={{ color: "var(--text)" }}>
              <div>
                En el repositorio: Settings → SonarQube Cloud → Settings → Log in with Bitbucket →
                activar "Show repository overview widget" → Save
              </div>
              <div>
                En SonarQube Cloud: "+" → Analyze new project → organización Banco Ripley → buscar
                el repositorio → Set Up → Create project
              </div>
              <div>
                Corregir la project key: Administración → Update key, usando la key de la tabla.
              </div>
            </div>

            <div>
              <div className="text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
                Key esperada por repositorio
              </div>
              <div className="rounded-md overflow-hidden" style={{ background: "var(--surface-raised)" }}>
                {creados.map((r) => (
                  <div
                    key={r.repo}
                    className="flex items-center justify-between gap-4 px-3 py-1.5 text-xs"
                    style={{ fontFamily: "var(--font-mono)" }}
                  >
                    <span style={{ color: "var(--text-muted)" }}>{r.repo}</span>
                    <span style={{ color: "var(--accent)" }}>{r.sonarKey}</span>
                  </div>
                ))}
              </div>
            </div>

            <div className="text-xs" style={{ color: "var(--text-faint)" }}>
              Procedimiento completo en los Runbooks S19 P003 y P004.
            </div>
          </div>
        </CollapsibleSection>
      )}
    </div>
  );
}
