import { useState } from "react";
import type { DspState, HarmonizerParams } from "../lib/types";

const MIX_MIN = 0;
const MIX_MAX = 1;
const VOICES_MIN = 1;
const VOICES_MAX = 4;

const PRESET_INTERVALS: { label: string; values: number[] }[] = [
  { label: "Octava", values: [-12, 0, 12] },
  { label: "Quinta", values: [-7, 0, 7] },
  { label: "Tercera", values: [-4, 0, 4] },
  { label: "Unísono", values: [0] },
  { label: "Octava↑", values: [0, 12] },
  { label: "Octava↓", values: [-12, 0] },
];

interface HarmonizerPanelProps {
  dsp: DspState | null;
  running: boolean;
  onSetHarmonizer: (params: HarmonizerParams) => void;
}

export function HarmonizerPanel({
  dsp,
  running,
  onSetHarmonizer,
}: HarmonizerPanelProps) {
  const link =
    dsp?.links.find((l) => l.name === "harmonizer") ?? null;
  const params = link?.harmonizerParams ?? null;
  const bypassed = (link?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  const [customInterval, setCustomInterval] = useState("");

  if (!running) {
    return (
      <div className="harmonizerpanel">
        <p className="harmonizerpanel__empty">
          Arranca el motor para ajustar el harmonizer.
        </p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="harmonizerpanel">
        <p className="harmonizerpanel__empty">
          El preset activo no incluye harmonizer.
        </p>
      </div>
    );
  }

  const isPresetMatch = (values: number[]) =>
    params.intervals.length === values.length &&
    [...params.intervals].sort((a, b) => a - b).every((v, i) =>
      v === [...values].sort((a, b) => a - b)[i],
    );

  const setIntervalPreset = (values: number[]) =>
    onSetHarmonizer({ ...params, intervals: values });

  const addCustomInterval = () => {
    const semitones = parseInt(customInterval, 10);
    if (Number.isNaN(semitones) || semitones < -24 || semitones > 24) return;
    if (params.intervals.includes(semitones)) return;
    onSetHarmonizer({
      ...params,
      intervals: [...params.intervals, semitones].sort((a, b) => a - b),
    });
    setCustomInterval("");
  };

  const removeInterval = (value: number) => {
    const next = params.intervals.filter((v) => v !== value);
    if (next.length === 0) return;
    onSetHarmonizer({ ...params, intervals: next });
  };

  return (
    <div className="harmonizerpanel">
      <div className="harmonizerpanel__header">
        <span className="harmonizerpanel__title">Harmonizer</span>
        <span
          className={`harmonizerpanel__state ${
            bypassed ? "harmonizerpanel__state--off" : "harmonizerpanel__state--on"
          }`}
        >
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="harmonizerpanel__row">
        <div className="harmonizerpanel__head">
          <span className="harmonizerpanel__name">Intervalos</span>
          <span className="harmonizerpanel__value">
            {params.intervals.map((v) =>
              v > 0 ? `+${v}` : `${v}`,
            ).join(", ") || "—"}
          </span>
        </div>
        <div className="harmonizerpanel__presets">
          {PRESET_INTERVALS.map((preset) => (
            <button
              key={preset.label}
              className={`harmonizerpanel__preset-btn ${
                isPresetMatch(preset.values)
                  ? "harmonizerpanel__preset-btn--active"
                  : ""
              }`}
              disabled={!enabled}
              onClick={() => setIntervalPreset(preset.values)}
            >
              {preset.label}
            </button>
          ))}
        </div>
      </div>

      <div className="harmonizerpanel__row">
        <div className="harmonizerpanel__head">
          <span className="harmonizerpanel__name">Intervalo custom</span>
        </div>
        <div className="harmonizerpanel__custom-row">
          <input
            type="number"
            className="harmonizerpanel__input"
            min={-24}
            max={24}
            step={1}
            placeholder="st"
            value={customInterval}
            onChange={(e) => setCustomInterval(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") addCustomInterval();
            }}
            disabled={!enabled}
            aria-label="Intervalo personalizado en semitonos"
          />
          <button
            className="harmonizerpanel__add-btn"
            disabled={!enabled || customInterval === ""}
            onClick={addCustomInterval}
          >
            +
          </button>
        </div>
        <div className="harmonizerpanel__chip-list">
          {params.intervals.map((v) => (
            <span key={v} className="harmonizerpanel__chip">
              {v > 0 ? `+${v}` : `${v}`} st
              <button
                className="harmonizerpanel__chip-x"
                disabled={!enabled}
                onClick={() => removeInterval(v)}
                aria-label={`Eliminar intervalo ${v}`}
              >
                ×
              </button>
            </span>
          ))}
        </div>
      </div>

      <div className="harmonizerpanel__row">
        <div className="harmonizerpanel__head">
          <span className="harmonizerpanel__name">Mezcla</span>
          <span className="harmonizerpanel__value">
            {Math.round(params.mix * 100)}%
          </span>
        </div>
        <div className="harmonizerpanel__track">
          <span className="harmonizerpanel__scale">{MIX_MIN * 100}%</span>
          <input
            type="range"
            className="harmonizerpanel__slider"
            min={MIX_MIN}
            max={MIX_MAX}
            step={0.01}
            value={params.mix}
            disabled={!enabled}
            onChange={(e) =>
              onSetHarmonizer({ ...params, mix: Number(e.target.value) })
            }
            aria-label="Mezcla seco/húmedo"
          />
          <span className="harmonizerpanel__scale">{MIX_MAX * 100}%</span>
        </div>
      </div>

      <div className="harmonizerpanel__row">
        <div className="harmonizerpanel__head">
          <span className="harmonizerpanel__name">Copias por voz</span>
          <span className="harmonizerpanel__value">{params.voicesPerInterval}</span>
        </div>
        <div className="harmonizerpanel__track">
          <span className="harmonizerpanel__scale">{VOICES_MIN}</span>
          <input
            type="range"
            className="harmonizerpanel__slider"
            min={VOICES_MIN}
            max={VOICES_MAX}
            step={1}
            value={params.voicesPerInterval}
            disabled={!enabled}
            onChange={(e) =>
              onSetHarmonizer({
                ...params,
                voicesPerInterval: Number(e.target.value),
              })
            }
            aria-label="Número de copias desafinadas por intervalo"
          />
          <span className="harmonizerpanel__scale">{VOICES_MAX}</span>
        </div>
      </div>

      <div className="harmonizerpanel__foot">
        <span className="harmonizerpanel__hint">
          Genera armonías en tiempo real. Cada intervalo produce voces desafinadas ±5 cents
          para un efecto de coro más rico.
        </span>
      </div>
    </div>
  );
}
