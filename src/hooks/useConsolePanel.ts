import { useCallback, useEffect, useState } from "react";
import { useResizablePanel } from "./useResizablePanel";

// Franja de la consola: altura arrastrable (lo que ya hacía
// useResizablePanel) más la posibilidad de contraerla a una sola línea.
//
// El colapso se agregó porque el formulario de CI/CD es largo y la consola
// ahí casi no se usa: solo informa la actualización de subredes. Vive en
// este hook y no dentro de GlobalConsole porque el contenedor necesita
// saberlo para esconder la barra de arrastre — arrastrar algo que no se
// puede agrandar es peor que no tener la barra.
//
// Las tres claves de almacenamiento son compartidas por los módulos, así
// que la preferencia es una sola para todo el hub. Como los tres módulos
// están montados a la vez, cada uno tiene su propia copia en memoria del
// valor; el evento propio los mantiene sincronizados. Sin eso, contraer la
// consola en un módulo dejaba a los otros mostrándola abierta hasta
// reiniciar la aplicación.
const KEY_COLLAPSED = "devops-hub-console-collapsed";
const EVENTO = "devops-hub-console-collapse";

// Alto de la barra de título de la consola. Contraída se ve solo eso: el
// nombre, el estado de la última línea y el botón para volver a abrirla.
export const CONSOLE_COLLAPSED_HEIGHT = 37;

function leerColapsada(): boolean {
  return localStorage.getItem(KEY_COLLAPSED) === "1";
}

export function useConsolePanel(defaultHeight = 320) {
  const panel = useResizablePanel("devops-hub-console-height", defaultHeight);
  const [collapsed, setCollapsed] = useState(leerColapsada);

  useEffect(() => {
    function onCambio() {
      setCollapsed(leerColapsada());
    }
    window.addEventListener(EVENTO, onCambio);
    return () => window.removeEventListener(EVENTO, onCambio);
  }, []);

  const toggleCollapsed = useCallback(() => {
    const siguiente = !leerColapsada();
    localStorage.setItem(KEY_COLLAPSED, siguiente ? "1" : "0");
    // El evento de almacenamiento del navegador solo avisa a OTRAS
    // pestañas, nunca a la propia, así que acá hace falta uno propio.
    window.dispatchEvent(new Event(EVENTO));
  }, []);

  return {
    height: collapsed ? CONSOLE_COLLAPSED_HEIGHT : panel.height,
    onDragStart: panel.onDragStart,
    collapsed,
    toggleCollapsed,
  };
}
