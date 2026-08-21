import type { DspState, DelayParams, DelayMode } from "../lib/types";

const TIME_MIN = 1;
const TIME_MAX = 2000;
const FEEDBACK_MIN = 0;
const FEEDBACK_MAX = 0.95;
const MIX_MIN = 0;
const MIX_MAX = 1;
const PRE_DELAY_MIN = 0;
const PRE_DELAY_MAX = 200;
const FILTER_MIN = 20;
const FILTER_MAX = 20000;
const DUCK_MIN = 0;
const DUCK_MAX = 1;

const DELAY_MODES: { value: DelayMode; label: string }[] = [
  { value: "digital", label: "Digital" },
  { value: "analog", label: "Análogo" },
  { value: "tape", label: "Cinta" },
  { value: "slapback", label: "Slapback" },
];

function formatMs(value: number): string {
  if (value >= 1000) return `${(value / 1000).toFixed(2)} s`;
  return `${value.toFixed(0)} ms`;
}

function formatHz(value: number): string {
  if (value >= 1000) return `${(value / 1000).toFixed(1)} kHz`;
  return `${value.toFixed(0)} Hz`;
}

interface DelayPanelProps {
  dsp: DspState | null;
  running: boolean;
  onSetDelay: (params: DelayParams) => void;
}

export function DelayPanel({ dsp, running, onSetDelay }: DelayPanelProps) {
  const link = dsp?.links.find((l) => l.name === "delay") ?? null;
  const params = link?.delayParams ?? null;
  const bypassed = (link?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="delaypanel">
        <p className="delaypanel__empty">Arranca el motor para ajustar el delay.</p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="delaypanel">
        <p className="delaypanel__empty">El preset activo no incluye delay.</p>
      </div>
    );
  }

  const update = (patch: Partial<DelayParams>) => {
    onSetDelay({ ...params, ...patch });
  };

  const feedbackPercent = Math.round(params.feedback * 100);
  const mixPercent = Math.round(params.mix * 100);
  const duckPercent = Math.round(params.duckAmount * 100);

  return (
    <div className="delaypanel">
      <div className="delaypanel__header">
        <span className="delaypanel__title">Delay</span>
        <span className={`delaypanel__state ${bypassed ? "delaypanel__state--off" : "delaypanel__state--on"}`}>
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="delaypanel__row">
        <div className="delaypanel__head">
          <span className="delaypanel__name">Modo</span>
        </div>
        <div className="delaypanel__modes">
          {DELAY_MODES.map((m) => (
            <button
              key={m.value}
              className={`delaypanel__mode ${params.mode === m.value ? "delaypanel__mode--active" : ""}`}
              disabled={!enabled}
              onClick={() => update({ mode: m.value })}
            >
              {m.label}
            </button>
          ))}
        </div>
      </div>

      <div className="delaypanel__row">
        <div className="delaypanel__head">
          <span className="delaypanel__name">Tiempo</span>
          <span className="delaypanel__value">{formatMs(params.timeMs)}</span>
        </div>
        <div className="delaypanel__track">
          <span className="delaypanel__scale">{TIME_MIN}</span>
          <input
            type="range"
            className="delaypanel__slider"
            min={TIME_MIN}
            max={TIME_MAX}
            step={1}
            value={params.timeMs}
            disabled={!enabled}
            onChange={(e) => update({ timeMs: Number(e.target.value) })}
            aria-label="Tiempo de delay"
          />
          <span className="delaypanel__scale">{TIME_MAX}</span>
        </div>
      </div>

      <div className="delaypanel__row">
        <div className="delaypanel__head">
          <span className="delaypanel__name">Feedback</span>
          <span className="delaypanel__value">{feedbackPercent}%</span>
        </div>
        <div className="delaypanel__track">
          <span className="delaypanel__scale">{FEEDBACK_MIN}%</span>
          <input
            type="range"
            className="delaypanel__slider"
            min={FEEDBACK_MIN}
            max={FEEDBACK_MAX}
            step={0.01}
            value={params.feedback}
            disabled={!enabled}
            onChange={(e) => update({ feedback: Number(e.target.value) })}
            aria-label="Feedback del delay"
          />
          <span className="delaypanel__scale">{FEEDBACK_MAX * 100}%</span>
        </div>
      </div>

      <div className="delaypanel__row">
        <div className="delaypanel__head">
          <span className="delaypanel__name">Mezcla</span>
          <span className="delaypanel__value">{mixPercent}%</span>
        </div>
        <div className="delaypanel__track">
          <span className="delaypanel__scale">{MIX_MIN * 100}%</span>
          <input
            type="range"
            className="delaypanel__slider"
            min={MIX_MIN}
            max={MIX_MAX}
            step={0.01}
            value={params.mix}
            disabled={!enabled}
            onChange={(e) => update({ mix: Number(e.target.value) })}
            aria-label="Mezcla del delay"
          />
          <span className="delaypanel__scale">{MIX_MAX * 100}%</span>
        </div>
      </div>

      <div className="delaypanel__row">
        <div className="delaypanel__head">
          <span className="delaypanel__name">Pre-delay</span>
          <span className="delaypanel__value">{formatMs(params.preDelayMs)}</span>
        </div>
        <div className="delaypanel__track">
          <span className="delaypanel__scale">{PRE_DELAY_MIN}</span>
          <input
            type="range"
            className="delaypanel__slider"
            min={PRE_DELAY_MIN}
            max={PRE_DELAY_MAX}
            step={1}
            value={params.preDelayMs}
            disabled={!enabled}
            onChange={(e) => update({ preDelayMs: Number(e.target.value) })}
            aria-label="Pre-delay"
          />
          <span className="delaypanel__scale">{PRE_DELAY_MAX}</span>
        </div>
      </div>

      <div className="delaypanel__row">
        <div className="delaypanel__head">
          <span className="delaypanel__name">Filtro Low Cut</span>
          <span className="delaypanel__value">{formatHz(params.lowCutHz)}</span>
        </div>
        <div className="delaypanel__track">
          <span className="delaypanel__scale">{FILTER_MIN}</span>
          <input
            type="range"
            className="delaypanel__slider"
            min={FILTER_MIN}
            max={FILTER_MAX}
            step={10}
            value={params.lowCutHz}
            disabled={!enabled}
            onChange={(e) => update({ lowCutHz: Number(e.target.value) })}
            aria-label="Filtro low cut del delay"
          />
          <span className="delaypanel__scale">{FILTER_MAX}</span>
        </div>
      </div>

      <div className="delaypanel__row">
        <div className="delaypanel__head">
          <span className="delaypanel__name">Filtro High Cut</span>
          <span className="delaypanel__value">{formatHz(params.highCutHz)}</span>
        </div>
        <div className="delaypanel__track">
          <span className="delaypanel__scale">{FILTER_MIN}</span>
          <input
            type="range"
            className="delaypanel__slider"
            min={FILTER_MIN}
            max={FILTER_MAX}
            step={10}
            value={params.highCutHz}
            disabled={!enabled}
            onChange={(e) => update({ highCutHz: Number(e.target.value) })}
            aria-label="Filtro high cut del delay"
          />
          <span className="delaypanel__scale">{FILTER_MAX}</span>
        </div>
      </div>

      <div className="delaypanel__row">
        <div className="delaypanel__head">
          <span className="delaypanel__name">Ducking</span>
          <span className="delaypanel__value">{duckPercent}%</span>
        </div>
        <div className="delaypanel__track">
          <span className="delaypanel__scale">{DUCK_MIN * 100}%</span>
          <input
            type="range"
            className="delaypanel__slider"
            min={DUCK_MIN}
            max={DUCK_MAX}
            step={0.01}
            value={params.duckAmount}
            disabled={!enabled}
            onChange={(e) => update({ duckAmount: Number(e.target.value) })}
            aria-label="Ducking del delay"
          />
          <span className="delaypanel__scale">{DUCK_MAX * 100}%</span>
        </div>
      </div>

      <div className="delaypanel__foot">
        <span className="delaypanel__hint">
          {params.mode === "slapback"
            ? "Slapback: repetición única sin feedback."
            : params.mode === "tape"
              ? "Cinta: modulation analógica con wow & flutter."
              : params.mode === "analog"
                ? "Análogo: filtro en el loop de feedback para degradación cálida."
                : "Digital: delay limpio y preciso."}
        </span>
      </div>
    </div>
  );
}
