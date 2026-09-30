import CollapsibleSection from "../../components/CollapsibleSection";
import ChileSection from "./sections/ChileSection";
import PeruSection from "./sections/PeruSection";
import GkeSection from "./sections/GkeSection";

export default function RefreshTab() {
  return (
    <div className="space-y-3">
      <CollapsibleSection title="EKS — Chile" subtitle="SSO multi-cuenta, rol real por cluster">
        <ChileSection />
      </CollapsibleSection>

      <CollapsibleSection title="EKS — Perú" subtitle="Acceso vía titan-pipeline, una verificación por cuenta">
        <PeruSection />
      </CollapsibleSection>

      <CollapsibleSection title="GKE — Chile" subtitle="Proyectos descubiertos dinámicamente en la organización">
        <GkeSection />
      </CollapsibleSection>
    </div>
  );
}
