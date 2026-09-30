import { useEffect, useRef, useState, type KeyboardEvent } from "react";

export interface SelectableRow {
  pais: string;
  proveedor: string;
  account_name: string;
  role: string;
  cluster: string;
  permisos_ok: number;
  permisos_total: number;
  sort_key: number;
  // Clave estable para la selección — necesaria porque con buscador los
  // índices cambian al filtrar y perderían la selección hecha antes.
  key: string;
}

// Resalta la parte de "text" que coincide con "query" (sin distinguir
// mayúsculas). Si no hay query o no hay coincidencia, devuelve el texto
// tal cual — sin envolver nada de más.
function highlight(text: string, query: string) {
  if (!query.trim()) return text;
  const idx = text.toLowerCase().indexOf(query.toLowerCase());
  if (idx === -1) return text;
  const before = text.slice(0, idx);
  const match = text.slice(idx, idx + query.length);
  const after = text.slice(idx + query.length);
  return (
    <>
      {before}
      <span style={{ background: "var(--accent-dim)", color: "var(--accent)", borderRadius: 2 }}>{match}</span>
      {after}
    </>
  );
}

function statusIcon(sortKey: number): string {
  if (sortKey === 0) return "●";
  if (sortKey === 1) return "◐";
  return "○";
}
function statusColorVar(sortKey: number): string {
  if (sortKey === 0) return "var(--ok)";
  if (sortKey === 1) return "var(--partial)";
  return "var(--fail)";
}

// Mismos anchos para el encabezado y el cuerpo — al ser 2 <table>
// independientes (no una sola con thead+tbody), no hay una fila
// compartida que los sincronice automáticamente. Un <colgroup> idéntico
// en ambas tablas es lo que los mantiene alineados.
function ColGroup({ showPais }: { showPais: boolean }) {
  return (
    <colgroup>
      <col style={{ width: 40 }} />
      <col style={{ width: 28 }} />
      {showPais && <col style={{ width: 70 }} />}
      <col style={{ width: 90 }} />
      <col style={{ width: "22%" }} />
      <col style={{ width: "20%" }} />
      <col style={{ width: "28%" }} />
      <col style={{ width: 70 }} />
    </colgroup>
  );
}

interface ClusterSelectTableProps {
  rows: SelectableRow[];
  selectedKeys: Set<string>;
  onChange: (next: Set<string>) => void;
  // La pestaña "Cargar clusters" mezcla Chile+Perú en una sola tabla y
  // necesita la columna País; dentro de "Actualizar" cada sección ya
  // está acotada a un solo país, así que ahí no hace falta.
  showPais?: boolean;
  // Texto de búsqueda activo — resalta la coincidencia en cuenta/rol/cluster.
  highlightQuery?: string;
}

// Selección múltiple accesible: roving tabindex (una sola fila enfocable
// a la vez), ↑/↓ mueven el foco, Espacio marca/desmarca, Home/End saltan
// a los extremos, y clic en cualquier parte de la fila hace lo mismo que
// Espacio — sin que el clic en el checkbox y el clic en la fila se
// disparen dos veces y se cancelen entre sí.
//
// La selección se guarda por clave (no por índice) para que sobreviva
// al filtrado por búsqueda — "seleccionar todos" solo afecta las filas
// visibles en este momento, sin tocar selecciones de filas ocultas por
// el filtro.
//
// Encabezado y cuerpo son 2 <table> separadas (no una con thead
// "sticky") — el sticky en una tabla cuyas filas cambian dinámicamente
// (al filtrar) es un caso de renderizado inconsistente entre
// navegadores, en particular WebKit, y ni aplicándolo a cada celda ni
// fijando el ancho de columnas lo resolvió del todo. Separando
// físicamente el encabezado del área que scrollea, no hace falta
// "sticky" en absoluto: el encabezado simplemente nunca se mueve.
export default function ClusterSelectTable({
  rows,
  selectedKeys,
  onChange,
  showPais = false,
  highlightQuery = "",
}: ClusterSelectTableProps) {
  const [focusedIndex, setFocusedIndex] = useState(0);
  const rowRefs = useRef<(HTMLTableRowElement | null)[]>([]);

  useEffect(() => {
    if (focusedIndex >= rows.length) {
      setFocusedIndex(Math.max(0, rows.length - 1));
    }
  }, [rows.length, focusedIndex]);

  function toggle(key: string) {
    const next = new Set(selectedKeys);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    onChange(next);
  }

  const visibleKeys = rows.map((r) => r.key);
  const allVisibleSelected = visibleKeys.length > 0 && visibleKeys.every((k) => selectedKeys.has(k));

  function toggleAll() {
    const next = new Set(selectedKeys);
    if (allVisibleSelected) {
      visibleKeys.forEach((k) => next.delete(k));
    } else {
      visibleKeys.forEach((k) => next.add(k));
    }
    onChange(next);
  }

  function focusRow(idx: number) {
    setFocusedIndex(idx);
    rowRefs.current[idx]?.focus();
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTableRowElement>, idx: number) {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        focusRow(Math.min(idx + 1, rows.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        focusRow(Math.max(idx - 1, 0));
        break;
      case "Home":
        e.preventDefault();
        focusRow(0);
        break;
      case "End":
        e.preventDefault();
        focusRow(rows.length - 1);
        break;
      case " ":
      case "Spacebar":
        e.preventDefault();
        toggle(rows[idx].key);
        break;
    }
  }

  const checkboxStyle = { width: 18, height: 18, accentColor: "var(--accent)" } as const;
  const thStyle = { background: "var(--surface-raised)", color: "var(--text-muted)" } as const;

  return (
    <div className="rounded-md border h-full flex flex-col" style={{ borderColor: "var(--border)" }}>
      {/* Encabezado — tabla propia, fuera del área con scroll. No necesita
          sticky porque estructuralmente nunca se mueve. El padding-right
          en el div (no en la tabla — con border-collapse, el padding
          puesto directo en <table> se ignora por especificación CSS)
          compensa el ancho del scrollbar del cuerpo (8px, definido en
          index.css), para que las columnas queden alineadas con el
          borde derecho real del cuerpo. */}
      <div className="shrink-0" style={{ paddingRight: 8 }}>
        <table className="w-full text-left text-xs border-collapse" style={{ tableLayout: "fixed" }}>
          <ColGroup showPais={showPais} />
          <thead>
            <tr>
              <th className="px-3 py-2" style={{ ...thStyle, borderLeft: "3px solid transparent" }}>
                <input
                  type="checkbox"
                  checked={allVisibleSelected}
                  onChange={toggleAll}
                  style={checkboxStyle}
                  aria-label="Seleccionar todos"
                />
              </th>
              <th className="px-2 py-2" style={thStyle}></th>
              {showPais && (
                <th className="px-2 py-2 font-medium" style={thStyle}>
                  País
                </th>
              )}
              <th className="px-2 py-2 font-medium" style={thStyle}>
                Proveedor
              </th>
              <th className="px-2 py-2 font-medium" style={thStyle}>
                Cuenta
              </th>
              <th className="px-2 py-2 font-medium" style={thStyle}>
                Rol
              </th>
              <th className="px-2 py-2 font-medium" style={thStyle}>
                Cluster
              </th>
              <th className="px-2 py-2 font-medium" style={thStyle}>
                Permisos
              </th>
            </tr>
          </thead>
        </table>
      </div>

      {/* Cuerpo — tabla separada, con su propio scroll. Mismo colgroup que
          el encabezado, así las columnas quedan siempre alineadas sin
          depender de que ambas tablas compartan una sola fila. */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        <table className="w-full text-left text-xs border-collapse" style={{ tableLayout: "fixed" }}>
          <ColGroup showPais={showPais} />
          <tbody>
            {rows.map((r, i) => {
              const isSelected = selectedKeys.has(r.key);
              const isFocused = i === focusedIndex;
              return (
                <tr
                  key={r.key}
                  ref={(el) => {
                    rowRefs.current[i] = el;
                  }}
                  tabIndex={isFocused ? 0 : -1}
                  role="row"
                  aria-selected={isSelected}
                  onFocus={() => setFocusedIndex(i)}
                  onKeyDown={(e) => handleKeyDown(e, i)}
                  onClick={() => {
                    focusRow(i);
                    toggle(r.key);
                  }}
                  className="cursor-pointer outline-none select-none"
                  style={{
                    borderTop: "1px solid var(--border)",
                    background: isSelected ? "var(--accent-dim)" : isFocused ? "var(--surface-raised)" : "transparent",
                  }}
                >
                  <td
                    className="px-3 py-2"
                    onClick={(e) => e.stopPropagation()}
                    style={{
                      // El foco se marca acá, no con box-shadow en <tr> —
                      // ese no se pinta de forma confiable en tablas con
                      // border-collapse (cada <td> pinta su propio fondo
                      // encima). El borde en la celda sí renderiza siempre.
                      borderLeft: isFocused ? "3px solid var(--accent)" : "3px solid transparent",
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={isSelected}
                      onChange={() => {
                        focusRow(i);
                        toggle(r.key);
                      }}
                      onMouseDown={(e) => e.preventDefault()}
                      style={checkboxStyle}
                      tabIndex={-1}
                    />
                  </td>
                  <td className="px-2 py-2" style={{ color: statusColorVar(r.sort_key) }}>
                    {statusIcon(r.sort_key)}
                  </td>
                  {showPais && (
                    <td className="px-2 py-2 capitalize" style={{ color: "var(--text-muted)" }}>
                      {r.pais}
                    </td>
                  )}
                  <td className="px-2 py-2 uppercase" style={{ color: "var(--text-muted)" }}>
                    {r.proveedor}
                  </td>
                  <td className="px-2 py-2" style={{ color: "var(--text)" }}>
                    {highlight(r.account_name, highlightQuery)}
                  </td>
                  <td className="px-2 py-2" style={{ color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
                    {highlight(r.role, highlightQuery)}
                  </td>
                  <td className="px-2 py-2" style={{ color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
                    {highlight(r.cluster, highlightQuery)}
                  </td>
                  <td className="px-2 py-2" style={{ color: "var(--text-muted)" }}>
                    {r.permisos_ok}/{r.permisos_total}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
