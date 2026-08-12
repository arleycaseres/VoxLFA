// Panel de la cadena DSP: módulos del preset activo con bypass por módulo.
//
// Muestra la señal fluyendo por los módulos (desde la entrada) y permite
// silenciar cada módulo con un toggle. También controla el bypass global.

import type { DspState } from "../lib/types";

/** Nombre legible de cada módulo para la UI (en español). */
const MODULE_LABELS: Record<string, string> = {
  gain: "Ganancia",
  highpass: "Pasa-altos",
  eq: "Ecualizador",
  compressor: "Compresor",
  deesser: "De-esser",
  saturator: "Saturación",
  delay: "Delay",
  reverb: "Reverb",
  limiter: "Limitador",
};

const MODULE_ICONS: Record<string, string> = {
  gain: "▲",
  highpass: "↗",
  eq: "≋",
  compressor: "⇉",
  deesser: "S",
  saturator: "∞",
  delay: "↶",
  reverb: "◌",
  limiter: "▬",
};

interface DspChainProps {
  /** Estado de la cadena DSP (o `null` si el motor no corre). */
  dsp: DspState | null;
  /** Cambia el bypass global. */
  onGlobalBypass: (bypass: boolean) => void;
  /** Cambia el bypass de un módulo por nombre. */
  onLinkBypass: (link: string, bypass: boolean) => void;
}

export function DspChain({ dsp, onGlobalBypass, onLinkBypass }: DspChainProps) {
  const running = dsp !== null;

  return (
    <div className="chain">
      <div className="chain__header">
        <span className="chain__label">Cadena de señal</span>
        <label className={`toggle ${dsp?.globalBypass ? "toggle--on" : ""}`}>
          <input
            type="checkbox"
            checked={dsp?.globalBypass ?? false}
            disabled={!running}
            onChange={(event) => onGlobalBypass(event.target.checked)}
          />
          <span className="toggle__track" aria-hidden="true" />
          <span className="toggle__label">Bypass total</span>
        </label>
      </div>

      {!running ? (
        <p className="chain__empty">Arranca el motor para ver la cadena DSP.</p>
      ) : (
        <div className="chain__links">
          <div className={`chain__link chain__link--source ${dsp.globalBypass ? "chain__link--muted" : ""}`}>
            <span className="chain__link-icon">🎤</span>
            <span className="chain__link-name">Entrada</span>
          </div>

          {dsp.links.map((link, index) => {
            const active = !dsp.globalBypass && link.enabled && !link.bypass;
            return (
              <div key={`${link.name}-${index}`} className="chain__segment">
                <div className="chain__wire">
                  <span className={`chain__dot ${active ? "chain__dot--live" : ""}`} />
                </div>
                <div
                  className={`chain__link ${
                    link.enabled ? "" : "chain__link--disabled"
                  } ${dsp.globalBypass || link.bypass ? "chain__link--muted" : ""}`}
                >
                  <span className="chain__link-icon">
                    {MODULE_ICONS[link.name] ?? "▣"}
                  </span>
                  <span className="chain__link-name">
                    {MODULE_LABELS[link.name] ?? link.name}
                  </span>
                  <label className="toggle">
                    <input
                      type="checkbox"
                      checked={!link.bypass}
                      disabled={!link.enabled}
                      onChange={(event) => onLinkBypass(link.name, !event.target.checked)}
                    />
                    <span className="toggle__track" aria-hidden="true" />
                  </label>
                </div>
              </div>
            );
          })}

          <div className="chain__segment">
            <div className="chain__wire">
              <span
                className={`chain__dot ${
                  dsp.links.length > 0 && !dsp.globalBypass
                    ? "chain__dot--live"
                    : ""
                }`}
              />
            </div>
            <div
              className={`chain__link chain__link--out ${
                dsp.globalBypass ? "chain__link--muted" : ""
              }`}
            >
              <span className="chain__link-icon">◀</span>
              <span className="chain__link-name">Salida</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
