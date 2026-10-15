import { Dropdown, WarningNotice } from "../../../components/ui";
import { DESTINOS, INFRAS_AWS, INFRAS_GCP, SUBTIPOS_APIGEE, TECNOLOGIAS } from "../data/catalogs";
import type { CampoDerivado, CicdForm, EnvKey } from "../shared";
import {
  ENV_KEYS,
  ENV_LABEL,
  esAWS,
  esApigee,
  esGCP,
  esK8s,
  lineasRepetidas,
  parsearRepos,
} from "../shared";
import { CheckRow, Grid, SubBlock, TextAreaField, TextField } from "./fields";

interface Props {
  f: CicdForm;
  set: (patch: Partial<CicdForm>) => void;
}

// Sección 1: todo lo que sale de la solicitud. Es el primer bloque que se
// llena y del que dependen casi todas las sugerencias del resto.
export default function Solicitud({ f, set }: Props) {
  const gcp = esGCP(f);
  const aws = esAWS(f);
  const k8s = esK8s(f);
  const apigee = esApigee(f);
  const activos = ENV_KEYS.filter((k) => f.ambientes[k]);
  const repos = parsearRepos(f.reposText);
  const repetidos = lineasRepetidas(f.reposText);
  const mayusculas = repos.filter((r) => r !== r.toLowerCase());

  // AWS no ofrece Cloud Run. Si estaba elegido y se cambia el destino, el
  // valor se corrige en el mismo paso para no quedar en una combinación
  // que no existe.
  function cambiarDestino(v: string) {
    const infrasValidas = (v === "AWS" ? INFRAS_AWS : INFRAS_GCP).map((i) => i.v);
    const infra = infrasValidas.includes(f.infra) ? f.infra : "kubernetes";
    set({ destino: v, infra });
  }

  // Al escribir a mano en un campo que se completa solo, ese campo deja de
  // seguir al valor de DEV.
  function editarDerivado(campo: "gcp" | "gke", env: EnvKey, valor: string) {
    const clave = `${campo}.${env}` as CampoDerivado;
    set({
      [campo]: { ...f[campo], [env]: valor },
      tocados: { ...f.tocados, [clave]: true },
    } as Partial<CicdForm>);
  }

  return (
    <div className="space-y-6">
      <SubBlock title="Datos de la solicitud">
        <div className="space-y-4">
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
                options={[
                  { value: "", label: "Seleccionar..." },
                  ...TECNOLOGIAS.map((t) => ({ value: t.v, label: t.l })),
                ]}
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
        </div>
      </SubBlock>

      {/* Proyectos y clusters. Solo se piden los ambientes marcados, y
          RELEASE y PROD se completan desde DEV mientras no se editen. */}
      {gcp && activos.length > 0 && (
        <SubBlock title="Proyectos GCP" nota="release y prod se completan desde dev">
          <div className="space-y-4">
            <Grid cols={3}>
              {activos.map((k: EnvKey) => (
                <TextField
                  key={k}
                  label={`Proyecto ${ENV_LABEL[k]}`}
                  requerido
                  value={f.gcp[k]}
                  onChange={(v) =>
                    k === "dev" ? set({ gcp: { ...f.gcp, dev: v } }) : editarDerivado("gcp", k, v)
                  }
                  placeholder={`ej: BR-GCP-VENTAS-PPFF-${k === "release" ? "PP" : ENV_LABEL[k]}`}
                />
              ))}
            </Grid>

            {k8s && !apigee && (
              <Grid cols={3}>
                {activos.map((k: EnvKey) => (
                  <TextField
                    key={k}
                    label={`Cluster GKE ${ENV_LABEL[k]}`}
                    requerido
                    mono
                    value={f.gke[k]}
                    onChange={(v) =>
                      k === "dev" ? set({ gke: { ...f.gke, dev: v } }) : editarDerivado("gke", k, v)
                    }
                    placeholder={`ej: gke-br-${k === "release" ? "pp" : k}-core-ventas-ppff-01`}
                  />
                ))}
              </Grid>
            )}
          </div>
        </SubBlock>
      )}

      {aws && activos.length > 0 && (
        <SubBlock title="Proyectos AWS">
          <div className="space-y-4">
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
            )}
          </div>
        </SubBlock>
      )}

      <SubBlock title="Repositorios" nota="uno por línea, solo minúsculas">
        <div className="space-y-3">
          <TextAreaField
            label=""
            requerido
            value={f.reposText}
            onChange={(v) => set({ reposText: v })}
            placeholder={"br-ms-sd00103-oi00093-amb-rpass-xborder-v1-cl\nbr-ms-sd00103-oi00094-otro-repo"}
          />
          {/* Sin límite de repositorios. El aviso es informativo: la
              verificación contra Bitbucket hace una consulta por archivo,
              así que con muchos repositorios va a tardar. */}
          {repos.length > 10 && (
            <WarningNotice>
              {repos.length} repositorios. La verificación antes de subir va a tardar: hace una
              consulta por archivo.
            </WarningNotice>
          )}
          {repetidos.length > 0 && (
            <WarningNotice>
              Nombres repetidos, se cuentan una sola vez: {repetidos.join(", ")}
            </WarningNotice>
          )}
          {mayusculas.length > 0 && (
            <WarningNotice>Deben estar en minúsculas: {mayusculas.join(", ")}</WarningNotice>
          )}
          {repos.length > 0 && !repetidos.length && !mayusculas.length && (
            <div className="text-xs" style={{ color: "var(--text-faint)" }}>
              {repos.length} repositorio{repos.length === 1 ? "" : "s"}. Todos comparten la misma
              configuración.
            </div>
          )}
        </div>
      </SubBlock>

      {/* Apigee no usa Project Key: su JenkinsParams no lleva esa variable. */}
      {!apigee && (
        <SubBlock title="Proyecto en Bitbucket">
          <Grid cols={2}>
            <TextField
              label="Project Key"
              requerido
              mayusculas
              value={f.bbKey}
              onChange={(v) => set({ bbKey: v })}
              placeholder="ej: CON"
              hint="Identificador corto del proyecto. Está en Bitbucket, en Repositorio -> Settings -> Project details."
            />
            <TextField
              label="Project Name"
              requerido
              value={f.bbName}
              onChange={(v) => set({ bbName: v })}
              placeholder="ej: consumer-loan"
              hint="En minúsculas con guiones. Mismo lugar que el Project Key."
            />
          </Grid>
        </SubBlock>
      )}
    </div>
  );
}
