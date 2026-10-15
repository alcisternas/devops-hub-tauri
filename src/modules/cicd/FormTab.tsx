import { useState } from "react";
import CollapsibleSection from "../../components/CollapsibleSection";
import { Button, ErrorNotice, SuccessNotice } from "../../components/ui";
import Solicitud from "./sections/Solicitud";
import ArchivosRepo from "./sections/ArchivosRepo";
import PipelineJenkins from "./sections/PipelineJenkins";
import Spinnaker from "./sections/Spinnaker";
import type { Hallazgo } from "./generators";
import type { CatalogoSubredes, CicdForm, EstadoSubredes } from "./shared";
import {
  ambientesActivos,
  contarSugeridos,
  esApigee,
  esCR,
  necesitaSpinnaker,
  parsearRepos,
  validar,
} from "./shared";

interface Props {
  f: CicdForm;
  set: (patch: Partial<CicdForm>) => void;
  onReiniciar: () => void;
  catalogo: CatalogoSubredes;
  subnetsEstado: EstadoSubredes | null;
  subnetsCargando: boolean;
  subnetsError: string;
  subnetsTotal: number;
  onActualizarSubnets: () => void;
  // Lo que va a quedar sin resolver en los archivos generados. Es lo que
  // define el estado de la sección 2: no tiene campos obligatorios propios,
  // así que lo único que importa es si algo quedaría escrito vacío o con
  // un marcador.
  huecos: Hallazgo[];
}

// Pantalla única con cuatro secciones, en el orden en que se llenan:
// primero lo que sale de la solicitud, después lo que hay que dejar en el
// repositorio, después el pipeline de Jenkins y al final Spinnaker.
//
// Las secciones que no aplican al flujo no se muestran: Apigee no tiene
// Spinnaker, y Cloud Run tampoco.
export default function FormTab({
  f,
  set,
  onReiniciar,
  catalogo,
  subnetsEstado,
  subnetsCargando,
  subnetsError,
  subnetsTotal,
  onActualizarSubnets,
  huecos,
}: Props) {
  const apigee = esApigee(f);
  const faltas = validar(f);
  // La sección 2 llega casi completa con valores sugeridos, así que decir
  // "completa" prometería una revisión que nadie hizo. Hasta que se abre,
  // el rótulo informa cuántos valores vienen sugeridos.
  const [revisada, setRevisada] = useState(false);
  const sugeridos = contarSugeridos(f);
  const repos = parsearRepos(f.reposText);
  const envs = ambientesActivos(f);

  // Estado por sección: se calcula de los campos obligatorios de cada una,
  // para que el encabezado diga qué falta sin tener que abrirla.
  const faltaSolicitud =
    !f.ticket.trim() || !f.tech || !envs.length || !repos.length || (!apigee && (!f.bbKey.trim() || !f.bbName.trim()));
  const faltaArchivos = (apigee && !f.apigeeProxyName.trim()) || huecos.length > 0;
  const faltaJenkins = apigee ? !f.apigeeSubtype : !f.folder.trim() || !f.jfpath.trim();

  const estado = (falta: boolean) =>
    ({
      estado: falta ? ("pendiente" as const) : ("completa" as const),
      estadoTexto: falta ? "falta completar" : "completa",
    });

  return (
    <div className="space-y-4 pb-4">
      <CollapsibleSection
        index={1}
        title="Solicitud"
        subtitle="Ticket, tecnología, ambientes, proyectos y repositorios"
        defaultOpen
        {...estado(faltaSolicitud)}
      >
        <Solicitud f={f} set={set} />
      </CollapsibleSection>

      <CollapsibleSection
        index={2}
        estado={faltaArchivos ? "pendiente" : revisada ? "completa" : "info"}
        estadoTexto={
          faltaArchivos
            ? huecos.length > 0
              ? `${huecos.length} sin resolver`
              : "falta completar"
            : revisada
              ? "revisada"
              : `${sugeridos} valores sugeridos`
        }
        onPrimeraApertura={() => setRevisada(true)}
        title="Configuración del repositorio"
        subtitle={
          apigee
            ? "JenkinsParams de Apigee"
            : esCR(f)
              ? "JenkinsParams, service.yaml y bitbucket-pipelines.yml"
              : "JenkinsParams, service.yaml, istio.yaml y bitbucket-pipelines.yml"
        }
      >
        <ArchivosRepo
          f={f}
          set={set}
          catalogo={catalogo}
          subnetsEstado={subnetsEstado}
          subnetsCargando={subnetsCargando}
          subnetsError={subnetsError}
          subnetsTotal={subnetsTotal}
          onActualizarSubnets={onActualizarSubnets}
        />
      </CollapsibleSection>

      <CollapsibleSection
        index={3}
        title="Pipeline de Jenkins"
        subtitle="Dónde se crea el pipeline y con qué plantilla"
        {...estado(faltaJenkins)}
      >
        <PipelineJenkins f={f} set={set} />
      </CollapsibleSection>

      {necesitaSpinnaker(f) && (
        <CollapsibleSection
          index={4}
          title="Spinnaker"
          subtitle="Aplicaciones, pipelines y accounts"
          estado="info"
          estadoTexto="solo revisión"
        >
          <Spinnaker f={f} />
        </CollapsibleSection>
      )}

      {/* Estado general. No bloquea nada: los archivos se generan igual y
          se ven en la pestaña de al lado, con los huecos a la vista.
          Bloquear tiene sentido recién cuando haya que escribir en
          Bitbucket. */}
      <div className="pt-1">
        {faltas.length === 0 ? (
          <SuccessNotice>
            Configuración completa. Los archivos están listos en la pestaña Archivos.
          </SuccessNotice>
        ) : (
          <ErrorNotice>
            Falta completar: {faltas.slice(0, 8).join(" · ")}
            {faltas.length > 8 ? ` · y ${faltas.length - 8} más` : ""}
          </ErrorNotice>
        )}
      </div>

      <div className="pt-2">
        <Button variant="secondary" onClick={onReiniciar}>
          Vaciar el formulario
        </Button>
      </div>
    </div>
  );
}
