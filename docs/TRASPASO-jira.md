# Traspaso — Modo "Desde ticket de Jira"

**Estado:** rama local `feat/flujo-tickets-jira`, sin versión asignada ni PR. Las
convenciones transversales están en `CONVENCIONES-HUB.md`; acá va solo lo propio
de este modo.

---

## 1. Qué hace

Procesa los tickets de **alta de repositorios** que llegan a Jira (proyecto
SCDO, tipo de solicitud "Alta, Baja y Modificación de Repositorios Bitbucket"),
asignados a la cuenta del token, en estado *Asignado* o *Trabajo en curso*.

Reemplaza la automatización que corría por terminal (Python). La parte de crear
repositorios **no se reescribe**: reutiliza `bitbucket_crear_repos`, que ya
hacía mejor la creación de Sonar (proyecto con la key correcta y vinculado).

**Por ticket, en orden:**

1. Releer y validar el ticket (la lista puede tener minutos)
2. Verificar en Bitbucket que el proyecto existe y que **ningún** repo del ticket
   existe todavía; si alguno existe se aborta el ticket completo
3. Crear con el motor de Bitbucket (commit inicial, Pipelines, ramas, SonarQube)
4. Solo si **todos** los repos quedaron bien: agregar las etiquetas que falten y
   cerrar el ticket
5. Responder al solicitante con las URLs y una imagen de repos y ramas

Cada paso posterior a la creación es una casilla en el panel de confirmación.

---

## 2. Reglas de negocio (probadas contra tickets reales)

| Regla | Detalle |
|---|---|
| Solo altas | El campo "Tipo de Solicitud" debe valer Alta |
| País | Chile o Perú, con o sin tilde. Define `development` (CL) o `develop` (PE) |
| Proyecto | Llega como `CLAVE - nombre`; la clave se conserva tal cual (hay una en minúsculas) |
| Nombres de repo | Siempre en minúsculas, sin duplicados; rechaza URLs y espacios |
| Base de Datos | "Lenguaje / Tecnología" = "Base de Datos" → **sin Pipelines ni SonarQube** |
| Lenguaje vacío | El ticket se rechaza: sin él no se sabe si es de BD |
| Etiquetas | `ACN`, `DEVOPS-CHAPTER` y `CHILE`/`PERU`; solo las que falten, nunca una nueva |
| Cierre | Asignado → Comenzar progreso → Marcar como hecho; Trabajo en curso → Marcar como hecho. Resolución `Listo` |
| Repos que ya existen | Aborta todo el ticket; no crea nada a medias |

Los avisos de SonarQube **no** bloquean el cierre del ticket: el repositorio
sirve y el proyecto de análisis se completa a mano.

---

## 3. Arquitectura

### Rust — `src-tauri/src/modules/jira/`

```
client.rs        HTTP: Bearer contra api.atlassian.com/ex/jira/<cloudId>; API de formularios; adjuntos
credentials.rs   token en el llavero (devops-hub-jira) + validación
tickets.rs       búsqueda de pendientes, lectura del formulario, reglas puras y validación
procesar.rs      orquestación de un ticket, etiquetas, cierre y publicación de la evidencia
```

**Comandos:** `jira_cred_status/save/delete`, `jira_tickets_pendientes`,
`jira_procesar_ticket`, `jira_publicar_evidencia`.

**Las trampas de la API de Jira**

- Con un token vencido o revocado Jira **no responde 401**: responde 200 con lista
  vacía (trata la petición como anónima). La validación busca algo que existe
  seguro (los tickets de SCDO) y una búsqueda vacía delata el token roto.
- Los tokens con scopes no funcionan contra `<sitio>.atlassian.net`; hay que usar
  `api.atlassian.com/ex/jira/<cloudId>`. El `cloudId` sale de
  `<sitio>/_edge/tenant_info`, sin autenticación.
- Los nombres de repo no están en los campos "Repositorio 1-5" (vienen vacíos):
  están en las respuestas del formulario, por la API de formularios.
- `/rest/api/3/search` ya no existe; se usa `search/jql` con paginación por
  `nextPageToken`.
- El comentario público usa la API v2 (acepta wiki markup) con la propiedad
  `sd.public.comment = {internal: false}`. Adjuntar exige `X-Atlassian-Token: no-check`.

### React — `src/modules/bitbucket/`

`JiraCredentialsPanel.tsx` · `TicketsPanel.tsx` · `evidencia.ts`, más cambios
pequeños en `index.tsx` (selector de modo), `shared.ts` (tipos) y
`ResultsPanel.tsx` (prop opcional `sinSonar`).

Pantalla: selector **Desde ticket de Jira / Manual**, con Jira por defecto. Al abrir
el modo, con las credenciales validadas, se listan solos los tickets asignados.
Tras gestionar un ticket, una tarjeta lista cada repo con la URL para activar el
widget de SonarQube (`urlWidgetSonar` en `shared.ts`) y un botón para abrirla. El
modo manual queda montado y oculto por CSS (nunca se desmonta). El token de Jira solo se consulta
al abrir el modo Jira.

**La imagen de evidencia se dibuja con canvas en la interfaz**, no en Rust: el
webview ya trae sus fuentes y no se suma ninguna librería de imágenes al backend.
Es una tarjeta por repo con sus ramas, y nada más. Rust recibe el PNG en base64.

---

## 4. Decisiones que no se leen en el código

- **Un ticket se procesa completo o no se toca.** Si algún repo ya existe, aborta
  antes de crear. Si algún repo falla, no se modifica el ticket y se informa.
- **El cierre y las etiquetas no se deshacen**: un fallo en uno no revierte lo ya
  hecho; se informa en pantalla.
- **La evidencia es un comando aparte** porque la imagen la dibuja la interfaz.
  Si la imagen no se puede dibujar se publica igual el texto con las URLs.
- **Si la imagen no se puede adjuntar, no se comenta**: el texto apuntaría a una
  imagen que no existe.
- **Las transiciones se buscan por nombre**, no por id: el id es interno del flujo
  de trabajo y el nombre es lo que ve el equipo.
- **El token de Jira no necesita correo** (Bearer), igual que el de SonarQube.

---

## 5. Verificación realizada

- `cargo check` limpio y 18 pruebas unitarias (`cargo test --lib jira`) con
  valores reales de tickets ya procesados: claves de proyecto, país con y sin
  tilde, nombres de repo válidos e inválidos, el texto exacto del comentario.
- Lecturas reales contra Jira y Bitbucket: token válido e inválido, tickets de
  código, de Base de Datos, de Perú y uno con una URL en vez del nombre, búsqueda
  de proyecto por clave y por nombre, repos libres y existentes, y la evidencia
  con las ramas reales.
- `tsc` y `vite build`. La interfaz se recorrió en el navegador con las respuestas
  de los comandos simuladas: lista, confirmación, procesamiento, resultado,
  publicación de la evidencia y el camino de Base de Datos.

## 6. Lo que NO se ha probado

- **Ninguna escritura real desde este código en Rust**: crear repos, etiquetar,
  cerrar y publicar el comentario. La misma lógica corrió en producción en la
  versión Python, pero este código no.
- **La app Tauri instalada.** Todo se probó en el navegador y con pruebas de Rust.
  Falta correrla con el binario en macOS, Windows y Linux.
- **La imagen en Windows y Linux**: usa la fuente del sistema (`system-ui`).
- El widget de SonarQube en Bitbucket sigue siendo manual: no tiene API (la
  propiedad solo la puede escribir la propia app de SonarCloud).

## 7. Pendientes

- Probar el flujo completo con un ticket real y la app instalada.
- Decidir el número de versión (`0.7.0` está reservado para Jenkins).
- Aviso previo del vencimiento del token de Jira (hoy no hay).
