// Modulo Bitbucket Repo Creator — migracion de la herramienta HTML
// standalone (bitbucket-repo-tool.html v1.2.0) al hub.
//
// Fase 1: credentials — token en el almacen del sistema + validacion.
// Fase 2: client    — cliente HTTP contra la API de Bitbucket.
//         repos     — crear repo, commit inicial, pipelines, ramas.
//         full_run  — orquestacion del lote + progreso a la consola.
// Fase 3 (pendiente): la interfaz React del modulo.
pub mod client;
pub mod credentials;
pub mod full_run;
pub mod repos;
