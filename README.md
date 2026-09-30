# DevOps Hub

Panel de control unificado para el equipo DevOps de Banco Ripley, construido con Tauri 2 + React + TypeScript.

## Módulos

- **Clusters** — descubrimiento, verificación de permisos y carga a kubeconfig para EKS (Chile, Perú) y GKE (Chile). Migrado desde `refresh-eks.sh`, `refresh-gke.sh` y `load-cluster.sh`.
- CI/CD Automation — pendiente de migrar (hoy vive como herramienta HTML separada).
- Bitbucket Repo Creator — pendiente de migrar (hoy vive como herramienta HTML separada).

## Requisitos

- [Node.js](https://nodejs.org/) 18 o superior
- [Rust](https://www.rust-lang.org/tools/install) (toolchain estable)
- [Tauri CLI](https://tauri.app/start/prerequisites/) — dependencias del sistema operativo según la plataforma (Xcode Command Line Tools en macOS, WebView2 en Windows, paquetes de desarrollo de WebKitGTK en Linux)

Además, para que el módulo Clusters funcione en tiempo de ejecución, la máquina necesita `aws`, `gcloud` y `kubectl` instalados y accesibles en el `PATH` — la app los verifica al abrir el módulo y avisa si falta alguno.

## Desarrollo

```bash
npm install
npm run tauri dev
```

## Compilar

```bash
npm run tauri build
```

Genera el instalador nativo para la plataforma actual en `src-tauri/target/release/bundle/`.

## Estado

Versión actual: ver `package.json`. El módulo Clusters está completo y funcional; CI/CD y Bitbucket siguen pendientes de migración.
