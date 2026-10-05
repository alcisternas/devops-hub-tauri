import type { CicdForm } from "../shared";
import { collectData, imagenSpinnaker, necesitaSpinnaker } from "../shared";

interface Props {
  f: CicdForm;
}

// Lo que Spinnaker va a necesitar, mostrado para revisar. En esta fase no
// se crea nada: la creación de aplicaciones y pipelines llega después.
//
// No hay campos editables acá a propósito. Los tres datos que Spinnaker usa
// salen de otras secciones (clusters, namespaces, REGISTRY_NAME), y tener
// una copia editable sería una segunda fuente de verdad para el mismo dato.
export default function Spinnaker({ f }: Props) {
  if (!necesitaSpinnaker(f)) {
    return (
      <div className="text-xs leading-relaxed" style={{ color: "var(--text-faint)" }}>
        Esta configuración no usa Spinnaker. Solo lo usa Kubernetes sobre GCP: en Cloud Run el
        despliegue lo hace el propio pipeline de Jenkins, y en Apigee también.
      </div>
    );
  }

  const d = collectData(f);
  const imagen = imagenSpinnaker(d);

  const filas: { k: string; v: string }[] = [
    { k: "Aplicación por repositorio", v: d.repos.map((r) => r.name).join(", ") || "—" },
    { k: "Pipeline por ambiente", v: d.envs.map((e) => `pipeline-gcp-${e.key}`).join(", ") || "—" },
    { k: "Imagen del trigger", v: imagen || "falta REGISTRY_NAME" },
    { k: "Canal de Slack", v: d.slack || "sin notificaciones" },
  ];

  return (
    <div className="space-y-4">
      <div className="text-xs leading-relaxed" style={{ color: "var(--text-faint)" }}>
        Resumen de lo que se va a crear en Spinnaker. La creación todavía no está en el hub: por
        ahora estos datos sirven para revisar que la configuración esté completa.
      </div>

      <div className="space-y-2">
        {filas.map((r) => (
          <div key={r.k} className="flex gap-4 text-xs">
            <div style={{ color: "var(--text-muted)", minWidth: 200 }}>{r.k}</div>
            <div style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>{r.v}</div>
          </div>
        ))}
      </div>

      <div>
        <div className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>
          Account calculado por ambiente
        </div>
        <div className="space-y-1.5">
          {d.envs.map((env) => (
            <div key={env.key} className="flex gap-4 text-xs">
              <div style={{ color: "var(--text-faint)", minWidth: 80 }}>{env.label}</div>
              <div style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>
                {d.accounts[env.key] || "—"}
              </div>
            </div>
          ))}
        </div>
        <div className="text-xs mt-2" style={{ color: "var(--partial)" }}>
          Es un valor calculado, no confirmado: la región del account depende del proyecto y no del
          ambiente, así que hay que verificarlo contra Spinnaker antes de usarlo.
        </div>
      </div>

      <div>
        <div className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>
          Etiqueta que dispara cada pipeline
        </div>
        <div className="space-y-1.5">
          {d.repos.map((r) =>
            d.envs.map((env) => (
              <div
                key={`${r.name}-${env.key}`}
                className="text-xs"
                style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}
              >
                {r.name}_{env.key}_.*
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
