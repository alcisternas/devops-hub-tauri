// Modulo Jira — lee los tickets de "Alta de repositorios Bitbucket" y deja
// el trabajo hecho: crea los repositorios (reutilizando el motor del modulo
// Bitbucket), etiqueta el ticket, lo cierra y responde al solicitante.
//
// Separado de bitbucket a proposito: es otro sistema externo, con su propio
// token y su propio item en el llavero (devops-hub-jira).
//
//   client       HTTP contra Jira Cloud y su API de formularios
//   credentials  token en el llavero + validacion
//   tickets      busqueda de pendientes, lectura del formulario y validacion
//   procesar     orquestacion de un ticket y publicacion de la evidencia
pub mod client;
pub mod credentials;
pub mod procesar;
pub mod tickets;
