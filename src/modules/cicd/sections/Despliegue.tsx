import { Button, WarningNotice } from "../../../components/ui";
import type { CicdForm } from "../shared";
import {
  ambientesActivos,
  calcURIPrefix,
  esCR,
  esK8s,
  parsearRepos,
} from "../shared";
import { CheckRow, Grid, GroupLabel, TextField } from "./fields";

interface Props {
  f: CicdForm;
  set: (patch: Partial<CicdForm>) => void;
  // Subredes que no encontraron coincidencia, para avisar acá mismo en vez
  // de que el usuario lo descubra leyendo el YAML generado.
  subredesSinMatch: string[];
}

export default function Despliegue({ f, set, subredesSinMatch }: Props) {
  const k8s = esK8s(f);
  const cr = esCR(f);
  const envs = ambientesActivos(f);
  const repos = parsearRepos(f.reposText);

  return (
    <div className="space-y-6">
      {/* ── Bitbucket: aplica a todos los flujos salvo Apigee ── */}
      <div>
        <GroupLabel>Proyecto en Bitbucket</GroupLabel>
        <Grid cols={2}>
          <TextField
            label="Project Key"
            requerido
            mayusculas
            value={f.bbKey}
            onChange={(v) => set({ bbKey: v })}
            placeholder="ej: CON"
            hint="Identificador corto del proyecto. Está en Bitbucket → Repositorio → Settings → Project details."
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
      </div>

      {/* ── Namespaces: solo Kubernetes ── */}
      {k8s && (
        <div>
          <GroupLabel>Namespaces</GroupLabel>
          <div className="mb-3">
            <CheckRow
              label="Mismo namespace para todos los ambientes"
              checked={f.nsSame}
              onChange={(v) => set({ nsSame: v })}
            />
          </div>
          {f.nsSame ? (
            <div className="max-w-md">
              <TextField
                label="Namespace"
                value={f.nsAll}
                onChange={(v) => set({ nsAll: v })}
                placeholder="ej: customer-service"
              />
            </div>
          ) : (
            <Grid cols={3}>
              {envs.map((env) => (
                <TextField
                  key={env.key}
                  label={`Namespace ${env.label}`}
                  value={f.ns[env.key] || f.nsAll}
                  onChange={(v) => set({ ns: { ...f.ns, [env.key]: v } })}
                />
              ))}
            </Grid>
          )}
          <div className="text-xs mt-2" style={{ color: "var(--text-faint)" }}>
            El VirtualService de Istio va siempre en istio-system, eso es fijo. Este namespace es
            el de la aplicación.
          </div>
        </div>
      )}

      {/* ── Prefijo URI por repositorio: solo Kubernetes (lo usa Istio) ── */}
      {k8s && repos.length > 0 && (
        <div>
          <GroupLabel>Prefijo URI por repositorio</GroupLabel>
          <div className="text-xs mb-3" style={{ color: "var(--text-faint)" }}>
            Se calcula quitando el prefijo del repositorio y el segundo código. El /v1 se agrega
            solo en el YAML.
          </div>
          <div className="space-y-2">
            {repos.map((name) => {
              const sugerido = calcURIPrefix(name);
              const valor = f.uri[name] ?? sugerido;
              return (
                <div key={name} className="flex items-center gap-2">
                  <div
                    className="text-xs truncate"
                    style={{ color: "var(--text-faint)", minWidth: 240, maxWidth: 240 }}
                    title={name}
                  >
                    {name}
                  </div>
                  <input
                    type="text"
                    value={valor}
                    onChange={(e) => set({ uri: { ...f.uri, [name]: e.target.value } })}
                    className="flex-1 px-3 py-2 rounded-md text-sm outline-none focus:border-[var(--accent)]"
                    style={{
                      background: "var(--surface-raised)",
                      border: "1px solid var(--border-strong)",
                      color: "var(--text)",
                      fontFamily: "var(--font-mono)",
                    }}
                  />
                  <Button
                    variant="accent-outline"
                    onClick={() => {
                      const uri = { ...f.uri };
                      delete uri[name];
                      set({ uri });
                    }}
                    title="Volver al valor calculado desde el nombre del repositorio"
                  >
                    Recalcular
                  </Button>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* ── Cloud Run ── */}
      {cr && (
        <div>
          <GroupLabel>Cloud Run</GroupLabel>
          <Grid cols={2}>
            <TextField
              label="GCP_REGISTRY_NAME"
              value={f.crRegistryName}
              onChange={(v) => set({ crRegistryName: v })}
              placeholder="ej: gcr-ventas-ppff"
            />
            <TextField
              label="DOCKERFILE_PATH"
              value={f.crDockerfile}
              onChange={(v) => set({ crDockerfile: v })}
              placeholder="Dockerfile"
            />
            <TextField
              label="REGION"
              value={f.crRegion}
              onChange={(v) => set({ crRegion: v })}
              placeholder="southamerica-west1"
            />
            <TextField
              label="VERSION"
              nota="opcional"
              value={f.crVersion}
              onChange={(v) => set({ crVersion: v })}
              placeholder="1.0.0"
            />
            <TextField
              label="SCAN_EXCLUDE_FILES"
              nota="opcional"
              value={f.crScanExclude}
              onChange={(v) => set({ crScanExclude: v })}
              placeholder="ej: src/models/sd00076_oi00003/**"
              hint="Rutas que el análisis de seguridad no debe revisar."
            />
          </Grid>

          <div className="mt-4 space-y-3">
            {envs.map((env) => (
              <Grid cols={2} key={env.key}>
                <TextField
                  label={`PROJECT_ID / SERVICE_ID ${env.label}`}
                  value={f.crProjId[env.key]}
                  onChange={(v) => set({ crProjId: { ...f.crProjId, [env.key]: v } })}
                />
                <TextField
                  label={`SERVICE_ACCOUNT_NAME ${env.label}`}
                  mono
                  value={f.crSA[env.key]}
                  onChange={(v) => set({ crSA: { ...f.crSA, [env.key]: v } })}
                />
              </Grid>
            ))}
          </div>

          {/* La red del service.yaml se resuelve sola desde el catálogo de
              subredes. Si el proyecto es nuevo y todavía no está en la
              caché, se avisa acá y no en el YAML. */}
          {subredesSinMatch.length > 0 && (
            <div className="mt-4">
              <WarningNotice>
                Sin subred para {subredesSinMatch.join(", ")}. El service.yaml va a quedar con
                PENDIENTE-SIN-COINCIDENCIA. Si el proyecto es nuevo, actualizá las subredes en vivo
                desde el panel de más abajo.
              </WarningNotice>
            </div>
          )}
        </div>
      )}

      {/* ── Imagen base del bitbucket-pipelines.yml ── */}
      <div>
        <GroupLabel>Bitbucket Pipelines</GroupLabel>
        <TextField
          label="Imagen base"
          nota="sugerida según la tecnología, editable"
          mono
          value={f.bbpipeImage}
          onChange={(v) => set({ bbpipeImage: v })}
          placeholder="ej: 841964127262.dkr.ecr.us-east-1.amazonaws.com/ecr-base-image:alpine3.21-node_22.0.0-sec"
        />
      </div>

      {/* ── Mapa de ambientes a carpetas y ramas ── */}
      <div className="text-xs leading-relaxed" style={{ color: "var(--text-faint)" }}>
        Los tres ambientes conviven en la misma rama de desarrollo, cada uno en su carpeta: dev en{" "}
        <span style={{ fontFamily: "var(--font-mono)" }}>dev</span>, release en{" "}
        <span style={{ fontFamily: "var(--font-mono)" }}>pp</span> y prod en{" "}
        <span style={{ fontFamily: "var(--font-mono)" }}>prod</span>. La promoción a release y
        master es por merge normal.
      </div>
    </div>
  );
}
