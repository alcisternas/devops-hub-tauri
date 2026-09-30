import { create } from "zustand";

// Store global. Por ahora cada módulo es independiente (sin datos compartidos reales),
// pero la estructura ya está pensada para el día que necesiten encadenamiento:
// ej. el nombre de repo creado en Bitbucket, pre-llenado automáticamente en CI/CD.
//
// Para agregar un dato compartido nuevo: se agrega el campo acá + su función "set".
// Ningún módulo existente se rompe por agregar campos nuevos.

interface DevOpsHubState {
  // Ejemplo de dato que podría compartirse a futuro entre Bitbucket y CI/CD:
  lastCreatedRepo: string | null;
  setLastCreatedRepo: (repo: string | null) => void;
}

export const useDevOpsStore = create<DevOpsHubState>((set) => ({
  lastCreatedRepo: null,
  setLastCreatedRepo: (repo) => set({ lastCreatedRepo: repo }),
}));
