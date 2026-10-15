import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button, ErrorNotice, MetaLine } from "../../components/ui";
import { parseErr, type SonarCredStatus } from "./shared";

const URL_TOKENS = "sonarcloud.io → My Account → Security → Generate Tokens";

interface Props {
  onEstadoCambia: (listo: boolean) => void;
}

// SonarQube solo necesita el token: usa autenticación Bearer, sin correo.
// Por eso este panel es más simple que el de Bitbucket.
export default function SonarCredentialsPanel({ onEstadoCambia }: Props) {
  const [estado, setEstado] = useState<SonarCredStatus | null>(null);
  const [cargando, setCargando] = useState(true);
  const [token, setToken] = useState("");
  const [guardando, setGuardando] = useState(false);
  const [error, setError] = useState("");

  async function consultar() {
    setCargando(true);
    setError("");
    try {
      const st = await invoke<SonarCredStatus>("sonar_cred_status");
      setEstado(st);
      onEstadoCambia(st.validas);
    } catch (e) {
      setError(parseErr(e));
      onEstadoCambia(false);
    } finally {
      setCargando(false);
    }
  }

  useEffect(() => {
    consultar();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function guardar() {
    setGuardando(true);
    setError("");
    try {
      const st = await invoke<SonarCredStatus>("sonar_cred_save", { token });
      setEstado(st);
      onEstadoCambia(st.validas);
      if (st.validas) setToken("");
    } catch (e) {
      setError(parseErr(e));
    } finally {
      setGuardando(false);
    }
  }

  async function borrar() {
    setError("");
    try {
      await invoke("sonar_cred_delete");
      await consultar();
    } catch (e) {
      setError(parseErr(e));
    }
  }

  if (cargando) {
    return (
      <div
        className="p-3 rounded-lg border"
        style={{ borderColor: "var(--border)", background: "var(--surface)" }}
      >
        <MetaLine>Verificando token de SonarQube…</MetaLine>
      </div>
    );
  }

  if (estado?.validas) {
    return (
      <div
        className="p-3 rounded-lg border flex items-center justify-between gap-3"
        style={{ borderColor: "var(--border)", background: "var(--surface)" }}
      >
        <div className="flex items-center gap-2 text-xs">
          <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ background: "var(--ok)" }} />
          <span style={{ color: "var(--text-muted)" }}>SonarQube Cloud —</span>
          <span style={{ color: "var(--text)" }}>{estado.organizacion}</span>
        </div>
        <Button variant="secondary" onClick={borrar} className="text-xs px-2.5 py-1">
          Cambiar token
        </Button>
      </div>
    );
  }

  return (
    <div
      className="p-4 rounded-lg border space-y-3"
      style={{ borderColor: "var(--border)", background: "var(--surface)" }}
    >
      <div>
        <div className="text-sm font-semibold mb-1" style={{ color: "var(--text)" }}>
          Token de SonarQube Cloud
        </div>
        <div className="text-xs" style={{ color: "var(--text-muted)" }}>
          {estado?.detalle ?? "Se necesita un token para crear los proyectos."}
        </div>
      </div>

      <div className="flex items-end gap-2">
        <label className="flex flex-col gap-1.5 flex-1">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            Token
          </span>
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
        <Button onClick={guardar} disabled={guardando || !token.trim()}>
          {guardando ? "Validando…" : "Validar y guardar"}
        </Button>
      </div>

      {error && <ErrorNotice>{error}</ErrorNotice>}

      <MetaLine>Se obtiene en {URL_TOKENS}. Debe dar acceso a la organización del banco.</MetaLine>
    </div>
  );
}
