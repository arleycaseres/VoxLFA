// Panel de control de supresión de ruido: slider de mezcla seco/húmedo.
//
// Cada cambio se aplica en vivo (`set_denoise`): el core reconstruye solo el
// módulo denoise y lo conmuta por puntero.

import type { DspState, DenoiseParams } from "../lib/types";

const MIX_MIN = 0;
const MIX_MAX = 1;

interface DenoisePanelProps {
  /** Estado de la cadena DSP (o `null` si el motor no corre). */
  dsp: DspState | null;
  running: boolean;
  /** Aplica los parámetros de denoise en vivo. */
  onSetDenoise: (params: DenoiseParams) => void;
}

export function DenoisePanel({ dsp, running, onSetDenoise }: DenoisePanelProps) {
  const denoiseLink = dsp?.links.find((link) => link.name === "denoise") ?? null;
  const params = denoiseLink?.denoiseParams ?? null;
  const bypassed = (denoiseLink?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="denoisepanel">
        <p className="denoisepanel__empty">Arranca el motor para ajustar la supresión de ruido.</p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="denoisepanel">
        <p className="denoisepanel__empty">El preset activo no incluye supresión de ruido.</p>
      </div>
    );
  }

  const percentage = Math.round(params.mix * 100);

  return (
    <div className="denoisepanel">
      <div className="denoisepanel__header">
        <span className="denoisepanel__title">Supresión de ruido</span>
        <span
          className={`denoisepanel__state ${
            bypassed ? "denoisepanel__state--off" : "denoisepanel__state--on"
          }`}
        >
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="denoisepanel__row">
        <div className="denoisepanel__head">
          <span className="denoisepanel__name">Mezcla</span>
          <span className="denoisepanel__value">{percentage}%</span>
        </div>
        <div className="denoisepanel__track">
          <span className="denoisepanel__scale">{MIX_MIN * 100}%</span>
          <input
            type="range"
            className="denoisepanel__slider"
            min={MIX_MIN}
            max={MIX_MAX}
            step={0.01}
            value={params.mix}
            disabled={!enabled}
            onChange={(event) => onSetDenoise({ mix: Number(event.target.value) })}
            aria-label="Mezcla de supresión de ruido"
          />
          <span className="denoisepanel__scale">{MIX_MAX * 100}%</span>
        </div>
      </div>

      <div className="denoisepanel__foot">
        <span className="denoisepanel__hint">
          {percentage === 0
            ? "Sin supresión de ruido."
            : percentage === 100
              ? "Supresión de ruido al máximo."
              : `Supresión de ruido al ${percentage}%.`}
        </span>
      </div>
    </div>
  );
}
