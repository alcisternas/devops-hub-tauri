# CONTRATOS.md — Acuerdos entre módulos

Acuerdos sobre código compartido, firmas de funciones y decisiones que afectan a más de un módulo.

---

## Infraestructura compartida

### `src-tauri/src/modules/llavero.rs`

Creado por el chat de Bitbucket/Sonar. Adoptado por Sonar (v0.6.0) y Bitbucket (v0.6.3). Pendiente adopción por CI/CD (solo usa `obtener_credenciales()` del módulo Bitbucket, no lee el llavero directamente).

**Contrato:**
```rust
llavero::obtener(servicio: &str, usuario: &str) -> Result<String, String>
llavero::guardar(servicio: &str, usuario: &str, valor: &str) -> Result<(), String>
llavero::borrar(servicio: &str, usuario: &str) -> Result<(), String>
```

Reglas:
- `borrar()` es idempotente: `NoEntry` es éxito, no error.
- El helper sostiene el candado durante la lectura: dos llamadas simultáneas producen una sola lectura del llavero.
- El helper no valida credenciales. Cada módulo valida contra la API antes de llamar a `guardar()`.
- El helper guarda texto opaco: Bitbucket guarda JSON, Sonar un token suelto. El helper no sabe la diferencia.
- Se recupera de Mutex envenenado con `into_inner()`.

Convención de nombres de servicio: `devops-hub-<sistema>`. Un ítem por sistema externo.

Servicios registrados:

| Servicio | Usuario | Contenido | Módulo dueño |
|---|---|---|---|
| `devops-hub-bitbucket` | `credenciales` | JSON con `email` y `token` | Bitbucket |
| `devops-hub-sonarqube` | `token` | Token en texto plano | Sonar |
| `devops-hub-jira` | `token` | Token en texto plano (Bearer, scopes `read:jira-work` y `write:jira-work`) | Jira |
| `devops-hub-jenkins` | pendiente | pendiente | CI/CD Fase C |

---

### `src-tauri/src/modules/system.rs`

Creado por el chat de Clusters. Propósito general.

**Comando expuesto:** `check_dependencies` — verifica que `aws`, `gcloud` y `kubectl` estén instalados, con guía por sistema operativo. Cualquier módulo puede mostrarlo; nadie debe reimplementarlo.

---

### `src-tauri/src/lib.rs`

Archivo de registro de todos los comandos Tauri. **Nadie entrega este archivo completo.** Solo se entregan las líneas a agregar. Razón: entregar el archivo completo sobre una base vieja elimina silenciosamente los comandos de otros módulos sin error de compilación.

Estado actual de módulos registrados: Clusters (~16 comandos), Bitbucket (4), Sonar (3), CI/CD (4), Jira (6), Hardening (6, más el estado `EstadoHardening` en `.manage`).

Comando remanente sin uso en frontend: `get_eks_accounts` — candidato a eliminar.

---

### `src-tauri/src/modules/mod.rs`

Mismo criterio que `lib.rs`: solo entregar las líneas a agregar.

---

## Dependencias entre módulos de backend

`jira::procesar` llama directamente a `bitbucket::full_run::bitbucket_crear_repos` y usa `ConfigRun`, `ResumenRun`, `ResultadoRepo`, `repos::verificar`, `repos::rama_desarrollo`, `BBClient` y `bitbucket::credentials::obtener_credenciales`. **Si cambia la firma o los campos de cualquiera de ellos, Jira se rompe** (agregar un campo a `ConfigRun` rompe su constructor en `procesar.rs`). Avisar antes de cambiarlos, igual que con CI/CD.

`hardening` es independiente: no usa código de otros módulos de backend, solo `eks::config::RefreshError` y `eks::permissions::emit_progress`, como el resto. Tampoco los usa nadie más.

`load` lee los inventarios que escribe `eks` (`~/.aws/eks-inventory-chile.json` y `eks-inventory-peru.json`) y el de `gke`, y usa el `profile` y el `role` de cada entrada para `aws eks update-kubeconfig`. Desde la v0.6.11 el inventario de Perú tiene **una entrada por cluster y rol SSO real** (`aws-view`, `aws-edit`, `aws-admin-e`), con `profile` = `<prefijo>--<rol>`; el valor `titan-pipeline` ya no existe. Quien lea esos archivos no debe suponer un rol fijo para Perú. Dentro de `eks`, `peru.rs` reutiliza `permissions::{role_profile_name, write_chile_profile_block}` y `discovery::list_account_roles` (ahora `pub(crate)`): si cambia su firma o el formato del bloque de perfil, Chile y Perú se ven afectados a la vez. `gke::test_cluster::gke_get_credentials` es compartida por `load`, el recorrido de GKE y la prueba de un cluster.

---

## Archivos de UI compartidos

### `src/App.tsx`

Dueño original: Clusters. Modificado por Bitbucket y CI/CD para agregar rutas.

Patrón: módulos siempre montados, visibilidad por CSS. Al agregar un módulo nuevo, agregarlo al array `MODULES`. No usar desmontaje al navegar.

### `src/components/Layout.tsx`

Sidebar de la app. Cada módulo agrega su entrada al incorporarse. Coordinación necesaria antes de modificar.

### `src/modules/clusters/GlobalConsole.tsx`

Creado por Clusters. Modificado por Bitbucket (etiqueta `bitbucket`), Hardening (etiqueta `hardening`) y CI/CD (etiqueta `cicd`, props `collapsed`/`onToggleCollapsed`). Tiene tres dueños implícitos. Coordinar antes de modificar.

Para agregar un módulo nuevo al mapa de etiquetas, agregar la entrada en `MODULE_LABELS`:
```tsx
const MODULE_LABELS: Record<string, string> = {
    clusters: "[Clusters]",
    bitbucket: "[Bitbucket]",
    cicd: "[CI/CD]",
    hardening: "[Hardening]",
    // nuevo: "[NuevoMódulo]"
};
```

### `src/components/CollapsibleSection.tsx`

Creado por Clusters. Modificado por CI/CD: se quitó `overflow-hidden` porque recortaba los Dropdown anidados. El cambio es intencional y permanente. Si se recupera `overflow-hidden`, los Dropdown de CI/CD dejan de funcionar.

### `src/components/ui.tsx`

Creado por Clusters. No modificado por otros módulos. Contiene: `Button`, `Dropdown`, `ErrorNotice`, `SuccessNotice`, `WarningNotice`, `FieldLabel`, `MetaLine`.

### `src/modules/home/index.tsx`

Pantalla de inicio. Cada módulo agrega su tarjeta al incorporarse, con `available: true`. Coordinación necesaria.

---

## Deuda técnica conocida

### ~~`bitbucket/credentials.rs` — migración pendiente al helper~~ — Resuelto en v0.6.3

`bitbucket/credentials.rs` fue migrado a `llavero.rs` en v0.6.3. La caché propia fue eliminada; el módulo usa `static LLAVERO: Llavero` igual que Sonar. CI/CD sigue consumiendo `obtener_credenciales()` del módulo Bitbucket sin cambios.

### `withGlobalTauri: true` en `tauri.conf.json`

Activado por Bitbucket para depurar desde la consola del navegador. Ningún módulo lo necesita en producción (verificado: no hay referencias a `window.__TAURI__` en `src/`). Se quita cuando Jenkins y Spinnaker estén completos.

---

## Temas abiertos que necesitan decisión

### Templates de archivos CI/CD

El chat de Templates/Definiciones identificó 24 preguntas abiertas y 7 contradicciones en los archivos generados (`JenkinsParams`, `service.yaml`, `istio.yaml`, `bitbucket-pipelines.yml`). Bloquea ajustes en los generadores de CI/CD. La revisión está en curso en conversación aparte.

Puntos específicos sin resolver:
- Cloud Run: condición para incluir red compartida (VPC) en `service.yaml`
- Cloud Run: `serviceAccountName` real (el ejemplo real usa `@appspot.gserviceaccount.com`, no confirmado como estándar)
- Cloud Run: variables de entorno de aplicación — dejar vacías como en K8S o definir subconjunto
- Apigee Perú: valores de `APIGEE_ORG`, `APIGEE_ENV`, `APIGEE_SERVICE_ID` por ambiente no definidos
- `GOLANG_TOOL_VERSION=1.21` (K8S) vs `GO_TOOL_VERSION=1.25` (Cloud Run): ¿intencional?

### Decoración de pull requests con SonarQube

El binding Bitbucket↔Sonar existe y está verificado por API. Que el comentario de Sonar aparezca en un PR real nunca se probó. Requiere un PR sobre un repositorio bindeado. Depende de dos tickets separados en el tiempo.

### ~~Diálogos del llavero en macOS~~ — Resuelto en v0.6.3

Los 4 diálogos al iniciar la app instalada quedaron en 0 al arrancar y máximo 2 en el primer uso (uno por ítem del Keychain). Ver entrada en HISTORIAL.md v0.6.3.

### AppImage en Linux con D-Bus

Las AppImage excluyen algunas librerías de sistema. No está verificado que el binario encuentre D-Bus en la máquina del usuario. Se descubre al correr el workflow en una máquina Linux real.

### Logo del camaleón

El origen y la licencia no están confirmados. La app se distribuye a ~15 personas. Confirmar antes de distribuir.

---

## Lo que cada módulo NO toca del otro

| Módulo | No tocar sin coordinar |
|---|---|
| Clusters | `GlobalConsole.tsx`, `CollapsibleSection.tsx`, `ui.tsx`, `index.css` |
| Bitbucket | Sin restricciones activas. `credentials.rs` ya usa `llavero.rs` (v0.6.3). |
| Hardening | `testdata/oro.json` se genera con el motor de Studio; no se edita a mano (ver `TRASPASO-hardening.md` §2) |
| CI/CD | `CollapsibleSection.tsx` — el cambio de `overflow-hidden` es suyo y los otros dependen de él |
| Todos | `lib.rs`, `modules/mod.rs` — nunca entregar completos |
