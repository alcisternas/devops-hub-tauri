import { useCallback, useEffect, useRef, useState } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";

const MIN_HEIGHT = 100;
const MAX_HEIGHT_RATIO = 0.85; // no más del 85% de la ventana, siempre queda algo arriba

export function useResizablePanel(storageKey: string, defaultHeight: number) {
  const [height, setHeight] = useState(() => {
    const saved = localStorage.getItem(storageKey);
    const parsed = saved ? Number(saved) : defaultHeight;
    return Number.isFinite(parsed) && parsed > 0 ? parsed : defaultHeight;
  });
  const draggingRef = useRef(false);

  useEffect(() => {
    localStorage.setItem(storageKey, String(height));
  }, [height, storageKey]);

  const onDragStart = useCallback(
    (e: ReactMouseEvent) => {
      e.preventDefault();
      draggingRef.current = true;
      const startY = e.clientY;
      const startHeight = height;

      function onMove(ev: MouseEvent) {
        if (!draggingRef.current) return;
        // Arrastrar hacia arriba agranda el panel (el panel vive abajo).
        const delta = startY - ev.clientY;
        const maxHeight = window.innerHeight * MAX_HEIGHT_RATIO;
        const next = Math.min(maxHeight, Math.max(MIN_HEIGHT, startHeight + delta));
        setHeight(next);
      }
      function onUp() {
        draggingRef.current = false;
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
      }
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    },
    [height]
  );

  return { height, onDragStart };
}
