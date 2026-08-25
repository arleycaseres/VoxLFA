import { memo } from "react";
import type { DspState, DynamicEqBandParams, DynamicEqParams } from "../lib/types";

const FREQ_MIN = 50;
const FREQ_MAX = 16000;
const Q_MIN = 0.1;
const Q_MAX = 10;
const THRESHOLD_MIN = -60;
const THRESHOLD_MAX = 0;
const RATIO_MIN = 1;
const RATIO_MAX = 20;
const ATTACK_MIN = 0.1;
const ATTACK_MAX = 100;
const RELEASE_MIN = 10;
const RELEASE_MAX = 1000;
const MAKEUP_MIN = -12;
const MAKEUP_MAX = 12;

function formatHz(value: number): string {
  if (value >= 1000) return `${(value / 1000).toFixed(1)} kHz`;
  return `${value.toFixed(0)} Hz`;
}

interface DynamicEqPanelProps {
  dsp: DspState | null;
  running: boolean;
  onSetDynamicEq: (params: DynamicEqParams) => void;
}

const DynamicEqPanel = memo(function DynamicEqPanel({ dsp, running, onSetDynamicEq }: DynamicEqPanelProps) {
  const link = dsp?.links.find((l) => l.name === "dynamic_eq") ?? null;
  const params = link?.dynamicEqParams ?? null;
  const bypassed = (link?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="dynamiceqpanel">
        <p className="dynamiceqpanel__empty">Arranca el motor para ajustar el EQ dinámico.</p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="dynamiceqpanel">
        <p className="dynamiceqpanel__empty">El preset activo no incluye EQ dinámico.</p>
      </div>
    );
  }

  const updateBand = (index: number, patch: Partial<DynamicEqBandParams>) => {
    const newBands = params.bands.map((b, i) =>
      i === index ? { ...b, ...patch } : b,
    );
    onSetDynamicEq({ bands: newBands });
  };

  return (
    <div className="dynamiceqpanel">
      <div className="dynamiceqpanel__header">
        <span className="dynamiceqpanel__title">EQ Dinámico</span>
        <span className={`dynamiceqpanel__state ${bypassed ? "dynamiceqpanel__state--off" : "dynamiceqpanel__state--on"}`}>
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      {params.bands.map((band, idx) => (
        <div key={idx} className="dynamiceqpanel__band">
          <span className="dynamiceqpanel__band-label">Banda {idx + 1}</span>

          <div className="dynamiceqpanel__row">
            <div className="dynamiceqpanel__head">
              <span className="dynamiceqpanel__name">Frecuencia</span>
              <span className="dynamiceqpanel__value">{formatHz(band.freqHz)}</span>
            </div>
            <div className="dynamiceqpanel__track">
              <span className="dynamiceqpanel__scale">{FREQ_MIN}</span>
              <input
                type="range"
                className="dynamiceqpanel__slider"
                min={FREQ_MIN}
                max={FREQ_MAX}
                step={1}
                value={band.freqHz}
                disabled={!enabled}
                onChange={(e) => updateBand(idx, { freqHz: Number(e.target.value) })}
                aria-label={`Frecuencia banda ${idx + 1}`}
              />
              <span className="dynamiceqpanel__scale">{formatHz(FREQ_MAX)}</span>
            </div>
          </div>

          <div className="dynamiceqpanel__row">
            <div className="dynamiceqpanel__head">
              <span className="dynamiceqpanel__name">Q</span>
              <span className="dynamiceqpanel__value">{band.q.toFixed(1)}</span>
            </div>
            <div className="dynamiceqpanel__track">
              <span className="dynamiceqpanel__scale">{Q_MIN}</span>
              <input
                type="range"
                className="dynamiceqpanel__slider"
                min={Q_MIN}
                max={Q_MAX}
                step={0.1}
                value={band.q}
                disabled={!enabled}
                onChange={(e) => updateBand(idx, { q: Number(e.target.value) })}
                aria-label={`Q banda ${idx + 1}`}
              />
              <span className="dynamiceqpanel__scale">{Q_MAX}</span>
            </div>
          </div>

          <div className="dynamiceqpanel__row">
            <div className="dynamiceqpanel__head">
              <span className="dynamiceqpanel__name">Umbral</span>
              <span className="dynamiceqpanel__value">{band.thresholdDb.toFixed(0)} dB</span>
            </div>
            <div className="dynamiceqpanel__track">
              <span className="dynamiceqpanel__scale">{THRESHOLD_MIN}</span>
              <input
                type="range"
                className="dynamiceqpanel__slider"
                min={THRESHOLD_MIN}
                max={THRESHOLD_MAX}
                step={1}
                value={band.thresholdDb}
                disabled={!enabled}
                onChange={(e) => updateBand(idx, { thresholdDb: Number(e.target.value) })}
                aria-label={`Umbral banda ${idx + 1}`}
              />
              <span className="dynamiceqpanel__scale">{THRESHOLD_MAX}</span>
            </div>
          </div>

          <div className="dynamiceqpanel__row">
            <div className="dynamiceqpanel__head">
              <span className="dynamiceqpanel__name">Ratio</span>
              <span className="dynamiceqpanel__value">{band.ratio.toFixed(1)}:1</span>
            </div>
            <div className="dynamiceqpanel__track">
              <span className="dynamiceqpanel__scale">{RATIO_MIN}:1</span>
              <input
                type="range"
                className="dynamiceqpanel__slider"
                min={RATIO_MIN}
                max={RATIO_MAX}
                step={0.5}
                value={band.ratio}
                disabled={!enabled}
                onChange={(e) => updateBand(idx, { ratio: Number(e.target.value) })}
                aria-label={`Ratio banda ${idx + 1}`}
              />
              <span className="dynamiceqpanel__scale">{RATIO_MAX}:1</span>
            </div>
          </div>

          <div className="dynamiceqpanel__row">
            <div className="dynamiceqpanel__head">
              <span className="dynamiceqpanel__name">Ataque</span>
              <span className="dynamiceqpanel__value">{band.attackMs.toFixed(1)} ms</span>
            </div>
            <div className="dynamiceqpanel__track">
              <span className="dynamiceqpanel__scale">{ATTACK_MIN}</span>
              <input
                type="range"
                className="dynamiceqpanel__slider"
                min={ATTACK_MIN}
                max={ATTACK_MAX}
                step={0.1}
                value={band.attackMs}
                disabled={!enabled}
                onChange={(e) => updateBand(idx, { attackMs: Number(e.target.value) })}
                aria-label={`Ataque banda ${idx + 1}`}
              />
              <span className="dynamiceqpanel__scale">{ATTACK_MAX}</span>
            </div>
          </div>

          <div className="dynamiceqpanel__row">
            <div className="dynamiceqpanel__head">
              <span className="dynamiceqpanel__name">Liberación</span>
              <span className="dynamiceqpanel__value">{band.releaseMs.toFixed(0)} ms</span>
            </div>
            <div className="dynamiceqpanel__track">
              <span className="dynamiceqpanel__scale">{RELEASE_MIN}</span>
              <input
                type="range"
                className="dynamiceqpanel__slider"
                min={RELEASE_MIN}
                max={RELEASE_MAX}
                step={1}
                value={band.releaseMs}
                disabled={!enabled}
                onChange={(e) => updateBand(idx, { releaseMs: Number(e.target.value) })}
                aria-label={`Liberación banda ${idx + 1}`}
              />
              <span className="dynamiceqpanel__scale">{RELEASE_MAX}</span>
            </div>
          </div>

          <div className="dynamiceqpanel__row">
            <div className="dynamiceqpanel__head">
              <span className="dynamiceqpanel__name">Makeup</span>
              <span className="dynamiceqpanel__value">{band.makeupDb >= 0 ? "+" : ""}{band.makeupDb.toFixed(1)} dB</span>
            </div>
            <div className="dynamiceqpanel__track">
              <span className="dynamiceqpanel__scale">{MAKEUP_MIN}</span>
              <input
                type="range"
                className="dynamiceqpanel__slider"
                min={MAKEUP_MIN}
                max={MAKEUP_MAX}
                step={0.5}
                value={band.makeupDb}
                disabled={!enabled}
                onChange={(e) => updateBand(idx, { makeupDb: Number(e.target.value) })}
                aria-label={`Makeup banda ${idx + 1}`}
              />
              <span className="dynamiceqpanel__scale">+{MAKEUP_MAX}</span>
            </div>
          </div>
        </div>
      ))}

      <div className="dynamiceqpanel__foot">
        <span className="dynamiceqpanel__hint">
          Compresión por banda de frecuencia: reduce picos en la zona específica sin afectar el resto del espectro.
        </span>
      </div>
    </div>
  );
});

export { DynamicEqPanel };
