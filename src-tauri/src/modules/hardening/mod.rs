// Modulo Hardening de imagenes — analiza una imagen de contenedor, deja elegir
// que componentes conservar segun su tipo y genera un Dockerfile endurecido.
//
// Port a Rust del motor de Container Hardening Studio (backend/app/hardening y
// scanner). La implementacion de Python es la REFERENCIA: las pruebas comparan
// la salida de este modulo con la de Python, campo por campo, sobre un escaneo
// real de node:26.12.0-alpine3.24 (ver testdata/).
//
// Fase 1 (esta): resolver la imagen, escanearla, mostrar componentes y su
// impacto, planificar los parches y generar el Dockerfile.
// Pendiente: construir con BuildKit, pruebas dentro de la imagen, reescaneo,
// politica y reportes. Necesitan Docker/BuildKit, que no estaba disponible
// al desarrollar esta fase.
//
// Estrategia de hardening: «rootfs limpio». La imagen final se ensambla en una
// etapa intermedia a partir del binario de Node.js de la imagen origen y solo
// los paquetes que ese binario exige mas los componentes elegidos, y se
// publica FROM scratch. Nunca se oculta ni se ignora una vulnerabilidad: si no
// existe una correccion compatible, el plan informa el bloqueo.
pub mod comandos;
pub mod componentes;
pub mod config;
pub mod dockerfile;
pub mod escaner;
pub mod impacto;
pub mod planificador;
pub mod publicacion;
pub mod referencia;
pub mod registro;
pub mod validar;
pub mod version;
#[cfg(test)]
mod pruebas_oro;
