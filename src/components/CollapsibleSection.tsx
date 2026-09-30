import { useState, type ReactNode } from "react";

interface CollapsibleSectionProps {
  title: string;
  subtitle?: string;
  defaultOpen?: boolean;
  children: ReactNode;
}

export default function CollapsibleSection({
  title,
  subtitle,
  defaultOpen = false,
  children,
}: CollapsibleSectionProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div
      className="rounded-lg border overflow-hidden"
      style={{ borderColor: "var(--border)", background: "var(--surface)" }}
    >
      <button
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center justify-between px-4 py-3 text-left"
      >
        <div>
          <div className="text-sm font-semibold" style={{ color: "var(--text)" }}>
            {title}
          </div>
          {subtitle && (
            <div className="text-xs mt-0.5" style={{ color: "var(--text-muted)" }}>
              {subtitle}
            </div>
          )}
        </div>
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
      </button>
      {/* display, no desmontaje condicional — contraer no debe destruir el
          estado de lo que hay adentro (descubrimientos, resultados, etc.),
          igual que el fix de navegación entre módulos en App.tsx. */}
      <div className="px-4 pb-4 border-t" style={{ borderColor: "var(--border)", display: open ? "block" : "none" }}>
        <div className="pt-4">{children}</div>
      </div>
    </div>
  );
}
