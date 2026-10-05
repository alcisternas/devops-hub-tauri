import { Dropdown } from "../../../components/ui";
import { APIGEE_FOLDER, JF_K8S } from "../data/catalogs";
import type { CicdForm } from "../shared";
import {
  ambientesActivos,
  calcApigeeJenkinsfile,
  esAWS,
  esApigee,
  esCR,
  esGCP,
  esK8s,
  imagenServiceYaml,
  collectData,
} from "../shared";
import { Grid, GroupLabel, TextField } from "./fields";

interface Props {
  f: CicdForm;
  set: (patch: Partial<CicdForm>) => void;
}

export default function JenkinsParamsSection({ f, set }: Props) {
  const apigee = esApigee(f);
  const k8s = esK8s(f);
  const gcp = esGCP(f);
  const envs = ambientesActivos(f);
  const d = collectData(f);

  // ── Apigee: folder y Jenkinsfile son fijos, el resto son las variables
  //    del JenkinsParams agrupadas por ambiente ──
  if (apigee) {
    const jf = calcApigeeJenkinsfile(f.apigeeSubtype, f.country);
    return (
      <div className="space-y-6">
        <div className="text-xs leading-relaxed" style={{ color: "var(--text-faint)" }}>
          Apigee usa el folder fijo{" "}
          <span style={{ fontFamily: "var(--font-mono)" }}>{APIGEE_FOLDER}</span>, no lleva canal
          de Slack y su Jenkinsfile se calcula del subtipo y el país:{" "}
          <span style={{ fontFamily: "var(--font-mono)" }}>{jf || "falta elegir el subtipo"}</span>
        </div>

        <Grid cols={2}>
          <TextField
            label="APIGEE_PROXY_NAME"
            requerido
            nota="sin comillas ni prefijos"
            mono
            value={f.apigeeProxyName}
            onChange={(v) => set({ apigeeProxyName: v })}
            placeholder="ej: APIBR-SD10001-ProductDirectory-OI00038-v110"
            hint="Sin este valor el pipeline falla."
          />
          <div>
            <label className="block text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
              APLICATION_INTEGRATION
            </label>
            <Dropdown
              value={f.apigeeAppIntegration ? "true" : "false"}
              onChange={(v) => set({ apigeeAppIntegration: v === "true" })}
              options={[
                { value: "false", label: "false" },
                { value: "true", label: "true" },
              ]}
            />
            <div className="text-xs mt-1" style={{ color: "var(--text-faint)" }}>
              En true se agrega la cuenta de servicio de cada ambiente al archivo.
            </div>
          </div>
        </Grid>

        {/* El formulario se agrupa por ambiente porque es más cómodo de
            llenar; el archivo final ordena por tipo de variable. */}
        {envs.map((env) => (
          <div key={env.key}>
            <GroupLabel>{env.label}</GroupLabel>
            <Grid cols={3}>
              <TextField
                label="APIGEE_ORG"
                mono
                value={f.apigeeEnv[env.key].org}
                onChange={(v) =>
                  set({
                    apigeeEnv: {
                      ...f.apigeeEnv,
                      [env.key]: { ...f.apigeeEnv[env.key], org: v },
                    },
                  })
                }
              />
              <TextField
                label="APIGEE_ENV"
                mono
                value={f.apigeeEnv[env.key].env}
                onChange={(v) =>
                  set({
                    apigeeEnv: {
                      ...f.apigeeEnv,
                      [env.key]: { ...f.apigeeEnv[env.key], env: v },
                    },
                  })
                }
              />
              <TextField
                label="APIGEE_SERVICE_ID"
                mono
                value={f.apigeeEnv[env.key].svc}
                onChange={(v) =>
                  set({
                    apigeeEnv: {
                      ...f.apigeeEnv,
                      [env.key]: { ...f.apigeeEnv[env.key], svc: v },
                    },
                  })
                }
              />
            </Grid>
            <div className="mt-3 space-y-3">
              {f.apigeeAppIntegration && (
                <TextField
                  label="APIGEE_SERVICE_ACCOUNT_NAME"
                  mono
                  value={f.apigeeEnv[env.key].sa}
                  onChange={(v) =>
                    set({
                      apigeeEnv: {
                        ...f.apigeeEnv,
                        [env.key]: { ...f.apigeeEnv[env.key], sa: v },
                      },
                    })
                  }
                />
              )}
              {/* Shared-Flow no publica rutas, así que no lleva BASEPATH. */}
              {f.apigeeSubtype !== "sharedflow" && (
                <TextField
                  label="BASEPATH"
                  mono
                  value={f.apigeeBasepath[env.key]}
                  onChange={(v) =>
                    set({ apigeeBasepath: { ...f.apigeeBasepath, [env.key]: v } })
                  }
                  placeholder="ej: /banco-ripley/ruta1,/banco-ripley/ruta2"
                />
              )}
            </div>
          </div>
        ))}

        {f.country === "PE" && (
          <div className="text-xs" style={{ color: "var(--partial)" }}>
            Perú todavía no tiene valores fijos de organización, ambiente y service ID: hay que
            completarlos a mano.
          </div>
        )}
      </div>
    );
  }

  // ── Kubernetes y Cloud Run ──
  const opcionesJf = JF_K8S[f.tech] || [];
  const variasOpciones = !esCR(f) && opcionesJf.length > 1;

  return (
    <div className="space-y-6">
      <div>
        <GroupLabel>Ubicación en Jenkins</GroupLabel>
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
                onChange={(v) => set({ jfpath: v })}
                options={[
                  { value: "", label: "Seleccionar..." },
                  ...opcionesJf.map((o) => ({ value: o.p, label: `${o.l} — ${o.p}` })),
                ]}
              />
            </div>
          ) : (
            <TextField
              label="Jenkinsfile path"
              requerido
              mono
              value={f.jfpath}
              onChange={(v) => set({ jfpath: v })}
              placeholder="ej: golang/lib/Jenkinsfile.template"
            />
          )}
          <TextField
            label="Canal de Slack"
            nota="opcional"
            value={f.slack}
            onChange={(v) => set({ slack: v })}
            placeholder="ej: ci-cd-ventas-ppff"
            hint="Está en el JenkinsParams de otro repositorio del mismo proyecto, en SLACK_CHANNEL."
          />
        </div>
      </div>

      {k8s && (
        <div>
          <GroupLabel>Variables del JenkinsParams</GroupLabel>
          <Grid cols={2}>
            {/* En GCP la URL y las credenciales del registro son fijas, así
                que solo se pide el nombre. En AWS las tres cambian por
                cuenta y son editables. */}
            {!gcp && (
              <>
                <TextField
                  label="REGISTRY_URL"
                  mono
                  value={f.k8sRegistryUrl}
                  onChange={(v) => set({ k8sRegistryUrl: v })}
                  placeholder="ej: 123456789.dkr.ecr.us-east-1.amazonaws.com"
                />
                <TextField
                  label="REGISTRY_CREDENTIALS"
                  mono
                  value={f.k8sRegistryCredentials}
                  onChange={(v) => set({ k8sRegistryCredentials: v })}
                />
              </>
            )}
            <TextField
              label="REGISTRY_NAME"
              mono
              value={f.k8sRegistryName}
              onChange={(v) => set({ k8sRegistryName: v })}
              placeholder="ej: gcr-gcp-rest"
              hint={
                <>
                  Define dónde Jenkins deja la imagen y dónde la busca Spinnaker. De acá sale la
                  imagen del service.yaml:{" "}
                  <span style={{ fontFamily: "var(--font-mono)" }}>{imagenServiceYaml(d)}</span>
                </>
              }
            />
            <TextField
              label="DOCKERFILE_PATH"
              mono
              value={f.k8sDockerfile}
              onChange={(v) => set({ k8sDockerfile: v })}
              placeholder="Dockerfile"
            />
            <TextField
              label="SCAN_COVERAGE_REPORT"
              mono
              value={f.k8sScanCoverage}
              onChange={(v) => set({ k8sScanCoverage: v })}
              placeholder="coverage/**/"
            />
            <TextField
              label="SCAN_EXCLUDE_FILES"
              nota="opcional"
              mono
              value={f.k8sScanExclude}
              onChange={(v) => set({ k8sScanExclude: v })}
              placeholder="ej: src/test/**"
            />
            <TextField
              label="DEPENDENCIES_FILE_PATH"
              mono
              value={f.k8sDepsFile}
              onChange={(v) => set({ k8sDepsFile: v })}
              placeholder={f.tech === "dotnet" ? "ej: MiProyecto.csproj" : ""}
              hint={f.tech === "dotnet" ? "En .NET cambia por proyecto, no hay sugerencia." : undefined}
            />
            <TextField
              label="CODE_ARTIFACT_DOMAIN"
              mono
              value={f.k8sCaDomain}
              onChange={(v) => set({ k8sCaDomain: v })}
            />
            <TextField
              label="CODE_ARTIFACT_REPOSITORY"
              mono
              value={f.k8sCaRepo}
              onChange={(v) => set({ k8sCaRepo: v })}
            />
            {esAWS(f) && (
              <TextField
                label="AWS_OWNER"
                mono
                value={f.k8sAwsOwner}
                onChange={(v) => set({ k8sAwsOwner: v })}
                hint="Solo existe en AWS: en GCP la variable no va ni vacía."
              />
            )}
          </Grid>
          <div className="text-xs mt-3" style={{ color: "var(--text-faint)" }}>
            ENABLE_FORTIFY_QUICK, FORTIFY_ENABLE_GATE y ENABLE_DEBRICKED son fijos y no se piden.
          </div>
        </div>
      )}
    </div>
  );
}
