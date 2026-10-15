import { useState } from "react";
import { WarningNotice } from "../../components/ui";
import CollapsibleSection from "../../components/CollapsibleSection";
import SubirPanel from "./SubirPanel";
import type { ArchivosPorRepo, Hallazgo } from "./generators";
import { MARCADOR } from "./generators";

interface Props {
  archivos: ArchivosPorRepo;
  hallazgos: Hallazgo[];
  ramaDestino: string;
  ticket: string;
  // Si la pestaña está a la vista: el panel de credenciales no consulta
  // el llavero hasta entonces.
  activo: boolean;
}

// Resalta las líneas con un marcador PENDIENTE. Es lo único que se pinta
// dentro del contenido: el resto va en un solo color, porque un YAML con
// medio archivo coloreado se vuelve más difícil de leer, no más fácil.
function Contenido({ texto }: { texto: string }) {
  return (
    <pre
      className="text-xs overflow-x-auto px-3 py-2.5 rounded-md m-0"
      style={{
        background: "var(--bg)",
        border: "1px solid var(--border)",
        color: "var(--text-muted)",
        fontFamily: "var(--font-mono)",
        maxHeight: 320,
        overflowY: "auto",
      }}
    >
      {texto.split("\n").map((linea, i) => (
        <div
          key={i}
          style={
            linea.includes(MARCADOR)
              ? { color: "var(--partial)", background: "var(--partial-dim)" }
              : undefined
          }
        >
          {linea || " "}
        </div>
      ))}
    </pre>
  );
}

export default function FilesTab({ archivos, hallazgos, ramaDestino, ticket, activo }: Props) {
  const repos = Object.keys(archivos);
  const [abierto, setAbierto] = useState<string>("");

  if (!repos.length) {
    return (
      <div className="text-sm" style={{ color: "var(--text-muted)" }}>
        Completa el formulario para ver los archivos. Hacen falta al menos un repositorio y un
        ambiente.
      </div>
    );
  }

  const totalArchivos = Object.values(archivos).reduce((a, f) => a + Object.keys(f).length, 0);

  return (
    <div className="space-y-4">
      <div className="text-sm" style={{ color: "var(--text-muted)" }}>
        {totalArchivos} archivo{totalArchivos === 1 ? "" : "s"} en {repos.length} repositorio
        {repos.length === 1 ? "" : "s"}, para la rama{" "}
        <span style={{ fontFamily: "var(--font-mono)", color: "var(--text)" }}>{ramaDestino}</span>.
      </div>

      {/* Los marcadores se listan arriba, antes de que haya que buscarlos
          leyendo cada archivo. Cuando la subida a Bitbucket sea automática,
          esta lista es la que va a frenar el envío. */}
      {hallazgos.length > 0 && (
        <WarningNotice>
          Hay {hallazgos.length} línea{hallazgos.length === 1 ? "" : "s"} con valores sin resolver:
          {hallazgos.slice(0, 6).map((h, i) => (
            <div key={i} style={{ fontFamily: "var(--font-mono)", marginTop: 4 }}>
              {h.repo} · {h.ruta} · {h.linea}
            </div>
          ))}
          {hallazgos.length > 6 && <div style={{ marginTop: 4 }}>…y {hallazgos.length - 6} más.</div>}
        </WarningNotice>
      )}

      <SubirPanel
        archivos={archivos}
        huecos={hallazgos}
        rama={ramaDestino}
        ticket={ticket}
        activo={activo}
      />

      <div className="text-xs pt-2" style={{ color: "var(--text-muted)" }}>
        Vista previa del contenido
      </div>

      {repos.map((repo) => (
        <CollapsibleSection
          key={repo}
          title={repo}
          subtitle={`${Object.keys(archivos[repo]).length} archivos`}
          defaultOpen={repos.length === 1}
        >
          <div className="space-y-4">
            {Object.entries(archivos[repo]).map(([ruta, contenido]) => {
              const clave = `${repo}|${ruta}`;
              const visible = abierto === clave || Object.keys(archivos[repo]).length <= 4;
              return (
                <div key={ruta}>
                  <button
                    onClick={() => setAbierto(abierto === clave ? "" : clave)}
                    className="text-xs mb-1.5 text-left"
                    style={{ color: "var(--accent)", fontFamily: "var(--font-mono)" }}
                  >
                    {ruta}
                  </button>
                  {visible && <Contenido texto={contenido} />}
                </div>
              );
            })}
          </div>
        </CollapsibleSection>
      ))}
    </div>
  );
}
