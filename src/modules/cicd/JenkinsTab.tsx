import { openUrl } from "@tauri-apps/plugin-opener";
import { Button, MetaLine } from "../../components/ui";
import {
  APIGEE_FOLDER,
  JENKINS_BASE,
  JENKINS_JOB_APIGEE,
  JENKINS_JOB_K8S,
  JENKINS_JOB_PATH,
} from "./data/catalogs";
import type { CicdForm } from "./shared";
import { calcApigeeJenkinsfile, esApigee, parsearRepos } from "./shared";

interface Props {
  f: CicdForm;
}

// Los 8 parámetros del job que crea los pipelines multibranch. Hoy se
// cargan a mano en Jenkins con "Build with Parameters"; en la fase de
// Jenkins esta misma tabla pasa a ser el lanzamiento real, con el avance en
// la consola y los enlaces a los pipelines creados.
export default function JenkinsTab({ f }: Props) {
  const apigee = esApigee(f);
  const repos = parsearRepos(f.reposText);
  const job = apigee ? JENKINS_JOB_APIGEE : JENKINS_JOB_K8S;
  const url = `${JENKINS_BASE}${JENKINS_JOB_PATH}/job/${job}/`;

  const folder = apigee ? APIGEE_FOLDER : f.folder;
  const jfpath = apigee ? calcApigeeJenkinsfile(f.apigeeSubtype, f.country) : f.jfpath;

  const params: { k: string; v: string; nota?: string }[] = [
    { k: "Command", v: "create_multibranch" },
    { k: "Foldername", v: folder || "—" },
    { k: "Repolist", v: repos.join("\n") || "—", nota: "un repositorio por línea" },
    { k: "Pipeline_disabled", v: "True", nota: "casilla marcada" },
    { k: "Type", v: "template" },
    { k: "Parent_repo", v: "devops-pipeline-templates" },
    { k: "Remote_branch", v: "gcp" },
    { k: "Jenkinsfile_path", v: jfpath || "—" },
  ];

  return (
    <div className="space-y-5">
      <div>
        <div className="text-sm mb-1" style={{ color: "var(--text)" }}>
          Job {job}
        </div>
        <MetaLine>
          Necesita VPN. Todos los repositorios se crean en una sola ejecución, con los mismos
          parámetros.
        </MetaLine>
        {/* openUrl abre en el navegador del sistema: un <a> normal
            intentaría navegar dentro de la ventana de la aplicación. */}
        <div className="mt-3">
          <Button variant="accent-outline" onClick={() => openUrl(url)}>
            Abrir el job en Jenkins
          </Button>
        </div>
      </div>

      <div className="rounded-lg border overflow-hidden" style={{ borderColor: "var(--border)" }}>
        {params.map((p, i) => (
          <div
            key={p.k}
            className="flex gap-4 px-4 py-2.5"
            style={{
              borderTop: i === 0 ? undefined : "1px solid var(--border)",
              background: "var(--surface)",
            }}
          >
            <div className="text-xs pt-0.5" style={{ color: "var(--text-muted)", minWidth: 170 }}>
              {p.k}
            </div>
            <div className="flex-1">
              <div
                className="text-xs whitespace-pre-wrap"
                style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}
              >
                {p.v}
              </div>
              {p.nota && (
                <div className="text-xs mt-0.5" style={{ color: "var(--text-faint)" }}>
                  {p.nota}
                </div>
              )}
            </div>
          </div>
        ))}
      </div>

      <div className="text-xs leading-relaxed" style={{ color: "var(--text-faint)" }}>
        Jenkins configura el resto al crear el pipeline: no hay correcciones posteriores que hacer a
        mano.
      </div>
    </div>
  );
}
