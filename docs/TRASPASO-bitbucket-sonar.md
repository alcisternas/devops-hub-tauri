# Traspaso — Módulos Bitbucket Repo Creator y SonarQube

**Estado:** v0.6.0. Las convenciones transversales del hub están en
`CONVENCIONES-HUB.md`; acá va solo lo propio de estos dos módulos.

---

## 1. Qué hace

Crea repositorios en Bitbucket con el estándar del equipo y, opcionalmente, su
proyecto en SonarQube Cloud ya vinculado.

Reemplaza la herramienta HTML `bitbucket-repo-tool.html` v1.2.0, que generaba un
script Python para ejecutar en terminal. Ya no se descarga ni se ejecuta nada:
Rust habla directo con las dos APIs.

**Por repositorio, en orden:**

1. Verificar que no exista — si existe, se omite y se sigue con el siguiente
2. Crear (privado, sin forks, sin issues ni wiki, rama principal `master`)
3. Commit inicial con `.gitignore` y `README.md` estándar
4. Activar Pipelines
5. Crear las ramas de desarrollo y `release` desde `master`
6. SonarQube: crear el proyecto, vincularlo al repositorio, verificar el vínculo

El paso 3 es obligatorio: Bitbucket no permite crear ramas en un repositorio
vacío porque no hay desde dónde ramificar.

---

## 2. SonarQube — corrección de un supuesto que estaba mal

El traspaso anterior afirmaba que SonarQube era **100% manual, sin workaround**.
**Eso era falso**, y la corrección salió de sondear la instancia real en vez de
confiar en la documentación.

| Paso del procedimiento manual | Estado |
|---|---|
| Activar el widget en Bitbucket | **Sigue manual.** No tiene API |
| "Analyze new project" e importar el repo | **Automatizado** |
| Corregir la project key | **Desaparece**: el proyecto nace con la key correcta |

### Lo que costó descubrirlo

- La documentación de `dop-translation/bound-projects` es de **SonarQube
  Server**, no de Cloud. Son productos distintos con APIs distintas. En Cloud el
  endpoint es `POST https://api.sonarcloud.io/dop-translation/project-bindings`,
  y no figura en la documentación oficial de Cloud.
- `api/webservices/list` **no es exhaustiva**: `organizations/search` y
  `navigation/component` funcionan y no aparecen ahí. No descartar un endpoint
  por no estar en esa lista; probarlo.
- `include_internals=true` no cambió nada: 156 endpoints en ambos casos.
- Un **403 "Client error"** es el gateway rechazando. Un **400 con mensaje de
  parámetros** es la aplicación contestando: el endpoint existe y el token llega.

### El flujo verificado

```
POST api/projects/create          → organization, project, name
                                     la respuesta trae project.uuid = projectId
POST dop-translation/project-bindings  → { projectId, repositoryId }
                                     responde 201
GET  dop-translation/project-bindings?projectId=X  → verificación
```

**`repositoryId` es el UUID de Bitbucket CON LLAVES**, ej
`{77e7e6b1-ce0f-4a4b-b355-64e06088e01c}`. Pasar el nombre del repositorio
devuelve 200 con lista vacía: no falla, pero no vincula nada. El UUID se captura
de la respuesta de Bitbucket al crear el repositorio, en el mismo cuerpo que la
URL, así que no cuesta una llamada extra.

**No hace falta `devOpsPlatform`:** con el UUID, SonarQube deduce la plataforma
y el slug.

**Prueba que lo confirmó:** el campo `alm` del proyecto pasó de `None` a
`{key: "bitbucketcloud", url: "..."}`, igual que un proyecto importado desde la
interfaz.

### Por qué el orden interno parece invertido

En la interfaz se busca el repositorio y el proyecto nace de esa importación.
Por API no se puede: el endpoint de vinculación necesita un `projectId`, que
solo existe después de crear el proyecto. Secuencia obligada: crear, obtener el
id, vincular.

**Consecuencia:** entre crear y vincular hay una ventana donde existe un
proyecto sin repositorio. Si la vinculación falla, queda huérfano. Se reporta
como error con el nombre, pero **no se borra automáticamente** — un borrado en
Sonar no se deshace y el fallo puede ser transitorio.

### Lo que sigue sin validarse

**La decoración de pull requests.** El binding existe y está verificado, pero que
el comentario de Sonar aparezca en un PR real requiere un análisis y un PR sobre
un repositorio bindeado por API. Depende de dos tickets separados en el tiempo
(crear repos y luego CI/CD), así que quedó pendiente a propósito.

---

## 3. Arquitectura

### Rust

```
modules/bitbucket/
  credentials.rs   token en el llavero + validación contra la API
  client.rs        HTTP: auth, timeout, GET/POST/PUT, multipart, uuid_repo
  repos.rs         las 5 operaciones por repo
  full_run.rs      recorre el lote y emite progreso
modules/sonar/
  credentials.rs   token en el llavero (usa el helper compartido)
  client.rs        DOS bases distintas, ver abajo
  projects.rs      crear proyecto, vincular, verificar
```

**Comandos:** `bitbucket_cred_status/save/delete`, `bitbucket_crear_repos`,
`sonar_cred_status/save/delete`.

**Las dos bases de SonarQube Cloud** — usar la equivocada no funciona:

| Base | Para qué | Formato |
|---|---|---|
| `https://sonarcloud.io` | v1: `projects/create`, `navigation/component` | form-urlencoded |
| `https://api.sonarcloud.io` | v2: `dop-translation/project-bindings` | JSON |

**Autenticación:** Bitbucket usa Basic con `correo:token`, con el header armado
a mano — el mecanismo automático esperaba un desafío-respuesta que Bitbucket no
entrega igual en todos los endpoints, y los POST fallaban. SonarQube usa Bearer.

### React — `src/modules/bitbucket/`

`index.tsx` · `CredentialsPanel.tsx` · `SonarCredentialsPanel.tsx` ·
`RepoForm.tsx` · `ConfirmPanel.tsx` · `ResultsPanel.tsx` · `shared.ts`

Pantalla única: credenciales arriba, formulario, panel de confirmación
desplegado (no diálogo), resultados. "Continuar" no crea nada: despliega el
resumen y recién ese botón ejecuta.

---

## 4. Decisiones que no se leen en el código

**Tres niveles de falla en el lote:**

- Repo ya existe o falla al crearse → se omite, sigue el siguiente. Con 10
  repos, que el 3º falle no justifica no crear los otros 7
- Falla el commit inicial → error **con la URL**: el repo existe pero vacío, y
  sin commit no se pueden crear ramas, así que no se intentan los pasos
  siguientes
- Falla Pipelines, una rama o SonarQube → **advertencia**: el repositorio sirve,
  eso se completa a mano
- Se cae la red → se corta el lote; los demás fallarían igual

**Un fallo de SonarQube nunca marca como error el repositorio de Bitbucket.** Son
sistemas distintos y aparece en su propia línea.

**Los nombres fuera del patrón `br-[proyecto]-sd#####-oi#####` no bloquean.** Se
marcan en amarillo y la confirmación dice cuántos son. Si el estándar cambia o
aparece una excepción legítima, la herramienta no deja atrapado al usuario.

**La casilla de SonarQube se comporta como las otras dos:** habilitada siempre.
Antes dependía de que el token estuviera validado, y como esa consulta es
asíncrona nacía gris y se encendía sola. El aviso de token faltante va en el
botón de continuar. El panel del token solo se monta si la casilla está marcada,
así que desmarcada ese ítem del llavero no se lee nunca.

**La verificación del binding es independiente:** que el POST responda 201 no
prueba que el vínculo quedó. Se consulta de vuelta y el resultado dice
"(verificado)". Si el POST dice que sí y la consulta no lo encuentra, se reporta
como error: ese proyecto quedaría sin decoración de PR.

**La regla de rama por país está duplicada** (Rust y TypeScript) a propósito: la
de TS solo muestra el nombre antes de ejecutar, la que manda al crear es la de
Rust. Chile `development`, Perú `develop`.

**Credenciales en una entrada del llavero, no dos.** La primera versión guardaba
correo y token como ítems separados y cada operación pedía la clave dos veces.

---

## 5. Lo que se conservó textual de la herramienta anterior

El patrón de nombre y su validación · la regla de rama por país · el contenido
del `.gitignore` y el `README.md` · la configuración del repositorio · la
omisión de repos existentes · el header Basic armado a mano · las instrucciones
de SonarQube (ahora reducidas al widget cuando se automatiza el resto).

---

## 6. Errores del desarrollo — para no repetirlos

1. **Se borró el registro del módulo CI/CD.** Se entregó `lib.rs` y
   `modules/mod.rs` completos, construidos sobre un zip de dos semanas antes.
   CI/CD quedó fuera del binario y **compiló sin quejarse**. Lo detectó el
   usuario, no la validación. De ahí sale la regla de auditar líneas eliminadas
   y de entregar solo líneas en archivos compartidos.
2. **Se revirtieron mejoras del usuario** en `bitbucket/index.tsx`,
   `CredentialsPanel.tsx`, `RepoForm.tsx` y `credentials.rs`, por el mismo
   motivo.
3. **Agregar campos a un struct** rompe todos sus constructores, y el balance de
   llaves no lo detecta: pasó dos veces con `ResultadoRepo`.
4. **`window.__TAURI__` no existe** sin `withGlobalTauri: true`.
5. **Archivos descargados sueltos se aplanan** y dos `mod.rs` distintos
   colisionan. Entregar zips con estructura.
6. **`devops-hub.key` y `.key.pub` son las claves de firma del actualizador**,
   no el token de Bitbucket. No mezclarlas.
7. **Una explicación descartada:** se afirmó que cada recompilación invalida el
   permiso del llavero en macOS. El log del usuario lo desmintió. No hay modelo
   confirmado de por qué.
8. **Un cálculo que no cerraba:** se atribuyeron 6 diálogos del llavero a
   "2 ítems × 2 montajes", que da 4. La otra conversación lo señaló. El arreglo
   (caché atómica) igual era correcto y bajó de 6 a 2.

---

## 7. Pendientes

- **AppImage y D-Bus en Linux** — se descubre al correr el workflow
- **Medir los diálogos con el binario instalado** — decide si lo de StrictMode
  era la causa
- **Decoración de PR** — requiere el flujo real con el ticket de CI/CD
- **Quitar `withGlobalTauri`** cuando el hub esté completo
- **Actualizador automático: no operativo.** `active: false`, `endpoints` y
  `pubkey` en PENDIENTE, plugin no instalado. Las claves de firma existen y el
  workflow ya pasa el secreto. Es tarea propia
- **Icono de la app** — pendiente confirmar el origen y la licencia del camaleón
- **Bitbucket adapta su `credentials.rs`** al helper `llavero.rs`
