# DevOps Hub

Panel de control unificado para el equipo DevOps de Banco Ripley, construido con Tauri 2 + React + TypeScript.

## Módulos

- **Clusters** — descubrimiento, verificación de permisos y carga a kubeconfig para EKS (Chile, Perú) y GKE (Chile).
- **Bitbucket Repo Creator** — creación de repositorios con ramas estándar, Pipelines y vinculación a SonarQube Cloud.
- **CI/CD Automation** — generación de archivos de configuración CI/CD y subida a Bitbucket. Jenkins y Spinnaker pendientes.

## Instalación

Descarga el instalador para tu sistema operativo desde la sección [Releases](../../releases).

### macOS

Al abrir el `.dmg` descargado, macOS puede mostrar el mensaje _"DevOps Hub está dañado y no se puede abrir"_ porque el binario no está firmado con una cuenta de desarrollador de Apple. Para solucionarlo, ejecuta en Terminal:

```bash
xattr -dr com.apple.quarantine /Applications/DevOps\ Hub.app
```

Luego abre la app normalmente.

### Windows

Al ejecutar el instalador, Windows SmartScreen puede mostrar _"Windows protegió tu PC"_. Haz clic en **Más información** y luego en **Ejecutar de todas formas**.

### Linux (AppImage)

Dale permiso de ejecución al archivo descargado antes de abrirlo:

```bash
chmod +x DevOps.Hub_*.AppImage
```

---

## Requisitos

- [Node.js](https://nodejs.org/) 18 o superior
- [Rust](https://www.rust-lang.org/tools/install) (toolchain estable)
- [Tauri CLI](https://tauri.app/start/prerequisites/) — dependencias del sistema operativo según la plataforma (Xcode Command Line Tools en macOS, WebView2 en Windows, paquetes de desarrollo de WebKitGTK en Linux)

Además, para que el módulo Clusters funcione en tiempo de ejecución, la máquina necesita `aws`, `gcloud` y `kubectl` instalados. La app amplía el PATH automáticamente al iniciar para encontrarlos en las rutas habituales de Homebrew (`/opt/homebrew/bin`, `/usr/local/bin`) y de usuario (`~/.local/bin`), y avisa si falta alguno.

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

Versión actual: ver `package.json` (v0.6.3). Módulos completos: Clusters, Bitbucket Repo Creator, CI/CD Automation (generación y subida a Bitbucket) y SonarQube Cloud. Pendientes en CI/CD: Jenkins (v0.7.0) y Spinnaker (v0.8.0).
