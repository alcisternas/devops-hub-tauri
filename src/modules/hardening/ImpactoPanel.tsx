import CollapsibleSection from "../../components/CollapsibleSection";
import { Button, MetaLine } from "../../components/ui";
import { ESTADO_VEREDICTO, type Bloqueo, type ConfigHardening, type Evaluacion, type ResultadoAnalisis } from "./shared";

interface Props {
  analisis: ResultadoAnalisis;
  evaluacion: Evaluacion;
  config: ConfigHardening;
  onConfig: (c: ConfigHardening) => void;
  evaluando: boolean;
}

const COLOR_CONFIANZA: Record<string, string> = {
  alta: "var(--ok)",
  media: "var(--partial)",
  baja: "var(--fail)",
};

function Cifra({ valor, texto, color }: { valor: number; texto: string; color?: string }) {
  return (
    <div className="px-3 py-2 rounded-md min-w-32" style={{ background: "var(--surface-raised)" }}>
      <div className="text-lg font-semibold leading-tight" style={{ color: valor > 0 && color ? color : "var(--text)" }}>
        {valor}
      </div>
      <div className="text-[11px]" style={{ color: "var(--text-muted)" }}>
        {texto}
      </div>
    </div>
  );
}

function BloqueoCard({
  b,
  etiquetaDe,
  autorizado,
  onAutorizar,
}: {
  b: Bloqueo;
  etiquetaDe: (id: string) => string | null;
  autorizado: boolean;
  onAutorizar: () => void;
}) {
  const etiqueta = etiquetaDe(b.owner);
  return (
    <div className="p-3 rounded-md space-y-2" style={{ background: "var(--fail-dim)", border: "1px solid #4a1d1d" }}>
      <div className="text-xs" style={{ color: "#fca5a5" }}>
        {b.reason}
      </div>
      <div className="text-[11px]" style={{ color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
        {b.cves.join(", ")}
      </div>
      <div className="space-y-1">
        <div className="text-[11px]" style={{ color: "var(--text-faint)" }}>
          Alternativas
        </div>
        {b.alternatives.map((a) => (
          <div key={a.kind} className="flex items-start justify-between gap-3">
            <div className="text-xs" style={{ color: "var(--text)" }}>
              {a.description}
              {a.requires_authorization && <span style={{ color: "var(--text-faint)" }}> (requiere tu autorización)</span>}
            </div>
            {/* Solo se puede ejecutar aquí la eliminación de un componente del catálogo;
                cambiar de base o de versión de Node.js aún no está soportado. */}
            {a.kind === "remove_component" && etiqueta && (
              <Button variant="danger-outline" className="text-xs px-2.5 py-1 shrink-0" onClick={onAutorizar} disabled={autorizado}>
                {autorizado ? "Autorizado" : `Eliminar ${etiqueta}`}
              </Button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

export default function ImpactoPanel({ analisis, evaluacion, config, onConfig, evaluando }: Props) {
  const { analisis: a, plan } = evaluacion;
  const veredicto = ESTADO_VEREDICTO[a.verdict];
  const errores = a.issues.filter((i) => i.level === "error");
  const avisos = a.issues.filter((i) => i.level === "warning");
  const etiquetaDe = (id: string) => analisis.detectados.components.find((c) => c.id === id)?.label ?? null;

  function autorizarEliminar(id: string) {
    if (config.authorize_remove_components.includes(id)) return;
    onConfig({ ...config, authorize_remove_components: [...config.authorize_remove_components, id] });
  }

  return (
    <div className="p-4 rounded-lg border space-y-4" style={{ borderColor: "var(--border)", background: "var(--surface)", opacity: evaluando ? 0.7 : 1 }}>
      <div className="flex items-start gap-2">
        <span className="w-2 h-2 rounded-full mt-1.5 shrink-0" style={{ background: veredicto.color }} />
        <div>
          <div className="text-sm font-semibold" style={{ color: veredicto.color }}>
            {veredicto.texto}
          </div>
          <div className="text-xs mt-0.5" style={{ color: "var(--text-muted)" }}>
            {a.message}
          </div>
        </div>
      </div>

      <div className="flex items-stretch gap-2 flex-wrap">
        <Cifra valor={a.vulnerabilities_removed.length} texto="hallazgos que desaparecen" color="var(--ok)" />
        <Cifra valor={a.remaining_high_critical} texto="HIGH/CRITICAL que permanecen" color="var(--fail)" />
        <Cifra valor={a.fixable.length} texto="paquetes con corrección" color="var(--partial)" />
        <Cifra valor={a.unfixable.length} texto="paquetes sin corrección" color="var(--fail)" />
      </div>

      {errores.map((i) => (
        <div key={i.message} className="text-xs" style={{ color: "var(--fail)" }}>
          {i.message}
        </div>
      ))}
      {avisos.map((i) => (
        <div key={i.message} className="text-xs" style={{ color: "var(--partial)" }}>
          {i.message}
        </div>
      ))}

      {plan.blockers.length > 0 && (
        <div className="space-y-2">
          <div className="text-xs font-semibold" style={{ color: "var(--fail)" }}>
            Bloqueos: no hay una estrategia compatible con esta selección
          </div>
          {plan.blockers.map((b) => (
            <BloqueoCard
              key={`${b.package}-${b.version}-${b.owner}`}
              b={b}
              etiquetaDe={etiquetaDe}
              autorizado={config.authorize_remove_components.includes(b.owner)}
              onAutorizar={() => autorizarEliminar(b.owner)}
            />
          ))}
        </div>
      )}

      {plan.strategies.length > 0 && (
        <CollapsibleSection title="Cómo se corregirían" subtitle={`${plan.strategies.length} estrategias del plan`} defaultOpen>
          <div className="space-y-3">
            {plan.strategies.map((s) => (
              <div key={s.kind + s.title}>
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-xs font-medium" style={{ color: "var(--text)" }}>
                    {s.title}
                  </span>
                  <span className="text-[11px]" style={{ color: COLOR_CONFIANZA[s.confidence] ?? "var(--text-muted)" }}>
                    confianza {s.confidence}
                  </span>
                  {s.requires_authorization && (
                    <span className="text-[11px]" style={{ color: "var(--text-faint)" }}>
                      autorizado por ti
                    </span>
                  )}
                </div>
                <div className="text-xs mt-0.5" style={{ color: "var(--text-muted)" }}>
                  {s.detail}
                </div>
                {s.addresses.length > 0 && (
                  <div className="text-[11px] mt-0.5" style={{ color: "var(--text-faint)", fontFamily: "var(--font-mono)" }}>
                    {s.addresses.join(", ")}
                  </div>
                )}
              </div>
            ))}
            <MetaLine>
              Es una evaluación preliminar sobre el escaneo de origen. Solo construir la imagen y reescanearla comprueba que cumple la política.
            </MetaLine>
          </div>
        </CollapsibleSection>
      )}

      {a.unfixable.length > 0 && (
        <CollapsibleSection title="Paquetes sin corrección publicada" subtitle="Siguen contando para la política" defaultOpen={false}>
          <div className="space-y-1">
            {a.unfixable.map((u) => (
              <div key={`${u.package}-${u.path}`} className="text-xs" style={{ fontFamily: "var(--font-mono)", color: "var(--text)" }}>
                {u.package} {u.version} <span style={{ color: "var(--text-faint)" }}>({u.owner}) — {u.cves.join(", ")}</span>
              </div>
            ))}
          </div>
        </CollapsibleSection>
      )}

      {evaluacion.recomendaciones.length > 0 && (
        <CollapsibleSection title="Recomendaciones" subtitle="Ventajas, costos e incompatibilidades" defaultOpen={false}>
          <div className="space-y-4">
            {evaluacion.recomendaciones.map((r) => (
              <div key={r.id} className="space-y-1">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-xs font-medium" style={{ color: "var(--text)" }}>
                    {r.title}
                  </span>
                  <span className="text-[11px]" style={{ color: COLOR_CONFIANZA[r.confidence] ?? "var(--text-muted)" }}>
                    confianza {r.confidence}
                  </span>
                  {r.requires_authorization && (
                    <span className="text-[11px]" style={{ color: "var(--text-faint)" }}>
                      requiere autorización
                    </span>
                  )}
                </div>
                {r.changes.slice(0, 4).map((c) => (
                  <div key={c} className="text-xs" style={{ color: "var(--text-muted)" }}>
                    {c}
                  </div>
                ))}
                {r.pros.map((p) => (
                  <div key={p} className="text-xs" style={{ color: "var(--ok)" }}>
                    + {p}
                  </div>
                ))}
                {[...r.cons, ...r.incompatibilities].map((c) => (
                  <div key={c} className="text-xs" style={{ color: "var(--partial)" }}>
                    − {c}
                  </div>
                ))}
              </div>
            ))}
          </div>
        </CollapsibleSection>
      )}
    </div>
  );
}
