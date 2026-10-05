// istio.yaml — el VirtualService que publica el microservicio en el gateway.
//
// Solo existe en Kubernetes: Cloud Run no usa Istio, y para esa
// infraestructura la función devuelve vacío y el archivo no se genera.
//
// El VirtualService vive siempre en el namespace istio-system, aunque el
// servicio al que apunta esté en otro. Eso no es un descuido: el gateway
// corre ahí.

import type { CicdData, EnvItem } from "../shared";

export function buildIstioYaml(d: CicdData, repoName: string, env: EnvItem): string {
  if (d.isCR) return "";

  const ns = d.ns[env.key] || "customer-service";
  const prefix = (d.uriPrefixes && d.uriPrefixes[repoName]) || repoName;

  // El /v1 se agrega acá, no en el campo del formulario: el prefijo que
  // escribe el usuario es solo la parte del nombre.
  return `apiVersion: networking.istio.io/v1alpha3
kind: VirtualService
metadata:
  name: ${repoName}
  namespace: istio-system
spec:
  hosts:
    - "*"
  gateways:
    - istio-gateway
  http:
    - match:
        - uri:
            prefix: /${prefix}/v1
      rewrite:
        uri: "/${prefix}/v1"
      route:
        - destination:
            host: ${repoName}.${ns}.svc.cluster.local
            port:
              number: 8090`;
}
