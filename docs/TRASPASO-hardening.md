# Traspaso — Módulo "Hardening de imágenes"

**Estado:** rama local `feat/hardening-imagenes`, sin versión asignada ni PR. Las
convenciones transversales están en `CONVENCIONES-HUB.md`; acá va solo lo propio
de este módulo.

**Alcance de esta fase (1):** analizar una imagen, elegir qué conservar y generar
el Dockerfile endurecido. **No construye ni reescanea la imagen** (no había Docker
ni BuildKit al desarrollarlo): hasta construirla y reescanearla no hay garantía de
que cumpla la política. La pantalla lo dice y entrega los comandos para hacerlo.

---

## 1. Qué hace

1. El usuario escribe la referencia de una imagen pública (`node:26.12.0-alpine3.24`).
2. **Resolver:** se consulta el registro (API OCI, sin descargar la imagen), se
   elige la plataforma y la referencia queda **fijada por digest**.
3. **Escanear:** Trivy contra esa referencia (`--image-src remote`). Docker no hace falta.
4. **Opciones por tipo de imagen:** el tipo se detecta por las variables de la
   imagen (`NODE_VERSION`...). Para Node/Alpine se muestra el catálogo de
   componentes (npm, npx, Corepack, Yarn, pnpm, compiladores, Python, Git, curl,
   shell, apk, certificados CA) con versión, hallazgos HIGH/CRITICAL y qué se pierde
   al quitarlos. **Una imagen de desarrollo normalmente necesita npm**, así que se
   parte del perfil «Node.js manteniendo npm».
5. **Impacto y plan** (se recalculan con cada casilla): veredicto, qué hallazgos
   desaparecen, qué se corrige y cómo, bloqueos y alternativas.
6. **Dockerfile:** rootfs limpio multi-stage terminado en `FROM scratch`, con
   pruebas funcionales dentro de la imagen. Se guarda en
   `~/devops-hub-hardening/<imagen>-<tag>-<fecha>/` con sus archivos de apoyo y
   `COMANDOS.txt` (pruebas, construcción, reescaneo y subida a ECR).

Para otros ecosistemas o bases distintas de Alpine se muestra el análisis de
vulnerabilidades, pero no hay generador (el motivo se explica en pantalla).

---

## 2. Origen: port de Container Hardening Studio

El motor es un **port a Rust** de `container-hardening-studio` (Python), que es la
**referencia**. Un port se verifica, no se supone:

- `testdata/oro.json` lo genera el Python de Studio sobre un escaneo real de
  `node:26.12.0-alpine3.24` (`testdata/trivy_node26_alpine324.json`): para 9
  configuraciones guarda análisis, dos planes, recomendaciones, configuración
  efectiva y el Dockerfile completo con su huella sha256.
- `pruebas_oro.rs` ejecuta el código de Rust sobre el mismo escaneo y exige la
  misma salida campo por campo, **incluido el texto del Dockerfile y la huella**.
- El escaneo real con Trivy 0.70.0 de esa imagen coincide con el guardado
  (`cargo test --lib hardening::pruebas_oro::escaneo_real -- --ignored`).
- El digest que resuelve el cliente de registro contra Docker Hub coincide con el
  del oro.

**Desviaciones deliberadas respecto a Studio**

| Qué | Por qué |
|---|---|
| `workdir` se valida (ruta absoluta, sin `..`) | Studio no lo valida y lo escribe sin comillas en el Dockerfile |
| La cabecera del Dockerfile dice «DevOps Hub (motor de Container Hardening Studio)» | La firma es un parámetro; las pruebas usan la de Studio para comparar la huella |
| `installable` en cada componente detectado | Distingue «no existe en la imagen» de «se instalaría»; la prueba lo quita antes de comparar |
| Sin cola de trabajos, base de datos ni usuarios | Es una app de escritorio de un solo usuario |

Si cambia el motor de Studio, hay que regenerar el oro con su generador y repasar
las diferencias; no se corrigen a mano en Rust sin entender por qué cambió.

---

## 3. Arquitectura

### Rust — `src-tauri/src/modules/hardening/`

```
validar.rs       reglas de formato (apk, npm, semver, digest, rutas, usuario) y shell_quote
version.rs       comparación de versiones (1.2 == 1.2.0; sin adivinar lo irreconocible)
referencia.rs    análisis de referencias de imagen OCI
registro.rs      cliente de registro: digest, plataforma, configuración; anti-SSRF
escaner.rs       Trivy y modelo normalizado de hallazgos (Docker Scout: pendiente)
componentes.rs   catálogo por ecosistema, detección y atribución de hallazgos
config.rs        configuración, perfiles, normalizar y adaptar a lo que existe
impacto.rs       validación de coherencia y análisis de impacto
planificador.rs  plan de remediación, bloqueos, alternativas y recomendaciones
dockerfile.rs    generador del Dockerfile (rootfs limpio) y su huella
comandos.rs      comandos Tauri, sesiones en memoria y guardado en disco
assets/          apply_patches.js y node_suite.js (copiados de Studio)
testdata/        escaneo real + oro de Python; pruebas_oro.rs los usa
```

**Comandos:** `hardening_herramientas`, `hardening_perfiles`, `hardening_analizar`,
`hardening_evaluar`, `hardening_generar`, `hardening_guardar`.

Los structs de este módulo se serializan en **snake_case** (mismos nombres que
Studio); los argumentos de los comandos van en camelCase (`idSesion`, `npmRegistry`).

**Sesión en memoria:** `hardening_analizar` guarda imagen, escaneo y componentes
detectados (máximo 8) y devuelve un `id_sesion`. `evaluar` se llama con cada casilla
y solo recibe la configuración: no repite red ni escaneo.

**Dependencia nueva:** `sha2` (huella del Dockerfile; ya estaba en el árbol de
dependencias).

### React — `src/modules/hardening/`

`index.tsx` (estado y flujo) · `ImagenForm` · `ResumenPanel` · `OpcionesPanel` ·
`ImpactoPanel` · `DockerfilePanel` · `HallazgosTabla` · `shared.ts` (tipos y
`alternarComponente`).

- Activar un componente activa lo que necesita; desactivarlo desactiva lo que depende
  de él (`requires`). «Solo runtime» se apaga solo si se activa algo incompatible.
- Quitar algo que la imagen no trae no se explica como «pérdida»; se marca «Se instalaría».
- Cambiar cualquier casilla **invalida el Dockerfile ya generado**.
- Las respuestas de `evaluar` se descartan si ya hay una más reciente (`secuencia`).

---

## 4. Decisiones que no se leen en el código

- **Lo que no tiene parche cuenta** (política BANK-STRICT): Trivy corre sin
  `--ignore-unfixed`, `--vex` ni `--skip-*`, con entorno mínimo y directorio vacío;
  los flags que ocultan hallazgos se rechazan en código.
- **Nada se elimina sin autorización explícita.** Si un componente no tiene
  corrección, el plan lo informa como bloqueo y la interfaz ofrece «Eliminar X»; solo
  entonces entra en `authorize_remove_components`. Cambiar de base o de versión de
  Node.js todavía no se puede ejecutar desde la pantalla.
- **Digest primero:** el Dockerfile parte de la imagen fijada por digest y del
  constructor (`alpine:<menor>`) también fijado por digest, resuelto al generar.
- **Solo imágenes públicas** por ahora: el cliente de registro no maneja credenciales.
- **El veredicto es preliminar.** Con npm conservado suele salir «Probable bloqueo»
  (p. ej. `http-cache-semantics` sin parche); el plan intenta actualizar npm y solo
  el reescaneo de la imagen construida confirma si el hallazgo persiste.
- **Docker Scout** aparece deshabilitado. Su salida de vulnerabilidades no es JSON
  como la de Trivy (SARIF, texto, SPDX...); no se escribe un lector sin una salida
  real contra la que probarlo.

---

## 5. Verificación realizada

- `cargo check` limpio y 33 pruebas (`cargo test --lib hardening`), incluidas las
  de equivalencia con Python (9 configuraciones × 2 planes) y rechazo de usuario y
  directorio de trabajo con caracteres peligrosos.
- Trivy real y Docker Hub real, solo lecturas (ver §2).
- `tsc` limpio. La interfaz se recorrió en el navegador con las respuestas **reales**
  de los comandos (volcadas con una prueba de Rust): análisis, perfil inicial, quitar
  npm (npx se desmarca, el veredicto pasa a «Candidata»), generar, guardar y la
  invalidación al cambiar una casilla. Esa recorrida también confirmó que la
  configuración que arma TypeScript coincide con la que espera Rust.
- La app Tauri en desarrollo recompila y arranca con el módulo.

## 6. Lo que NO se ha probado

- **Construir la imagen, correr las pruebas funcionales dentro de ella y reescanearla.**
  Es la fase 2 y necesita Docker/BuildKit. El Dockerfile es idéntico al de Studio,
  pero aquí nunca se ha construido.
- **Un clic real en la app de escritorio** (guardar en disco, mostrar carpeta): se
  probó con comandos simulados en el navegador; el guardado lo cubre solo lectura
  de código.
- Imágenes con registros privados, otros ecosistemas (Python, Java...) y bases
  distintas de Alpine: solo análisis.
- Windows y Linux.

## 7. Pendientes

- Fase 2: construcción con BuildKit, pruebas, reescaneo con la política y reporte.
- Docker Scout: pedir una salida real (`docker scout cves registry://node:26.12.0-alpine3.24
  --format sarif --output muestra.sarif.json`) para escribir y probar el lector.
- Credenciales de registro (ECR privado) y ayuda para subir a ECR desde la app.
- Decidir el número de versión (0.7.0 está reservado para Jenkins).
