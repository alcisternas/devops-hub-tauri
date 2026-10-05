// JenkinsParams de Apigee — el único archivo que genera ese flujo.
//
// El orden importa y está confirmado con un archivo real: primero el nombre
// del proxy y el interruptor de integración, y después las variables
// AGRUPADAS POR TIPO, no por ambiente. O sea todos los ORG juntos, después
// todos los ENV, y así. El formulario en cambio se muestra agrupado por
// ambiente, porque es más cómodo de llenar; la diferencia es intencional.
//
// Dos variables son condicionales:
//   APIGEE_SERVICE_ACCOUNT_NAME_*  solo si la integración está activa
//   BASEPATH_*                     nunca en Shared-Flow (no publica rutas)
//
// Sin APIGEE_PROXY_NAME el pipeline falla, así que el formulario lo exige.

import type { CicdData } from "../shared";
import { ENV_SFX } from "../shared";

export function buildApigeeJenkinsParams(d: CicdData): string {
  const isSharedFlow = d.apigeeSubtype === "sharedflow";
  const lines: string[] = [];

  lines.push(`APIGEE_PROXY_NAME=${d.apigeeProxyName || ""}`);
  lines.push(`APLICATION_INTEGRATION=${d.apigeeAppIntegration ? "true" : "false"}`);

  lines.push("");
  d.envs.forEach((env) => {
    const sfx = ENV_SFX[env.key] || env.key.toUpperCase();
    const ed = (d.apigeeEnvData && d.apigeeEnvData[env.key]) || { org: "", env: "", svc: "", sa: "" };
    lines.push(`APIGEE_ORG_${sfx}=${ed.org || ""}`);
  });

  lines.push("");
  d.envs.forEach((env) => {
    const sfx = ENV_SFX[env.key] || env.key.toUpperCase();
    const ed = (d.apigeeEnvData && d.apigeeEnvData[env.key]) || { org: "", env: "", svc: "", sa: "" };
    lines.push(`APIGEE_ENV_${sfx}=${ed.env || ""}`);
  });

  lines.push("");
  d.envs.forEach((env) => {
    const sfx = ENV_SFX[env.key] || env.key.toUpperCase();
    const ed = (d.apigeeEnvData && d.apigeeEnvData[env.key]) || { org: "", env: "", svc: "", sa: "" };
    lines.push(`APIGEE_SERVICE_ID_${sfx}=${ed.svc || ""}`);
  });

  if (d.apigeeAppIntegration) {
    lines.push("");
    d.envs.forEach((env) => {
      const sfx = ENV_SFX[env.key] || env.key.toUpperCase();
      const ed =
        (d.apigeeEnvData && d.apigeeEnvData[env.key]) || { org: "", env: "", svc: "", sa: "" };
      lines.push(`APIGEE_SERVICE_ACCOUNT_NAME_${sfx}=${ed.sa || ""}`);
    });
  }

  if (!isSharedFlow) {
    lines.push("");
    d.envs.forEach((env) => {
      const sfx = ENV_SFX[env.key] || env.key.toUpperCase();
      lines.push(`BASEPATH_${sfx}=${(d.apigeeBasepath && d.apigeeBasepath[env.key]) || ""}`);
    });
  }

  return lines.join("\n");
}
