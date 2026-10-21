import { useState } from "react";
import CollapsibleSection from "../../components/CollapsibleSection";
import { Button } from "../../components/ui";
import { SEVERIDADES, type HallazgoVista, type Severidad } from "./shared";

const COLOR_SEV: Record<Severidad, string> = {
  CRITICAL: "var(--fail)",
  HIGH: "var(--fail)",
  MEDIUM: "var(--partial)",
  LOW: "var(--text-muted)",
  UNKNOWN: "var(--text-faint)",
};

const MAX_FILAS = 200;

export default function HallazgosTabla({ hallazgos }: { hallazgos: HallazgoVista[] }) {
  const [filtro, setFiltro] = useState<Severidad | "TODAS">("TODAS");
  const orden = (s: Severidad) => SEVERIDADES.indexOf(s);
  const lista = hallazgos
    .filter((h) => filtro === "TODAS" || h.severity === filtro)
    .sort((a, b) => orden(a.severity) - orden(b.severity) || a.pkg_name.localeCompare(b.pkg_name));

  return (
    <CollapsibleSection title="Hallazgos del escaneo" subtitle={`${hallazgos.length} vulnerabilidades`} defaultOpen={false}>
      <div className="space-y-3">
        <div className="flex items-center gap-1.5 flex-wrap">
          {(["TODAS", ...SEVERIDADES] as const).map((s) => (
            <Button key={s} variant={filtro === s ? "accent-outline" : "secondary"} className="text-[11px] px-2 py-1" onClick={() => setFiltro(s)}>
              {s === "TODAS" ? "Todas" : s}
            </Button>
          ))}
        </div>
        <div className="rounded-md overflow-y-auto max-h-80" style={{ background: "var(--surface-raised)" }}>
          {lista.slice(0, MAX_FILAS).map((h, i) => (
            <div key={`${h.id}-${h.pkg_name}-${h.pkg_path}-${i}`} className="grid grid-cols-[84px_150px_1fr_130px] gap-3 px-3 py-1.5 text-xs items-baseline">
              <span style={{ color: COLOR_SEV[h.severity] }}>{h.severity}</span>
              <span style={{ fontFamily: "var(--font-mono)", color: "var(--text)" }}>{h.id}</span>
              <span className="truncate" style={{ color: "var(--text-muted)" }} title={h.title}>
                {h.pkg_name} {h.installed_version}
                {h.fixed_version ? ` → ${h.fixed_version}` : " (sin corrección)"}
              </span>
              <span className="text-right" style={{ color: "var(--text-faint)" }}>
                {h.owner}
              </span>
            </div>
          ))}
          {lista.length === 0 && (
            <div className="px-3 py-3 text-xs" style={{ color: "var(--text-faint)" }}>
              Sin hallazgos con este filtro.
            </div>
          )}
          {lista.length > MAX_FILAS && (
            <div className="px-3 py-2 text-xs" style={{ color: "var(--text-faint)" }}>
              Se muestran {MAX_FILAS} de {lista.length}. Filtra por severidad para ver el resto.
            </div>
          )}
        </div>
      </div>
    </CollapsibleSection>
  );
}
