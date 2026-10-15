import { useRef, useState, type ReactNode } from "react";

// Estado de la sección, mostrado a la derecha del encabezado. "pendiente"
// no es un error: es lo normal en una sección que todavía no se llenó.
export type EstadoSeccion = "completa" | "pendiente" | "info";

interface CollapsibleSectionProps {
  title: string;
  subtitle?: string;
  defaultOpen?: boolean;
  children: ReactNode;
  // Número de orden, cuando la sección forma parte de una secuencia.
  index?: number;
  // Texto corto de estado y su tono.
  estado?: EstadoSeccion;
  estadoTexto?: string;
  // Se llama la primera vez que la sección se abre. Lo usa el formulario
  // para distinguir "sin revisar" de "revisada": que los campos estén
  // completos con sugerencias no significa que alguien los haya mirado.
  onPrimeraApertura?: () => void;
}

const COLOR_ESTADO: Record<EstadoSeccion, string> = {
  completa: "var(--ok)",
  pendiente: "var(--partial)",
  info: "var(--text-faint)",
};

export default function CollapsibleSection({
  title,
  subtitle,
  defaultOpen = false,
  children,
  index,
  estado,
  estadoTexto,
  onPrimeraApertura,
}: CollapsibleSectionProps) {
  const [open, setOpen] = useState(defaultOpen);
  const [yaAbierta, setYaAbierta] = useState(defaultOpen);
  const caja = useRef<HTMLDivElement>(null);

  // Al contraer desde el pie, el encabezado queda a la vista: si no, el
  // contenido de abajo sube de golpe y se pierde la referencia de dónde
  // estaba uno.
  function contraerDesdeElPie() {
    setOpen(false);
    caja.current?.scrollIntoView({ block: "nearest" });
  }

  return (
    // Sin overflow-hidden: la lista del Dropdown se dibuja con
    // posicionamiento absoluto y quedaba cortada en el borde de la caja
    // cuando el campo estaba cerca del final de la sección. El redondeado
    // que ese overflow resolvía se compensa en el encabezado y en el
    // contenido.
    <div
      ref={caja}
      className="rounded-lg border"
      style={{
        borderColor: "var(--border)",
        background: "var(--surface)",
        // Barra de acento en la sección abierta, para saber de un vistazo
        // dónde empieza y dónde termina cuando hay varias seguidas. Cerrada
        // lleva la misma barra en transparente: si no, abrirla correría
        // todo el contenido 3 píxeles. Mismo recurso que usa la fila
        // enfocada de la tabla de clusters.
        borderLeft: open ? "3px solid var(--accent)" : "3px solid transparent",
      }}
    >
      <button
        onClick={() => {
          setOpen((o) => !o);
          if (!yaAbierta) {
            setYaAbierta(true);
            onPrimeraApertura?.();
          }
        }}
        className="w-full flex items-center justify-between px-4 py-3 text-left gap-4 rounded-t-lg"
      >
        <div className="flex items-baseline gap-2 min-w-0">
          {index !== undefined && (
            <span
              className="text-sm font-semibold shrink-0"
              style={{ color: open ? "var(--accent)" : "var(--text-faint)" }}
            >
              {index}
            </span>
          )}
          <div className="min-w-0">
            <div className="text-sm font-semibold" style={{ color: "var(--text)" }}>
              {title}
            </div>
            {subtitle && (
              <div className="text-xs mt-0.5" style={{ color: "var(--text-muted)" }}>
                {subtitle}
              </div>
            )}
          </div>
        </div>

        <div className="flex items-center gap-3 shrink-0">
          {estadoTexto && (
            <span className="text-xs" style={{ color: COLOR_ESTADO[estado || "info"] }}>
              {estadoTexto}
            </span>
          )}
          <svg
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
            style={{
              color: "var(--text-muted)",
              transform: open ? "rotate(180deg)" : "rotate(0deg)",
              transition: "transform 0.15s ease",
              flexShrink: 0,
            }}
          >
            <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </div>
      </button>
      {/* display, no desmontaje condicional — contraer no debe destruir el
          estado de lo que hay adentro (descubrimientos, resultados, etc.),
          igual que el fix de navegación entre módulos en App.tsx. */}
      <div
        className="px-4 pb-4 border-t rounded-b-lg"
        style={{ borderColor: "var(--border)", display: open ? "block" : "none" }}
      >
        <div className="pt-4">{children}</div>
        <div className="pt-4 flex justify-end">
          <button
            onClick={contraerDesdeElPie}
            className="text-xs px-2.5 py-1 rounded-md"
            style={{ color: "var(--text-muted)", border: "1px solid var(--border-strong)" }}
          >
            Contraer sección
          </button>
        </div>
      </div>
    </div>
  );
}
