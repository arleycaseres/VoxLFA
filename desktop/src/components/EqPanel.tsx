// Panel de control fino del ecualizador: un slider por banda del preset activo.
//
// El slider ajusta la ganancia en pasos finos (0.1 dB) dentro del rango
// soportado por el EQ paramétrico. Cuando el EQ está en bypass o el motor
// detenido, los controles quedan deshabilitados.

import type { DspState, EqBandKind } from "../lib/types";

/** Rango de ganancia por banda (dB), cubre los presets actuales. */
const GAIN_MIN = -18;
const GAIN_MAX = 18;
/** Paso del slider: ajuste "fino" (0.1 dB). */
const GAIN_STEP = 0.1;

/** Nombre legible de cada tipo de banda (en español). */
const EQ_KIND_LABELS: Record<EqBandKind, string> = {
  lowShelf: "Shelf graves",
  peaking: "Pico",
  highShelf: "Shelf agudos",
};

/** Formatea una frecuencia en Hz a una etiqueta compacta. */
function formatFreq(freqHz: number): string {
  if (freqHz >= 1000) return `${(freqHz / 1000).toFixed(1)} kHz`;
  return `${freqHz} Hz`;
}

/** Formatea una ganancia en dB con signo explícito. */
function formatGain(gainDb: number): string {
  const sign = gainDb > 0 ? "+" : "";
  return `${sign}${gainDb.toFixed(1)} dB`;
}

interface EqPanelProps {
  /** Estado de la cadena DSP (o `null` si el motor no corre). */
  dsp: DspState | null;
  running: boolean;
  /** Ajusta la ganancia de una banda por su índice. */
  onSetEqBand: (bandIndex: number, gainDb: number) => void;
}

export function EqPanel({ dsp, running, onSetEqBand }: EqPanelProps) {
  const eqLink = dsp?.links.find((link) => link.name === "eq") ?? null;
  const bands = eqLink?.eqBands ?? null;
  const bypassed = (eqLink?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && bands !== null && !bypassed;

  return (
    <div className="eqpanel">
      <div className="eqpanel__header">
        <span className="eqpanel__title">Bandas del ecualizador</span>
        {bands !== null && (
          <span
            className={`eqpanel__state ${
              bypassed ? "eqpanel__state--off" : "eqpanel__state--on"
            }`}
          >
            {bypassed ? "en bypass" : "en vivo"}
          </span>
        )}
      </div>

      {!running ? (
        <p className="eqpanel__empty">Arranca el motor para ajustar el EQ.</p>
      ) : bands === null ? (
        <p className="eqpanel__empty">
          El preset activo no incluye ecualizador.
        </p>
      ) : (
        <div className="eqpanel__bands">
          {bands.map((band, index) => (
            <div className="eqband" key={`${band.kind}-${band.freqHz}-${index}`}>
              <div className="eqband__head">
                <span className="eqband__name">
                  {EQ_KIND_LABELS[band.kind] ?? band.kind}
                </span>
                <span className="eqband__freq">{formatFreq(band.freqHz)}</span>
                <span className="eqband__gain">{formatGain(band.gainDb)}</span>
              </div>
              <div className="eqband__row">
                <span className="eqband__scale">{GAIN_MIN}</span>
                <input
                  type="range"
                  className="eqband__slider"
                  min={GAIN_MIN}
                  max={GAIN_MAX}
                  step={GAIN_STEP}
                  value={band.gainDb}
                  disabled={!enabled}
                  onChange={(event) =>
                    onSetEqBand(index, Number(event.target.value))
                  }
                  aria-label={`Ganancia de la banda ${index + 1} (${EQ_KIND_LABELS[band.kind] ?? band.kind})`}
                />
                <span className="eqband__scale">+{GAIN_MAX}</span>
                <button
                  type="button"
                  className="eqband__reset"
                  disabled={!enabled || band.gainDb === 0}
                  onClick={() => onSetEqBand(index, 0)}
                  title="Restablecer la banda a 0 dB"
                  aria-label={`Restablecer la banda ${index + 1} a 0 dB`}
                >
                  0
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
