# Historial de versiones — DevOps Hub

Registro de lo que entró en cada entrega. Se actualiza en el mismo trabajo
que produce la versión, no después.

**Convención:** cuatro archivos deben coincidir siempre en el número de
versión — `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`
y `src/version.ts`. Patch para correcciones y ajustes visuales, minor para
funcionalidad nueva.

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
