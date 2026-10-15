import { Dropdown } from "../../../components/ui";
import SubnetsPanel from "../SubnetsPanel";
import type { CatalogoSubredes, CicdForm, EstadoSubredes } from "../shared";
import {
  ambientesActivos,
  calcURIPrefix,
  collectData,
  esAWS,
  esApigee,
  esCR,
  esGCP,
  esK8s,
  extractProjectKey,
  findSubnet,
  imagenServiceYaml,
  parsearRepos,
  patchTocado,
} from "../shared";
import { CheckRow, Grid, SubBlock, SubCollapse, TextField } from "./fields";

interface Props {
  f: CicdForm;
  set: (patch: Partial<CicdForm>) => void;
  catalogo: CatalogoSubredes;
  subnetsEstado: EstadoSubredes | null;
  subnetsCargando: boolean;
  subnetsError: string;
  subnetsTotal: number;
  onActualizarSubnets: () => void;
}

// Sección 2: lo que hay que dejar configurado dentro del repositorio.
//
// Agrupado por archivo, no por tema: así se ve de dónde sale cada valor y
// qué archivo se está armando. El JenkinsParams vive acá, no en la sección
// de Jenkins, porque es un archivo del repositorio — la configuración del
// pipeline es otra cosa y va en la sección siguiente.
export default function ArchivosRepo({
  f,
  set,
  catalogo,
  subnetsEstado,
  subnetsCargando,
  subnetsError,
  subnetsTotal,
  onActualizarSubnets,
}: Props) {
  const apigee = esApigee(f);
  const k8s = esK8s(f);
  const cr = esCR(f);
  const gcp = esGCP(f);
  const envs = ambientesActivos(f);
  const repos = parsearRepos(f.reposText);
  const d = collectData(f);
  const projKey = extractProjectKey(f.gcp.dev || f.gcp.release || f.gcp.prod || "");

  // ── Apigee: un solo archivo, todas sus variables agrupadas por ambiente ──
  if (apigee) {
    return (
      <div className="space-y-6">
        <SubBlock title="JenkinsParams" nota="único archivo que genera Apigee">
          <div className="space-y-4">
            <Grid cols={2}>
              <TextField
                label="APIGEE_PROXY_NAME"
                requerido
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

            {/* El formulario agrupa por ambiente porque es más cómodo de
                llenar; el archivo final ordena por tipo de variable. */}
            {envs.map((env) => (
              <div key={env.key} className="pt-1">
                <div className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>
                  {env.label}
                </div>
                <Grid cols={3}>
                  <TextField
                    label="APIGEE_ORG"
                    mono
                    value={f.apigeeEnv[env.key].org}
                    onChange={(v) =>
                      set(
                        patchTocado(
                          f,
                          {
                            apigeeEnv: {
                              ...f.apigeeEnv,
                              [env.key]: { ...f.apigeeEnv[env.key], org: v },
                            },
                          },
                          `apigee.${env.key}` as const
                        )
                      )
                    }
                  />
                  <TextField
                    label="APIGEE_ENV"
                    mono
                    value={f.apigeeEnv[env.key].env}
                    onChange={(v) =>
                      set(
                        patchTocado(
                          f,
                          {
                            apigeeEnv: {
                              ...f.apigeeEnv,
                              [env.key]: { ...f.apigeeEnv[env.key], env: v },
                            },
                          },
                          `apigee.${env.key}` as const
                        )
                      )
                    }
                  />
                  <TextField
                    label="APIGEE_SERVICE_ID"
                    mono
                    value={f.apigeeEnv[env.key].svc}
                    onChange={(v) =>
                      set(
                        patchTocado(
                          f,
                          {
                            apigeeEnv: {
                              ...f.apigeeEnv,
                              [env.key]: { ...f.apigeeEnv[env.key], svc: v },
                            },
                          },
                          `apigee.${env.key}` as const
                        )
                      )
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
                        set(
                          patchTocado(
                            f,
                            {
                              apigeeEnv: {
                                ...f.apigeeEnv,
                                [env.key]: { ...f.apigeeEnv[env.key], sa: v },
                              },
                            },
                            `apigee.${env.key}` as const
                          )
                        )
                      }
                    />
                  )}
                  {/* Shared-Flow no publica rutas, así que no lleva BASEPATH. */}
                  {f.apigeeSubtype !== "sharedflow" && (
                    <TextField
                      label="BASEPATH"
                      mono
                      value={f.apigeeBasepath[env.key]}
                      onChange={(v) => set({ apigeeBasepath: { ...f.apigeeBasepath, [env.key]: v } })}
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
        </SubBlock>
      </div>
    );
  }

  // ── Kubernetes y Cloud Run ──
  return (
    <div className="space-y-6">
      <SubBlock title="JenkinsParams" nota="en la raíz del repositorio">
        <div className="space-y-4">
          <Grid cols={2}>
            <TextField
              label={cr ? "GCP_REGISTRY_NAME" : "REGISTRY_NAME"}
              mono
              value={cr ? f.crRegistryName : f.k8sRegistryName}
              onChange={(v) =>
                cr
                  ? set(patchTocado(f, { crRegistryName: v }, "cr.registryName"))
                  : set(patchTocado(f, { k8sRegistryName: v }, "k8s.registryName"))
              }
              placeholder="ej: gcr-gcp-rest"
              hint={
                k8s ? (
                  <>
                    Define dónde Jenkins deja la imagen y dónde la busca Spinnaker. De acá sale la
                    imagen del service.yaml:{" "}
                    <span style={{ fontFamily: "var(--font-mono)" }}>{imagenServiceYaml(d)}</span>
                  </>
                ) : (
                  "Define dónde Jenkins deja la imagen del servicio."
                )
              }
            />
            <TextField
              label="SLACK_CHANNEL"
              nota="opcional"
              value={f.slack}
              onChange={(v) => set({ slack: v })}
              placeholder="ej: ci-cd-ventas-ppff"
              hint="Está en el JenkinsParams de otro repositorio del mismo proyecto."
            />
            {/* En AWS estos dos no tienen valor sugerido: escondidos, el
                archivo saldría con las líneas vacías. */}
            {k8s && esAWS(f) && (
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
          </Grid>

          {k8s && (
            <SubCollapse title="Valores avanzados" nota={esAWS(f) ? "7 campos" : "6 campos"}>
              <Grid cols={2}>
                <TextField
                  label="DOCKERFILE_PATH"
                  mono
                  value={f.k8sDockerfile}
                  onChange={(v) => set({ k8sDockerfile: v })}
                />
                <TextField
                  label="SCAN_COVERAGE_REPORT"
                  mono
                  value={f.k8sScanCoverage}
                  onChange={(v) => set({ k8sScanCoverage: v })}
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
                  onChange={(v) => set(patchTocado(f, { k8sDepsFile: v }, "depsFile"))}
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
            </SubCollapse>
          )}

          {/* Tres variables por ambiente, de tres fuentes distintas. Las
              dos primeras salen de la credencial registrada en Jenkins y
              no se pueden derivar del proyecto: hay credenciales con
              prefijo "gcp-" y sin él para proyectos del mismo tipo. */}
          {cr &&
            envs.map((env) => (
              <div key={env.key}>
                <div className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>
                  {env.label}
                </div>
                <Grid cols={3}>
                  <TextField
                    label="PROJECT_ID"
                    nota="proyecto GCP"
                    mono
                    value={f.crProjId[env.key]}
                    onChange={(v) =>
                      set(
                        patchTocado(
                          f,
                          { crProjId: { ...f.crProjId, [env.key]: v } },
                          `cr.projId.${env.key}` as const
                        )
                      )
                    }
                  />
                  <TextField
                    label="SERVICE_ID"
                    nota="ID de la credencial en Jenkins"
                    mono
                    value={f.crServiceId[env.key]}
                    onChange={(v) => set({ crServiceId: { ...f.crServiceId, [env.key]: v } })}
                    placeholder="ej: gcp-br-gcp-ard-co-dev"
                    hint="Jenkins → Manage Credentials, dominio GCP. Se copia el ID tal cual, con prefijo si lo tiene."
                  />
                  <TextField
                    label="SERVICE_ACCOUNT_NAME"
                    nota="cuenta de DevOps"
                    mono
                    value={f.crSA[env.key]}
                    onChange={(v) => set({ crSA: { ...f.crSA, [env.key]: v } })}
                    placeholder="ej: id-devops-user@br-gcp-ard-co-dev.iam.gserviceaccount.com"
                    hint="El campo client_email del JSON de esa credencial. No es la cuenta de servicio del proyecto."
                  />
                </Grid>
              </div>
            ))}

          {cr && (
            <SubCollapse title="Valores avanzados" nota="4 campos">
              <Grid cols={2}>
                <TextField
                  label="DOCKERFILE_PATH"
                  mono
                  value={f.crDockerfile}
                  onChange={(v) => set({ crDockerfile: v })}
                />
                <TextField
                  label="REGION"
                  mono
                  value={f.crRegion}
                  onChange={(v) => set({ crRegion: v })}
                />
                <TextField
                  label="VERSION"
                  nota="opcional"
                  value={f.crVersion}
                  onChange={(v) => set({ crVersion: v })}
                />
                <TextField
                  label="SCAN_EXCLUDE_FILES"
                  nota="opcional"
                  mono
                  value={f.crScanExclude}
                  onChange={(v) => set({ crScanExclude: v })}
                />
              </Grid>
            </SubCollapse>
          )}
        </div>
      </SubBlock>

      {/* ── service.yaml e istio.yaml de Kubernetes ── */}
      {k8s && (
        <SubBlock title="service.yaml e istio.yaml" nota={`en ${gcp ? "k8s/gcp" : "k8s/aws"}, uno por ambiente`}>
          <div className="space-y-4">
            <div>
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

            {repos.length > 0 && (
              <div>
                <div className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>
                  Prefijo URI por repositorio
                </div>
                <div className="text-xs mb-3" style={{ color: "var(--text-faint)" }}>
                  Se calcula quitando el prefijo del repositorio y el segundo código. El /v1 se
                  agrega solo en el YAML. Dejar el campo vacío vuelve al valor calculado.
                </div>
                <div className="space-y-2">
                  {/* La clave incluye la posición: durante la edición puede
                      haber nombres repetidos por un instante, y con claves
                      iguales la lista no logra emparejar las filas y quedan
                      filas viejas en pantalla. */}
                  {repos.map((name, i) => (
                    <div key={`${i}-${name}`} className="flex items-center gap-2">
                      <div
                        className="text-xs truncate"
                        style={{ color: "var(--text-faint)", minWidth: 240, maxWidth: 240 }}
                        title={name}
                      >
                        {name}
                      </div>
                      <input
                        type="text"
                        value={f.uri[name] ?? calcURIPrefix(name)}
                        onChange={(e) => {
                          // Vaciar el campo quita el valor manual y vuelve
                          // al calculado, que es lo que hacía el botón.
                          const uri = { ...f.uri };
                          if (e.target.value.trim() === "") delete uri[name];
                          else uri[name] = e.target.value;
                          set({ uri });
                        }}
                        autoCapitalize="none"
                        autoCorrect="off"
                        spellCheck={false}
                        className="flex-1 px-3 py-2 rounded-md text-sm outline-none focus:border-[var(--accent)]"
                        style={{
                          background: "var(--surface-raised)",
                          border: "1px solid var(--border-strong)",
                          color: "var(--text)",
                          fontFamily: "var(--font-mono)",
                        }}
                      />
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        </SubBlock>
      )}

      {/* ── service.yaml de Cloud Run: lo único variable es la red ── */}
      {cr && (
        <SubBlock title="service.yaml" nota="en cloud-run, uno por ambiente">
          <div className="space-y-3">
            <div className="text-xs" style={{ color: "var(--text-faint)" }}>
              El contenido es fijo salvo la red y la subred, que se resuelven desde el catálogo
              buscando el proyecto{" "}
              <span style={{ fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
                {projKey || "— falta el proyecto GCP"}
              </span>
              .
            </div>

            {/* Tabla, no notas al pie: antes la subred aparecía como una
                línea gris debajo de campos grandes y no se veía. Acá es el
                dato principal del bloque y se pueden comparar los tres
                ambientes de un vistazo. */}
            <div className="rounded-md border overflow-hidden" style={{ borderColor: "var(--border)" }}>
              <div
                className="grid gap-3 px-3 py-2 text-xs"
                style={{
                  gridTemplateColumns: "80px 1fr 150px 1fr",
                  color: "var(--text-faint)",
                  background: "var(--surface-raised)",
                }}
              >
                <div>Ambiente</div>
                <div>Subred</div>
                <div>Región</div>
                <div>Red</div>
              </div>
              {envs.map((env) => {
                const subred = projKey ? findSubnet(catalogo, projKey, env.key) : null;
                return (
                  <div
                    key={env.key}
                    className="grid gap-3 px-3 py-2 text-xs items-center"
                    style={{
                      gridTemplateColumns: "80px 1fr 150px 1fr",
                      borderTop: "1px solid var(--border)",
                      fontFamily: "var(--font-mono)",
                    }}
                  >
                    <div style={{ color: "var(--text-muted)" }}>{env.label}</div>
                    {subred ? (
                      <>
                        <div style={{ color: "var(--text)" }}>{subred.name}</div>
                        <div style={{ color: "var(--text)" }}>{subred.region}</div>
                        <div style={{ color: "var(--text-muted)" }}>{subred.network}</div>
                      </>
                    ) : (
                      <div style={{ color: "var(--fail)", gridColumn: "2 / span 3" }}>
                        Sin coincidencia — el archivo queda con PENDIENTE-SIN-COINCIDENCIA
                      </div>
                    )}
                  </div>
                );
              })}
            </div>

            <SubnetsPanel
              estado={subnetsEstado}
              cargando={subnetsCargando}
              error={subnetsError}
              total={subnetsTotal}
              onActualizar={onActualizarSubnets}
            />
          </div>
        </SubBlock>
      )}

      {/* ── bitbucket-pipelines.yml ── */}
      <SubBlock title="bitbucket-pipelines.yml" nota="análisis de SonarCloud, contenido fijo">
        <SubCollapse title="Imagen base" nota="sugerida según la tecnología">
          <TextField
            label="image.name"
            mono
            value={f.bbpipeImage}
            onChange={(v) => set(patchTocado(f, { bbpipeImage: v }, "bbpipeImage"))}
            placeholder="ej: 841964127262.dkr.ecr.us-east-1.amazonaws.com/ecr-base-image:alpine3.21-node_22.0.0-sec"
          />
        </SubCollapse>
      </SubBlock>

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
