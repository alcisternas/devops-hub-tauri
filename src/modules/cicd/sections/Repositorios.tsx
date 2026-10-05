import { Dropdown, WarningNotice } from "../../../components/ui";
import { SUBTIPOS_APIGEE } from "../data/catalogs";
import type { CicdForm } from "../shared";
import { esApigee, parsearRepos, totalLineasRepos } from "../shared";
import { TextAreaField } from "./fields";

interface Props {
  f: CicdForm;
  set: (patch: Partial<CicdForm>) => void;
}

export default function Repositorios({ f, set }: Props) {
  const repos = parsearRepos(f.reposText);
  const total = totalLineasRepos(f.reposText);
  const mayusculas = repos.filter((r) => r !== r.toLowerCase());
  const apigee = esApigee(f);

  return (
    <div className="space-y-4">
      <TextAreaField
        label="Repositorios"
        requerido
        nota="uno por línea, máximo 5, solo minúsculas"
        value={f.reposText}
        onChange={(v) => set({ reposText: v })}
        placeholder={"br-ms-sd00103-oi00093-amb-rpass-xborder-v1-cl\nbr-ms-sd00103-oi00094-otro-repo"}
      />

      <div className="space-y-1">
        {/* Avisos, no bloqueos: el usuario ve qué va a pasar y decide. */}
        {total > 5 && (
          <WarningNotice>
            Hay {total} líneas y el máximo es 5 — se van a usar solo las primeras 5.
          </WarningNotice>
        )}
        {mayusculas.length > 0 && (
          <WarningNotice>Deben estar en minúsculas: {mayusculas.join(", ")}</WarningNotice>
        )}
        {repos.length > 0 && total <= 5 && mayusculas.length === 0 && (
          <div className="text-xs" style={{ color: "var(--text-faint)" }}>
            {repos.length} repositorio{repos.length === 1 ? "" : "s"}. Todos comparten la misma
            configuración.
          </div>
        )}
      </div>

      {/* El subtipo decide cuál de los 6 Jenkinsfiles se usa y si el
          JenkinsParams lleva BASEPATH. Shared-Flow no publica rutas. */}
      {apigee && (
        <div className="max-w-xs">
          <label className="block text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
            Subtipo Apigee <span style={{ color: "var(--fail)" }}>*</span>
          </label>
          <Dropdown
            value={f.apigeeSubtype}
            onChange={(v) => set({ apigeeSubtype: v })}
            options={[
              { value: "", label: "Seleccionar..." },
              ...SUBTIPOS_APIGEE.map((s) => ({ value: s.v, label: s.l })),
            ]}
          />
          {f.apigeeSubtype === "facade" && (
            <div className="text-xs mt-1.5" style={{ color: "var(--text-faint)" }}>
              Facade usa por ahora la misma estructura de archivo que Proxy.
            </div>
          )}
        </div>
      )}
    </div>
  );
}
