import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ErrorNotice, SuccessNotice, WarningNotice, FieldLabel, MetaLine } from "../../../components/ui";
import { formatDuration, splitPath, type RefreshError } from "../shared";

interface AccountRoleInfo {
  account_id: string;
  account_name: string;
  profile: string;
  roles: string[];
}
interface DiscoveryResult {
  total_cuentas_portal: number;
  cuentas: AccountRoleInfo[];
  omitidas: string[];
  duracion_segundos: number;
}
interface FullRunResult {
  total_intentos_rol: number;
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

export default function ChileSection() {
  const [discovery, setDiscovery] = useState<DiscoveryResult | null>(null);
  const [fullRunResult, setFullRunResult] = useState<FullRunResult | null>(null);
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
    let disc: DiscoveryResult;
    try {
      disc = await invoke<DiscoveryResult>("discover_chile_accounts");
      setDiscovery(disc);
    } catch (err) {
      setError(parseErr(err));
      setPhase("idle");
      return;
    }

    setPhase("scanning");
    try {
      setFullRunResult(await invoke<FullRunResult>("run_full_chile_scan", { accounts: disc.cuentas }));
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
          Descubre cuentas/roles mapeados en accounts.json y recorre todos sus clusters, en un solo
          paso. Asegura la sesión SSO sola (abre el browser solo si de verdad hace falta).
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
                ✓ {discovery.total_cuentas_portal} cuentas en el portal — {discovery.cuentas.length} mapeadas,{" "}
                {discovery.omitidas.length} omitidas — detalle en la consola
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
                {fullRunResult.total_intentos_rol} combinaciones cuenta+rol · duración total:{" "}
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
