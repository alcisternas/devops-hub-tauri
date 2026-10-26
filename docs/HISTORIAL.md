# Historial de versiones — DevOps Hub

Registro de lo que entró en cada entrega. Se actualiza en el mismo trabajo
que produce la versión, no después.

**Convención:** cuatro archivos deben coincidir siempre en el número de
versión — `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
y `src/version.ts`. Patch para correcciones y ajustes visuales, minor para
funcionalidad nueva.

---

## 0.6.12

**Mejorado**

- **EKS Perú: la fase A del recorrido (sts, listado y describe por cuenta y rol) ahora corre en paralelo.** Al pasar Perú a roles SSO reales, la fase A pasó de unas 27 pasadas por cuenta a unas 87 por cuenta y rol, todas en serie. Se divide en dos: **A0** escribe todos los perfiles en `~/.aws/config` de a uno (siguen serializados porque comparten archivo; es solo escritura local) y **A1** ejecuta `sts`, `list-clusters` y `describe-cluster` de cada intento en paralelo, acotado por un semáforo (`MAX_CONCURRENT_ACCOUNTS = 6`) y con `spawn_blocking`, porque las llamadas usan `std::process`. Cada tarea escribe solo en su ranura, así que el inventario y la lista de omitidos conservan el orden de siempre, sin depender de quién termine primero. El progreso usa un contador atómico propio, con un solo evento por intento al terminar. La fase B, el formato del inventario, los CSV y la interfaz no cambian. Solo toca `eks/peru.rs`; Chile queda igual.
- **Medición que respalda el valor 6** (2026-10-26, simulación con la CLI sobre 87 combinaciones cuenta+rol, credenciales en caché, una corrida de cada tipo): secuencial 324 s, 3 a la vez 103 s, 6 a la vez 60 s. Es una simulación, no la app: no incluye la escritura de perfiles ni el descubrimiento (que sigue secuencial, con una llamada a `list-account-roles` por cuenta), y el script descartaba los errores, así que no se comprobó entonces que el resultado fuera idéntico ni la ausencia de throttling de AWS. **Pendiente de validar con el binario instalado:** repetir el recorrido de Perú, comparar el inventario con una copia del anterior (mismas entradas de cluster, rol y permisos; el orden no importa), revisar que la consola no muestre errores nuevos de throttling y comparar el tiempo.

---

## 0.6.11

**Corregido**

- **GKE: los clusters con IP privada cargaban "bien" pero Lens daba timeout.** `gke_get_credentials` ejecutaba `gcloud container clusters get-credentials` sin `--dns-endpoint`, y gcloud dejaba en el kubeconfig la IP privada del control plane (por ejemplo `https://10.73.0.2`), inalcanzable desde la red del usuario. La carga terminaba sin error porque `get-credentials` sí funcionaba; el timeout aparecía recién cuando Lens se conectaba (`dial tcp 10.73.0.2:443: i/o timeout`). Ahora primero se ejecuta con `--dns-endpoint` y, si gcloud falla (cluster sin DNS endpoint habilitado o gcloud que no conoce el flag), se reintenta sin él, como antes. Verificado con `gke-br-dev-core-audi-experian-01` sobre un kubeconfig temporal: sin el flag el `server` es la IP privada y `kubectl` da timeout; con el flag el `server` es `https://gke-….gke.goog` y `kubectl` responde; el nombre del contexto (`gke_<proyecto>_<zona>_<cluster>`) y su renombrado al alias no cambian. **Efecto en el recorrido de GKE:** con DNS endpoint el kubeconfig no trae CA embebido, así que `read_server_and_ca` devuelve `None` y las 14 verificaciones corren en modo `--context` (funciona, pero es más lento que server+token+CA). Los inventarios de GKE generados antes de este cambio pueden marcar "sin acceso" a clusters con IP privada: hay que repetir el recorrido de GKE.
- **EKS Perú: el rol `titan-pipeline` se retiró y los permisos pasaron a la SSO.** Con titan, hasta `eks:ListClusters` daba `AccessDenied` (probado en la cuenta 147882410889), y la carga de Perú fallaba en `update-kubeconfig`. Perú ahora entra igual que Chile: `discover_peru_accounts` pide los roles reales de cada cuenta (`list-account-roles`) y `run_full_peru_scan` escribe un perfil SSO por cuenta+rol (`<prefijo>--<rol>`, con `sso_role_name`), verifica y guarda en el inventario cada rol por separado. Se eliminan el AssumeRole en cadena y el perfil base `devops-base` (`ensure_peru_sso` ya no lo escribe). Una cuenta cuenta si tiene al menos un cluster ACTIVE; las demás se omiten, como antes. Las cuentas fuera del mapeo de `accounts.json` ya no se omiten: usan un prefijo derivado del nombre de la cuenta. Medido el 2026-10-26 con el usuario de prueba: 38 cuentas en el portal, 87 combinaciones cuenta+rol, 86 con `list-clusters` correcto (la excepción es `lambda-widget`, que no es un rol de EKS) y 17 cuentas con al menos un cluster en us-east-1. En `mastercardc2pxbdr-dev-cluster` (cuenta 147882410889), `aws-edit` y `aws-admin-e` tienen 14 de 14 permisos y `aws-view` tiene 11 de 14 (sin eliminar pods, `exec` ni `rollout restart`).
- **Cambios visibles:** el inventario de Perú pasa a tener una entrada por cluster y rol (el campo `role` deja de ser `titan-pipeline`) y el contexto de kubectl incluye el rol real (`[Peru] [AWS] / <cuenta> / aws-edit / <cluster>`). El recorrido de Perú verifica unas 112 combinaciones cluster-rol y tarda más que antes. `PeruFullRunResult.total_cuentas_intentadas` pasa a `total_intentos_rol`, igual que en Chile.
- **Perfiles viejos de Perú** (`core-dev`, `devops-base`, etc.) quedan en `~/.aws/config` sin usar; no se borran. Los campos `base_profile`, `base_account_id`, `base_role` y `role_arn_name` de `PeruConfig` también quedan sin usar para no tocar los `accounts.json` existentes.
- **Archivos fuera del módulo propio tocados:** ninguno. Los cambios de EKS están en `eks/` (`peru.rs`, `sso.rs`, `discovery.rs` con `list_account_roles` ahora `pub(crate)`, y solo comentarios en `permissions.rs` y `full_run.rs`), los de GKE en `gke/test_cluster.rs` y los de interfaz en `clusters/RefreshTab.tsx` y `clusters/sections/PeruSection.tsx`.
- **Pendiente de validar con el binario instalado:** abrir `gke-br-dev-core-audi-experian-01` en Lens, repetir el recorrido de GKE y el de Perú, cargar `mastercardc2pxbdr-dev-cluster` y confirmar que Chile no cambió. Las pruebas de este cambio se hicieron con la CLI y kubeconfigs temporales; la app compilada no se ejecutó.

---

## 0.6.10

**Corregido**

- **Con la sesión de GCP vencida no había forma de renovarla desde la app.** La v0.6.8 convirtió `ensure_gcp_session` en "solo verificar" y dejó el login únicamente en `setup_gcloud_login`, pero ningún archivo del frontend invoca ese comando. Resultado: al vencer la sesión (por ejemplo, tras cambiar la contraseña de la cuenta) "Cargar clusters" y "Descubrir clusters" devolvían `[sesion_gcp_vencida] ... Ve a Clusters → GKE Chile ...`, y ese camino terminaba en el mismo error. Antes de 0.6.8 el login ocurría dentro de `ensure_gcp_session`, por eso en macOS funcionaba sin que se notara. Cambios: (1) `ensure_gcp_session` vuelve a renovar la sesión sola, igual que `ensure_chile_sso` / `ensure_peru_sso` para AWS: reutiliza el token si es válido; si no (token vencido, cuenta vacía o cuenta heredada sin sesión) abre `gcloud auth login` sin `--account`, detecta la cuenta activa, verifica el token y la guarda en `accounts.json`. No fuerza ninguna cuenta ni ejecuta `gcloud config set account` (se mantiene lo corregido en 0.6.7 y 0.6.8). (2) Ahora recibe el `AppHandle`, es bloqueante y devuelve `GcpLoginResult` con la cuenta efectiva; los tres puntos de llamada (`gke/discovery.rs`, `load/loader.rs` y `cicd/subnets.rs`, este con dos llamadas) la invocan con `spawn_blocking` y siguen con la cuenta devuelta. (3) `setup_gcloud_login` pasa a delegar en `ensure_gcp_session`. (4) El mensaje de respaldo ya no remite a una pantalla: indica que no se pudo iniciar sesión y qué hacer. **Pendiente de validar con el binario instalado en macOS y Windows**: el login con navegador no se pudo ejecutar aquí. Si la cuenta activa de gcloud difiere de la guardada, `get_gcp_token()` usa la activa (sin `--account`); queda como hipótesis por confirmar con `gcloud auth list`.
- **Archivos fuera del módulo tocados:** `load/loader.rs` y `cicd/subnets.rs`, solo en la llamada a `ensure_gcp_session`.
- **Las marcas de selección se limpiaban solo al actualizar la lista.** En "Cargar clusters", tras una carga los clusters seguían marcados; al filtrar y marcar otros, los anteriores se volvían a cargar. Ahora `loadSelected` lee el `LoadSummary` y, tras la carga (agregando o reemplazando), quita de la selección los clusters con estado `cargado` u `omitido`. Los que dieron `error` siguen marcados para reintentar. Si el comando falla del todo (por ejemplo, login de GCP fallido), la selección se conserva. Solo cambia `clusters/LoadTab.tsx`.

---

## 0.6.9

**Agregado**

- **Módulo "Hardening de imágenes" (fase 1).** Recibe la referencia de una imagen pública, la resuelve por digest en el registro, la escanea con Trivy (Docker no hace falta), muestra los componentes que se pueden conservar o quitar según el tipo de imagen (por defecto conserva npm, que las imágenes de desarrollo necesitan), calcula el impacto y el plan de corrección con cada cambio y genera el Dockerfile endurecido (rootfs limpio, `FROM scratch`, con pruebas funcionales), listo para guardar y construir. **No construye ni reescanea todavía**: la pantalla entrega los comandos para construir, verificar y subir a ECR. Es un port a Rust del motor de Container Hardening Studio, verificado contra la salida de su código Python con un escaneo real (9 configuraciones; Dockerfile y huella idénticos). Detalle, decisiones y límites en `docs/TRASPASO-hardening.md`.
- **`publicar.sh` para llevar la imagen a ECR.** Junto al Dockerfile se guarda un script que sigue el procedimiento manual del equipo (credenciales del access portal, Docker Scout como filtro de vulnerabilidades críticas y altas, tag con sufijo `-sec`) y agrega pruebas funcionales y la comprobación de que la imagen no exista ya en ECR. Se ejecuta en un equipo con Docker, por ejemplo Cloud Shell de GCP; la cuenta, región, repositorio y tag se editan en la pantalla. No se pudo ejecutar de punta a punta (sin Docker ni AWS aquí).
- **Archivos compartidos tocados, con cambios mínimos de líneas:** `src-tauri/src/lib.rs` (6 comandos, el `use` y el `.manage` del estado), `src-tauri/src/modules/mod.rs` (`pub mod hardening;`), `src-tauri/Cargo.toml` (`sha2`), `src/App.tsx` (ruta), `src/components/Layout.tsx` (entrada del menú), `src/modules/clusters/GlobalConsole.tsx` (etiqueta `[Hardening]`) y `src/modules/home/index.tsx` (tarjeta).
- **Modo "Desde ticket de Jira" en Bitbucket Repo Creator.** Lee los tickets de altas de repositorios asignados a la cuenta del token (estado Asignado o Trabajo en curso), valida el formulario ABM, crea los repositorios con el motor existente (`bitbucket_crear_repos`) y deja el ticket listo: etiquetas, cierre y respuesta al solicitante con las URLs y una imagen de repositorios y ramas. El modo manual queda intacto. Detalle, reglas y verificación en `docs/TRASPASO-jira.md`.
- **El modo Jira es el modo por defecto** de la pantalla y lista solo los tickets asignados a la cuenta al abrirlo (con las credenciales ya validadas). Cada ticket tiene un botón "Gestionar ticket". Tras gestionarlo, una tarjeta muestra por repositorio la URL para activar a mano el widget de SonarQube (esa opción no tiene API), con un botón para abrirla.
- **Repositorios de Base de Datos** (campo "Lenguaje / Tecnología" = "Base de Datos") se crean sin Pipelines ni proyecto en SonarQube.
- **Archivos compartidos tocados, con cambios mínimos de líneas:** `src-tauri/src/lib.rs` (6 comandos nuevos y el `use`) y `src-tauri/src/modules/mod.rs` (`pub mod jira;`). En el módulo propio: `bitbucket/index.tsx`, `bitbucket/shared.ts` y `bitbucket/ResultsPanel.tsx` (prop opcional `sinSonar`).

**Seguridad**

- **Dependencias de npm con vulnerabilidades conocidas.** `npm audit` reportaba 11 hallazgos (7 altos y 4 moderados) en `vite`, `source-map-js`, `react-router-dom` 6 y la cadena de `tailwindcss` 3. Paso 1: `npm audit fix` (sin saltos mayores) actualiza `vite` y `source-map-js`; `react-router-dom` pasa de 6.30 a 7.18 (solo se usan `HashRouter`, `useLocation`, `NavLink` y `Link`, cuya API no cambia). Verificado con `tsc`, `vite build` y una comparación de estilos computados de las 4 pantallas (431 elementos) antes y después: 0 diferencias.
  Paso 2: migración de `tailwindcss` 3.4 a 4.3 con la herramienta oficial `@tailwindcss/upgrade`: `@tailwindcss/postcss` reemplaza a `tailwindcss` y `autoprefixer` en `postcss.config.js`, `@tailwind base/components/utilities` pasa a `@import 'tailwindcss'`, se elimina `tailwind.config.js` (estaba vacío) y se renombran utilidades (`outline-none` a `outline-hidden`, `rounded` a `rounded-sm`, `bg-[var(--x)]` a `bg-(--x)`) en 11 archivos. Dos ajustes en `index.css` para conservar el aspecto: `--font-sans` fijada a la fuente de Tailwind 3 (Tailwind 4.3 cambió la suya) y cursor de mano en los botones (Tailwind 4 usa flecha). Resultado: `npm audit` pasa de 11 hallazgos a 0. Verificado con la misma comparación de estilos computados: ninguna propiedad de estilo cambia salvo dos diferencias de implementación sin efecto visual, el margen de `space-y` (ahora en `margin-bottom` de cada elemento en vez de `margin-top` del siguiente, con las mismas posiciones) y el contorno de los inputs (`outline-style: none` en vez de un contorno transparente). **Pendiente de validar en la app instalada** (Tauri en macOS, Windows y Linux), porque la comparación se hizo en un navegador.

---

## 0.6.8

**Corregido**

- **Sesión GCP se destruía al abrir la app en Windows.** `ensure_gcp_session` llamaba `set_active_account(account)` y `run_gcloud_login(account)` con la cuenta almacenada en `accounts.json`. Si ese valor era incorrecto (cuenta del desarrollador heredada de v0.6.6), el comando `gcloud config set account <cuenta_incorrecta>` pisaba la sesión activa del usuario y `gcloud auth print-access-token` fallaba con "no active account selected". Además, `check_gcloud_token` usaba `--format value(token)` que en Windows produce stdout vacío porque `print-access-token` emite el token como string plano, no como JSON estructurado. Cambios: (1) `check_gcloud_token` elimina el flag `--format value(token)`; (2) `ensure_gcp_session` pasa a ser solo verificación de token — sin `set_active_account` ni `run_gcloud_login` — y devuelve error claro si la sesión no es válida; (3) `setup_gcloud_login` unifica ambos flujos: si el token es válido para la cuenta guardada reutiliza sesión, si no hace login libre en el navegador (sin `--account`) y actualiza `accounts.json` con la cuenta real del usuario; (4) `get_gcp_token` en `test_cluster.rs` también corregido con el mismo fix del flag de formato.

---

## 0.6.7

**Corregido**

- **Cuenta GCP hardcodeada en instalaciones nuevas.** `default_gcp_chile()` en `eks/config.rs` tenía la cuenta del desarrollador (`eb_acisternasg@bancoripley.cl`) como valor por defecto. Al instalar la app por primera vez, `load_or_create_accounts` creaba `accounts.json` con esa cuenta y el módulo Clusters intentaba autenticar a otro usuario con ella. Cambios: `account` pasa a `""` en el default; `setup_gcloud_login` detecta cuenta vacía y hace `gcloud auth login` sin `--account` (el usuario elige en el navegador), luego lee la cuenta activa con `gcloud config get-value account` y la guarda en `accounts.json` mediante la nueva función `save_accounts`. Las instalaciones existentes con cuenta ya guardada no se ven afectadas.

---

## 0.6.6

**Corregido**

- **`gcloud` no se podía ejecutar en Windows aunque estuviera en el PATH.** La carpeta `bin` del SDK contiene tres archivos: `gcloud` (script de shell Unix, sin extensión), `gcloud.cmd` y `gcloud.ps1`. `CreateProcess` de Windows encuentra el script Unix primero — porque está en PATH — pero no puede ejecutarlo al no tener extensión reconocida. El wrapper que realmente funciona en Windows es `gcloud.cmd`. Se define la constante `GCLOUD_BIN` en `gke/mod.rs` con `#[cfg(target_os = "windows")]` y se aplica a todos los sitios que invocan gcloud: `gke/login.rs` (3 llamadas), `gke/discovery.rs` (1), `gke/test_cluster.rs` (4) y `cicd/subnets.rs` (1, constante local ya que es un módulo distinto). `system.rs` (`check_dependencies`) también corregido para mostrar correctamente si gcloud está disponible. En macOS y Linux el comportamiento es idéntico al anterior.

---

## 0.6.5

**Corregido**

- **La app en Windows no detectaba `gcloud` instalado vía consola.** `ampliar_path()` era un no-op en Windows. Explorer hereda el PATH del momento en que arrancó, por lo que herramientas instaladas después de iniciar sesión (o en otra sesión de consola) no son visibles para apps lanzadas desde el escritorio. La versión Windows de `ampliar_path()` agrega `%APPDATA%\gcloud`, `%APPDATA%\gcloud\bin` (instalador por consola / winget), `%LOCALAPPDATA%\Google\Cloud SDK\google-cloud-sdk\bin` (instalador interactivo), rutas de sistema de gcloud (`Program Files` x86 y x64) y `C:\Program Files\Amazon\AWSCLIV2`.

---

## 0.6.4

Sin cambios funcionales. Release que publica las instrucciones de instalación por plataforma en el README y en el cuerpo del release de GitHub.

**Documentación**

- README: sección Módulos al día (Clusters, Bitbucket, CI/CD completos); nueva sección Instalación con el workaround `xattr` para macOS, SmartScreen para Windows y `chmod` para Linux; sección Estado refleja v0.6.3 y roadmap 0.7.0–0.8.0.
- release.yml: `releaseBody` con instrucciones de instalación por plataforma; bump a `actions/checkout@v5`, `actions/setup-node@v5` y `node-version: 22`.

---

## 0.6.3

**Corregido**

- **Diálogos duplicados del Keychain al iniciar la app instalada.** `App.tsx` monta todos los módulos al arrancar con CSS, por lo que dos instancias de `CredentialsPanel` (módulo Bitbucket + CI/CD `SubirPanel`) quedaban activas al inicio. Sin caché en `bitbucket/credentials.rs`, cada una leía el Keychain por separado: 2 diálogos al arrancar y 2 más al entrar al módulo.
  - `bitbucket/credentials.rs` migrado a `llavero.rs`: la caché con Mutex garantiza una sola lectura al Keychain por sesión desde cualquier módulo.
  - `CredentialsPanel` respeta el prop `activo` y comparte `estadoCompartido` entre instancias, igual que `SonarCredentialsPanel` desde v0.5.0.
  - Resultado: 0 diálogos al iniciar; máximo 2 al entrar por primera vez a Bitbucket (uno por ítem del Keychain: Bitbucket y Sonar), y 0 en adelante al elegir "Permitir siempre".

---

## 0.6.2

**Corregido**

- **La app instalada desde Finder reportaba "Faltan herramientas del sistema"** aunque `aws`, `gcloud` y `kubectl` estaban instalados. macOS lanza las apps GUI con el PATH mínimo del sistema (`/usr/bin:/bin`), sin las rutas del shell del usuario. En modo dev no se manifestaba porque la app se lanzaba desde terminal. Función `ampliar_path()` en `lib.rs` prepende `/opt/homebrew/bin`, `/usr/local/bin` y `~/.local/bin` al iniciar, antes de que Tauri arranque sus hilos.

---

## 0.6.1

Sin cambios funcionales. Bump de versión de mantenimiento sobre 0.6.0.

---

## 0.6.0

Módulo SonarQube Cloud integrado al flujo de creación de repositorios de Bitbucket.

**Agregado**

- Módulo Sonar: credenciales en el llavero (`devops-hub-sonarqube`), validación contra la API de SonarQube Cloud y creación de proyectos.
- Helper `llavero.rs` compartido para acceso al Keychain del SO, con caché en memoria y protección con Mutex.
- `SonarCredentialsPanel` con caché de módulo y respeto al prop `activo`.
- Binding Bitbucket↔Sonar en el flujo de creación de repositorios: si la casilla está marcada y el token es válido, se crea el proyecto en Sonar junto con el repositorio en Bitbucket.

---

## 0.5.2

**Corregido**

- **`SERVICE_ACCOUNT_NAME` y `SERVICE_ID` de Cloud Run salían inventados.**
  La herramienta rellenaba la cuenta con el patrón
  `devops-user@{proyecto}.iam.gserviceaccount.com` y el service id con el
  proyecto. Ninguno de los dos es derivable: ambos salen de la credencial
  registrada en Jenkins, y en Jenkins conviven credenciales con prefijo
  `gcp-` y sin él para proyectos del mismo tipo. Los dos campos ahora
  quedan vacíos y se completan a mano; un valor que parece correcto y hace
  fallar el pipeline es peor que uno ausente.
  Las tres variables del bloque quedan documentadas en el generador:
  `SERVICE_ACCOUNT_NAME` es el `client_email` del JSON de la credencial —
  una cuenta de DevOps, siempre con "devops" en el nombre—, `SERVICE_ID`
  es el ID de esa credencial en Jenkins, y `PROJECT_ID` es el proyecto de
  GCP, el único que sí sale del formulario.
- **Los campos de Cloud Run estaban en el bloque equivocado.** `PROJECT_ID`,
  `SERVICE_ID` y `SERVICE_ACCOUNT_NAME` se pedían bajo "service.yaml"
  cuando pertenecen al `JenkinsParams`. El `service.yaml` de Cloud Run no
  lleva cuenta de servicio.
- **El bloque de red y subred no se veía.** La subred aparecía como una
  línea gris al pie de campos grandes. Ahora es una tabla con los tres
  ambientes, su subred, región y red, más el proyecto con el que se buscó,
  y los ambientes sin coincidencia se marcan en rojo.

**Cambiado**

- Los conflictos al subir se resuelven con una sola pregunta —actualizar
  todos, subir copia de todos o no subir ninguno— con **Actualizar**
  marcado por omisión. El detalle archivo por archivo queda plegado para
  las excepciones. Antes el valor por omisión era la copia, lo que obligaba
  después a borrar el original y renombrar a mano.
- Se quitó el límite de 5 repositorios. Pasados 10 aparece un aviso
  informativo: la verificación hace una consulta por archivo y va a tardar.
- Cada sección tiene un control "Contraer sección" en el pie, que además
  deja el encabezado a la vista.

**Nota sobre los archivos generados**

La comparación contra la herramienta HTML pasa de 12 a 16 diferencias, y
las 4 nuevas son los `JenkinsParams` de Cloud Run: `SERVICE_ACCOUNT_NAME` y
`SERVICE_ID` salen vacíos donde el HTML escribía valores derivados. Es el
cambio intencional descrito arriba.

**Pendiente**

- `serviceAccountName` del `service.yaml` de Kubernetes: un ejemplo real
  trae un correo de IAM (`br-gcp-ard-co-dev@appspot.gserviceaccount.com`),
  distinto de la cuenta de DevOps de Cloud Run y distinto del literal fijo
  `SA-DEL-PROYECTO` que la herramienta escribe hoy. Falta confirmar con
  `kubectl` qué queda realmente aplicado en el cluster antes de definir qué
  validación corresponde.
- Las credenciales de GCP en Jenkins viven en el dominio `GCP`, no en el
  dominio por defecto. La consulta correcta apunta a
  `/credentials/store/system/domain/GCP/api/json`.

---

## 0.5.1

**Corregido**

- **El llavero pedía la clave en cada acción.** La causa quedó identificada
  con las pruebas del usuario: el caché agregado en 0.5.0 era del lado de
  React y solo evitaba repetir la consulta de estado, pero en Rust cada
  comando llamaba a `obtener_credenciales()` y eso leía el llavero de
  nuevo. Una sesión normal pedía la clave cuatro veces: al abrir el módulo,
  al verificar repositorios, al subir archivos y al crear repositorios.
  Ahora `credentials.rs` guarda la primera lectura exitosa en memoria del
  proceso y el resto de las operaciones la reutilizan. El caché se limpia
  al guardar credenciales nuevas y al borrarlas.
  Consecuencia a tener presente: el token queda en memoria mientras la
  aplicación está abierta. Antes ya estaba en memoria durante cada
  operación; lo que cambia es que persiste entre operaciones.
- **Mayúscula automática en los campos.** El webview de macOS aplicaba
  autocapitalización y autocorrección: escribir `test1-cicd-rust` quedaba
  `Test1-cicd-rust`. Desactivadas en los campos técnicos de los tres
  módulos: CI/CD (incluido el prefijo URI), Bitbucket (repositorios,
  workspace, proyecto, correo y token) y Clusters (filtro de rutas).

**Descartado**

- La hipótesis de que la firma del binario cambiara en cada compilación.
  El patrón observado —dos solicitudes por cada acción distinta, no por
  arranque— no la sostiene.

---

## 0.5.0

Fase B de CI/CD: los archivos generados se suben a Bitbucket desde el hub.

**Agregado**

- Subida de archivos a Bitbucket, en la pestaña Archivos. Dos comandos:
  `cicd_bb_verificar` (solo lee: repositorio, rama y qué archivos ya
  existen) y `cicd_bb_subir` (un commit por repositorio, multipart).
- Panel de conflictos con las tres opciones por archivo — sobrescribir,
  guardar como `.new` o no subir — y la posibilidad de aplicar una misma
  decisión a todos o cancelar un repositorio completo.
- Panel de resultados con el detalle por repositorio, los renombrados y el
  enlace a la rama en Bitbucket.
- Mensaje de commit `feat: agregar configuracion CI/CD [TICKET]`, sin
  tildes, igual al del flujo anterior.

**Decisiones**

- Se reutiliza el cliente HTTP y las credenciales del módulo Bitbucket. No
  hay servicio nuevo en el llavero: la convención es uno por sistema
  externo, no uno por módulo. Se evaluó crear un módulo aparte que
  centralizara las credenciales y se descartó: el panel existente ya es
  reutilizable y el problema que iba a resolver era otro (ver abajo).
- Los marcadores `PENDIENTE` no bloquean la subida. Se avisan en la
  confirmación y quedan para que el desarrollador los complete en el
  repositorio.
- Todos los archivos van a la rama de desarrollo: `development` en Chile,
  `develop` en Perú. Los ambientes se separan por carpeta.

**Cambiado**

- La consulta de credenciales al llavero ya no ocurre al arrancar la
  aplicación. Como `App.tsx` mantiene todos los módulos montados, el panel
  de credenciales se montaba al inicio y hacía que macOS pidiera la clave
  del llavero por algo que quizá no se iba a usar. Ahora espera a que el
  panel esté a la vista, y el resultado se comparte entre módulos: una
  sola lectura por sesión.
- El rótulo de la sección 2 del formulario informa cuántos valores vienen
  sugeridos y solo dice "revisada" cuando el usuario abrió la sección.
  Antes decía "completa" sin que nadie hubiera mirado esos valores.

**Pendiente al momento de esta versión** (resuelto en 0.5.1)

- Las 4 solicitudes de clave del llavero en macOS. Lo corregido acá —
  diferir la consulta hasta que el panel esté a la vista — bajó una de
  ellas, pero no era la causa principal.

---

## 0.4.2

Correcciones sobre la Fase A de CI/CD, a partir de pruebas reales en
Kubernetes y Cloud Run.

**Corregido**

- **Sugerencias congeladas.** El valor sugerido se escribía apenas el campo
  estaba vacío, o sea con la primera letra tecleada en el proyecto GCP:
  escribir "T" dejaba `gcr-t` fijo para siempre. Ahora cada campo calculado
  sigue a su origen y se recalcula cuando el origen cambia, hasta que el
  usuario lo edita a mano. Alcanza a `REGISTRY_NAME` de Kubernetes y Cloud
  Run, `PROJECT_ID` y cuentas de servicio de Cloud Run, Jenkinsfile, archivo
  de dependencias, imagen base y las variables de Apigee.
- **Lista del Dropdown recortada.** `CollapsibleSection` tenía
  `overflow-hidden` y la lista, que se dibuja con posicionamiento absoluto,
  se cortaba en el borde de la sección: con Node.js se veía una sola de las
  seis plantillas. Se quitó el recorte y el redondeado se compensa en el
  encabezado y el contenido.
- **Estado falso en la sección 2.** Decía "completa" sin haber llenado nada,
  porque esa sección no tiene campos obligatorios propios. Ahora el estado
  sale de los archivos generados: marcadores `PENDIENTE` y variables que
  quedarían escritas sin valor.

**Cambiado**

- Los botones "Recalcular" del prefijo URI se eliminaron. Dejar el campo
  vacío devuelve el valor calculado.
- Al actualizar las subredes, la consola muestra el detalle por proyecto y
  después un resumen comparativo: subredes nuevas, eliminadas y las que
  cambiaron de región, o "sin cambios" cuando no hubo ninguno.

**Agregado**

- Este archivo.

---

## 0.4.1

Correcciones sobre la Fase A, a partir de la primera prueba en Kubernetes.

**Corregido**

- **Filas de prefijo URI duplicadas.** Aparecían seis filas con tres
  repositorios. La lista usaba el nombre del repositorio como clave, y
  mientras se escribe hay momentos en que varias líneas coinciden: con
  claves repetidas las filas viejas quedaban en pantalla. Se eliminan los
  nombres repetidos y la clave incluye la posición.
- **Sesión de gcloud.** La consulta de subredes ejecutaba gcloud directo y
  un token vencido llegaba como error crudo. Ahora usa `ensure_gcp_session()`
  del módulo GKE, con reintento único si el token vence a mitad de la
  operación. El panel muestra con qué cuenta consulta.
- **Errores en el panel.** El texto completo de gcloud ocupaba la caja de la
  pantalla; pasó a la consola global.
- Dos textos con voseo.

**Agregado**

- Proyecto GCP y cluster GKE de release y prod se completan desde dev. Si
  dev cambia, los otros dos se corrigen; si se editan a mano, dejan de
  seguirlo.

**Cambiado**

- Formulario reorganizado en cuatro secciones, en el orden en que se llena:
  Solicitud, Configuración del repositorio, Pipeline de Jenkins y Spinnaker.
  El `JenkinsParams` pasó a la sección del repositorio, que es donde
  corresponde: es un archivo del repositorio, no configuración del pipeline.
- Jerarquía visual: barra de acento en la sección abierta (transparente al
  cerrar, para que no se corra el contenido), secciones numeradas con su
  estado, y bloques internos sin caja, con guía vertical.
- Valores avanzados del `JenkinsParams` plegados. Quedan visibles
  `REGISTRY_NAME` y el canal de Slack, más `REGISTRY_URL` y
  `REGISTRY_CREDENTIALS` en AWS, que no tienen valor sugerido.

---

## 0.4.0

Fase A de la migración de CI/CD Automation, desde la herramienta HTML
standalone `cicd-tool-v4.html` v4.28.0.

**Agregado**

- Módulo CI/CD con los tres flujos: Kubernetes, Cloud Run y Apigee.
- Los seis generadores de archivos: `service.yaml` de Kubernetes y de Cloud
  Run, `istio.yaml`, `JenkinsParams` de Kubernetes/Cloud Run y de Apigee, y
  `bitbucket-pipelines.yml`.
- Catálogo de subredes GCP con caché en el directorio de datos de la
  aplicación y consulta en vivo con gcloud contra los tres proyectos de
  networking. Dos comandos: `cicd_subnets_leer` y `cicd_subnets_actualizar`.
- Consola global contraíble, disponible para los tres módulos
  (`useConsolePanel`).

**Cambiado respecto de la herramienta HTML**

- El `service.yaml` de Kubernetes se genera sin `:TAG`, igual que los
  repositorios en producción: Spinnaker inyecta al desplegar la versión que
  disparó el trigger.
- La imagen sale de un único valor, `REGISTRY_NAME`, del que se derivan las
  tres formas en que se escribe (JenkinsParams, `service.yaml` y el trigger
  de Spinnaker). Antes el campo pedía el nombre con la organización incluida
  y el `service.yaml` se la agregaba de nuevo.

**Verificado**

- Comparación byte a byte de los seis flujos contra la salida del HTML: 56
  archivos, 44 idénticos y 12 con la única diferencia de la línea `image:`,
  que es el cambio intencional del `:TAG`.

**Fuera de alcance**

- Subida a Bitbucket, lanzamiento de Jenkins, creación en Spinnaker y los
  valores fijos de Apigee para Perú.

---

## 0.3.0

Migración del Bitbucket Repo Creator (HTML `bitbucket-repo-tool.html` v1.2.0)
a módulo nativo.

**Agregado**

- Módulo Bitbucket completo: Rust habla directo con la API, sin Python ni
  scripts descargados. Cuatro comandos: estado, guardado y borrado de
  credenciales, y creación de repositorios.
- Credenciales en el almacén del sistema con `keyring`, en una sola entrada
  (`devops-hub-bitbucket`). Convención a futuro: `devops-hub-<sistema>`, uno
  por sistema externo y no por módulo.
- Dependencias nuevas: `reqwest` con `rustls-tls` y `default-features = false`
  — con las features por defecto arrastra `native-tls`, que falla al compilar
  en Linux sin OpenSSL — y `keyring` declarado por plataforma.
- Icono de la aplicación: el camaleón del equipo.

**Corregido**

- `withGlobalTauri` activado: sin eso `window.__TAURI__` no existe.
- Las credenciales del llavero pedían la clave cuatro veces por operación,
  por estar guardadas en dos entradas separadas.

**Pendiente**

- AppImage y D-Bus en Linux: no está verificado que el AppImage encuentre
  `libdbus` en la máquina del usuario.
- Actualizador automático sin operar (ver abajo).

---

## 0.2.20 y anteriores

Módulo Clusters: EKS Chile y Perú, GKE Chile y carga unificada a kubeconfig.
Consola global, tokens de diseño en `index.css`, componentes de UI
compartidos y `check_dependencies`.

El detalle de las versiones anteriores a 0.2.20 no quedó registrado en los
documentos de traspaso; si hace falta, está en el historial del repositorio.

---

## Planificado

| Versión | Contenido |
|---|---|
| 0.7.0 | CI/CD Fase C — Jenkins |
| 0.8.0 | CI/CD Fase D — Spinnaker |
| 0.9.0 | CI/CD Fase E — encadenado |

**Fuera de esta secuencia**

- Consola de administración de Spinnaker (`spinnaker_consola.py`): se migra
  aparte y sigue viviendo en la herramienta HTML.
- Actualizador automático: el bloque existe en `tauri.conf.json` pero está en
  `active: false`, el plugin `tauri-plugin-updater` no está instalado y
  faltan definir el repositorio de publicación y la clave pública. Las claves
  de firma sí existen y el workflow ya pasa el secreto.
- `withGlobalTauri` está activo a propósito para depurar durante las
  migraciones. Conviene quitarlo cuando el hub esté completo.
