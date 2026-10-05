// service.yaml — el archivo de despliegue. Son dos archivos muy distintos
// según la infraestructura, y por eso la función arranca con la bifurcación:
//
//   Cloud Run  → un Service de Knative, con la red/subred del proyecto
//                resuelta desde el catálogo de subredes.
//   Kubernetes → tres documentos en uno: ConfigMap, Service y Deployment.
//
// Los literales de ${SERVICE_NAME_GCP}, ${IMAGE_URL} y ${SERVICE_NAME} del
// bloque de Cloud Run van escapados porque los reemplaza el pipeline de
// Jenkins al desplegar, no esta herramienta.

import type { CatalogoSubredes, CicdData, EnvItem } from "../shared";
import { extractProjectKey, findSubnet, imagenServiceYaml } from "../shared";

export function buildServiceYaml(
  d: CicdData,
  repoName: string,
  env: EnvItem,
  subnets: CatalogoSubredes
): string {
  if (d.isCR) {
    const nodeEnvMap: Record<string, string> = {
      dev: "development",
      release: "release",
      prod: "production",
    };
    const nodeEnv = nodeEnvMap[env.key] || env.key;
    const projKey = extractProjectKey(d.gcpDev || d.gcpRelease || d.gcpProd || "");
    const subnetMatch = findSubnet(subnets, projKey, env.key);

    // Sin coincidencia se escribe el marcador en vez de inventar una red.
    // La vista de archivos lo resalta, y la subida a Bitbucket (fase B) lo
    // va a bloquear antes de escribir nada en el repositorio.
    const networkJson = subnetMatch
      ? `[{"network":"projects/${subnetMatch.networkProject}/global/networks/${subnetMatch.network}","subnetwork":"projects/${subnetMatch.networkProject}/regions/${subnetMatch.region}/subnetworks/${subnetMatch.name}"}]`
      : `[{"network":"PENDIENTE-SIN-COINCIDENCIA","subnetwork":"PENDIENTE-SIN-COINCIDENCIA"}]`;

    return `apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: \${SERVICE_NAME_GCP}
  labels:
    run.googleapis.com/satisfiesPzs: "true"
  annotations:
    run.googleapis.com/client-name: cloud-console
    run.googleapis.com/ingress: all
    run.googleapis.com/ingress-status: all
    run.googleapis.com/minScale: "0"
    run.googleapis.com/max-surge: "0"
    run.googleapis.com/invoker-iam-disabled: "true"
spec:
  template:
    metadata: 
      annotations: 
        autoscaling.knative.dev/maxScale: '1'
        run.googleapis.com/vpc-access-egress: all-traffic
        run.googleapis.com/network-interfaces: >
          ${networkJson}
    spec:
      containerConcurrency: 80
      timeoutSeconds: 300
      containers:
        - image: \${IMAGE_URL}
          ports:
            - containerPort: 8080
          env:
          - name: NODE_ENV
            value: ${nodeEnv}
          - name: SERVICE_NAME
            value: \${SERVICE_NAME}
          resources:
            limits:
              cpu: 1000m
              memory: 512Mi
  traffic:
    - percent: 100
      latestRevision: true
`;
  }

  const ns = d.ns[env.key] || "customer-service";

  // serviceAccountName queda fijo como literal: acá se espera un recurso
  // ServiceAccount de Kubernetes, no un correo de IAM de GCP. Son cosas
  // distintas y mezclarlas fue un error real que ya se corrigió.
  //
  // La imagen va sin tag, igual que los repositorios en producción:
  // Spinnaker inyecta al desplegar la versión que disparó el trigger.
  return `apiVersion: v1
kind: ConfigMap
metadata:
  name: ${repoName}
  namespace: ${ns}
data:
  # Agregar variables de entorno del microservicio
---
apiVersion: v1
kind: Service
metadata:
  name: ${repoName}
  namespace: ${ns}
  labels:
    app: ${repoName}
spec:
  type: ClusterIP
  ports:
    - name: http
      port: 8090
      targetPort: 8090
      protocol: TCP
    - name: grpc
      port: 50051
      targetPort: 50051
      protocol: TCP
  selector:
    app: ${repoName}
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${repoName}
  namespace: ${ns}
  labels:
    app: ${repoName}
spec:
  replicas: 1
  selector:
    matchLabels:
      app: ${repoName}
  template:
    metadata:
      labels:
        app: ${repoName}
    spec:
      serviceAccountName: SA-DEL-PROYECTO
      containers:
        - name: ${repoName}
          image: ${imagenServiceYaml(d)}
          ports:
            - containerPort: 8090
            - containerPort: 50051
          envFrom:
            - configMapRef:
                name: ${repoName}
          resources:
            requests:
              cpu: "100m"
              memory: "256Mi"
            limits:
              cpu: "200m"
              memory: "512Mi"`;
}
