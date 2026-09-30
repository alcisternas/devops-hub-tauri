import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ErrorNotice, SuccessNotice, WarningNotice, FieldLabel, MetaLine } from "../../../components/ui";
import { formatDuration, splitPath, type RefreshError } from "../shared";

interface GcpProject {
  project_id: string;
  name: string;
  project_number: string;
}
interface GcpDiscoveryResult {
  total_proyectos: number;
  proyectos: GcpProject[];
  reauth_detectado: boolean;
  duracion_segundos: number;
}
interface GkeFullRunResult {
  total_proyectos_intentados: number;
  total_clusters_probados: number;
  ok: number;
  insuficientes: number;
  omitidos: string[];
  inventory_path: string;
  csv_path: string;
  failures_csv_path: string;
  shrink_warning: string | null;
  duracion_segundos: number;
  accesos_revocados: number;
  revoked_csv_path: string | null;
}

export default function GkeSection() {
  const [discovery, setDiscovery] = useState<GcpDiscoveryResult | null>(null);
  const [fullRunResult, setFullRunResult] = useState<GkeFullRunResult | null>(null);
  const [error, setError] = useState<string>("");
  const [phase, setPhase] = useState<"idle" | "discovering" | "scanning">("idle");

  function parseErr(err: unknown): string {
    const e = err as RefreshError;
    return e && e.etapa && e.mensaje ? `[${e.etapa}] ${e.mensaje}` : String(err);
  }

  async function discoverAndScan() {
    setError("");
    setDiscovery(null);
    setFullRunResult(null);

    setPhase("discovering");
    let disc: GcpDiscoveryResult;
    try {
      disc = await invoke<GcpDiscoveryResult>("discover_gcp_projects");
      setDiscovery(disc);
    } catch (err) {
      setError(parseErr(err));
      setPhase("idle");
      return;
    }

    setPhase("scanning");
    try {
      setFullRunResult(await invoke<GkeFullRunResult>("run_full_gke_scan", { projects: disc.proyectos }));
    } catch (err) {
      setError(parseErr(err));
    } finally {
      setPhase("idle");
    }
  }

  const buttonLabel =
    phase === "discovering" ? "Descubriendo proyectos…" : phase === "scanning" ? "Recorriendo clusters…" : "Descubrir clusters";

  return (
    <div className="space-y-5">
      <div>
        <FieldLabel>
          Una sola cuenta GCP, sin tabla de roles — los proyectos se descubren dinámicamente en la
          organización. Un solo botón descubre y recorre en un solo paso.
        </FieldLabel>
        <Button onClick={discoverAndScan} disabled={phase !== "idle"}>
          {buttonLabel}
        </Button>
      </div>

      {error && <ErrorNotice>{error}</ErrorNotice>}

      {(discovery || fullRunResult) && (
        <div className="text-xs">
          <div className="space-y-0.5">
            {discovery && (
              <SuccessNotice>
                ✓ {discovery.total_proyectos} proyecto(s) activo(s)
                {discovery.reauth_detectado && " (sesión reautenticada automáticamente)"} — detalle en la consola
              </SuccessNotice>
            )}
            {fullRunResult && (
              <div>
                <span style={{ color: "var(--ok)" }}>
                  ✓ {fullRunResult.total_clusters_probados} cluster(s) probados — {fullRunResult.ok} OK
                </span>
                {fullRunResult.insuficientes > 0 && (
                  <span style={{ color: "var(--partial)" }}>, {fullRunResult.insuficientes} insuficientes</span>
                )}
                {fullRunResult.omitidos.length > 0 && (
                  <span style={{ color: "var(--fail)" }}>, {fullRunResult.omitidos.length} omitidos</span>
                )}
              </div>
            )}
            {fullRunResult && fullRunResult.accesos_revocados > 0 && (
              <div style={{ color: "var(--fail)" }}>
                ⚠ {fullRunResult.accesos_revocados} acceso(s) revocado(s) desde la corrida anterior — CSV:{" "}
                {splitPath(fullRunResult.revoked_csv_path ?? "").file}
              </div>
            )}
          </div>

          {fullRunResult && (
            <div className="space-y-1 pt-2 mt-2 border-t" style={{ borderColor: "var(--border)" }}>
              <MetaLine>
                {fullRunResult.total_proyectos_intentados} proyectos intentados · duración total:{" "}
                {formatDuration((discovery?.duracion_segundos ?? 0) + fullRunResult.duracion_segundos)}
              </MetaLine>
              {fullRunResult.shrink_warning && <WarningNotice>⚠ {fullRunResult.shrink_warning}</WarningNotice>}
              <MetaLine>Directorio: {splitPath(fullRunResult.inventory_path).dir}</MetaLine>
              <MetaLine>
                Archivos: {splitPath(fullRunResult.inventory_path).file} ·{" "}
                {splitPath(fullRunResult.csv_path).file} · {splitPath(fullRunResult.failures_csv_path).file}
              </MetaLine>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
