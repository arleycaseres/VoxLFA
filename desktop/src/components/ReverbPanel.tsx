import type { DspState, ReverbParams, ReverbMode } from "../lib/types";

const ROOM_MIN = 0;
const ROOM_MAX = 1;
const DAMPING_MIN = 0;
const DAMPING_MAX = 1;
const WET_MIN = 0;
const WET_MAX = 1;
const PRE_DELAY_MIN = 0;
const PRE_DELAY_MAX = 200;
const FILTER_MIN = 20;
const FILTER_MAX = 20000;

const REVERB_MODES: { value: ReverbMode; label: string }[] = [
  { value: "plate", label: "Placa" },
  { value: "hall", label: "Sala" },
  { value: "room", label: "Habitación" },
];

function formatHz(value: number): string {
  if (value >= 1000) return `${(value / 1000).toFixed(1)} kHz`;
  return `${value.toFixed(0)} Hz`;
}

interface ReverbPanelProps {
  dsp: DspState | null;
  running: boolean;
  onSetReverb: (params: ReverbParams) => void;
}

export function ReverbPanel({ dsp, running, onSetReverb }: ReverbPanelProps) {
  const link = dsp?.links.find((l) => l.name === "reverb") ?? null;
  const params = link?.reverbParams ?? null;
  const bypassed = (link?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="reverbpanel">
        <p className="reverbpanel__empty">Arranca el motor para ajustar el reverb.</p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="reverbpanel">
        <p className="reverbpanel__empty">El preset activo no incluye reverb.</p>
      </div>
    );
  }

  const update = (patch: Partial<ReverbParams>) => {
    onSetReverb({ ...params, ...patch });
  };

  const roomPercent = Math.round(params.roomSize * 100);
  const dampingPercent = Math.round(params.damping * 100);
  const wetPercent = Math.round(params.wet * 100);

  return (
    <div className="reverbpanel">
      <div className="reverbpanel__header">
        <span className="reverbpanel__title">Reverb</span>
        <span className={`reverbpanel__state ${bypassed ? "reverbpanel__state--off" : "reverbpanel__state--on"}`}>
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="reverbpanel__row">
        <div className="reverbpanel__head">
          <span className="reverbpanel__name">Modo</span>
        </div>
        <div className="reverbpanel__modes">
          {REVERB_MODES.map((m) => (
            <button
              key={m.value}
              className={`reverbpanel__mode ${params.mode === m.value ? "reverbpanel__mode--active" : ""}`}
              disabled={!enabled}
              onClick={() => update({ mode: m.value })}
            >
              {m.label}
            </button>
          ))}
        </div>
      </div>

      <div className="reverbpanel__row">
        <div className="reverbpanel__head">
          <span className="reverbpanel__name">Tamaño</span>
          <span className="reverbpanel__value">{roomPercent}%</span>
        </div>
        <div className="reverbpanel__track">
          <span className="reverbpanel__scale">{ROOM_MIN * 100}%</span>
          <input
            type="range"
            className="reverbpanel__slider"
            min={ROOM_MIN}
            max={ROOM_MAX}
            step={0.01}
            value={params.roomSize}
            disabled={!enabled}
            onChange={(e) => update({ roomSize: Number(e.target.value) })}
            aria-label="Tamaño de sala del reverb"
          />
          <span className="reverbpanel__scale">{ROOM_MAX * 100}%</span>
        </div>
      </div>

      <div className="reverbpanel__row">
        <div className="reverbpanel__head">
          <span className="reverbpanel__name">Amortiguación</span>
          <span className="reverbpanel__value">{dampingPercent}%</span>
        </div>
        <div className="reverbpanel__track">
          <span className="reverbpanel__scale">{DAMPING_MIN * 100}%</span>
          <input
            type="range"
            className="reverbpanel__slider"
            min={DAMPING_MIN}
            max={DAMPING_MAX}
            step={0.01}
            value={params.damping}
            disabled={!enabled}
            onChange={(e) => update({ damping: Number(e.target.value) })}
            aria-label="Amortiguación del reverb"
          />
          <span className="reverbpanel__scale">{DAMPING_MAX * 100}%</span>
        </div>
      </div>

      <div className="reverbpanel__row">
        <div className="reverbpanel__head">
          <span className="reverbpanel__name">Mezcla</span>
          <span className="reverbpanel__value">{wetPercent}%</span>
        </div>
        <div className="reverbpanel__track">
          <span className="reverbpanel__scale">{WET_MIN * 100}%</span>
          <input
            type="range"
            className="reverbpanel__slider"
            min={WET_MIN}
            max={WET_MAX}
            step={0.01}
            value={params.wet}
            disabled={!enabled}
            onChange={(e) => update({ wet: Number(e.target.value) })}
            aria-label="Mezcla del reverb"
          />
          <span className="reverbpanel__scale">{WET_MAX * 100}%</span>
        </div>
      </div>

      <div className="reverbpanel__row">
        <div className="reverbpanel__head">
          <span className="reverbpanel__name">Pre-delay</span>
          <span className="reverbpanel__value">{params.preDelayMs.toFixed(0)} ms</span>
        </div>
        <div className="reverbpanel__track">
          <span className="reverbpanel__scale">{PRE_DELAY_MIN}</span>
          <input
            type="range"
            className="reverbpanel__slider"
            min={PRE_DELAY_MIN}
            max={PRE_DELAY_MAX}
            step={1}
            value={params.preDelayMs}
            disabled={!enabled}
            onChange={(e) => update({ preDelayMs: Number(e.target.value) })}
            aria-label="Pre-delay del reverb"
          />
          <span className="reverbpanel__scale">{PRE_DELAY_MAX}</span>
        </div>
      </div>

      <div className="reverbpanel__row">
        <div className="reverbpanel__head">
          <span className="reverbpanel__name">Filtro Low Cut</span>
          <span className="reverbpanel__value">{formatHz(params.lowCutHz)}</span>
        </div>
        <div className="reverbpanel__track">
          <span className="reverbpanel__scale">{FILTER_MIN}</span>
          <input
            type="range"
            className="reverbpanel__slider"
            min={FILTER_MIN}
            max={FILTER_MAX}
            step={10}
            value={params.lowCutHz}
            disabled={!enabled}
            onChange={(e) => update({ lowCutHz: Number(e.target.value) })}
            aria-label="Filtro low cut del reverb"
          />
          <span className="reverbpanel__scale">{FILTER_MAX}</span>
        </div>
      </div>

      <div className="reverbpanel__row">
        <div className="reverbpanel__head">
          <span className="reverbpanel__name">Filtro High Cut</span>
          <span className="reverbpanel__value">{formatHz(params.highCutHz)}</span>
        </div>
        <div className="reverbpanel__track">
          <span className="reverbpanel__scale">{FILTER_MIN}</span>
          <input
            type="range"
            className="reverbpanel__slider"
            min={FILTER_MIN}
            max={FILTER_MAX}
            step={10}
            value={params.highCutHz}
            disabled={!enabled}
            onChange={(e) => update({ highCutHz: Number(e.target.value) })}
            aria-label="Filtro high cut del reverb"
          />
          <span className="reverbpanel__scale">{FILTER_MAX}</span>
        </div>
      </div>

      <div className="reverbpanel__foot">
        <span className="reverbpanel__hint">
          {params.mode === "plate"
            ? "Placa: reverberación densa y brillante, ideal para vocales."
            : params.mode === "hall"
              ? "Sala: cola larga y envolvente para espacios grandes."
              : "Habitación: reverberación corta y natural."}
        </span>
      </div>
    </div>
  );
}
