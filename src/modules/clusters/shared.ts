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
