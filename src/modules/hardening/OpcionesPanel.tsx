import { useEffect, useState } from "react";
import CollapsibleSection from "../../components/CollapsibleSection";
import { Dropdown, MetaLine } from "../../components/ui";
import {
  PERFIL_PERSONALIZADO,
  alternarComponente,
  type ComponenteDetectado,
  type ConfigHardening,
  type Evaluacion,
  type Perfil,
  type ResultadoAnalisis,
} from "./shared";

const TITULO_GRUPO: Record<string, string> = {
  node: "Herramientas de Node.js",
  system: "Sistema",
  security: "Seguridad",
};

interface Props {
  analisis: ResultadoAnalisis;
  config: ConfigHardening;
  onConfig: (c: ConfigHardening) => void;
  perfiles: Perfil[];
  evaluacion: Evaluacion | null;
  deshabilitado: boolean;
}

function Casilla({
  marcada,
  onCambio,
  titulo,
  detalle,
  deshabilitada,
}: {
  marcada: boolean;
  onCambio: (v: boolean) => void;
  titulo: string;
  detalle?: string;
  deshabilitada?: boolean;
}) {
  return (
    <label className={`flex items-start gap-2 ${deshabilitada ? "opacity-50" : "cursor-pointer"}`}>
      <input
        type="checkbox"
        checked={marcada}
        onChange={(e) => onCambio(e.target.checked)}
        disabled={deshabilitada}
        className="w-4 h-4 mt-0.5 cursor-pointer"
        style={{ accentColor: "var(--accent)" }}
      />
      <span>
        <span className="text-sm" style={{ color: "var(--text)" }}>
          {titulo}
        </span>
        {detalle && (
          <span className="block text-xs" style={{ color: "var(--text-faint)" }}>
            {detalle}
          </span>
        )}
      </span>
    </label>
  );
}

function Insignia({ texto, color }: { texto: string; color: string }) {
  return (
    <span className="text-[11px] px-1.5 py-0.5 rounded-sm whitespace-nowrap" style={{ color, background: "var(--surface)", border: "1px solid var(--border)" }}>
      {texto}
    </span>
  );
}

function FilaComponente({
  comp,
  conservar,
  onCambio,
  mensajes,
  deshabilitado,
}: {
  comp: ComponenteDetectado;
  conservar: boolean;
  onCambio: (v: boolean) => void;
  mensajes: string[];
  deshabilitado: boolean;
}) {
  const noExiste = !comp.present && !comp.installable;
  return (
    <div className="px-3 py-2.5 rounded-md" style={{ background: "var(--surface-raised)" }}>
      <div className="flex items-start justify-between gap-3">
        <Casilla
          marcada={conservar && !noExiste}
          onCambio={onCambio}
          titulo={comp.label}
          detalle={comp.description}
          deshabilitada={deshabilitado || noExiste}
        />
        <div className="flex items-center gap-1.5 flex-wrap justify-end shrink-0">
          {noExiste && <Insignia texto="No existe en esta imagen" color="var(--text-faint)" />}
          {!comp.present && comp.installable && <Insignia texto="Se instalaría" color="var(--text-muted)" />}
          {comp.present && comp.version && <Insignia texto={`v${comp.version}`} color="var(--text-muted)" />}
          {comp.present && comp.packages > 1 && <Insignia texto={`${comp.packages} paquetes`} color="var(--text-muted)" />}
          {comp.high_critical > 0 && (
            <Insignia
              texto={`${comp.high_critical} HIGH/CRITICAL${comp.unfixed_high_critical > 0 ? ` · ${comp.unfixed_high_critical} sin parche` : ""}`}
              color="var(--fail)"
            />
          )}
          {comp.present && comp.high_critical === 0 && comp.vulnerabilities > 0 && (
            <Insignia texto={`${comp.vulnerabilities} hallazgos menores`} color="var(--partial)" />
          )}
        </div>
      </div>
      {mensajes.length > 0 && (
        <ul className="mt-2 ml-6 space-y-0.5">
          {mensajes.map((m) => (
            <li key={m} className="text-xs" style={{ color: "var(--text-muted)" }}>
              {m}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// Lista editable: se interpreta al salir del campo, para no perder los saltos
// de línea mientras se escribe.
function CampoLista({
  etiqueta,
  ayuda,
  valores,
  onCambio,
  placeholder,
  deshabilitado,
}: {
  etiqueta: string;
  ayuda: string;
  valores: string[];
  onCambio: (v: string[]) => void;
  placeholder: string;
  deshabilitado: boolean;
}) {
  const [texto, setTexto] = useState(valores.join("\n"));
  useEffect(() => setTexto(valores.join("\n")), [valores]);
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-xs" style={{ color: "var(--text-muted)" }}>
        {etiqueta}
      </span>
      <textarea
        value={texto}
        rows={2}
        disabled={deshabilitado}
        onChange={(e) => setTexto(e.target.value)}
        onBlur={() => onCambio(texto.split(/[\s,]+/).filter(Boolean))}
        placeholder={placeholder}
        spellCheck={false}
        className="px-3 py-2 rounded-md text-xs outline-hidden resize-y"
        style={{ background: "var(--surface-raised)", border: "1px solid var(--border-strong)", color: "var(--text)", fontFamily: "var(--font-mono)" }}
      />
      <span className="text-[11px]" style={{ color: "var(--text-faint)" }}>
        {ayuda}
      </span>
    </label>
  );
}

function CampoTexto({
  etiqueta,
  valor,
  onCambio,
  ancho,
  deshabilitado,
}: {
  etiqueta: string;
  valor: string;
  onCambio: (v: string) => void;
  ancho: string;
  deshabilitado: boolean;
}) {
  return (
    <label className={`flex flex-col gap-1.5 ${ancho}`}>
      <span className="text-xs" style={{ color: "var(--text-muted)" }}>
        {etiqueta}
      </span>
      <input
        value={valor}
        disabled={deshabilitado}
        onChange={(e) => onCambio(e.target.value)}
        spellCheck={false}
        className="px-3 py-2 rounded-md text-sm outline-hidden"
        style={{ background: "var(--surface-raised)", border: "1px solid var(--border-strong)", color: "var(--text)", fontFamily: "var(--font-mono)" }}
      />
    </label>
  );
}

export default function OpcionesPanel({ analisis, config, onConfig, perfiles, evaluacion, deshabilitado }: Props) {
  const comps = analisis.detectados.components;
  const efectos = new Map((evaluacion?.analisis.effects ?? []).map((e) => [e.component, e]));
  const grupos = ["node", "system", "security"].filter((g) => comps.some((c) => c.group === g));

  if (!analisis.soporte.generador) {
    return (
      <div className="p-4 rounded-lg border space-y-2" style={{ borderColor: "var(--border)", background: "var(--surface)" }}>
        <div className="text-sm font-semibold" style={{ color: "var(--text)" }}>
          Componentes de la imagen
        </div>
        <div className="text-xs" style={{ color: "var(--text-muted)" }}>
          {analisis.soporte.motivo}
        </div>
        {analisis.soporte.componentes_tipicos.length > 0 && (
          <div className="text-xs" style={{ color: "var(--text-faint)" }}>
            Componentes típicos de este tipo de imagen: {analisis.soporte.componentes_tipicos.join(", ")}.
          </div>
        )}
      </div>
    );
  }

  const opcionesPerfil = [
    ...perfiles.map((p) => ({ value: p.nombre, label: p.nombre })),
    ...(perfiles.some((p) => p.nombre === config.profile) ? [] : [{ value: config.profile, label: config.profile }]),
  ];

  function aplicarPerfil(nombre: string) {
    const p = perfiles.find((x) => x.nombre === nombre);
    if (!p) return;
    onConfig({
      ...config,
      profile: p.nombre,
      keep: { ...p.config.keep },
      runtime_only: p.config.runtime_only,
      non_root: p.config.non_root,
      allow_install_deps: p.config.allow_install_deps,
      authorize_remove_components: [],
    });
  }

  const cambiar = (parcial: Partial<ConfigHardening>) => onConfig({ ...config, ...parcial, profile: PERFIL_PERSONALIZADO });
  const descripcionPerfil = perfiles.find((p) => p.nombre === config.profile)?.descripcion;

  return (
    <div className="p-4 rounded-lg border space-y-4" style={{ borderColor: "var(--border)", background: "var(--surface)" }}>
      <div className="flex items-start justify-between gap-4 flex-wrap">
        <div>
          <div className="text-sm font-semibold mb-1" style={{ color: "var(--text)" }}>
            ¿Qué se conserva en la imagen final?
          </div>
          <div className="text-xs max-w-xl" style={{ color: "var(--text-muted)" }}>
            Lo que no marques no se incluye: así sus vulnerabilidades desaparecen de la imagen. Una imagen de desarrollo normalmente necesita npm,
            por eso se parte de un perfil que lo conserva.
          </div>
        </div>
        <div className="flex flex-col gap-1.5 w-64">
          <span className="text-xs" style={{ color: "var(--text-muted)" }}>
            Perfil
          </span>
          <Dropdown value={config.profile} onChange={aplicarPerfil} options={opcionesPerfil} />
        </div>
      </div>
      {descripcionPerfil && <MetaLine>{descripcionPerfil}</MetaLine>}

      {evaluacion && evaluacion.descartados.length > 0 && (
        <div className="text-xs" style={{ color: "var(--partial)" }}>
          Se quitó de la selección lo que no existe en esta imagen y no se puede instalar:{" "}
          {evaluacion.descartados.map((id) => comps.find((c) => c.id === id)?.label ?? id).join(", ")}.
        </div>
      )}

      {grupos.map((g) => (
        <div key={g} className="space-y-2">
          <div className="text-[11px] uppercase tracking-wide" style={{ color: "var(--text-faint)" }}>
            {TITULO_GRUPO[g] ?? g}
          </div>
          {comps
            .filter((c) => c.group === g)
            .map((c) => {
              const conservar = !!config.keep[c.id];
              const ef = efectos.get(c.id);
              // Quitar algo que la imagen no trae no cambia nada: solo se explica lo que sí se pierde.
              // Si se pide algo que no trae, se avisa de que se agrega.
              const mensajes = c.present
                ? ef && ef.kept === conservar
                  ? ef.messages
                  : []
                : conservar && c.installable
                  ? ["Se agrega a la imagen final: no viene en la imagen original."]
                  : [];
              return (
                <FilaComponente
                  key={c.id}
                  comp={c}
                  conservar={conservar}
                  onCambio={(v) => onConfig(alternarComponente(config, comps, c.id, v))}
                  mensajes={mensajes}
                  deshabilitado={deshabilitado}
                />
              );
            })}
        </div>
      ))}

      <CollapsibleSection title="Opciones avanzadas" subtitle="Usuario, dependencias, paquetes adicionales" defaultOpen={false}>
        <div className="space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <Casilla
              marcada={config.runtime_only}
              onCambio={(v) => cambiar({ runtime_only: v })}
              titulo="Imagen solo runtime"
              detalle="Sin compiladores, git, yarn, pnpm ni apk"
              deshabilitada={deshabilitado}
            />
            <Casilla
              marcada={config.non_root}
              onCambio={(v) => cambiar({ non_root: v })}
              titulo="Ejecutar como usuario no root"
              detalle="Exigido por la política bancaria"
              deshabilitada={deshabilitado}
            />
            <Casilla
              marcada={config.allow_install_deps}
              onCambio={(v) => cambiar({ allow_install_deps: v })}
              titulo="Permitir npm install en la imagen"
              detalle="Requiere conservar npm y certificados CA"
              deshabilitada={deshabilitado || !config.keep.npm}
            />
            <Casilla
              marcada={config.allow_npm_upgrade}
              onCambio={(v) => cambiar({ allow_npm_upgrade: v })}
              titulo="Permitir actualizar npm dentro de su versión mayor"
              detalle="No cambia la versión de Node.js"
              deshabilitada={deshabilitado || !config.keep.npm}
            />
          </div>

          <div className="flex items-end gap-3 flex-wrap">
            <CampoTexto etiqueta="Usuario" valor={config.run_user} onCambio={(v) => cambiar({ run_user: v })} ancho="w-36" deshabilitado={deshabilitado} />
            <CampoTexto
              etiqueta="UID"
              valor={String(config.run_uid)}
              onCambio={(v) => cambiar({ run_uid: Math.max(0, parseInt(v, 10) || 0) })}
              ancho="w-24"
              deshabilitado={deshabilitado}
            />
            <CampoTexto etiqueta="Directorio de trabajo" valor={config.workdir} onCambio={(v) => cambiar({ workdir: v })} ancho="w-56" deshabilitado={deshabilitado} />
          </div>

          <CampoLista
            etiqueta="Paquetes apk adicionales"
            ayuda="Uno por línea o separados por coma (por ejemplo: jq, tzdata). Se instalan en la imagen final."
            valores={config.extra_apk}
            onCambio={(v) => cambiar({ extra_apk: v })}
            placeholder="jq"
            deshabilitado={deshabilitado}
          />
          <CampoLista
            etiqueta="Rutas de la imagen original a conservar"
            ayuda="Rutas absolutas que se copian tal cual desde la imagen de origen."
            valores={config.preserve_paths}
            onCambio={(v) => cambiar({ preserve_paths: v })}
            placeholder="/usr/local/share/mi-recurso"
            deshabilitado={deshabilitado}
          />
        </div>
      </CollapsibleSection>
    </div>
  );
}
