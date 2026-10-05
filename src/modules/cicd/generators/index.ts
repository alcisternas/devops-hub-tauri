// Punto único desde donde se piden los archivos generados.
//
// collectArchivos devuelve {repositorio: {ruta: contenido}}, que es la misma
// forma que va a necesitar la subida a Bitbucket. Eso es deliberado: la
// vista previa y la subida leen de la misma función, así que no hay manera
// de que lo que se ve en pantalla sea distinto de lo que se sube.

import type { CatalogoSubredes, CicdData } from "../shared";
import { envSub, getFolder } from "../shared";
import { buildServiceYaml } from "./serviceYaml";
import { buildIstioYaml } from "./istioYaml";
import { buildJenkinsParams } from "./jenkinsParams";
import { buildApigeeJenkinsParams } from "./apigeeJenkinsParams";
import { buildBitbucketPipelines } from "./bitbucketPipelines";

export { buildServiceYaml, buildIstioYaml, buildJenkinsParams, buildApigeeJenkinsParams, buildBitbucketPipelines };

export type ArchivosPorRepo = Record<string, Record<string, string>>;

export function collectArchivos(d: CicdData, subnets: CatalogoSubredes): ArchivosPorRepo {
  const out: ArchivosPorRepo = {};

  (d.repos || []).forEach((repo) => {
    if (!repo.name) return;
    const files: Record<string, string> = {};

    if (d.isApigee) {
      // Apigee solo necesita el JenkinsParams, al lado de la carpeta
      // apiproxy/ que ya existe en el repositorio.
      files["JenkinsParams"] = buildApigeeJenkinsParams(d);
    } else {
      const folder = getFolder(d);
      d.envs.forEach((env) => {
        files[`${folder}/${envSub(env.key)}/service.yaml`] = buildServiceYaml(
          d,
          repo.name,
          env,
          subnets
        );
        if (d.isK8s) {
          files[`${folder}/${envSub(env.key)}/istio.yaml`] = buildIstioYaml(d, repo.name, env);
        }
      });
      files["JenkinsParams"] = buildJenkinsParams(d);
      files["bitbucket-pipelines.yml"] = buildBitbucketPipelines(d);
    }

    out[repo.name] = files;
  });

  return out;
}

// ─────────────────────────────────────────────────────────────────────────
// Marcadores de valores sin resolver
//
// Los generadores nunca inventan un valor que no tienen: escriben un
// marcador con la palabra PENDIENTE. Mientras el contenido se revisaba a
// mano eso alcanzaba; cuando la subida a Bitbucket sea automática, este
// listado es lo que va a frenar el envío antes de escribir en el repo.
//
// Importante: SA-DEL-PROYECTO NO es un marcador. Es un literal fijo e
// intencional del Deployment de Kubernetes.
// ─────────────────────────────────────────────────────────────────────────
export const MARCADOR = "PENDIENTE";

export interface Hallazgo {
  repo: string;
  ruta: string;
  linea: string;
}

export function buscarMarcadores(archivos: ArchivosPorRepo): Hallazgo[] {
  const hallazgos: Hallazgo[] = [];
  Object.entries(archivos).forEach(([repo, files]) => {
    Object.entries(files).forEach(([ruta, contenido]) => {
      contenido.split("\n").forEach((linea) => {
        if (linea.includes(MARCADOR)) {
          hallazgos.push({ repo, ruta, linea: linea.trim().slice(0, 120) });
        }
      });
    });
  });
  return hallazgos;
}
