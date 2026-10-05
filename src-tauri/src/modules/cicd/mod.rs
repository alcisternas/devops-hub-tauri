// Modulo CI/CD Automation — migracion de la herramienta HTML standalone
// (cicd-tool-v4.html v4.28.0) al hub.
//
// Fase A: subnets — catalogo de subredes GCP, con cache en disco y
//         consulta en vivo mediante gcloud. Todo el resto de la fase A
//         (formulario y generadores de archivos) es logica pura y vive en
//         el frontend, porque no necesita red ni sistema operativo.
//
// Fases siguientes, todavia no desarrolladas:
//         bitbucket — subida de los archivos generados al repositorio
//         jenkins   — lanzamiento del job que crea los pipelines
//         spinnaker — creacion de aplicaciones y pipelines de despliegue
pub mod subnets;
