import { Button } from "../../components/ui";
import { ramaDesarrolloPreview, validarRepo, type ConfigRun } from "./shared";

interface Props {
  config: ConfigRun;
  ejecutando: boolean;
  onConfirmar: () => void;
  onCancelar: () => void;
}

// Panel de confirmación — se despliega en la misma pantalla, no es un
// diálogo encima. Crear repositorios no se deshace solo, así que el paso
// existe para que se pueda revisar lo que va a pasar antes de que pase.
export default function ConfirmPanel({ config, ejecutando, onConfirmar, onCancelar }: Props) {
  const ramaDev = ramaDesarrolloPreview(config.pais);
  const fueraDePatron = config.repos.filter((r) => !validarRepo(r).ok);

  const filas: [string, string][] = [
    ["Workspace", config.workspace],
    ["Proyecto", config.projectKey],
    ["País", config.pais === "PE" ? "Perú" : "Chile"],
    ["Rama por defecto", "master"],
  ];

  const acciones = [
    `Crear ${config.repos.length} repositorio${config.repos.length === 1 ? "" : "s"} privado${
      config.repos.length === 1 ? "" : "s"
    }, sin forks, en el proyecto ${config.projectKey}`,
    "Commit inicial con .gitignore y README.md estándar",
    config.activarPipelines ? "Activar Pipelines" : null,
    config.crearRamas ? `Crear ramas ${ramaDev} y release desde master` : null,
  ].filter(Boolean) as string[];

  return (
    <div
      className="p-4 rounded-lg border space-y-4"
      style={{ borderColor: "var(--accent)", background: "var(--surface)" }}
    >
      <div className="text-sm font-semibold" style={{ color: "var(--text)" }}>
        Revisa antes de crear
      </div>

      <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
        {filas.map(([k, v]) => (
          <div
            key={k}
            className="px-3 py-2 rounded-md"
            style={{ background: "var(--surface-raised)" }}
          >
            <div className="text-xs mb-0.5" style={{ color: "var(--text-faint)" }}>
              {k}
            </div>
            <div className="text-sm" style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>
              {v}
            </div>
          </div>
        ))}
      </div>

      <div>
        <div className="text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
          Repositorios ({config.repos.length})
        </div>
        <div
          className="rounded-md px-3 py-2 max-h-40 overflow-y-auto"
          style={{ background: "var(--surface-raised)" }}
        >
          {config.repos.map((r) => {
            const v = validarRepo(r);
            return (
              <div
                key={r}
                className="text-xs py-0.5"
                style={{
                  fontFamily: "var(--font-mono)",
                  color: v.ok ? "var(--text)" : "var(--partial)",
                }}
              >
                {r}
              </div>
            );
          })}
        </div>
      </div>

      <div>
        <div className="text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
          Por cada repositorio
        </div>
        <div className="space-y-1">
          {acciones.map((a) => (
            <div key={a} className="text-xs" style={{ color: "var(--text)" }}>
              {a}
            </div>
          ))}
        </div>
      </div>

      {fueraDePatron.length > 0 && (
        <div
          className="p-3 rounded-md text-xs"
          style={{ background: "var(--partial-dim)", border: "1px solid #4a3a10", color: "#fde68a" }}
        >
          {fueraDePatron.length} nombre{fueraDePatron.length === 1 ? "" : "s"} no sigue
          {fueraDePatron.length === 1 ? "" : "n"} el patrón estándar. Se pueden crear igual, pero
          conviene revisarlos.
        </div>
      )}

      <div className="flex items-center gap-2 pt-1">
        <Button onClick={onConfirmar} disabled={ejecutando}>
          {ejecutando ? "Creando…" : `Crear ${config.repos.length}`}
        </Button>
        <Button variant="secondary" onClick={onCancelar} disabled={ejecutando}>
          Volver a editar
        </Button>
        {ejecutando && (
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            El detalle va apareciendo en la consola.
          </span>
        )}
      </div>
    </div>
  );
}
