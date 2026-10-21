import { useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import CollapsibleSection from "../../components/CollapsibleSection";
import { Button, ErrorNotice, MetaLine } from "../../components/ui";
import { digestCorto, type Herramientas, type ResultadoGeneracion } from "./shared";

interface Props {
  generado: ResultadoGeneracion | null;
  // Por qué no se puede generar todavía; null si se puede.
  motivoBloqueo: string | null;
  generando: boolean;
  onGenerar: () => void;
  error: string;
  npmRegistry: string;
  onNpmRegistry: (v: string) => void;
  pruebasRed: boolean;
  onPruebasRed: (v: boolean) => void;
  ruta: string;
  guardando: boolean;
  onGuardar: () => void;
  errorGuardar: string;
  herramientas: Herramientas | null;
}

function Comando({ titulo, comando }: { titulo: string; comando: string }) {
  const [copiado, setCopiado] = useState(false);
  async function copiar() {
    try {
      await navigator.clipboard.writeText(comando);
      setCopiado(true);
      setTimeout(() => setCopiado(false), 1500);
    } catch {
      // Sin portapapeles disponible el usuario aún puede seleccionar el texto.
    }
  }
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs" style={{ color: "var(--text-muted)" }}>
          {titulo}
        </span>
        <Button variant="secondary" className="text-[11px] px-2 py-0.5" onClick={copiar}>
          {copiado ? "Copiado" : "Copiar"}
        </Button>
      </div>
      <pre
        className="px-3 py-2 rounded-md text-xs overflow-x-auto whitespace-pre-wrap break-all"
        style={{ background: "var(--surface-raised)", color: "var(--text)", fontFamily: "var(--font-mono)" }}
      >
        {comando}
      </pre>
    </div>
  );
}

function VisorDockerfile({ doc }: { doc: ResultadoGeneracion["doc"] }) {
  const lineas = doc.text.split("\n");
  return (
    <div className="rounded-md overflow-auto max-h-[28rem] py-2" style={{ background: "var(--surface-raised)" }}>
      {doc.explanations.map((e) => (
        <div key={e.start} className="mb-3 last:mb-0">
          <div className="px-3 pb-1 text-[11px]" style={{ color: "var(--text-faint)" }}>
            {e.text}
          </div>
          {lineas.slice(e.start - 1, e.end).map((l, i) => (
            <div key={i} className="flex gap-3 px-3 text-xs leading-relaxed" style={{ fontFamily: "var(--font-mono)" }}>
              <span className="w-8 text-right shrink-0 select-none" style={{ color: "var(--text-faint)" }}>
                {e.start + i}
              </span>
              <span className="whitespace-pre" style={{ color: l.trimStart().startsWith("#") ? "var(--text-muted)" : "var(--text)" }}>
                {l}
              </span>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

export default function DockerfilePanel({
  generado,
  motivoBloqueo,
  generando,
  onGenerar,
  error,
  npmRegistry,
  onNpmRegistry,
  pruebasRed,
  onPruebasRed,
  ruta,
  guardando,
  onGuardar,
  errorGuardar,
  herramientas,
}: Props) {
  return (
    <div className="p-4 rounded-lg border space-y-4" style={{ borderColor: "var(--border)", background: "var(--surface)" }}>
      <div>
        <div className="text-sm font-semibold mb-1" style={{ color: "var(--text)" }}>
          Dockerfile endurecido
        </div>
        <div className="text-xs" style={{ color: "var(--text-muted)" }}>
          Se arma una imagen nueva desde cero con solo lo que elegiste conservar; las capas de la imagen original no forman parte del resultado.
        </div>
      </div>

      <div className="flex items-end gap-4 flex-wrap">
        <label className="flex flex-col gap-1.5 flex-1 min-w-64">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            Registro de npm (usado para corregir dependencias y en las pruebas)
          </span>
          <input
            value={npmRegistry}
            onChange={(e) => onNpmRegistry(e.target.value)}
            disabled={generando}
            spellCheck={false}
            className="px-3 py-2 rounded-md text-sm outline-hidden"
            style={{ background: "var(--surface-raised)", border: "1px solid var(--border-strong)", color: "var(--text)", fontFamily: "var(--font-mono)" }}
          />
        </label>
        <label className="flex items-center gap-2 cursor-pointer pb-2">
          <input
            type="checkbox"
            checked={pruebasRed}
            onChange={(e) => onPruebasRed(e.target.checked)}
            disabled={generando}
            className="w-4 h-4 cursor-pointer"
            style={{ accentColor: "var(--accent)" }}
          />
          <span className="text-xs" style={{ color: "var(--text)" }}>
            Incluir pruebas que usan la red
          </span>
        </label>
        <Button onClick={onGenerar} disabled={generando || motivoBloqueo !== null}>
          {generando ? "Generando…" : generado ? "Regenerar" : "Generar Dockerfile"}
        </Button>
      </div>

      {motivoBloqueo && <MetaLine>{motivoBloqueo}</MetaLine>}
      {error && <ErrorNotice>{error}</ErrorNotice>}

      {generado && (
        <div className="space-y-4">
          <div className="flex items-center gap-x-6 gap-y-1 flex-wrap text-xs" style={{ color: "var(--text-muted)" }}>
            <span>
              Origen <span style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>{digestCorto(generado.doc.meta.src_ref.split("@")[1] ?? "")}</span>
            </span>
            <span>
              Constructor <span style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>alpine {generado.doc.meta.alpine}</span>
            </span>
            <span>
              Huella <span style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>{generado.doc.sha256.slice(0, 12)}…</span>
            </span>
            <span>{generado.doc.expected_tests.length} pruebas funcionales incluidas</span>
          </div>

          <VisorDockerfile doc={generado.doc} />

          <MetaLine>
            Archivos que acompañan al Dockerfile: {Object.keys(generado.doc.files).join(", ") || "ninguno"}.
          </MetaLine>

          <div className="flex items-center gap-3 flex-wrap">
            <Button variant="accent-outline" onClick={onGuardar} disabled={guardando}>
              {guardando ? "Guardando…" : ruta ? "Guardar de nuevo" : "Guardar en una carpeta"}
            </Button>
            {ruta && (
              <>
                <span className="text-xs" style={{ color: "var(--ok)", fontFamily: "var(--font-mono)" }}>
                  {ruta}
                </span>
                <Button variant="secondary" className="text-xs px-2.5 py-1" onClick={() => revealItemInDir(ruta)}>
                  Mostrar carpeta
                </Button>
              </>
            )}
          </div>
          {errorGuardar && <ErrorNotice>{errorGuardar}</ErrorNotice>}

          <CollapsibleSection
            title="Siguientes pasos — construir, verificar y subir a ECR"
            subtitle={herramientas?.docker.instalada ? "Docker detectado" : "Requiere Docker con BuildKit"}
            defaultOpen
          >
            <div className="space-y-3">
              <div className="text-xs" style={{ color: "var(--text-muted)" }}>
                Esta versión genera el Dockerfile pero todavía no construye ni reescanea la imagen: hasta hacerlo no hay garantía de que cumpla la
                política. Ejecuta estos pasos desde la carpeta guardada, en este orden. Reemplaza los valores entre {"<>"} en los pasos de ECR.
              </div>
              {generado.pasos.map((p) => (
                <Comando key={p.titulo} titulo={p.titulo} comando={p.comando} />
              ))}
            </div>
          </CollapsibleSection>
        </div>
      )}
    </div>
  );
}
