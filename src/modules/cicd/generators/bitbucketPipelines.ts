// bitbucket-pipelines.yml — corre el análisis de SonarCloud en Bitbucket.
//
// El contenido es fijo salvo la imagen base: lo único que varía entre
// proyectos es con qué imagen arranca el runner, y eso depende de la
// tecnología.
//
// Las variables del archivo son de tres fuentes distintas y conviene no
// confundirlas al leerlo:
//   $AWS_ACCESS_KEY_ID, $BITBUCKET_*  las pone Bitbucket Pipelines
//   $SCAN_SOURCE, $SCAN_EXCLUDE_FILES vienen del JenkinsParams, que el
//                                     primer paso carga con "source"
//   $VERSION, $REPOSITORY_NAME        se calculan dentro del mismo script
//
// Los ${...} van escapados para que los escriba literalmente en el archivo
// en vez de interpolarlos acá.

import type { CicdData } from "../shared";

export function buildBitbucketPipelines(d: CicdData): string {
  const image = d.bbpipeImage || "IMAGEN-BASE-PENDIENTE";

  return `image:
  name: ${image}
  aws: 
    access-key: $AWS_ACCESS_KEY_ID
    secret-key: $AWS_SECRET_ACCESS_KEY
clone:
  depth: full
definitions:
  caches:
    sonar: ~/.sonar/cache
  steps:
    - step: &build-test-sonarcloud
        name: Build, test and analyze on SonarCloud
        caches:
          - node
          - sonar
        script:
          - source JenkinsParams
          - export VERSION=$(grep "vars.Version" settings/settings.go | awk -F'["]' '{print $2}')
          - export REPOSITORY_NAME=$(echo $BITBUCKET_REPO_FULL_NAME |  sed 's|.*/||')
          - pipe: sonarsource/sonarcloud-scan:4.1.0
            variables:
              EXTRA_ARGS: >
                -Dsonar.projectKey="\${BITBUCKET_PROJECT_KEY}_\${REPOSITORY_NAME}"
                -Dsonar.projectName="\${BITBUCKET_PROJECT_KEY}_\${REPOSITORY_NAME}"
                -Dsonar.projectVersion="$VERSION"
                -Dsonar.sources="$SCAN_SOURCE"
                -Dsonar.exclusions="$SCAN_EXCLUDE_FILES"
    - step: &check-quality-gate-sonarcloud
        name: Check the Quality Gate on SonarCloud
        caches:
          - sonar
        script:
          - pipe: sonarsource/sonarcloud-quality-gate:0.1.6

pipelines:
  custom: # Pipelines that can only be triggered manually
    sonar:
      - step: *build-test-sonarcloud
      - step: *check-quality-gate-sonarcloud
  branches:
    development:
      - step: *build-test-sonarcloud
      - step: *check-quality-gate-sonarcloud
    master:
      - step: *build-test-sonarcloud
  pull-requests:
    "**":
      - step: *build-test-sonarcloud
      - step: *check-quality-gate-sonarcloud
`;
}
