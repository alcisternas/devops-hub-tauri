import type { ReactNode } from "react";

// Campos del formulario de CI/CD. Son 34 campos repartidos en cinco
// secciones, así que tenerlos acá evita repetir la misma combinación de
// etiqueta, input y pista treinta veces.
//
// Todo el color sale de las variables de diseño del hub: ningún color
// Tailwind crudo. El borde en foco usa el acento, igual que el Dropdown.

export function Hint({ children }: { children: ReactNode }) {
  return (
    <div className="text-xs mt-1 leading-relaxed" style={{ color: "var(--text-faint)" }}>
      {children}
    </div>
  );
}

interface TextFieldProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  hint?: ReactNode;
  nota?: string;
  requerido?: boolean;
  mono?: boolean;
  // Mayúsculas automáticas, para el Project Key de Bitbucket.
  mayusculas?: boolean;
}

export function TextField({
  label,
  value,
  onChange,
  placeholder,
  hint,
  nota,
  requerido,
  mono,
  mayusculas,
}: TextFieldProps) {
  return (
    <div>
      <label className="block text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
        {label}
        {requerido && <span style={{ color: "var(--fail)" }}> *</span>}
        {nota && <span style={{ color: "var(--text-faint)" }}> — {nota}</span>}
      </label>
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(mayusculas ? e.target.value.toUpperCase() : e.target.value)}
        className="w-full px-3 py-2 rounded-md text-sm outline-none focus:border-[var(--accent)]"
        style={{
          background: "var(--surface-raised)",
          border: "1px solid var(--border-strong)",
          color: "var(--text)",
          fontFamily: mono ? "var(--font-mono)" : undefined,
        }}
      />
      {hint && <Hint>{hint}</Hint>}
    </div>
  );
}

interface TextAreaFieldProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  rows?: number;
  nota?: string;
  requerido?: boolean;
}

export function TextAreaField({
  label,
  value,
  onChange,
  placeholder,
  rows = 7,
  nota,
  requerido,
}: TextAreaFieldProps) {
  return (
    <div>
      <label className="block text-xs mb-1.5" style={{ color: "var(--text-muted)" }}>
        {label}
        {requerido && <span style={{ color: "var(--fail)" }}> *</span>}
        {nota && <span style={{ color: "var(--text-faint)" }}> — {nota}</span>}
      </label>
      <textarea
        value={value}
        rows={rows}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className="w-full px-3 py-2 rounded-md text-sm outline-none focus:border-[var(--accent)]"
        style={{
          background: "var(--surface-raised)",
          border: "1px solid var(--border-strong)",
          color: "var(--text)",
          fontFamily: "var(--font-mono)",
          resize: "vertical",
        }}
      />
    </div>
  );
}

export function CheckRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-sm cursor-pointer" style={{ color: "var(--text)" }}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}

// Encabezado de un grupo dentro de una sección (por ejemplo cada ambiente
// en el bloque de Apigee). Sentence case, no mayúsculas.
export function GroupLabel({ children }: { children: ReactNode }) {
  return (
    <div className="text-xs font-medium mb-2" style={{ color: "var(--text-muted)" }}>
      {children}
    </div>
  );
}

export function Grid({ cols = 2, children }: { cols?: number; children: ReactNode }) {
  return (
    <div className="grid gap-4" style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}>
      {children}
    </div>
  );
}
