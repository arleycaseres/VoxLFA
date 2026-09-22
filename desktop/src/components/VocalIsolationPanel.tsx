import { memo } from "react";
import type { DspState, VocalIsolationParams } from "../lib/types";

const STRENGTH_MIN = 0;
const STRENGTH_MAX = 1;
const MIX_MIN = 0;
const MIX_MAX = 1;

interface VocalIsolationPanelProps {
  dsp: DspState | null;
  running: boolean;
  onSetVocalIsolation: (params: VocalIsolationParams) => void;
}

const VocalIsolationPanel = memo(function VocalIsolationPanel({
  dsp,
  running,
  onSetVocalIsolation,
}: VocalIsolationPanelProps) {
  const link = dsp?.links.find((l) => l.name === "vocal_isolation") ?? null;
  const params = link?.vocalIsolationParams ?? null;
  const bypassed = (link?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="vocalisolationpanel">
        <p className="vocalisolationpanel__empty">
          Arranca el motor para ajustar el aislamiento de voz.
        </p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="vocalisolationpanel">
        <p className="vocalisolationpanel__empty">
          El preset activo no incluye aislamiento de voz.
        </p>
      </div>
    );
  }

  const update = (patch: Partial<VocalIsolationParams>) => {
    onSetVocalIsolation({ ...params, ...patch });
  };

  const strengthPercent = Math.round(params.strength * 100);
  const mixPercent = Math.round(params.mix * 100);

  return (
    <div className="vocalisolationpanel">
      <div className="vocalisolationpanel__header">
        <span className="vocalisolationpanel__title">Aislamiento de voz</span>
        <span
          className={`vocalisolationpanel__state ${
            bypassed ? "vocalisolationpanel__state--off" : "vocalisolationpanel__state--on"
          }`}
        >
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="vocalisolationpanel__row">
        <div className="vocalisolationpanel__head">
          <span className="vocalisolationpanel__name">Intensidad</span>
          <span className="vocalisolationpanel__value">{strengthPercent}%</span>
        </div>
        <div className="vocalisolationpanel__track">
          <span className="vocalisolationpanel__scale">{STRENGTH_MIN * 100}%</span>
          <input
            type="range"
            className="vocalisolationpanel__slider"
            min={STRENGTH_MIN}
            max={STRENGTH_MAX}
            step={0.01}
            value={params.strength}
            disabled={!enabled}
            onChange={(e) => update({ strength: Number(e.target.value) })}
            aria-label="Intensidad del aislamiento de voz"
          />
          <span className="vocalisolationpanel__scale">{STRENGTH_MAX * 100}%</span>
        </div>
      </div>

      <div className="vocalisolationpanel__row">
        <div className="vocalisolationpanel__head">
          <span className="vocalisolationpanel__name">Mezcla</span>
          <span className="vocalisolationpanel__value">{mixPercent}%</span>
        </div>
        <div className="vocalisolationpanel__track">
          <span className="vocalisolationpanel__scale">{MIX_MIN * 100}%</span>
          <input
            type="range"
            className="vocalisolationpanel__slider"
            min={MIX_MIN}
            max={MIX_MAX}
            step={0.01}
            value={params.mix}
            disabled={!enabled}
            onChange={(e) => update({ mix: Number(e.target.value) })}
            aria-label="Mezcla del aislamiento de voz"
          />
          <span className="vocalisolationpanel__scale">{MIX_MAX * 100}%</span>
        </div>
      </div>

      <div className="vocalisolationpanel__foot">
        <span className="vocalisolationpanel__hint">
          Realza la voz propia y atenúa el ruido de fondo mediante un filtro
          comb guiado por la frecuencia fundamental.
        </span>
      </div>
    </div>
  );
});

export { VocalIsolationPanel };