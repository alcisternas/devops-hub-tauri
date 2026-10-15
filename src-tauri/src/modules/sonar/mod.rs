// Modulo SonarQube Cloud.
//
// Separado de bitbucket a proposito: es otro sistema externo, con su propio
// token y su propio almacen de credenciales (devops-hub-sonarqube).
//
// Automatiza los dos primeros pasos del procedimiento manual:
//   1. Crear el proyecto con la key correcta de entrada, lo que elimina el
//      paso de "Update key" que antes habia que hacer a mano
//   2. Vincularlo al repositorio de Bitbucket (el "Analyze new project")
//
// El tercer paso, activar el widget en Bitbucket, NO tiene API y sigue
// siendo manual. Confirmado en la documentacion oficial de SonarQube Cloud.
//
// Todo el flujo fue verificado contra la instancia real del banco antes de
// escribirse: sondeo de endpoints disponibles y una prueba de escritura
// completa (crear, vincular, verificar y borrar).
pub mod client;
pub mod credentials;
pub mod projects;
