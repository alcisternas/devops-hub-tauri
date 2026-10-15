import { useState } from "react";
import { Button, WarningNotice } from "../../components/ui";
import type { Accion, VerificacionRepo } from "./shared";

interface Props {
  verificaciones: VerificacionRepo[];
  // Acción elegida por archivo, con la clave "repo|ruta".
  acciones: Record<string, Accion>;
  onCambiar: (clave: string, accion: Accion) => void;
  onAplicarATodos: (accion: Accion) => void;
  // Repositorios cancelados completos.
  cancelados: string[];
  onCancelarRepo: (repo: string, cancelar: boolean) => void;
}

const ETIQUETA: Record<Accion, string> = {
  sobrescribir: "Sobrescribir",
  renombrar: "Guardar como .new",
  omitir: "No subir",
};

// Qué archivos ya existen en la rama y qué hacer con cada uno.
//
// Se muestra la lista completa antes de decidir, no archivo por archivo:
// así se ve el alcance real y se puede aplicar una sola decisión a todos.
// Nada de esto escribe en Bitbucket: la verificación solo leyó.
export default function ConflictosPanel({
  verificaciones,
  acciones,
  onCambiar,
  onAplicarATodos,
  cancelados,
  onCancelarRepo,
}: Props) {
  const [detalle, setDetalle] = useState(false);
  const conConflictos = verificaciones.filter((v) => v.conflictos.length > 0);
  if (!conConflictos.length) return null;

  const total = conConflictos.reduce((a, v) => a + v.conflictos.length, 0);

  return (
    <div className="space-y-4">
      <WarningNotice>
        {total} archivo{total === 1 ? "" : "s"} ya existe{total === 1 ? "" : "n"} en la rama. Hay que
        decidir qué hacer con cada uno antes de subir.
      </WarningNotice>

      {/* La decisión habitual es una sola para todos: o se actualizan los
          archivos o se sube una copia. Tener que elegir archivo por
          archivo era trabajo de más, y dejar copias obliga después a
          borrar el original y renombrar a mano. */}
      <div
        className="rounded-lg border p-4"
        style={{ borderColor: "var(--border)", background: "var(--surface)" }}
      >
        <div className="text-sm mb-1" style={{ color: "var(--text)" }}>
          ¿Qué hacer con los archivos que ya existen?
        </div>
        <div className="text-xs mb-3" style={{ color: "var(--text-faint)" }}>
          Actualizar reemplaza el contenido del archivo. Subir una copia deja el original intacto y
          agrega uno nuevo con sufijo .new, que después hay que revisar a mano.
        </div>
        <div className="flex items-center gap-2">
          <Button variant="primary" onClick={() => onAplicarATodos("sobrescribir")}>
            Actualizar todos
          </Button>
          <Button variant="secondary" onClick={() => onAplicarATodos("renombrar")}>
            Subir copia de todos
          </Button>
          <Button variant="secondary" onClick={() => onAplicarATodos("omitir")}>
            No subir ninguno
          </Button>
        </div>
      </div>

      <button
        onClick={() => setDetalle((v) => !v)}
        className="text-xs"
        style={{ color: "var(--accent)" }}
      >
        {detalle ? "− Ocultar el detalle por archivo" : `+ Decidir archivo por archivo (${total})`}
      </button>

      <div style={{ display: detalle ? "block" : "none" }} className="space-y-4">
      {conConflictos.map((v) => {
        const cancelado = cancelados.includes(v.repo);
        return (
          <div
            key={v.repo}
            className="rounded-lg border p-4"
            style={{ borderColor: "var(--border)", background: "var(--surface)" }}
          >
            <div className="flex items-center justify-between gap-4 mb-3">
              <div className="text-sm" style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>
                {v.repo}
              </div>
              {/* Cancelar el repositorio completo: ninguno de sus archivos
                  se sube. Los repositorios ya subidos no se revierten. */}
              <Button
                variant={cancelado ? "accent-outline" : "danger-outline"}
                onClick={() => onCancelarRepo(v.repo, !cancelado)}
              >
                {cancelado ? "Volver a incluir" : "Cancelar este repositorio"}
              </Button>
            </div>

            <div style={{ opacity: cancelado ? 0.45 : 1 }}>
              {v.conflictos.map((ruta) => {
                const clave = `${v.repo}|${ruta}`;
                const actual = acciones[clave] || "renombrar";
                return (
                  <div key={ruta} className="flex items-center gap-3 py-1.5">
                    <div
                      className="text-xs flex-1 truncate"
                      style={{ color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}
                      title={ruta}
                    >
                      {ruta}
                    </div>
                    <div className="flex gap-1.5 shrink-0">
                      {(["sobrescribir", "renombrar", "omitir"] as Accion[]).map((a) => (
                        <button
                          key={a}
                          disabled={cancelado}
                          onClick={() => onCambiar(clave, a)}
                          className="text-xs px-2.5 py-1 rounded-md disabled:opacity-40"
                          style={{
                            color: actual === a ? "white" : "var(--text-muted)",
                            background: actual === a ? "var(--accent)" : "transparent",
                            border: `1px solid ${actual === a ? "var(--accent)" : "var(--border-strong)"}`,
                          }}
                        >
                          {ETIQUETA[a]}
                        </button>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        );
      })}

      </div>
    </div>
  );
}
