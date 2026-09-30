import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ClusterSelectTable, { type SelectableRow } from "../../components/ClusterSelectTable";
import { Button, ErrorNotice, SuccessNotice, MetaLine, Dropdown } from "../../components/ui";
import type { RefreshError } from "./shared";

interface CombinedClusterEntry {
  pais: string;
  proveedor: string;
  account_id: string;
  account_name: string;
  role: string;
  profile: string;
  cluster: string;
  region: string;
  context_alias: string;
  permisos: Record<string, boolean>;
  permisos_ok: number;
  permisos_total: number;
  sort_key: number;
}
interface CombinedInventoryResult {
  country: string;
  has_aws: boolean;
  has_gcp: boolean;
  entries: CombinedClusterEntry[];
}
interface LoadResultItem {
  cluster: string;
  context_alias: string;
  estado: string;
  detalle: string | null;
}
interface LoadSummary {
  cargados: number;
  omitidos: number;
  con_errores: number;
  items: LoadResultItem[];
  contextos_eliminados: number;
}

function parseErr(err: unknown): string {
  const e = err as RefreshError;
  return e && e.etapa && e.mensaje ? `[${e.etapa}] ${e.mensaje}` : String(err);
}

export default function LoadTab() {
  const [entries, setEntries] = useState<CombinedClusterEntry[]>([]);
  const [status, setStatus] = useState<string>("");
  const [loadErrors, setLoadErrors] = useState<string[]>([]);
  const [fetching, setFetching] = useState(true);

  const [query, setQuery] = useState("");
  const [paisFilter, setPaisFilter] = useState<string>("todos");
  const [proveedorFilter, setProveedorFilter] = useState<string>("todos");
  const [estadoFilter, setEstadoFilter] = useState<string>("todos");
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());

  const [loadResult, setLoadResult] = useState<LoadSummary | null>(null);
  const [loadError, setLoadError] = useState<string>("");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    fetchAll();
  }, []);

  async function fetchAll() {
    setFetching(true);
    setLoadErrors([]);
    setSelectedKeys(new Set());

    const results = await Promise.allSettled([
      invoke<CombinedInventoryResult>("read_combined_inventory", { country: "chile" }),
      invoke<CombinedInventoryResult>("read_combined_inventory", { country: "peru" }),
    ]);

    const merged: CombinedClusterEntry[] = [];
    const errs: string[] = [];
    const statusParts: string[] = [];

    results.forEach((r, i) => {
      const country = i === 0 ? "Chile" : "Perú";
      if (r.status === "fulfilled") {
        merged.push(...r.value.entries);
        statusParts.push(`${country}: EKS ${r.value.has_aws ? "sí" : "no"} · GKE ${r.value.has_gcp ? "sí" : "no"}`);
      } else {
        errs.push(`${country}: ${parseErr(r.reason)}`);
      }
    });

    merged.sort((a, b) => {
      if (a.sort_key !== b.sort_key) return a.sort_key - b.sort_key;
      if (a.pais !== b.pais) return a.pais.localeCompare(b.pais);
      if (a.proveedor !== b.proveedor) return a.proveedor.localeCompare(b.proveedor);
      return a.account_name.localeCompare(b.account_name);
    });

    setEntries(merged);
    setStatus(statusParts.join(" · "));
    setLoadErrors(errs);
    setFetching(false);
  }

  const paisOptions = useMemo(() => Array.from(new Set(entries.map((e) => e.pais))).sort(), [entries]);
  const proveedorOptions = useMemo(() => Array.from(new Set(entries.map((e) => e.proveedor))).sort(), [entries]);

  const filteredRows: SelectableRow[] = useMemo(() => {
    const q = query.trim().toLowerCase();
    const filtered = entries.filter((e) => {
      if (paisFilter !== "todos" && e.pais !== paisFilter) return false;
      if (proveedorFilter !== "todos" && e.proveedor !== proveedorFilter) return false;
      if (estadoFilter !== "todos" && String(e.sort_key) !== estadoFilter) return false;
      if (!q) return true;
      return (
        e.account_name.toLowerCase().includes(q) ||
        e.cluster.toLowerCase().includes(q) ||
        e.role.toLowerCase().includes(q)
      );
    });
    return filtered.map((e) => ({
      pais: e.pais,
      proveedor: e.proveedor,
      account_name: e.account_name,
      role: e.role,
      cluster: e.cluster,
      permisos_ok: e.permisos_ok,
      permisos_total: e.permisos_total,
      sort_key: e.sort_key,
      key: e.context_alias,
    }));
  }, [entries, query, paisFilter, proveedorFilter, estadoFilter]);

  async function loadSelected(mode: "reemplazar" | "agregar") {
    const selections = entries.filter((e) => selectedKeys.has(e.context_alias));
    if (selections.length === 0) return;
    setLoading(true);
    setLoadError("");
    setLoadResult(null);
    try {
      setLoadResult(await invoke<LoadSummary>("load_selected_clusters", { selections, mode }));
    } catch (err) {
      setLoadError(parseErr(err));
    } finally {
      setLoading(false);
    }
  }

  if (fetching) {
    return <div className="text-sm" style={{ color: "var(--text-muted)" }}>Buscando inventarios…</div>;
  }

  return (
    <div className="h-full flex flex-col gap-3">
      {/* ── Fijo arriba: estado, búsqueda, filtros ── */}
      <div className="shrink-0 space-y-3">
        {status && <SuccessNotice>✓ {entries.length} cluster(s) encontrados — {status}</SuccessNotice>}
        {loadErrors.length > 0 && (
          <div className="space-y-1">
            {loadErrors.map((e, i) => (
              <MetaLine key={i}>{e}</MetaLine>
            ))}
          </div>
        )}

        {entries.length === 0 ? (
          <div className="text-sm" style={{ color: "var(--text-muted)" }}>
            No se encontró ningún inventario. Corre primero un recorrido completo en la pestaña
            "Actualizar".
          </div>
        ) : (
          <>
            <div className="flex flex-wrap gap-2">
              <div className="relative flex-1 min-w-[200px]">
                <svg
                  className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5"
                  viewBox="0 0 16 16"
                  fill="none"
                  style={{ color: "var(--text-faint)" }}
                >
                  <circle cx="7" cy="7" r="5" stroke="currentColor" strokeWidth="1.5" />
                  <path d="M11 11l3.5 3.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                </svg>
                <input
                  type="text"
                  placeholder="Buscar por cuenta, cluster o rol…"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  className="w-full pl-8 pr-3 py-2 rounded-md text-sm outline-none transition-colors"
                  style={{
                    background: "var(--surface-raised)",
                    border: "1px solid var(--border-strong)",
                    color: "var(--text)",
                  }}
                  onFocus={(e) => (e.currentTarget.style.borderColor = "var(--accent)")}
                  onBlur={(e) => (e.currentTarget.style.borderColor = "var(--border-strong)")}
                />
              </div>
              <Dropdown
                value={paisFilter}
                onChange={setPaisFilter}
                options={[
                  { value: "todos", label: "País: todos" },
                  ...paisOptions.map((p) => ({ value: p, label: <span className="capitalize">{p}</span> })),
                ]}
              />
              <Dropdown
                value={proveedorFilter}
                onChange={setProveedorFilter}
                options={[
                  { value: "todos", label: "Proveedor: todos" },
                  ...proveedorOptions.map((p) => ({ value: p, label: p.toUpperCase() })),
                ]}
              />
              <Dropdown
                value={estadoFilter}
                onChange={setEstadoFilter}
                options={[
                  { value: "todos", label: "Estado: todos" },
                  { value: "0", label: <span style={{ color: "var(--ok)" }}>● Completo</span> },
                  { value: "1", label: <span style={{ color: "var(--partial)" }}>◐ Parcial</span> },
                  { value: "2", label: <span style={{ color: "var(--fail)" }}>○ Sin acceso</span> },
                ]}
              />
            </div>

            {filteredRows.length !== entries.length && (
              <MetaLine>
                Mostrando {filteredRows.length} de {entries.length}
              </MetaLine>
            )}
          </>
        )}

        {loading && <div className="text-xs" style={{ color: "var(--text-muted)" }}>Cargando clusters seleccionados…</div>}
        {loadError && <ErrorNotice>{loadError}</ErrorNotice>}
      </div>

      {/* ── Medio: tabla + resultado de carga, con scroll propio ── */}
      {entries.length > 0 && (
        <div className="flex-1 min-h-0 overflow-y-auto space-y-4">
          <ClusterSelectTable
            rows={filteredRows}
            selectedKeys={selectedKeys}
            onChange={setSelectedKeys}
            showPais
            highlightQuery={query}
          />

          {loadResult && (
            <div className="space-y-2">
              <SuccessNotice>
                ✓ {loadResult.cargados} cargado(s) · {loadResult.omitidos} omitido(s) · {loadResult.con_errores}{" "}
                con error
                {loadResult.contextos_eliminados > 0 &&
                  ` · ${loadResult.contextos_eliminados} contexto(s) anterior(es) eliminado(s)`}
              </SuccessNotice>
              <div className="rounded-md border overflow-hidden" style={{ borderColor: "var(--border)" }}>
                <table className="w-full text-left text-xs">
                  <thead style={{ background: "var(--surface-raised)" }}>
                    <tr>
                      <th className="px-2 py-2 font-medium" style={{ color: "var(--text-muted)" }}>Cluster</th>
                      <th className="px-2 py-2 font-medium" style={{ color: "var(--text-muted)" }}>Contexto</th>
                      <th className="px-2 py-2 font-medium" style={{ color: "var(--text-muted)" }}>Estado</th>
                      <th className="px-2 py-2 font-medium" style={{ color: "var(--text-muted)" }}>Detalle</th>
                    </tr>
                  </thead>
                  <tbody>
                    {loadResult.items.map((it, i) => (
                      <tr key={i} style={{ borderTop: "1px solid var(--border)" }}>
                        <td className="px-2 py-2" style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>
                          {it.cluster}
                        </td>
                        <td className="px-2 py-2" style={{ color: "var(--text-faint)", fontFamily: "var(--font-mono)" }}>
                          {it.context_alias}
                        </td>
                        <td
                          className="px-2 py-2"
                          style={{
                            color:
                              it.estado === "cargado"
                                ? "var(--ok)"
                                : it.estado === "omitido"
                                ? "var(--partial)"
                                : "var(--fail)",
                          }}
                        >
                          {it.estado}
                        </td>
                        <td className="px-2 py-2" style={{ color: "var(--text-faint)" }}>{it.detalle ?? ""}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}

      {/* ── Fijo abajo: barra de acción, pegada directamente a la consola,
          sin ningún hueco — es el último elemento del flex-col, no un
          "sticky" que depende de cuánto se haya scrolleado. ── */}
      {entries.length > 0 && (
        <div
          className="shrink-0 px-3 py-2.5 rounded-md border"
          style={{ background: "var(--surface-raised)", borderColor: "var(--border-strong)" }}
        >
          {selectedKeys.size > 0 ? (
            <div className="space-y-2">
              <span className="text-xs" style={{ color: "var(--text-muted)" }}>
                {selectedKeys.size} seleccionado(s)
              </span>
              <div className="flex gap-2">
                <Button
                  variant="danger-outline"
                  className="flex-1"
                  onClick={() => loadSelected("reemplazar")}
                  disabled={loading}
                >
                  Reemplazar contextos actuales
                </Button>
                <Button
                  variant="secondary"
                  className="flex-1"
                  onClick={() => loadSelected("agregar")}
                  disabled={loading}
                >
                  Agregar a los actuales
                </Button>
              </div>
            </div>
          ) : (
            <span className="text-xs" style={{ color: "var(--text-faint)" }}>
              Selecciona clusters en la tabla para cargarlos.
            </span>
          )}
        </div>
      )}
    </div>
  );
}
