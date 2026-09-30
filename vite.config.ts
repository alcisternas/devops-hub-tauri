import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Configuración de Vite ajustada a los requisitos de Tauri:
// puerto fijo 1420 y "strictPort" para que falle en vez de saltar a otro puerto
// (Tauri espera exactamente ese puerto durante el desarrollo).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
