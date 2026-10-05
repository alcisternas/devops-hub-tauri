import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ErrorNotice, MetaLine } from "../../components/ui";
import { parseErr, type CredStatus } from "./shared";

// Scopes exactos que necesita el token para todo lo que hace el módulo.
// Vienen de la herramienta anterior, donde fueron confirmados en uso real.
const SCOPES = [
  "admin:repository:bitbucket",
  "write:repository:bitbucket",
  "read:repository:bitbucket",
  "write:pipeline:bitbucket",
  "read:project:bitbucket",
  "read:user:bitbucket",
];

const URL_TOKENS = "id.atlassian.com/manage-profile/security/api-tokens";

interface Props {
  workspace: string;
  // El formulario de arriba necesita saber si puede dejar crear o no.
  onEstadoCambia: (listo: boolean) => void;
}

export default function CredentialsPanel({ workspace, onEstadoCambia }: Props) {
  const [estado, setEstado] = useState<CredStatus | null>(null);
  const [cargando, setCargando] = useState(true);
  const [error, setError] = useState("");

  // Formulario de ingreso
  const [email, setEmail] = useState("");
  const [token, setToken] = useState("");
  const [guardando, setGuardando] = useState(false);

  async function consultar() {
    setCargando(true);
    setError("");
    try {
      const st = await invoke<CredStatus>("bitbucket_cred_status", { workspace });
      setEstado(st);
      onEstadoCambia(st.validas);
    } catch (e) {
      setError(parseErr(e));
      onEstadoCambia(false);
    } finally {
      setCargando(false);
    }
  }

  // Solo al montar. No se re-consulta cuando cambia el workspace: eso
  // dispararía una llamada a la API con cada tecla que el usuario escriba
  // en ese campo.
  useEffect(() => {
    consultar();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function guardar() {
    setGuardando(true);
    setError("");
    try {
      const st = await invoke<CredStatus>("bitbucket_cred_save", { email, token, workspace });
      setEstado(st);
      onEstadoCambia(st.validas);
      if (st.validas) {
        // El token no se conserva en el estado de React una vez guardado:
        // ya vive en el almacén del sistema y no hay razón para tenerlo
        // dando vueltas en memoria de la interfaz.
        setToken("");
        setEmail("");
      }
    } catch (e) {
      setError(parseErr(e));
    } finally {
      setGuardando(false);
    }
  }

  async function borrar() {
    setError("");
    try {
      await invoke("bitbucket_cred_delete");
      await consultar();
    } catch (e) {
      setError(parseErr(e));
    }
  }

  if (cargando) {
    return (
      <div
        className="p-4 rounded-lg border"
        style={{ borderColor: "var(--border)", background: "var(--surface)" }}
      >
        <MetaLine>Verificando credenciales…</MetaLine>
      </div>
    );
  }

  // ── Todo en orden: una sola línea, sin ocupar pantalla.
  if (estado?.validas) {
    return (
      <div
        className="p-3 rounded-lg border flex items-center justify-between gap-3"
        style={{ borderColor: "var(--border)", background: "var(--surface)" }}
      >
        <div className="flex items-center gap-2 text-xs">
          <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ background: "var(--ok)" }} />
          <span style={{ color: "var(--text-muted)" }}>Autenticado como</span>
          <span style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>
            {estado.email}
          </span>
        </div>
        <Button variant="secondary" onClick={borrar} className="text-xs px-2.5 py-1">
          Cambiar cuenta
        </Button>
      </div>
    );
  }

  // ── Falta algo: formulario + instrucciones.
  return (
    <div
      className="p-4 rounded-lg border space-y-4"
      style={{ borderColor: "var(--border)", background: "var(--surface)" }}
    >
      <div>
        <div className="text-sm font-semibold mb-1" style={{ color: "var(--text)" }}>
          Credenciales de Bitbucket
        </div>
        <div className="text-xs" style={{ color: "var(--text-muted)" }}>
          {estado?.detalle ?? "Se necesitan credenciales para continuar."}
        </div>
      </div>

      {!estado?.almacen_disponible && (
        <div
          className="p-3 rounded-md text-xs"
          style={{ background: "var(--partial-dim)", border: "1px solid #4a3a10", color: "#fde68a" }}
        >
          El almacén de credenciales del sistema no respondió. Puedes trabajar igual, pero habrá
          que ingresar el token cada vez que abras la aplicación.
        </div>
      )}

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <label className="flex flex-col gap-1.5">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            Correo Atlassian
          </span>
          <input
            type="text"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="nombre@bancoripley.com"
            className="px-3 py-2 rounded-md text-sm outline-none"
            style={{
              background: "var(--surface-raised)",
              border: "1px solid var(--border-strong)",
              color: "var(--text)",
            }}
          />
        </label>

        <label className="flex flex-col gap-1.5">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            API Token
          </span>
          {/* type password: el token no se muestra en pantalla mientras se
              escribe, por si alguien comparte su escritorio. */}
          <input
            type="password"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="••••••••••••"
            className="px-3 py-2 rounded-md text-sm outline-none"
            style={{
              background: "var(--surface-raised)",
              border: "1px solid var(--border-strong)",
              color: "var(--text)",
              fontFamily: "var(--font-mono)",
            }}
          />
        </label>
      </div>

      <div className="flex items-center gap-2">
        <Button onClick={guardar} disabled={guardando || !email.trim() || !token.trim()}>
          {guardando ? "Validando…" : "Validar y guardar"}
        </Button>
        <MetaLine>Se valida contra Bitbucket antes de guardar.</MetaLine>
      </div>

      {error && <ErrorNotice>{error}</ErrorNotice>}

      <div className="pt-3 border-t" style={{ borderColor: "var(--border)" }}>
        <div className="text-xs mb-2" style={{ color: "var(--text-muted)" }}>
          Para obtener el token: entra a{" "}
          <span style={{ color: "var(--text)", fontFamily: "var(--font-mono)" }}>{URL_TOKENS}</span>
          , crea un token de API con alcances y marca estos scopes:
        </div>
        <div className="flex flex-wrap gap-1.5">
          {SCOPES.map((s) => (
            <span
              key={s}
              className="text-xs px-2 py-0.5 rounded"
              style={{
                background: "var(--surface-raised)",
                color: "var(--text-muted)",
                fontFamily: "var(--font-mono)",
              }}
            >
              {s}
            </span>
          ))}
        </div>
        <MetaLine>
          <span className="block mt-2">El token se muestra una sola vez al crearlo.</span>
        </MetaLine>
      </div>
    </div>
  );
}
