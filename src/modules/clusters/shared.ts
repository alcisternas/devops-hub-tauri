// Separa una ruta absoluta en (directorio, nombre de archivo) — para no
// repetir el mismo directorio en cada línea cuando varios archivos
// generados en la misma corrida viven en la misma carpeta.
export function splitPath(fullPath: string): { dir: string; file: string } {
  const idx = Math.max(fullPath.lastIndexOf("/"), fullPath.lastIndexOf("\\"));
  if (idx === -1) return { dir: "", file: fullPath };
  return { dir: fullPath.slice(0, idx), file: fullPath.slice(idx + 1) };
}

// Mismo formato que el bash (ELAPSED_MIN/ELAPSED_SEC): "Xm Ys"
export function formatDuration(totalSeconds: number): string {
  const min = Math.floor(totalSeconds / 60);
  const sec = totalSeconds % 60;
  return `${min}m ${sec}s`;
}

// Error estructurado — el mismo formato que devuelve cualquier comando Rust
// que use RefreshError (etapa + mensaje) en vez de un string plano.
export interface RefreshError {
  etapa: string;
  mensaje: string;
}
