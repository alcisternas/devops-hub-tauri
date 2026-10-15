import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ErrorNotice, MetaLine, WarningNotice } from "../../components/ui";
import CredentialsPanel from "../bitbucket/CredentialsPanel";
import ConflictosPanel from "./ConflictosPanel";
import ResultadosPanel from "./ResultadosPanel";
import type { ArchivosPorRepo, Hallazgo } from "./generators";
import type { Accion, RepoArchivos, ResultadoRepo, VerificacionRepo } from "./shared";
import { WORKSPACE, mensajeCommit, parseErr } from "./shared";

interface Props {
  archivos: ArchivosPorRepo;
  huecos: Hallazgo[];
  rama: string;
  ticket: string;
  activo: boolean;
}

type Etapa = "inicio" | "verificado" | "resultados";

// Subida de los archivos generados a Bitbucket.
//
// Tres momentos, y el orden importa: verificar solo lee y deja a la vista
// qué va a pasar; la confirmación muestra el alcance completo; recién el
// tercer botón escribe. Nada se sube sin pasar por los tres.
//
// El panel de credenciales es el mismo del módulo Bitbucket, importado tal
// cual: una sola entrada en el llavero y un solo lugar donde cambiarla.
export default function SubirPanel({ archivos, huecos, rama, ticket, activo }: Props) {
  const [credOk, setCredOk] = useState(false);
  const [etapa, setEtapa] = useState<Etapa>("inicio");
  const [verificaciones, setVerificaciones] = useState<VerificacionRepo[]>([]);
  const [acciones, setAcciones] = useState<Record<string, Accion>>({});
  const [cancelados, setCancelados] = useState<string[]>([]);
  const [resultados, setResultados] = useState<ResultadoRepo[]>([]);
  const [trabajando, setTrabajando] = useState(false);
  const [error, setError] = useState("");

  const repos = Object.keys(archivos);
  const mensaje = mensajeCommit(ticket);

  // Lo que se manda es exactamente lo que muestra la vista previa: ambos
  // leen del mismo mapa de archivos.
  function armarPayload(conAcciones: boolean): RepoArchivos[] {
    return repos
      .filter((repo) => !cancelados.includes(repo))
      .map((repo) => ({
        repo,
        archivos: Object.entries(archivos[repo]).map(([ruta, contenido]) => ({
          ruta,
          contenido,
          accion: conAcciones ? acciones[`${repo}|${ruta}`] || "sobrescribir" : "sobrescribir",
        })),
      }));
  }

  async function verificar() {
    setTrabajando(true);
    setError("");
    try {
      const res = await invoke<VerificacionRepo[]>("cicd_bb_verificar", {
        workspace: WORKSPACE,
        rama,
        repos: armarPayload(false),
      });
      setVerificaciones(res);
      // Por omisión se actualiza: el caso habitual es volver a generar la
      // configuración de un repositorio que ya la tenía. Dejar copias
      // obliga después a borrar el original y renombrar a mano.
      const iniciales: Record<string, Accion> = {};
      res.forEach((v) => v.conflictos.forEach((ruta) => (iniciales[`${v.repo}|${ruta}`] = "sobrescribir")));
      setAcciones(iniciales);
      setEtapa("verificado");
    } catch (e) {
      setError(parseErr(e));
    } finally {
      setTrabajando(false);
    }
  }

  async function subir() {
    setTrabajando(true);
    setError("");
    try {
      const res = await invoke<ResultadoRepo[]>("cicd_bb_subir", {
        workspace: WORKSPACE,
        rama,
        mensaje,
        repos: armarPayload(true),
      });
      setResultados(res);
      setEtapa("resultados");
    } catch (e) {
      setError(parseErr(e));
    } finally {
      setTrabajando(false);
    }
  }

  function aplicarATodos(accion: Accion) {
    const todas: Record<string, Accion> = {};
    verificaciones.forEach((v) => v.conflictos.forEach((ruta) => (todas[`${v.repo}|${ruta}`] = accion)));
    setAcciones(todas);
  }

  if (etapa === "resultados") {
    return (
      <ResultadosPanel
        resultados={resultados}
        onVolver={() => {
          setEtapa("inicio");
          setVerificaciones([]);
          setResultados([]);
          setCancelados([]);
        }}
      />
    );
  }

  const bloqueados = verificaciones.filter((v) => v.error);
  const listos = verificaciones.filter((v) => !v.error && !cancelados.includes(v.repo));
  const totalArchivos = listos.reduce((a, v) => a + Object.keys(archivos[v.repo] || {}).length, 0);

  return (
    <div className="space-y-4">
      <CredentialsPanel workspace={WORKSPACE} onEstadoCambia={setCredOk} activo={activo} />

      <div className="rounded-lg border p-4" style={{ borderColor: "var(--border)", background: "var(--surface)" }}>
        <div className="text-sm mb-1" style={{ color: "var(--text)" }}>
          Subir a Bitbucket
        </div>
        <MetaLine>
          Workspace {WORKSPACE} · rama {rama} · un commit por repositorio
        </MetaLine>
        <MetaLine>Mensaje: {mensaje}</MetaLine>

        {/* Los marcadores sin resolver no bloquean: el desarrollador puede
            completarlos después en el repositorio. Pero conviene saber que
            van a quedar escritos así. */}
        {huecos.length > 0 && (
          <div className="mt-3">
            <WarningNotice>
              {huecos.length} valor{huecos.length === 1 ? "" : "es"} sin resolver se va
              {huecos.length === 1 ? "" : "n"} a subir tal como está
              {huecos.length === 1 ? "" : "n"}, para completar en el repositorio.
            </WarningNotice>
          </div>
        )}

        <div className="mt-3 flex items-center gap-3">
          {etapa === "inicio" ? (
            <Button variant="primary" onClick={verificar} disabled={!credOk || !repos.length || trabajando}>
              {trabajando ? "Verificando…" : "Verificar repositorios"}
            </Button>
          ) : (
            <>
              <Button variant="primary" onClick={subir} disabled={trabajando || !listos.length}>
                {trabajando ? "Subiendo…" : `Subir ${totalArchivos} archivos`}
              </Button>
              <Button variant="secondary" onClick={verificar} disabled={trabajando}>
                Verificar de nuevo
              </Button>
            </>
          )}
          {!credOk && (
            <span className="text-xs" style={{ color: "var(--text-faint)" }}>
              Hacen falta credenciales válidas de Bitbucket.
            </span>
          )}
        </div>

        {etapa === "verificado" && (
          <div className="mt-2 text-xs" style={{ color: "var(--text-faint)" }}>
            Verificar no modificó nada. La subida ocurre al presionar el botón de arriba.
          </div>
        )}
      </div>

      {/* Repositorios que no se pueden usar: no existen, o les falta la
          rama destino. No se incluyen en la subida. */}
      {bloqueados.map((v) => (
        <ErrorNotice key={v.repo}>
          {v.repo}: {v.error}
        </ErrorNotice>
      ))}

      {etapa === "verificado" && (
        <ConflictosPanel
          verificaciones={verificaciones}
          acciones={acciones}
          onCambiar={(clave, accion) => setAcciones({ ...acciones, [clave]: accion })}
          onAplicarATodos={aplicarATodos}
          cancelados={cancelados}
          onCancelarRepo={(repo, cancelar) =>
            setCancelados(cancelar ? [...cancelados, repo] : cancelados.filter((r) => r !== repo))
          }
        />
      )}

      {error && <ErrorNotice>{error}</ErrorNotice>}
    </div>
  );
}
