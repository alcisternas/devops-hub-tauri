import CollapsibleSection from "../../components/CollapsibleSection";
import { MetaLine } from "../../components/ui";
import { NOMBRE_SEVERIDAD, SEVERIDADES, digestCorto, tamano, type ResultadoAnalisis, type Severidad } from "./shared";

const COLOR_SEV: Record<Severidad, string> = {
  CRITICAL: "var(--fail)",
  HIGH: "var(--fail)",
  MEDIUM: "var(--partial)",
  LOW: "var(--text-muted)",
  UNKNOWN: "var(--text-faint)",
};

function Dato({ etiqueta, valor, mono = false }: { etiqueta: string; valor: string; mono?: boolean }) {
  return (
    <div className="min-w-0">
      <div className="text-[11px]" style={{ color: "var(--text-faint)" }}>
        {etiqueta}
      </div>
      <div className="text-xs truncate" style={{ color: "var(--text)", fontFamily: mono ? "var(--font-mono)" : undefined }} title={valor}>
        {valor}
      </div>
    </div>
  );
}

export default function ResumenPanel({ analisis }: { analisis: ResultadoAnalisis }) {
  const { imagen, os, counts, scanner } = analisis;
  const hc = (counts.CRITICAL ?? 0) + (counts.HIGH ?? 0);
  const sinParche = counts.high_critical_unfixed ?? 0;

  return (
    <div className="p-4 rounded-lg border space-y-4" style={{ borderColor: "var(--border)", background: "var(--surface)" }}>
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-x-6 gap-y-3">
        <Dato etiqueta="Imagen" valor={imagen.reference} mono />
        <Dato etiqueta="Digest fijado" valor={digestCorto(imagen.manifest_digest)} mono />
        <Dato etiqueta="Sistema base" valor={`${os.family ?? "desconocido"} ${os.name ?? ""}`.trim()} />
        <Dato etiqueta="Tipo detectado" valor={analisis.ecosistema === "generic" ? "genérica" : analisis.ecosistema} />
        <Dato etiqueta="Plataforma" valor={imagen.platform} />
        <Dato etiqueta="Tamaño comprimido" valor={`${tamano(imagen.size_bytes)} · ${imagen.layers} capas`} />
        <Dato etiqueta="Paquetes escaneados" valor={String(counts.packages ?? 0)} />
        <Dato etiqueta="Escáner" valor={`${scanner.name} ${scanner.version ?? ""}`.trim()} />
      </div>

      <div className="flex items-stretch gap-2 flex-wrap">
        {SEVERIDADES.map((s) => (
          <div key={s} className="px-3 py-2 rounded-md min-w-24" style={{ background: "var(--surface-raised)" }}>
            <div className="text-lg font-semibold leading-tight" style={{ color: (counts[s] ?? 0) > 0 ? COLOR_SEV[s] : "var(--text-faint)" }}>
              {counts[s] ?? 0}
            </div>
            <div className="text-[11px]" style={{ color: "var(--text-muted)" }}>
              {NOMBRE_SEVERIDAD[s]}
            </div>
          </div>
        ))}
        <div className="px-3 py-2 rounded-md flex-1 min-w-48 flex items-center text-xs" style={{ background: "var(--surface-raised)", color: "var(--text-muted)" }}>
          {hc === 0
            ? "Sin hallazgos HIGH/CRITICAL."
            : `${hc} HIGH/CRITICAL${sinParche > 0 ? `, ${sinParche} sin corrección publicada` : ", todos con corrección publicada"}. La política bancaria cuenta también los que no tienen parche.`}
        </div>
      </div>

      {analisis.detectados.derived_image_warning && (
        <CollapsibleSection title="La imagen incluye una aplicación propia" subtitle="Paquetes de lenguaje fuera de las herramientas de Node.js" defaultOpen>
          <div className="space-y-1 text-xs" style={{ color: "var(--text)" }}>
            <div>Esta imagen parece derivada: además de Node.js contiene dependencias de una aplicación. El Dockerfile endurecido las dejaría fuera.</div>
            {analisis.detectados.other_language_locations.map((u) => (
              <div key={u.path} style={{ fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
                /{u.path} — {u.packages} paquetes
              </div>
            ))}
          </div>
        </CollapsibleSection>
      )}

      {!analisis.soporte.generador && analisis.soporte.motivo && <MetaLine>{analisis.soporte.motivo}</MetaLine>}
    </div>
  );
}
