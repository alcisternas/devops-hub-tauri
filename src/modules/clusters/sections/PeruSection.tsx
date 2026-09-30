import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ErrorNotice, SuccessNotice, WarningNotice, MutedList, FieldLabel, MetaLine } from "../../../components/ui";
import { formatDuration, type RefreshError } from "../shared";
import { useConsole } from "../ConsoleContext";

interface AccountRoleInfo {
  account_id: string;
  account_name: string;
  profile: string;
  roles: string[];
}
interface PeruDiscoveryResult {
  total_cuentas_portal: number;
  cuentas: AccountRoleInfo[];
  omitidas: string[];
}
interface PeruFullRunResult {
  total_cuentas_intentadas: number;
  total_clusters_probados: number;
  ok: number;
  insuficientes: number;
  omitidos: string[];
  inventory_path: string;
  csv_path: string;
  shrink_warning: string | null;
  duracion_segundos: number;
  failures_csv_path: string;
  accesos_revocados: number;
  revoked_csv_path: string | null;
}

export default function PeruSection() {
  const [discovery, setDiscovery] = useState<PeruDiscoveryResult | null>(null);
  const [fullRunResult, setFullRunResult] = useState<PeruFullRunResult | null>(null);
  const [error, setError] = useState<string>("");
  const [phase, setPhase] = useState<"idle" | "discovering" | "scanning">("idle");
  const { bump } = useConsole();

  function parseErr(err: unknown): string {
    const e = err as RefreshError;
    return e && e.etapa && e.mensaje ? `[${e.etapa}] ${e.mensaje}` : String(err);
  }

  async function discoverAndScan() {
    setError("");
    setDiscovery(null);
    setFullRunResult(null);
    bump();

    setPhase("discovering");
    let disc: PeruDiscoveryResult;
    try {
      disc = await invoke<PeruDiscoveryResult>("discover_peru_accounts");
      setDiscovery(disc);
    } catch (err) {
      setError(parseErr(err));
      setPhase("idle");
      return;
    }

    setPhase("scanning");
    try {
      setFullRunResult(await invoke<PeruFullRunResult>("run_full_peru_scan", { accounts: disc.cuentas }));
    } catch (err) {
      setError(parseErr(err));
    } finally {
      setPhase("idle");
    }
  }

  const buttonLabel =
    phase === "discovering" ? "Descubriendo cuentas…" : phase === "scanning" ? "Recorriendo clusters…" : "Descubrir clusters";

  return (
    <div className="space-y-5">
      <div>
        <FieldLabel>
          Perú entra siempre vía titan-pipeline (AssumeRole desde devops-base) — el rol de la SSO no
          determina el acceso real, así que se verifica una vez por cuenta, no por rol. Un solo botón
          descubre y recorre en un solo paso.
        </FieldLabel>
        <Button onClick={discoverAndScan} disabled={phase !== "idle"}>
          {buttonLabel}
        </Button>
      </div>

      {error && <ErrorNotice>{error}</ErrorNotice>}

      {discovery && (
        <SuccessNotice>
          ✓ {discovery.total_cuentas_portal} cuentas en el portal — {discovery.cuentas.length} mapeadas,{" "}
          {discovery.omitidas.length} omitidas — detalle en la consola
        </SuccessNotice>
      )}

      {fullRunResult && (
        <div className="space-y-2 text-xs pt-2 border-t" style={{ borderColor: "var(--border)" }}>
          <SuccessNotice>
            ✓ {fullRunResult.total_clusters_probados} cluster(s) probados — {fullRunResult.ok} OK,{" "}
            {fullRunResult.insuficientes} insuficientes, {fullRunResult.omitidos.length} omitidos
          </SuccessNotice>
          <MetaLine>
            {fullRunResult.total_cuentas_intentadas} cuentas intentadas · duración:{" "}
            {formatDuration(fullRunResult.duracion_segundos)}
          </MetaLine>
          {fullRunResult.shrink_warning && <WarningNotice>⚠ {fullRunResult.shrink_warning}</WarningNotice>}
          {fullRunResult.accesos_revocados > 0 && (
            <div className="text-xs" style={{ color: "var(--fail)" }}>
              ⚠ {fullRunResult.accesos_revocados} acceso(s) revocado(s) — CSV: {fullRunResult.revoked_csv_path}
            </div>
          )}
          <MetaLine>Inventario: {fullRunResult.inventory_path}</MetaLine>
          <MetaLine>CSV: {fullRunResult.csv_path}</MetaLine>
          <MetaLine>CSV de fallos: {fullRunResult.failures_csv_path}</MetaLine>
          <MutedList items={fullRunResult.omitidos} />
        </div>
      )}
    </div>
  );
}
