// Imagen de evidencia que se adjunta al ticket: una tarjeta por repositorio
// con sus ramas, y nada más. Se dibuja con canvas en la propia interfaz: el
// webview ya trae sus fuentes y evita sumar librerías de imagen al backend.

import type { EvidenciaRepo } from "./shared";

const COLORES = {
  fondo: "#f4f5f7",
  tarjeta: "#ffffff",
  borde: "#dfe1e6",
  titulo: "#0052cc",
  pildora: "#e9f2ff",
  textoPildora: "#172b4d",
};

const FUENTE = 'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';

function rectRedondeado(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  // arcTo en vez de ctx.roundRect: este último no existe en los WebKit más
  // viejos que usa Tauri en Linux.
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

// Devuelve el PNG en base64, sin el prefijo "data:image/png;base64,".
export function dibujarEvidenciaPng(evidencia: EvidenciaRepo[], escala = 2): string {
  if (evidencia.length === 0) throw new Error("No hay repositorios que dibujar.");

  const k = escala;
  const lienzo = document.createElement("canvas");
  const ctx = lienzo.getContext("2d");
  if (!ctx) throw new Error("No se pudo crear el lienzo para la imagen de evidencia.");

  const fuenteTitulo = `bold ${19 * k}px ${FUENTE}`;
  const fuentePildora = `${15 * k}px ${FUENTE}`;
  const pad = 22 * k;
  const hueco = 18 * k;
  const padX = 13 * k;
  const padY = 7 * k;
  const huecoPildora = 10 * k;
  const margen = 24 * k;
  const altoPildora = 15 * k + 2 * padY;

  const anchoTitulo = (t: string) => {
    ctx.font = fuenteTitulo;
    return ctx.measureText(t).width;
  };
  const anchoPildora = (t: string) => {
    ctx.font = fuentePildora;
    return ctx.measureText(t).width + 2 * padX;
  };

  const anchoInterior = Math.max(
    ...evidencia.map((e) => anchoTitulo(e.nombre)),
    ...evidencia.map(
      (e) =>
        e.ramas.reduce((suma, r) => suma + anchoPildora(r), 0) +
        huecoPildora * Math.max(0, e.ramas.length - 1),
    ),
  );
  const anchoTarjeta = Math.ceil(anchoInterior + 2 * pad);
  const altoTarjeta = pad + 19 * k + 14 * k + altoPildora + pad;
  const columnas = evidencia.length === 1 ? 1 : 2;
  const filas = Math.ceil(evidencia.length / columnas);

  // Cambiar el tamaño del lienzo reinicia su estado: por eso se mide antes y
  // se vuelve a fijar la fuente al dibujar.
  lienzo.width = margen * 2 + columnas * anchoTarjeta + (columnas - 1) * hueco;
  lienzo.height = margen * 2 + filas * altoTarjeta + (filas - 1) * hueco;

  ctx.fillStyle = COLORES.fondo;
  ctx.fillRect(0, 0, lienzo.width, lienzo.height);

  evidencia.forEach((e, i) => {
    const x0 = margen + (i % columnas) * (anchoTarjeta + hueco);
    const y0 = margen + Math.floor(i / columnas) * (altoTarjeta + hueco);

    rectRedondeado(ctx, x0, y0, anchoTarjeta, altoTarjeta, 10 * k);
    ctx.fillStyle = COLORES.tarjeta;
    ctx.fill();
    ctx.lineWidth = k;
    ctx.strokeStyle = COLORES.borde;
    ctx.stroke();

    ctx.font = fuenteTitulo;
    ctx.fillStyle = COLORES.titulo;
    ctx.textAlign = "left";
    ctx.textBaseline = "top";
    ctx.fillText(e.nombre, x0 + pad, y0 + pad);

    let x = x0 + pad;
    const y = y0 + pad + 19 * k + 14 * k;
    for (const rama of e.ramas) {
      const w = anchoPildora(rama);
      rectRedondeado(ctx, x, y, w, altoPildora, altoPildora / 2);
      ctx.fillStyle = COLORES.pildora;
      ctx.fill();

      ctx.font = fuentePildora;
      ctx.fillStyle = COLORES.textoPildora;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(rama, x + w / 2, y + altoPildora / 2);
      x += w + huecoPildora;
    }
  });

  return lienzo.toDataURL("image/png").split(",")[1];
}
