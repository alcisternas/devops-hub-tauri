import CollapsibleSection from "../../components/CollapsibleSection";
import { Button, ErrorNotice, SuccessNotice } from "../../components/ui";
import Solicitud from "./sections/Solicitud";
import Repositorios from "./sections/Repositorios";
import Despliegue from "./sections/Despliegue";
import JenkinsParamsSection from "./sections/JenkinsParams";
import Spinnaker from "./sections/Spinnaker";
import SubnetsPanel from "./SubnetsPanel";
import type { CicdForm, EstadoSubredes } from "./shared";
import { esApigee, esCR, necesitaSpinnaker, parsearRepos, validar } from "./shared";

interface Props {
  f: CicdForm;
  set: (patch: Partial<CicdForm>) => void;
  onReiniciar: () => void;
  subredesSinMatch: string[];
  subnetsEstado: EstadoSubredes | null;
  subnetsCargando: boolean;
  subnetsError: string;
  subnetsTotal: number;
  onActualizarSubnets: () => void;
}

// Pantalla única con secciones plegables, en vez del asistente de pasos del
// HTML. Dos razones: los archivos se regeneran en cada tecleo, así que el
// orden de llenado dejó de importar; y así se puede corregir un campo del
// primer bloque sin volver atrás tres pantallas.
//
// Las secciones que no aplican al flujo elegido no se muestran: Apigee no
// tiene despliegue ni Spinnaker, y Cloud Run no tiene Spinnaker.
export default function FormTab({
  f,
  set,
  onReiniciar,
  subredesSinMatch,
  subnetsEstado,
  subnetsCargando,
  subnetsError,
  subnetsTotal,
  onActualizarSubnets,
}: Props) {
  const apigee = esApigee(f);
  const faltas = validar(f);
  const repos = parsearRepos(f.reposText);

  return (
    <div className="space-y-3 pb-4">
      <CollapsibleSection
        title="Solicitud"
        subtitle="Ticket, tecnología, país, destino y ambientes"
        defaultOpen
      >
        <Solicitud f={f} set={set} />
      </CollapsibleSection>

      <CollapsibleSection
        title="Repositorios"
        subtitle={repos.length ? `${repos.length} cargado${repos.length === 1 ? "" : "s"}` : "Ninguno todavía"}
        defaultOpen
      >
        <Repositorios f={f} set={set} />
      </CollapsibleSection>

      {!apigee && (
        <CollapsibleSection
          title="Despliegue"
          subtitle="Bitbucket, namespaces, rutas y datos de la infraestructura"
        >
          <Despliegue f={f} set={set} subredesSinMatch={subredesSinMatch} />
        </CollapsibleSection>
      )}

      <CollapsibleSection
        title={apigee ? "Jenkins y JenkinsParams de Apigee" : "Jenkins y JenkinsParams"}
        subtitle={apigee ? "Proxy, organizaciones y rutas base" : "Ubicación del pipeline y variables del archivo"}
      >
        <JenkinsParamsSection f={f} set={set} />
      </CollapsibleSection>

      {necesitaSpinnaker(f) && (
        <CollapsibleSection title="Spinnaker" subtitle="Aplicaciones, pipelines y accounts">
          <Spinnaker f={f} />
        </CollapsibleSection>
      )}

      {/* El panel de subredes solo tiene sentido en Cloud Run: es el único
          service.yaml que lleva red y subred. */}
      {esCR(f) && (
        <CollapsibleSection title="Subredes GCP" subtitle="Origen de la red del service.yaml">
          <SubnetsPanel
            estado={subnetsEstado}
            cargando={subnetsCargando}
            error={subnetsError}
            total={subnetsTotal}
            onActualizar={onActualizarSubnets}
          />
        </CollapsibleSection>
      )}

      {/* Estado de lo que falta. No bloquea nada: los archivos se generan
          igual y se ven en la pestaña de al lado, con los huecos a la
          vista. Bloquear tiene sentido recién cuando haya que escribir en
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
