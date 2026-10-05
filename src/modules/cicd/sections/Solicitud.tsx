import { Dropdown } from "../../../components/ui";
import { DESTINOS, INFRAS_AWS, INFRAS_GCP, TECNOLOGIAS } from "../data/catalogs";
import type { CicdForm, EnvKey } from "../shared";
import { ENV_KEYS, ENV_LABEL, esAWS, esApigee, esGCP, esK8s } from "../shared";
import { CheckRow, Grid, TextField } from "./fields";

interface Props {
  f: CicdForm;
  set: (patch: Partial<CicdForm>) => void;
}

export default function Solicitud({ f, set }: Props) {
  const gcp = esGCP(f);
  const aws = esAWS(f);
  const k8s = esK8s(f);
  const apigee = esApigee(f);
  const activos = ENV_KEYS.filter((k) => f.ambientes[k]);

  // AWS no ofrece Cloud Run. Si estaba elegido y se cambia el destino, el
  // valor se corrige en el mismo paso para no quedar en una combinación
  // que no existe.
  function cambiarDestino(v: string) {
    const infrasValidas = (v === "AWS" ? INFRAS_AWS : INFRAS_GCP).map((i) => i.v);
    const infra = infrasValidas.includes(f.infra) ? f.infra : "kubernetes";
    set({ destino: v, infra });
  }

  return (
    <div className="space-y-5">
      <Grid cols={3}>
        <TextField
          label="Ticket / Iniciativa"
          requerido
          value={f.ticket}
          onChange={(v) => set({ ticket: v })}
          placeholder="ej: CMP123563"
        />
        <div>
          <label className="block text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
            Lenguaje / Tecnología <span style={{ color: "var(--fail)" }}>*</span>
          </label>
          <Dropdown
            value={f.tech}
            onChange={(v) => set({ tech: v })}
            options={[{ value: "", label: "Seleccionar..." }, ...TECNOLOGIAS.map((t) => ({ value: t.v, label: t.l }))]}
          />
        </div>
        <div>
          <label className="block text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
            País
          </label>
          <Dropdown
            value={f.country}
            onChange={(v) => set({ country: v })}
            options={[
              { value: "CL", label: "Chile" },
              { value: "PE", label: "Perú" },
            ]}
          />
        </div>
      </Grid>

      <Grid cols={2}>
        <div>
          <label className="block text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
            Destino <span style={{ color: "var(--fail)" }}>*</span>
          </label>
          <Dropdown
            value={f.destino}
            onChange={cambiarDestino}
            options={DESTINOS.map((d) => ({ value: d.v, label: d.l }))}
          />
        </div>
        <div>
          <label className="block text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
            Tipo de infraestructura <span style={{ color: "var(--fail)" }}>*</span>
          </label>
          <Dropdown
            value={f.infra}
            onChange={(v) => set({ infra: v })}
            options={(aws ? INFRAS_AWS : INFRAS_GCP).map((i) => ({ value: i.v, label: i.l }))}
          />
        </div>
      </Grid>

      <div>
        <div className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>
          Ambientes <span style={{ color: "var(--fail)" }}>*</span>
        </div>
        <div className="flex gap-6">
          {ENV_KEYS.map((k) => (
            <CheckRow
              key={k}
              label={k === "release" ? "QA / PP (Release)" : ENV_LABEL[k]}
              checked={f.ambientes[k]}
              onChange={(v) => set({ ambientes: { ...f.ambientes, [k]: v } })}
            />
          ))}
        </div>
      </div>

      {/* Solo se piden los proyectos de los ambientes marcados: pedir los
          tres cuando se despliega a dos era una de las cosas que hacía
          fallar la validación sin explicar por qué. */}
      {gcp && activos.length > 0 && (
        <div className="space-y-4">
          <div className="text-xs" style={{ color: "var(--text-muted)" }}>
            Proyectos GCP
          </div>
          <Grid cols={3}>
            {activos.map((k: EnvKey) => (
              <TextField
                key={k}
                label={`Proyecto ${ENV_LABEL[k]}`}
                requerido
                value={f.gcp[k]}
                onChange={(v) => set({ gcp: { ...f.gcp, [k]: v } })}
                placeholder={`ej: BR-GCP-VENTAS-PPFF-${k === "release" ? "PP" : ENV_LABEL[k]}`}
              />
            ))}
          </Grid>

          {k8s && !apigee && (
            <>
              <div className="text-xs pt-1" style={{ color: "var(--text-muted)" }}>
                Clusters GKE
              </div>
              <Grid cols={3}>
                {activos.map((k: EnvKey) => (
                  <TextField
                    key={k}
                    label={`Cluster ${ENV_LABEL[k]}`}
                    requerido
                    mono
                    value={f.gke[k]}
                    onChange={(v) => set({ gke: { ...f.gke, [k]: v } })}
                    placeholder={`ej: gke-br-${k === "release" ? "pp" : k}-core-ventas-ppff-01`}
                  />
                ))}
              </Grid>
              <div className="text-xs" style={{ color: "var(--text-faint)" }}>
                El nombre del cluster se encuentra en repositorios ya configurados del mismo
                proyecto. Se usa para calcular el account de Spinnaker, que después se confirma
                contra Spinnaker: la región depende del proyecto, no del ambiente.
              </div>
            </>
          )}
        </div>
      )}

      {aws && activos.length > 0 && (
        <div className="space-y-4">
          <div className="text-xs" style={{ color: "var(--text-muted)" }}>
            Proyectos AWS
          </div>
          <Grid cols={3}>
            {activos.map((k: EnvKey) => (
              <TextField
                key={k}
                label={`Proyecto ${ENV_LABEL[k]}`}
                requerido
                value={f.aws[k]}
                onChange={(v) => set({ aws: { ...f.aws, [k]: v } })}
                placeholder={`ej: BR-AWS-APP-${k === "release" ? "PP" : ENV_LABEL[k]}-CL`}
              />
            ))}
          </Grid>

          {k8s && !apigee && (
            <>
              <div className="text-xs pt-1" style={{ color: "var(--text-muted)" }}>
                Clusters EKS
              </div>
              <Grid cols={1}>
                {activos.map((k: EnvKey) => (
                  <TextField
                    key={k}
                    label={`Cluster EKS ${ENV_LABEL[k]}`}
                    requerido
                    mono
                    value={f.eks[k]}
                    onChange={(v) => set({ eks: { ...f.eks, [k]: v } })}
                    placeholder="ej: arn:aws:eks:us-east-1:847309888826:cluster/EKS-BR-DEV-..."
                  />
                ))}
              </Grid>
            </>
          )}
        </div>
      )}
    </div>
  );
}
