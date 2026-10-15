# Convenciones compartidas — DevOps Hub

Aplica a **todos los módulos** y a **todas las conversaciones** que trabajen el
proyecto en paralelo. Lo que está acá no pertenece a Bitbucket, Sonar, CI/CD ni
Clusters: pertenece al hub.

Estado al escribirse: **v0.6.0**.

---

## 1. Superficie compartida — avisar antes de tocar

Estos archivos los usan varios módulos. **Antes de modificar cualquiera, avisar
a la otra conversación.** Si dos lados los editan en paralelo, la siguiente
entrega pisa la anterior.

| Archivo | Qué contiene |
|---|---|
| `src-tauri/src/modules/mod.rs` | Declaración de todos los módulos Rust |
| `src-tauri/src/lib.rs` | Registro de todos los comandos Tauri |
| `src-tauri/src/modules/llavero.rs` | Acceso al almacén de credenciales |
| `src/App.tsx` | Montaje de módulos y rutas |
| `src/components/Layout.tsx` | Sidebar |
| `src/modules/home/index.tsx` | Tarjetas de la pantalla de inicio |
| `src/components/ui.tsx` | Button, Dropdown, Notices, FieldLabel, MetaLine |
| `src/components/CollapsibleSection.tsx` | Secciones plegables |
| `src/modules/clusters/GlobalConsole.tsx` | Consola global y `MODULE_LABELS` |
| `src/modules/clusters/ConsoleContext.tsx` | Contexto de la consola |
| `src/hooks/` | `useConsolePanel`, `useResizablePanel`, `useFontScale` |
| Los 4 de versión | `package.json`, `tauri.conf.json`, `Cargo.toml`, `version.ts` |

**Entregar solo las líneas a cambiar, nunca el archivo completo**, cuando se
trate de uno de estos. Un archivo completo construido sobre una copia vieja
revierte todo lo que el otro lado hizo mientras tanto.

---

## 2. Versiones

Las cuatro versiones son **del hub completo**, no por módulo. No pueden existir
dos `0.6.0` distintos.

- **El número lo toma quien entrega primero**; el otro corre el suyo.
- **Cada entrega dice de qué versión parte.** Si un zip parte de una base
  anterior a la actual, revierte trabajo.
- Patch para arreglos, minor para funcionalidad nueva.

Histórico: `0.5.2` CI/CD fase B · **`0.6.0` SonarQube** · `0.7.0` reservado
para Jenkins (Fase C) · `0.8.0` Spinnaker · `0.9.0` encadenado.

---

## 3. Credenciales — `modules/llavero.rs`

Helper único. **Ningún módulo escribe lógica de llavero propia.**

```rust
static LLAVERO: Llavero = Llavero::nuevo("devops-hub-<sistema>", "<usuario-fijo>");

LLAVERO.leer()            // -> Result<Option<String>, String>
LLAVERO.guardar(&texto)   // -> Result<(), String>
LLAVERO.borrar()          // -> Result<(), String>
```

**Guarda texto opaco.** Bitbucket serializa un JSON con correo y token, Sonar
guarda el token directo, Jenkins lo que necesite. Una struct común obligaría a
los demás a rellenar campos que no usan.

**Reglas de uso:**

1. **Siempre dentro de `tokio::task::spawn_blocking`.** Es síncrono y bloquea:
   en Linux cada acceso viaja por D-Bus a otro proceso, y desde que la lectura
   ocurre con el candado tomado, una segunda llamada espera.
2. **El helper no valida nada.** Nunca guardar una credencial sin haberla
   validado antes contra su servicio: un token roto queda escondido hasta el
   siguiente uso.
3. **Un ítem por sistema**, con la convención `devops-hub-<sistema>`. Unificar
   ahorraría un diálogo y costaría no poder rotar un token sin tocar el otro.
4. **No leer el llavero hasta que haga falta.** El panel de credenciales recibe
   `activo` y no consulta mientras sea falso. Si en una sesión no se usa un
   sistema, su ítem no se lee nunca.

**Por qué el candado se sostiene durante la lectura:** sin eso, dos llamadas
simultáneas ven la caché vacía y las dos van al llavero. En macOS eso son dos
solicitudes de clave en vez de una.

**Qué no cachea:** la ausencia. Si se cacheara "no hay nada", guardar
credenciales después pasaría inadvertido hasta reiniciar.

**Medición real:** de 6 solicitudes por sesión se bajó a 2, una por sistema.
**Pendiente:** verificar con el binario compilado e instalado. `React.StrictMode`
duplica efectos solo en desarrollo; si en producción siguen siendo dos por
sistema, hay dos comprobaciones por acceso y es otro problema.

---

## 4. Patrones del hub

**Errores:** todo comando falible devuelve `Result<T, RefreshError>`, con
`RefreshError { etapa, mensaje }` (vive en `eks::config` por ser el primero que
lo necesitó). En React se parsea con `parseErr`.

**Progreso:** `eks::permissions::emit_progress(modulo, app, etapa, item, progreso, total, estado)`.
La consola no filtra: cada módulo emite con su tag y agrega su prefijo en
`MODULE_LABELS` de `GlobalConsole.tsx`.

**Tokens de diseño:** todo en `src/index.css`. Nunca colores Tailwind crudos.

**Nunca desmontar módulos al navegar:** `App.tsx` los mantiene montados y
alterna visibilidad por CSS. Mismo criterio para secciones plegables y pestañas.

**Dropdown propio, no `<select>` nativo**, cuando alguna opción necesite color.

---

## 5. Dependencias cruzadas conocidas

`cicd/bitbucket.rs` importa `BBClient` y `obtener_credenciales` del módulo
Bitbucket. **Si cambia la firma de algo en `bitbucket/client.rs` o
`bitbucket/credentials.rs`, CI/CD se rompe.** Asumido por ambos lados, pero hay
que avisar antes de cambiar esas firmas.

Ejemplo real: al agregar SonarQube, `repos::crear` pasó de devolver `String` a
`(String, String)`. Se verificó que solo `full_run` la llamaba.

---

## 6. Compilación multiplataforma

**TLS:** `reqwest` con `default-features = false` y `rustls-tls`. Con
`native-tls` el build de Linux depende de OpenSSL y falla.

**Llavero en Linux:** el backend de Secret Service arrastra `libdbus-sys`, que
no compila sin `libdbus-1-dev` y `pkg-config`. Ya están en el paso de Linux de
`release.yml`, que tiene su guardia `if` y no afecta a macOS ni Windows.

`keyring` se declara **por plataforma** (`[target.'cfg(target_os = "...")']`)
para que el backend de D-Bus no se cuele en los builds de macOS y Windows.
Verificado: en Mac aparece en el lockfile pero nunca se compila.

**Pendiente:** confirmar que el AppImage encuentre `libdbus` en la máquina del
usuario. Las AppImage excluyen algunas librerías consideradas de sistema. Si
falla, evaluar publicar `.deb` declarando `libdbus-1-3`.

---

## 7. Validaciones antes de entregar

Lo que la experiencia mostró que hace falta:

- Sintaxis y balance de delimitadores, respetando cadenas y raw strings
- **Cada constructor de struct con todos sus campos** — agregar un campo rompe
  todos los constructores y el balance de llaves no lo detecta
- Comandos definidos == comandos registrados en `lib.rs`
- Tipos TypeScript vs lo que serializa Rust, campo por campo (ojo: unos van en
  `snake_case` y otros en `camelCase` según lleven `rename_all`)
- **Diff contra la base, listando toda línea eliminada**, y revisar una por una
  que sea un reemplazo intencional y no una pérdida

La última es la que faltaba cuando se borró el registro del módulo CI/CD sin
que nadie lo notara hasta que la app compiló sin él.

---

## 8. Proceso

- **Presentar el plan completo y esperar aprobación explícita antes de
  desarrollar**, sin excepciones. Validar un resultado no es aprobación.
- Entregar los datos disponibles y las alternativas; el usuario decide o aporta
  más información, y el ciclo se repite.
- Explicar cada cambio: qué hace y por qué.
- Nunca asumir rutas de archivo.
- **No suponer datos técnicos.** Validar en documentación oficial o con pruebas
  reales. Varias veces en este proyecto un supuesto razonable resultó falso al
  probarlo, incluida la creencia de que SonarQube no tenía API para esto.
- Español latinoamericano neutro, sin voseo.
- Entregar zips con la estructura de carpetas armada, para descomprimir en la
  raíz del repositorio.
