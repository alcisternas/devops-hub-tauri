import { Button, ErrorNotice, MetaLine, WarningNotice } from "../../components/ui";
import type { EstadoSubredes } from "./shared";

interface Props {
  estado: EstadoSubredes | null;
  cargando: boolean;
  error: string;
  total: number;
  onActualizar: () => void;
}

function fecha(iso: string | null): string {
  if (!iso) return "nunca";
  const d = new Date(iso);
  return isNaN(d.getTime()) ? iso : d.toLocaleString();
}

// Estado del catálogo de subredes y botón para volver a consultarlo.
//
// El módulo abre con lo que haya en caché, o con el catálogo incluido en la
// aplicación si es la primera vez. La consulta en vivo es manual porque
// tarda y puede pedir iniciar sesión en gcloud: hacerla sola al abrir el
// módulo frenaría el formulario sin que nadie la haya pedido.
//
// El detalle de los errores NO se muestra acá: va a la consola global, que
// tiene el espacio para leerlo. En este panel queda solo qué falló.
export default function SubnetsPanel({ estado, cargando, error, total, onActualizar }: Props) {
  const origen = estado?.origen ?? "";

  const descripcion =
    origen === "consulta"
      ? "Consultado en vivo en esta sesión."
      : origen === "cache"
        ? "Desde la caché de este equipo."
        : "Catálogo incluido en la aplicación — este equipo todavía no consultó en vivo.";

  return (
    <div className="space-y-3">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="text-xs" style={{ color: "var(--text-muted)" }}>
            {total} subredes disponibles
          </div>
          <MetaLine>
            {descripcion} Última consulta en vivo: {fecha(estado?.actualizado ?? null)}
          </MetaLine>
          {estado?.cuenta && (
            <MetaLine>
              Se consulta con la cuenta {estado.cuenta}, la misma configurada para GCP en el hub.
            </MetaLine>
          )}
        </div>
        <Button variant="secondary" onClick={onActualizar} disabled={cargando}>
          {cargando ? "Consultando…" : "Actualizar en vivo"}
        </Button>
      </div>

      <div className="text-xs leading-relaxed" style={{ color: "var(--text-faint)" }}>
        La consulta usa gcloud contra los tres proyectos de networking. Si la sesión está vencida,
        se renueva antes de consultar. Sirve cuando el proyecto es nuevo y su subred todavía no
        figura en el listado.
      </div>

      {/* Un proyecto que falla no invalida los otros dos: lo consultado se
          guarda igual y acá se dice cuál quedó sin actualizar. */}
      {estado?.errores && estado.errores.length > 0 && (
        <WarningNotice>
          Sin actualizar: {estado.errores.join(", ")}. El detalle está en la consola.
        </WarningNotice>
      )}

      {error && <ErrorNotice>{error}</ErrorNotice>}
    </div>
  );
}
