import { Dropdown } from "../../../components/ui";
import { APIGEE_FOLDER, JF_K8S } from "../data/catalogs";
import type { CicdForm } from "../shared";
import { calcApigeeJenkinsfile, esApigee, esCR, patchTocado } from "../shared";
import { SubBlock, TextField } from "./fields";

interface Props {
  f: CicdForm;
  set: (patch: Partial<CicdForm>) => void;
}

// Sección 3: la configuración del pipeline en Jenkins, separada de los
// archivos del repositorio. Son solo dos datos, y es correcto que sean
// pocos: el resto de los parámetros del job son fijos o se derivan.
export default function PipelineJenkins({ f, set }: Props) {
  const apigee = esApigee(f);

  if (apigee) {
    const jf = calcApigeeJenkinsfile(f.apigeeSubtype, f.country);
    return (
      <SubBlock title="Ubicación en Jenkins" nota="valores fijos para Apigee">
        <div className="space-y-2 text-xs" style={{ color: "var(--text-muted)" }}>
          <div>
            Foldername:{" "}
            <span style={{ fontFamily: "var(--font-mono)", color: "var(--text)" }}>
              {APIGEE_FOLDER}
            </span>
          </div>
          <div>
            Jenkinsfile:{" "}
            <span style={{ fontFamily: "var(--font-mono)", color: "var(--text)" }}>
              {jf || "falta elegir el subtipo"}
            </span>
          </div>
          <div style={{ color: "var(--text-faint)" }}>
            Se calcula del subtipo y el país. Apigee tampoco usa canal de Slack.
          </div>
        </div>
      </SubBlock>
    );
  }

  const opcionesJf = JF_K8S[f.tech] || [];
  const variasOpciones = !esCR(f) && opcionesJf.length > 1;

  return (
    <SubBlock title="Ubicación en Jenkins">
      <div className="space-y-4">
        <TextField
          label="Foldername"
          requerido
          nota="ruta hasta el nivel del proyecto, sin el repositorio"
          mono
          value={f.folder}
          onChange={(v) => set({ folder: v })}
          placeholder="ej: VST-CAPTACION-Y-VENTAS/CL-CONSUMER-LOAN"
        />
        {variasOpciones ? (
          <div>
            <label className="block text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
              Jenkinsfile <span style={{ color: "var(--fail)" }}>*</span>
            </label>
            <Dropdown
              value={f.jfpath}
              onChange={(v) => set(patchTocado(f, { jfpath: v }, "jfpath"))}
              options={[
                { value: "", label: "Seleccionar..." },
                ...opcionesJf.map((o) => ({ value: o.p, label: `${o.l} — ${o.p}` })),
              ]}
            />
            <div className="text-xs mt-1.5" style={{ color: "var(--text-faint)" }}>
              Esta tecnología tiene varias plantillas, por eso hay que elegir.
            </div>
          </div>
        ) : (
          <TextField
            label="Jenkinsfile path"
            requerido
            mono
            value={f.jfpath}
            onChange={(v) => set(patchTocado(f, { jfpath: v }, "jfpath"))}
            placeholder="ej: golang/lib/Jenkinsfile.template"
          />
        )}
        <div className="text-xs" style={{ color: "var(--text-faint)" }}>
          Los demás parámetros del job son fijos o se derivan de lo ya cargado. Se pueden revisar en
          la pestaña Jenkins.
        </div>
      </div>
    </SubBlock>
  );
}
