import { useEffect, useRef, useState } from "react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "danger" | "danger-outline";

// Clases, no estilos inline — un style inline nunca puede expresar :hover,
// así que --accent-hover estaba definido en los tokens pero nunca se podía
// aplicar de verdad con el enfoque anterior.
const variantClass: Record<Variant, string> = {
  primary: "bg-[var(--accent)] text-white hover:bg-[var(--accent-hover)]",
  secondary:
    "bg-[var(--surface-raised)] text-[var(--text)] border border-[var(--border-strong)] hover:bg-[var(--border-strong)]",
  danger: "bg-[#7f1d1d] text-white hover:bg-[#991b1b]",
  // Mismo peso visual que "secondary" — el color de alerta va en
  // borde/texto, no en un relleno sólido, para que una acción destructiva
  // no domine la pantalla frente a la alternativa más segura ("agregar").
  "danger-outline":
    "bg-[var(--surface-raised)] text-[var(--fail)] border border-[var(--fail)] hover:bg-[var(--fail-dim)]",
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
}

export function Button({ variant = "primary", className = "", ...props }: ButtonProps) {
  return (
    <button
      {...props}
      className={`px-3.5 py-2 rounded-md text-sm font-medium transition-colors disabled:opacity-40 disabled:cursor-not-allowed ${variantClass[variant]} ${className}`}
    />
  );
}

export interface DropdownOption {
  value: string;
  label: ReactNode;
}

interface DropdownProps {
  value: string;
  onChange: (value: string) => void;
  options: DropdownOption[];
  className?: string;
}

// Desplegable propio, no un <select> nativo — el <select> deja estilizar
// el disparador cerrado, pero la lista abierta la dibuja el sistema
// operativo (en Safari/WebKit, que es lo que usa Tauri en Mac, ignora
// casi cualquier color que se le ponga a las <option>). Acá el panel
// abierto es HTML/CSS normal, así que los colores por opción sí se ven.
export function Dropdown({ value, onChange, options, className = "" }: DropdownProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const selected = options.find((o) => o.value === value);

  useEffect(() => {
    if (!open) return;
    function onDocClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    function onEscape(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onDocClick);
    document.addEventListener("keydown", onEscape);
    return () => {
      document.removeEventListener("mousedown", onDocClick);
      document.removeEventListener("keydown", onEscape);
    };
  }, [open]);

  return (
    <div ref={ref} className={`relative ${className}`}>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center justify-between gap-2 pl-3 pr-2.5 py-2 rounded-md text-sm outline-none cursor-pointer transition-colors"
        style={{
          background: "var(--surface-raised)",
          border: `1px solid ${open ? "var(--accent)" : "var(--border-strong)"}`,
          color: "var(--text)",
        }}
      >
        <span>{selected?.label ?? value}</span>
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          style={{ color: "var(--text-muted)", transform: open ? "rotate(180deg)" : "none", flexShrink: 0 }}
        >
          <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {open && (
        <div
          className="absolute z-20 mt-1 min-w-full rounded-md border overflow-hidden py-1"
          style={{ background: "var(--surface-raised)", borderColor: "var(--border-strong)" }}
        >
          {options.map((o) => (
            <button
              key={o.value}
              type="button"
              onClick={() => {
                onChange(o.value);
                setOpen(false);
              }}
              className="w-full text-left px-3 py-1.5 text-sm whitespace-nowrap transition-colors"
              style={{
                background: o.value === value ? "var(--accent-dim)" : "transparent",
                color: "var(--text)",
              }}
              onMouseEnter={(e) => {
                if (o.value !== value) e.currentTarget.style.background = "var(--border)";
              }}
              onMouseLeave={(e) => {
                if (o.value !== value) e.currentTarget.style.background = "transparent";
              }}
            >
              {o.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export function ErrorNotice({ children }: { children: ReactNode }) {
  return (
    <div
      className="mt-3 p-3 rounded-md text-xs whitespace-pre-wrap"
      style={{ background: "var(--fail-dim)", border: "1px solid #4a1d1d", color: "#fca5a5" }}
    >
      {children}
    </div>
  );
}

export function SuccessNotice({ children }: { children: ReactNode }) {
  return (
    <div className="text-xs" style={{ color: "var(--ok)" }}>
      {children}
    </div>
  );
}

export function WarningNotice({ children }: { children: ReactNode }) {
  return (
    <div className="text-xs" style={{ color: "var(--partial)" }}>
      {children}
    </div>
  );
}

export function MutedList({ items }: { items: string[] }) {
  if (items.length === 0) return null;
  return (
    <div>
      <div className="text-xs mb-1" style={{ color: "var(--partial)" }}>
        Omitidos:
      </div>
      <ul
        className="text-xs space-y-0.5 max-h-40 overflow-auto pl-3 border-l"
        style={{ color: "var(--text-faint)", borderColor: "var(--border)" }}
      >
        {items.map((o, i) => (
          <li key={i}>{o}</li>
        ))}
      </ul>
    </div>
  );
}

export function FieldLabel({ children }: { children: ReactNode }) {
  return (
    <p className="text-xs mb-3" style={{ color: "var(--text-muted)" }}>
      {children}
    </p>
  );
}

export function MetaLine({ children }: { children: ReactNode }) {
  return (
    <div className="text-xs" style={{ color: "var(--text-faint)" }}>
      {children}
    </div>
  );
}
