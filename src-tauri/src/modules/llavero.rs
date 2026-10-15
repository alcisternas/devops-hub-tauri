// ─────────────────────────────────────────────────────────────────────────
// Acceso al almacen de credenciales del sistema — comun a todos los modulos
//
// macOS Keychain · Windows Credential Manager · Linux Secret Service
//
// Existe para que Bitbucket, SonarQube y lo que venga despues no escriban
// cada uno su propia logica de llavero. Un modulo declara su instancia:
//
//     static LLAVERO: Llavero = Llavero::nuevo("devops-hub-bitbucket", "credenciales");
//
// ...y usa leer / guardar / borrar. Si manana cambia el comportamiento del
// almacen, se cambia una sola vez.
//
// ── Dos reglas de uso que NO puede imponer este archivo ──
//
// 1. SIEMPRE dentro de tokio::task::spawn_blocking.
//    Estas funciones son sincronas y bloquean el hilo donde corren: en Linux
//    cada acceso viaja por D-Bus a otro proceso. Ademas, desde que la lectura
//    ocurre con el candado tomado, una segunda llamada simultanea ESPERA a
//    que la primera termine. Llamar esto desde un hilo async congelaria la
//    interfaz, y ahora mas que antes.
//
// 2. ESTE HELPER NO VALIDA NADA.
//    No sabe de APIs ni de que significa el texto que guarda. La regla del
//    equipo es no guardar jamas una credencial sin haberla validado antes
//    contra su servicio: guardar un token roto lo esconde hasta el proximo
//    uso. Esa validacion es responsabilidad del modulo, ANTES de llamar a
//    guardar().
// ─────────────────────────────────────────────────────────────────────────

use keyring::Entry;
use std::sync::Mutex;

pub struct Llavero {
    servicio: &'static str,
    // El almacen indexa por <servicio, usuario>. Como al abrir la app no se
    // conoce todavia el correo del usuario, cada modulo usa un nombre FIJO
    // (ej. "credenciales", "token") para que la lectura sea determinista.
    usuario: &'static str,
    // Texto opaco a proposito: Bitbucket guarda un JSON con correo y token,
    // SonarQube solo el token, y Jenkins guardara lo suyo. Una struct comun
    // obligaria a los demas a rellenar campos que no usan.
    cache: Mutex<Option<String>>,
}

impl Llavero {
    // const fn permite declararlo como `static` directamente, sin OnceLock.
    // Mutex::new es const desde Rust 1.63.
    pub const fn nuevo(servicio: &'static str, usuario: &'static str) -> Self {
        Self {
            servicio,
            usuario,
            cache: Mutex::new(None),
        }
    }

    fn entrada(&self) -> Result<Entry, String> {
        Entry::new(self.servicio, self.usuario).map_err(|e| e.to_string())
    }

    // El candado se toma ANTES de consultar la cache y NO se suelta hasta
    // despues de leer el llavero.
    //
    // Sin eso hay una carrera real: dos llamadas simultaneas ven la cache
    // vacia, las dos van al llavero, y en macOS eso son dos solicitudes de
    // clave en vez de una. Sosteniendo el candado, la segunda espera y
    // encuentra el valor ya cargado.
    pub fn leer(&self) -> Result<Option<String>, String> {
        // Un panic mientras se sostiene el candado envenena el Mutex y
        // dejaria la aplicacion sin poder leer credenciales hasta reiniciar.
        // Para una cache eso es peor que el dato a medio escribir: lo peor
        // que puede pasar al recuperarse es una lectura de mas.
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());

        if let Some(valor) = guard.as_ref() {
            return Ok(Some(valor.clone()));
        }

        let leido = match self.entrada()?.get_password() {
            Ok(v) => Some(v),
            // NoEntry significa "nunca se guardo nada": es el caso normal de
            // la primera vez, no un error.
            Err(keyring::Error::NoEntry) => None,
            Err(e) => return Err(e.to_string()),
        };

        // Solo se cachea lo que existe. Cachear la ausencia haria que, tras
        // guardar credenciales nuevas, la app siguiera creyendo que no hay
        // ninguna hasta reiniciarla.
        if leido.is_some() {
            *guard = leido.clone();
        }

        Ok(leido)
    }

    // Guarda y deja la cache al dia, sin releer el llavero.
    //
    // Recordatorio: el modulo ya debio validar este valor contra su API.
    pub fn guardar(&self, valor: &str) -> Result<(), String> {
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        self.entrada()?.set_password(valor).map_err(|e| e.to_string())?;
        *guard = Some(valor.to_string());
        Ok(())
    }

    // Idempotente: si no habia nada que borrar, la intencion se cumplio igual.
    //
    // La cache se limpia PRIMERO. Si el borrado del llavero fallara, lo que
    // habia en memoria queda descartado de todos modos: nunca se sigue usando
    // un token que el usuario pidio eliminar.
    pub fn borrar(&self) -> Result<(), String> {
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;

        match self.entrada()?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
