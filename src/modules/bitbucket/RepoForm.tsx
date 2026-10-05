import { Dropdown } from "../../components/ui";
import { parsearRepos, validarRepo } from "./shared";

export interface DatosForm {
  workspace: string;
  projectKey: string;
  reposTexto: string;
  pais: string;
  activarPipelines: boolean;
  crearRamas: boolean;
}

interface Props {
  datos: DatosForm;
  onCambio: (datos: DatosForm) => void;
  deshabilitado: boolean;
}

const estiloInput = {
  background: "var(--surface-raised)",
  border: "1px solid var(--border-strong)",
  color: "var(--text)",
};

export default function RepoForm({ datos, onCambio, deshabilitado }: Props) {
  // Helper para no repetir el spread en cada campo.
  const set = <K extends keyof DatosForm>(campo: K, valor: DatosForm[K]) =>
    onCambio({ ...datos, [campo]: valor });

  const repos = parsearRepos(datos.reposTexto);

  return (
    <div
      className="p-4 rounded-lg border space-y-4"
      style={{ borderColor: "var(--border)", background: "var(--surface)" }}
    >
      {/* ── Workspace y proyecto ── */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <label className="flex flex-col gap-1.5">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            Workspace
          </span>
          <input
            type="text"
            value={datos.workspace}
            onChange={(e) => set("workspace", e.target.value.trim())}
            disabled={deshabilitado}
            className="px-3 py-2 rounded-md text-sm outline-none disabled:opacity-50"
            style={{ ...estiloInput, fontFamily: "var(--font-mono)" }}
          />
        </label>

        <label className="flex flex-col gap-1.5">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            Proyecto — key de Bitbucket, misma que usará SonarQube
          </span>
          {/* Mayúsculas automáticas: las keys de proyecto de Bitbucket
              siempre lo son, y así se evita un error silencioso. */}
          <input
            type="text"
            value={datos.projectKey}
            onChange={(e) => set("projectKey", e.target.value.toUpperCase().trim())}
            disabled={deshabilitado}
            placeholder="API, CON, VST…"
            className="px-3 py-2 rounded-md text-sm outline-none disabled:opacity-50"
            style={{ ...estiloInput, fontFamily: "var(--font-mono)" }}
          />
        </label>
      </div>

      {/* ── Repositorios ── */}
      <div className="flex flex-col gap-1.5">
        <span className="text-xs" style={{ color: "var(--text-muted)" }}>
          Repositorios — uno por línea · patrón br-[proyecto]-sd#####-oi#####
        </span>
        <textarea
          rows={5}
          value={datos.reposTexto}
          onChange={(e) => set("reposTexto", e.target.value.toLowerCase())}
          disabled={deshabilitado}
          placeholder={"br-api-sd00295-oi00026\nbr-api-sd00310-oi00031"}
          className="px-3 py-2 rounded-md text-sm outline-none resize-y disabled:opacity-50"
          style={{ ...estiloInput, fontFamily: "var(--font-mono)", lineHeight: 1.7 }}
        />

        {/* Validación en vivo. Es una advertencia, no un bloqueo: un nombre
            fuera de patrón se marca pero igual se puede crear. */}
        {repos.length > 0 && (
          <div className="flex flex-col gap-0.5 mt-1">
            {repos.map((r, i) => {
              const v = validarRepo(r);
              return (
                <div
                  key={`${r}-${i}`}
                  className="text-xs"
                  style={{
                    fontFamily: "var(--font-mono)",
                    color: v.ok ? "var(--ok)" : "var(--partial)",
                  }}
                >
                  {v.ok ? "✓" : "⚠"} {r}
                  {!v.ok && v.msg ? ` — ${v.msg}` : ""}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* ── País y acciones ── */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div className="flex flex-col gap-1.5">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            País — define el nombre de la rama de desarrollo
          </span>
          {/* Dropdown propio, no un select nativo: en WebKit la lista
              desplegada la dibuja el sistema y no respeta los colores. */}
          <Dropdown
            value={datos.pais}
            onChange={(v) => set("pais", v)}
            options={[
              { value: "CL", label: "Chile — rama development" },
              { value: "PE", label: "Perú — rama develop" },
            ]}
          />
        </div>

        <div className="flex flex-col gap-2 justify-end pb-0.5">
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={datos.activarPipelines}
              onChange={(e) => set("activarPipelines", e.target.checked)}
              disabled={deshabilitado}
              className="w-4 h-4 cursor-pointer"
              style={{ accentColor: "var(--accent)" }}
            />
            <span className="text-sm" style={{ color: "var(--text)" }}>
              Activar Pipelines
            </span>
          </label>

          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={datos.crearRamas}
              onChange={(e) => set("crearRamas", e.target.checked)}
              disabled={deshabilitado}
              className="w-4 h-4 cursor-pointer"
              style={{ accentColor: "var(--accent)" }}
            />
            <span className="text-sm" style={{ color: "var(--text)" }}>
              Crear ramas base desde master
            </span>
          </label>
        </div>
      </div>

      <div className="text-xs" style={{ color: "var(--text-faint)" }}>
        Pipelines debe quedar activo para poder vincular SonarQube después.
      </div>
    </div>
  );
}
