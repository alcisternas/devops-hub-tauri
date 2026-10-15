import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, ErrorNotice, SuccessNotice, WarningNotice } from "../../components/ui";
import type { ResultadoRepo } from "./shared";

interface Props {
  resultados: ResultadoRepo[];
  onVolver: () => void;
}

// Resultado de la subida, un bloque por repositorio.
//
// Los repositorios que fallaron se muestran con su error completo: si uno
// falló y otros no, hay que poder reintentar solo ese, y para eso hace
// falta saber qué pasó.
export default function ResultadosPanel({ resultados, onVolver }: Props) {
  const ok = resultados.filter((r) => r.ok);
  const fallidos = resultados.filter((r) => !r.ok);
  const totalArchivos = ok.reduce((a, r) => a + r.subidos.length + r.renombrados.length, 0);

  return (
    <div className="space-y-4">
      {fallidos.length === 0 ? (
        <SuccessNotice>
          {totalArchivos} archivo{totalArchivos === 1 ? "" : "s"} subido
          {totalArchivos === 1 ? "" : "s"} en {ok.length} repositorio{ok.length === 1 ? "" : "s"}.
        </SuccessNotice>
      ) : (
        <ErrorNotice>
          {fallidos.length} de {resultados.length} repositorios fallaron. Los que terminaron bien no
          se revierten.
        </ErrorNotice>
      )}

      {resultados.map((r) => (
        <div
          key={r.repo}
          className="rounded-lg border p-4"
          style={{
            borderColor: "var(--border)",
            background: "var(--surface)",
            borderLeft: `3px solid ${r.ok ? "var(--ok)" : "var(--fail)"}`,
          }}
        >
          <div className="flex items-center justify-between gap-4">
            <div className="text-sm" style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>
              {r.repo}
            </div>
            {/* openUrl abre en el navegador del sistema: un <a> normal
                intentaría navegar dentro de la ventana de la aplicación. */}
            {r.url && (
              <Button variant="accent-outline" onClick={() => openUrl(r.url!)}>
                Ver en Bitbucket
              </Button>
            )}
          </div>

          <div className="mt-2 space-y-1">
            {r.subidos.map((ruta) => (
              <div
                key={ruta}
                className="text-xs"
                style={{ color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}
              >
                {ruta}
              </div>
            ))}
            {r.renombrados.map(([pedida, efectiva]) => (
              <div
                key={pedida}
                className="text-xs"
                style={{ color: "var(--partial)", fontFamily: "var(--font-mono)" }}
              >
                {pedida} → {efectiva}
              </div>
            ))}
          </div>

          {r.omitidos.length > 0 && (
            <div className="mt-2">
              <WarningNotice>
                Sin subir: {r.omitidos.join(", ")}. Hay que aplicarlos a mano si hacen falta.
              </WarningNotice>
            </div>
          )}

          {r.error && (
            <div className="mt-2">
              <ErrorNotice>{r.error}</ErrorNotice>
            </div>
          )}
        </div>
      ))}

      <div className="text-xs leading-relaxed" style={{ color: "var(--text-faint)" }}>
        Los archivos quedaron en la rama de desarrollo. La promoción a release y master sigue siendo
        por merge normal.
      </div>

      <Button variant="secondary" onClick={onVolver}>
        Volver a los archivos
      </Button>
    </div>
  );
}
