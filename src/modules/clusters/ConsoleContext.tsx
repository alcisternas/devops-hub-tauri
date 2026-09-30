import { createContext, useContext, useState, type ReactNode } from "react";

interface ConsoleContextValue {
  resetSignal: number;
  bump: () => void;
}

const ConsoleContext = createContext<ConsoleContextValue | null>(null);

export function ConsoleProvider({ children }: { children: ReactNode }) {
  const [resetSignal, setResetSignal] = useState(0);
  const bump = () => setResetSignal((n) => n + 1);
  return <ConsoleContext.Provider value={{ resetSignal, bump }}>{children}</ConsoleContext.Provider>;
}

// Cualquier sección llama a bump() al arrancar una operación nueva — limpia
// la consola global para que solo muestre lo que está pasando ahora mismo,
// sin importar qué otra sección se usó antes.
export function useConsole() {
  const ctx = useContext(ConsoleContext);
  if (!ctx) throw new Error("useConsole debe usarse dentro de <ConsoleProvider>");
  return ctx;
}
