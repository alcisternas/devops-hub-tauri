import { Button, Dropdown, MetaLine } from "../../components/ui";
import { PLATAFORMAS, type Herramientas } from "./shared";

interface Props {
  referencia: string;
  onReferencia: (v: string) => void;
  plataforma: string;
  onPlataforma: (v: string) => void;
  escaner: string;
  onEscaner: (v: string) => void;
  herramientas: Herramientas | null;
  analizando: boolean;
  onAnalizar: () => void;
}

function Estado({ ok, texto }: { ok: boolean; texto: string }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-xs" style={{ color: "var(--text-muted)" }}>
      <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ background: ok ? "var(--ok)" : "var(--text-faint)" }} />
      {texto}
    </span>
  );
}

export default function ImagenForm({
  referencia,
  onReferencia,
  plataforma,
  onPlataforma,
  escaner,
  onEscaner,
  herramientas,
  analizando,
  onAnalizar,
}: Props) {
  const trivy = herramientas?.trivy;
  const scout = herramientas?.scout;
  const docker = herramientas?.docker;
  // Sin el escáner elegido no se puede analizar: se avisa antes de pulsar.
  const sinEscaner = escaner === "trivy" && herramientas !== null && !trivy?.instalada;

  return (
    <div className="p-4 rounded-lg border space-y-4" style={{ borderColor: "var(--border)", background: "var(--surface)" }}>
      <label className="flex flex-col gap-1.5">
        <span className="text-xs" style={{ color: "var(--text-muted)" }}>
          Imagen a endurecer
        </span>
        <input
          value={referencia}
          onChange={(e) => onReferencia(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && referencia.trim() && !analizando && !sinEscaner) onAnalizar();
          }}
          placeholder="node:26.12.0-alpine3.24"
          autoCapitalize="none"
          autoCorrect="off"
          spellCheck={false}
          disabled={analizando}
          className="px-3 py-2 rounded-md text-sm outline-hidden"
          style={{
            background: "var(--surface-raised)",
            border: "1px solid var(--border-strong)",
            color: "var(--text)",
            fontFamily: "var(--font-mono)",
          }}
        />
      </label>

      <div className="flex items-end gap-6 flex-wrap">
        <div className="flex flex-col gap-1.5">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            Plataforma
          </span>
          <Dropdown
            value={plataforma}
            onChange={onPlataforma}
            options={PLATAFORMAS.map((p) => ({ value: p, label: p }))}
            className="w-40"
          />
        </div>

        <div className="flex flex-col gap-1.5">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            Escáner de vulnerabilidades
          </span>
          <div className="flex items-center gap-2">
            <Button
              variant={escaner === "trivy" ? "accent-outline" : "secondary"}
              className="text-xs px-3 py-2"
              onClick={() => onEscaner("trivy")}
              disabled={analizando}
            >
              Trivy
            </Button>
            {/* Docker Scout aparece, pero no se puede elegir hasta validar su lector
                contra una salida real; así queda claro que existe y por qué no se usa. */}
            <Button variant="secondary" className="text-xs px-3 py-2" disabled title="Pendiente de validar con una salida real de Docker Scout">
              Docker Scout
            </Button>
          </div>
        </div>

        <Button onClick={onAnalizar} disabled={analizando || !referencia.trim() || sinEscaner}>
          {analizando ? "Analizando…" : "Analizar imagen"}
        </Button>
      </div>

      <div className="flex items-center gap-5 flex-wrap">
        <Estado ok={!!trivy?.instalada} texto={trivy?.instalada ? `Trivy ${trivy.version ?? ""}` : "Trivy no instalado"} />
        <Estado
          ok={!!scout?.instalada}
          texto={scout?.instalada ? "Docker Scout instalado" : "Docker Scout no disponible"}
        />
        <Estado ok={!!docker?.instalada} texto={docker?.instalada ? `Docker ${docker.version ?? ""}` : "Docker no instalado (no hace falta para analizar)"} />
      </div>

      {sinEscaner && <MetaLine>Trivy es necesario para analizar. Instálalo (por ejemplo con "brew install trivy") y vuelve a abrir esta pantalla.</MetaLine>}
      {!sinEscaner && (
        <MetaLine>
          La imagen no se descarga: se resuelve en el registro, se fija por digest y se escanea. Se admiten imágenes públicas
          (Docker Hub, GHCR y registros compatibles).
        </MetaLine>
      )}
    </div>
  );
}
