import CollapsibleSection from "../../components/CollapsibleSection";
import ChileSection from "./sections/ChileSection";
import PeruSection from "./sections/PeruSection";
import GkeSection from "./sections/GkeSection";

export default function RefreshTab() {
  return (
    <div className="space-y-3">
      <CollapsibleSection title="Chile" subtitle="EKS — SSO multi-cuenta, rol real por cluster">
        <ChileSection />
      </CollapsibleSection>

      <CollapsibleSection title="Perú" subtitle="EKS — acceso vía titan-pipeline, una verificación por cuenta">
        <PeruSection />
      </CollapsibleSection>

      <CollapsibleSection title="GKE" subtitle="Chile — proyectos descubiertos dinámicamente en la organización">
        <GkeSection />
      </CollapsibleSection>
    </div>
  );
}
