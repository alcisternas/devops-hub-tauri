# CLAUDE.md — DevOps Hub

Léelo completo antes de tocar cualquier archivo. Es corto a propósito.

---

## Qué es esto

App de escritorio nativa (Tauri 2 + React 18 + TypeScript + Rust) para el equipo DevOps de Banco Ripley. Gestiona clusters Kubernetes, repositorios Bitbucket, proyectos SonarQube y pipelines CI/CD.

---

## Stack

| Capa | Tecnología |
|---|---|
| Backend | Rust — `src-tauri/src/` |
| Frontend | React 18 + TypeScript + Tailwind CSS — `src/` |
| Estado compartido | Zustand — `src/store/index.ts` |
| Navegación | React Router — `src/App.tsx` |
| Bundler | Vite |

---

## Estructura de módulos

```
src/modules/clusters/      ← EKS Chile/Perú + GKE + carga kubeconfig
src/modules/bitbucket/     ← creación de repos, Pipelines, ramas, SonarQube; modo "Desde ticket de Jira"
src/modules/cicd/          ← generadores CI/CD + subida a Bitbucket (Jenkins y Spinnaker pendientes)
src/modules/hardening/     ← hardening de imágenes: analiza, elige qué conservar, genera el Dockerfile (fase 1)
src/modules/home/          ← pantalla de inicio

src-tauri/src/modules/eks/
src-tauri/src/modules/gke/
src-tauri/src/modules/load/
src-tauri/src/modules/bitbucket/
src-tauri/src/modules/sonar/
src-tauri/src/modules/jira/        ← tickets de altas: lectura, validación, cierre y evidencia (usa el motor de bitbucket/)
src-tauri/src/modules/cicd/
src-tauri/src/modules/hardening/   ← port a Rust de Container Hardening Studio; pruebas contra su salida en Python
src-tauri/src/modules/llavero.rs   ← infraestructura compartida de credenciales
src-tauri/src/modules/system.rs    ← check_dependencies, propósito general
```

---

## Reglas de proceso — sin excepciones

1. **Presentar el plan completo y esperar aprobación explícita** antes de escribir cualquier código. "Funcionó" o "probado" no es aprobación. La aprobación es una respuesta directa al plan presentado para ese archivo específico.
2. **Entregar los datos disponibles y las alternativas**. El usuario decide o aporta más información. El ciclo se repite hasta que apruebe una opción.
3. **Explicar cada cambio**: qué hace y por qué.
4. **Verificar sintaxis antes de entregar** (`cargo check`, `tsc --noEmit` o balance de delimitadores mínimo).
5. **Nunca asumir rutas de archivo** — el usuario las define.
6. **Antes de construir algo nuevo**, revisar si ya existe en el proyecto.
7. **En archivos compartidos**, entregar solo las líneas a cambiar. Auditar toda línea eliminada antes de entregar. Nunca entregar un archivo compartido completo.
8. **Avisar antes de tocar cualquier archivo fuera de la carpeta del módulo propio.**
9. **Español latinoamericano neutro, sin voseo.**
10. **Entregar en zip con la estructura de carpetas armada** cuando hay más de un archivo.
11. **Actualizar la documentación en el mismo commit que el cambio**, nunca después en un commit separado. Mínimo: tabla de versiones en `CLAUDE.md` y entrada en `docs/HISTORIAL.md`. Si el cambio introduce un patrón nuevo, un error conocido o modifica un contrato entre módulos, actualizar también la sección correspondiente de `CLAUDE.md` y `docs/CONTRATOS.md`. El orden es: editar código → editar docs → `cargo check` / `tsc` → commit único.

---

## Convención de versiones

Cuatro archivos siempre alineados en cada entrega:
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `src/version.ts`

**Patch** (`0.x.N`) para correcciones y ajustes visuales.
**Minor** (`0.N.0`) para funcionalidad nueva o un módulo completo.

Versiones publicadas:

| Versión | Contenido |
|---|---|
| 0.2.20 | Módulo Clusters completo |
| 0.3.0 | Módulo Bitbucket |
| 0.5.2 | CI/CD Fase A+B + correcciones |
| 0.6.0 | SonarQube Cloud integrado a Bitbucket |
| 0.6.2 | Fix PATH en app instalada (macOS/Linux) |
| 0.6.3 | Fix diálogos duplicados del Keychain al iniciar |
| 0.6.4 | README e instrucciones de instalación por plataforma; bump CI Node.js 22 |
| 0.6.5 | Fix PATH en Windows para detectar gcloud y AWS CLI |
| 0.6.6 | Fix invocación de gcloud en Windows (usar gcloud.cmd, no gcloud) |
| 0.6.7 | Fix cuenta GCP hardcodeada en instalaciones nuevas; login libre en primer uso |
| 0.6.8 | Fix sesión GCP en Windows: eliminar --format value(token) y set_active_account |
| 0.6.9 | Módulo Hardening de imágenes (fase 1), modo "Desde ticket de Jira" en Bitbucket, migración a Tailwind 4 y dependencias sin vulnerabilidades (`npm audit` en 0) |
| 0.6.10 | Fix: `ensure_gcp_session` vuelve a renovar la sesión GCP sola (login en el navegador) en vez de devolver un error sin salida; "Cargar clusters" limpia las marcas de lo cargado |
| 0.6.11 | Fix: GKE carga con `--dns-endpoint` (sin él Lens daba timeout contra la IP privada); EKS Perú pasa de `titan-pipeline` a roles SSO reales por cuenta, como Chile |

Versiones planificadas:

| Versión | Contenido |
|---|---|
| 0.7.0 | CI/CD Fase C — Jenkins |
| 0.8.0 | CI/CD Fase D — Spinnaker |
| 0.9.0 | CI/CD Fase E — encadenado |

---

## Patrones establecidos — usar siempre estos, no reinventar

### Comunicación Rust ↔ React

```rust
// Rust: definir y registrar
#[tauri::command]
fn mi_comando(param: &str) -> Result<String, String> { ... }

.invoke_handler(tauri::generate_handler![mi_comando])
```

```tsx
// React: llamar
import { invoke } from "@tauri-apps/api/core";
const resultado = await invoke<string>("mi_comando", { param: "valor" });
```

### Eventos de progreso para operaciones largas

```rust
// Rust emite
app.emit("module_progress", json!({
    "modulo": "clusters",
    "etapa": "verificando_permisos",
    "item": "cluster-name",
    "progreso": 3,
    "total": 52,
    "estado": "running"  // "ok" | "error" | "running"
})).unwrap();
```

```tsx
// React escucha
import { listen } from "@tauri-apps/api/event";
useEffect(() => {
    const unlisten = listen("module_progress", (e) => setProgreso(e.payload));
    return () => { unlisten.then(f => f()); };
}, []);
```

### Credenciales — usar `llavero.rs`

```rust
// Leer
let cred = llavero::obtener("devops-hub-<sistema>", "usuario")?;
// Guardar (validar contra la API ANTES de llamar a esto)
llavero::guardar("devops-hub-<sistema>", "usuario", &valor)?;
// Borrar
llavero::borrar("devops-hub-<sistema>", "usuario")?;
```

Convención de nombres: `devops-hub-<sistema>`, un ítem por sistema externo.
**Validar siempre contra la API antes de guardar** — no dejar credenciales rotas en el llavero.

### Diseño — tokens de `index.css`, no colores Tailwind crudos

Usar `var(--bg)`, `var(--surface)`, `var(--text)`, etc. definidos en `src/index.css`.

### Navegación — módulos siempre montados

`App.tsx` monta todos los módulos al iniciar y controla visibilidad por CSS. No usar desmontaje al navegar. Al agregar un módulo nuevo, agregarlo al array `MODULES` en `App.tsx`.

---

## Errores que ya se cometieron — no repetir

- **Entregar `lib.rs` o `modules/mod.rs` completos sobre una base vieja**: borra silenciosamente el registro de otros módulos. El binario compila sin error pero el módulo desaparece. → Entregar solo las líneas a agregar.
- **Revertir mejoras del usuario** al entregar archivos completos desde una copia local desactualizada. → Siempre trabajar sobre el zip más reciente entregado por el usuario.
- **`get_eks_accounts`**: comando Rust registrado en `lib.rs` sin consumidor en el frontend. Candidato a eliminar.
- **`window.__TAURI__`** no funciona sin `withGlobalTauri: true` en `tauri.conf.json`. Ese flag está activo temporalmente; no depender de él en código nuevo.
- **`box-shadow` en `<tr>` con `border-collapse`** no se pinta. Usar borde en `<td>` individual o separar header/body en dos tablas.
- **`padding` en `<table>` con `border-collapse`** se ignora. Envolver en un `div`.
- **`overflow-hidden` en `CollapsibleSection`** recorta los Dropdown anidados. Está quitado intencionalmente.
- **`position: sticky`** en tablas con filas dinámicas es poco confiable. Usar dos tablas separadas con `colgroup` idéntico.
- **Nombrar una variable `console`** tapa el objeto global de JavaScript.
- **Balanceador de llaves propio** no detecta errores de tipos ni de borrow-checker. No es sustituto de `cargo check` o `tsc`.
- **`borrar()` en el llavero debe ser idempotente**: `NoEntry` es éxito, no error.
- **Voseo en textos de interfaz**: prohibido. Español latinoamericano neutro.
- **PATH en app instalada (macOS/Linux)**: macOS y Linux lanzan las apps GUI con PATH mínimo del sistema (`/usr/bin:/bin`). `aws`, `gcloud`, `kubectl` instalados en `/opt/homebrew/bin` o `/usr/local/bin` no se encuentran. → `ampliar_path()` en `lib.rs` lo corrige al iniciar antes de que Tauri arranque sus hilos. El bug **no aparece en `npm run tauri dev`** — probar siempre con el binario instalado antes de cada release.
- **PATH en app instalada (Windows)**: `ampliar_path()` era no-op en Windows. Explorer hereda el PATH del momento en que arrancó, así que `gcloud` instalado después o en otra sesión de consola no era visible. → Implementar `ampliar_path()` para Windows agregando `%APPDATA%\gcloud`, `%LOCALAPPDATA%\Google\Cloud SDK\...\bin`, rutas de sistema de gcloud y `C:\Program Files\Amazon\AWSCLIV2`. El bug **no aparece en `npm run tauri dev`** — mismo patrón que el de macOS/Linux.
- **Cuenta GCP hardcodeada en `default_gcp_chile()`**: `eks/config.rs` tenía la cuenta del desarrollador en el valor por defecto. Cualquier instalación nueva heredaba esa cuenta al crear `accounts.json` por primera vez. → Dejar `account: ""` en el default; en `setup_gcloud_login`, si la cuenta está vacía, hacer `gcloud auth login` sin `--account` (el usuario elige en el navegador), leer la cuenta activa con `gcloud config get-value account` y guardarla en `accounts.json` para las siguientes sesiones.
- **`gcloud` en Windows no es ejecutable directamente**: la carpeta `bin` del SDK contiene tres archivos con el mismo nombre base — `gcloud` (script Unix sin extensión), `gcloud.cmd` y `gcloud.ps1`. `CreateProcess` de Windows encuentra el script Unix y no puede ejecutarlo. El wrapper real es `gcloud.cmd`. → Definir `GCLOUD_BIN` en `gke/mod.rs` con `#[cfg]` y usarlo en todos los archivos que invoquen gcloud (`gke/login.rs`, `gke/discovery.rs`, `gke/test_cluster.rs`, `cicd/subnets.rs`). Lo mismo aplica a cualquier herramienta que tenga un wrapper `.cmd` en Windows — no asumir que el nombre sin extensión funciona.
- **Un comando Tauri registrado sin consumidor en el frontend dejó el login de GCP sin camino**: en 0.6.8 `ensure_gcp_session` pasó a "solo verificar" y el login quedó en `setup_gcloud_login`, que ninguna pantalla invoca; con la sesión vencida el usuario no podía renovarla. → Quien necesita la sesión (`ensure_gcp_session`) debe renovarla sola, como `ensure_chile_sso` para AWS; es bloqueante, se llama con `spawn_blocking` y se sigue con la cuenta que devuelve. Antes de quitar lógica de una función, comprobar quién más hacía depender de ella un flujo.
- **Diálogos duplicados del Keychain**: `App.tsx` monta todos los módulos al iniciar, incluyendo `CredentialsPanel` dos veces (Bitbucket + CI/CD `SubirPanel`). Sin caché, cada instancia leía el Keychain por separado. → `bitbucket/credentials.rs` migrado a `llavero.rs` (caché en memoria); `CredentialsPanel` respeta `activo` y comparte `estadoCompartido` entre instancias. Mismo patrón que `SonarCredentialsPanel`.
- **Jira no devuelve 401 con un token vencido o revocado**: responde 200 con lista vacía, porque trata la petición como anónima. Validar un token Jira con "la búsqueda funcionó" lo da por bueno y el módulo diría "no hay tickets" para siempre. → `jira/credentials.rs` busca algo que existe seguro (los tickets del proyecto SCDO) y una búsqueda vacía delata el token roto.
- **Los tokens de Jira con scopes no sirven contra `<sitio>.atlassian.net`**: hay que usar `api.atlassian.com/ex/jira/<cloudId>` con Bearer. El `cloudId` sale de `<sitio>/_edge/tenant_info`, sin autenticación. La API de formularios tiene otra base (`api.atlassian.com/jira/forms/cloud/<cloudId>`).
- **Los nombres de repositorio del ticket no están en los campos "Repositorio 1-5"** (vienen vacíos): viven en las respuestas del formulario "ABM de Repositorios Bitbucket".
- **La clave del proyecto de Bitbucket no se pasa a mayúsculas**: existe una real en minúsculas (`peru_control_de_cambios`). El campo `projectKey` del modo manual sí la fuerza a mayúsculas; el modo Jira usa la del ticket tal cual.
- **Etiquetas de Jira**: cualquier texto distinto crea una etiqueta nueva. Solo se agregan `ACN`, `DEVOPS-CHAPTER` y `CHILE`/`PERU` (sin tilde, aunque el campo País diga "Perú").
- **Tailwind 4 cambia valores por defecto sin avisar**: la fuente del tema (4.3 usa otra lista que Tailwind 3), el cursor de los botones (flecha en vez de mano) y el color de borde. `index.css` conserva los de Tailwind 3 con las que se diseñó la app.
- **El módulo Hardening es un port verificado, no una reescritura libre.** `testdata/oro.json` lo genera el Python de Studio; si el motor cambia, se regenera y se revisan las diferencias. `workdir` se valida acá (Studio no lo hace) y la cabecera del Dockerfile lleva la firma del hub (la prueba usa la de Studio para comparar la huella).
- **Trivy corre con entorno mínimo y sin flags que oculten hallazgos** (`--ignore-unfixed`, `--vex`, `--skip-*`): la política bancaria cuenta también lo que no tiene parche.
- **Docker Scout no se simula.** Está deshabilitado hasta tener una salida SARIF real para probar su lector.
- **`get-credentials` de GKE sin `--dns-endpoint` deja la IP privada en el kubeconfig**: la carga termina "bien" pero Lens da timeout (`dial tcp 10.x.x.x:443: i/o timeout`) porque la IP no es alcanzable desde la red del usuario. → `gke_get_credentials` prueba primero con `--dns-endpoint` y reintenta sin él si gcloud falla. Con DNS endpoint el kubeconfig no trae CA embebido: `read_server_and_ca` devuelve `None` y las verificaciones corren con `--context`. Probar con un cluster de IP privada y abrirlo en Lens, no solo mirar el mensaje de la app.
- **Perú ya no usa `titan-pipeline` ni `devops-base`**: el rol se retiró (daba `AccessDenied` hasta en `eks:ListClusters`) y los permisos pasaron a los roles SSO (`aws-view`, `aws-edit`, `aws-admin-e`). Perú se descubre y recorre como Chile: un perfil `<prefijo>--<rol>` por cuenta y rol, y una entrada del inventario por cluster y rol. No reintroducir AssumeRole en cadena. Un permiso denegado por una SCP (`explicit deny in a service control policy`) no es un bug de la app: en la consola de AWS verificar la región (el error visto era de us-east-2 y la app usa us-east-1).

---

## Distribución

GitHub Actions compila para Mac (Apple Silicon + Intel), Windows y Linux al crear un tag.

```bash
git add .
git commit -m "feat: descripción"
git push
git tag vX.Y.Z
git push origin vX.Y.Z
```

El actualizador automático está configurado en `tauri.conf.json` pero **no está operativo**: el plugin no está instalado. Las claves de firma existen y el workflow pasa el secreto. Pendiente activar.

---

## Pendientes globales

- `withGlobalTauri: true` — quitar cuando Jenkins y Spinnaker estén completos
- `bitbucket/credentials.rs` — migrado a `llavero.rs` en v0.6.3 ✓
- `get_eks_accounts` — eliminar de `lib.rs` si no se reactiva
- Actualizador automático — activar el plugin
- Validar `CollapsibleSection` sin `overflow-hidden` en Clusters y Bitbucket (cosmético)
- Medir diálogos del llavero con el binario compilado
- Verificar AppImage en Linux con D-Bus
- Confirmar origen/licencia del logo del camaleón antes de distribuir
- Templates de archivos CI/CD en revisión — bloquea ajustes en generadores
- Fix del checkbox de selección en `LoadTab` nunca se versionó (está en el código, no tiene tag)
