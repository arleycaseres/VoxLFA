// Panel de control de la puerta de ruido: umbral, ataque, liberación y rango.
//
// Cada cambio se aplica en vivo (`set_noise_gate`): el core reconstruye solo
// la puerta de ruido y la conmuta por puntero. El *hold* se muestra en solo
// lectura (se fija por preset).

import type { DspState, NoiseGateParams } from "../lib/types";

/** Rango del umbral (dBFS). */
const THRESHOLD_MIN = -80;
const THRESHOLD_MAX = -20;
/** Rango de atenuación máxima (dB). */
const RANGE_MIN = 6;
const RANGE_MAX = 60;
/** Rango del tiempo de ataque (ms). */
const ATTACK_MIN = 0;
const ATTACK_MAX = 20;
/** Rango del tiempo de liberación (ms). */
const RELEASE_MIN = 20;
const RELEASE_MAX = 500;

/** Formatea un nivel dBFS con signo. */
function formatDb(value: number): string {
  const sign = value > 0 ? "+" : "";
  return `${sign}${value.toFixed(1)} dB`;
}

interface GatePanelProps {
  /** Estado de la cadena DSP (o `null` si el motor no corre). */
  dsp: DspState | null;
  running: boolean;
  /** Aplica los parámetros de la puerta de ruido en vivo. */
  onSetNoiseGate: (params: NoiseGateParams) => void;
}

export function GatePanel({ dsp, running, onSetNoiseGate }: GatePanelProps) {
  const gateLink = dsp?.links.find((link) => link.name === "noisegate") ?? null;
  const params = gateLink?.gateParams ?? null;
  const bypassed = (gateLink?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="gatepanel">
        <p className="gatepanel__empty">Arranca el motor para ajustar la puerta de ruido.</p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="gatepanel">
        <p className="gatepanel__empty">El preset activo no incluye puerta de ruido.</p>
      </div>
    );
  }

  const update = (patch: Partial<NoiseGateParams>) => {
    onSetNoiseGate({ ...params, ...patch });
  };

  return (
    <div className="gatepanel">
      <div className="gatepanel__header">
        <span className="gatepanel__title">Parámetros de la puerta</span>
        <span
          className={`gatepanel__state ${
            bypassed ? "gatepanel__state--off" : "gatepanel__state--on"
          }`}
        >
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="gatepanel__row">
        <div className="gatepanel__head">
          <span className="gatepanel__name">Umbral</span>
          <span className="gatepanel__value">{formatDb(params.thresholdDb)}</span>
        </div>
        <div className="gatepanel__track">
          <span className="gatepanel__scale">{THRESHOLD_MIN}</span>
          <input
            type="range"
            className="gatepanel__slider"
            min={THRESHOLD_MIN}
            max={THRESHOLD_MAX}
            step={1}
            value={params.thresholdDb}
            disabled={!enabled}
            onChange={(event) => update({ thresholdDb: Number(event.target.value) })}
            aria-label="Umbral de la puerta de ruido"
          />
          <span className="gatepanel__scale">+{THRESHOLD_MAX}</span>
        </div>
      </div>

      <div className="gatepanel__row">
        <div className="gatepanel__head">
          <span className="gatepanel__name">Rango</span>
          <span className="gatepanel__value">{formatDb(-params.rangeDb)}</span>
        </div>
        <div className="gatepanel__track">
          <span className="gatepanel__scale">{RANGE_MIN}</span>
          <input
            type="range"
            className="gatepanel__slider"
            min={RANGE_MIN}
            max={RANGE_MAX}
            step={1}
            value={params.rangeDb}
            disabled={!enabled}
            onChange={(event) => update({ rangeDb: Number(event.target.value) })}
            aria-label="Atenuación máxima de la puerta de ruido"
          />
          <span className="gatepanel__scale">{RANGE_MAX}</span>
        </div>
      </div>

      <div className="gatepanel__row">
        <div className="gatepanel__head">
          <span className="gatepanel__name">Ataque</span>
          <span className="gatepanel__value">{params.attackMs.toFixed(1)} ms</span>
        </div>
        <div className="gatepanel__track">
          <span className="gatepanel__scale">{ATTACK_MIN}</span>
          <input
            type="range"
            className="gatepanel__slider"
            min={ATTACK_MIN}
            max={ATTACK_MAX}
            step={0.5}
            value={params.attackMs}
            disabled={!enabled}
            onChange={(event) => update({ attackMs: Number(event.target.value) })}
            aria-label="Ataque de la puerta de ruido"
          />
          <span className="gatepanel__scale">{ATTACK_MAX}</span>
        </div>
      </div>

      <div className="gatepanel__row">
        <div className="gatepanel__head">
          <span className="gatepanel__name">Liberación</span>
          <span className="gatepanel__value">{params.releaseMs.toFixed(0)} ms</span>
        </div>
        <div className="gatepanel__track">
          <span className="gatepanel__scale">{RELEASE_MIN}</span>
          <input
            type="range"
            className="gatepanel__slider"
            min={RELEASE_MIN}
            max={RELEASE_MAX}
            step={5}
            value={params.releaseMs}
            disabled={!enabled}
            onChange={(event) => update({ releaseMs: Number(event.target.value) })}
            aria-label="Liberación de la puerta de ruido"
          />
          <span className="gatepanel__scale">{RELEASE_MAX}</span>
        </div>
      </div>

      <div className="gatepanel__foot">
        <span className="gatepanel__foot-label">
          Hold <span className="gatepanel__foot-value">{params.holdMs.toFixed(0)} ms</span>
        </span>
        <span className="gatepanel__hint">
          La puerta atenúa {params.rangeDb.toFixed(0)} dB el ruido de fondo.
        </span>
      </div>
    </div>
  );
}
