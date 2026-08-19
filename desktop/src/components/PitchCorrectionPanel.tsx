// Panel de control de corrección de tono: escala, nota raíz, intensidad y mezcla.
//
// Cada cambio se aplica en vivo (`set_pitch_correction`): el core reconstruye
// solo el módulo de corrección de tono y lo conmuta por puntero.

import type {
  DspState,
  MusicalNote,
  MusicalScale,
  PitchCorrectionParams,
} from "../lib/types";

const STRENGTH_MIN = 0;
const STRENGTH_MAX = 1;
const MIX_MIN = 0;
const MIX_MAX = 1;

const SCALE_OPTIONS: { value: MusicalScale; label: string }[] = [
  { value: "chromatic", label: "Cromática" },
  { value: "major", label: "Mayor" },
  { value: "minorNatural", label: "Menor natural" },
  { value: "minorHarmonic", label: "Menor armónica" },
  { value: "pentatonicMajor", label: "Pentatónica mayor" },
  { value: "pentatonicMinor", label: "Pentatónica menor" },
  { value: "blues", label: "Blues" },
];

const NOTE_OPTIONS: { value: MusicalNote; label: string }[] = [
  { value: "c", label: "C" },
  { value: "cs", label: "C#" },
  { value: "d", label: "D" },
  { value: "ds", label: "D#" },
  { value: "e", label: "E" },
  { value: "f", label: "F" },
  { value: "fs", label: "F#" },
  { value: "g", label: "G" },
  { value: "gs", label: "G#" },
  { value: "a", label: "A" },
  { value: "as", label: "A#" },
  { value: "b", label: "B" },
];

interface PitchCorrectionPanelProps {
  /** Estado de la cadena DSP (o `null` si el motor no corre). */
  dsp: DspState | null;
  running: boolean;
  /** Aplica los parámetros de corrección de tono en vivo. */
  onSetPitchCorrection: (params: PitchCorrectionParams) => void;
}

export function PitchCorrectionPanel({
  dsp,
  running,
  onSetPitchCorrection,
}: PitchCorrectionPanelProps) {
  const pcLink =
    dsp?.links.find((link) => link.name === "pitch_correction") ?? null;
  const params = pcLink?.pitchCorrectionParams ?? null;
  const bypassed = (pcLink?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="pitchcorrectionpanel">
        <p className="pitchcorrectionpanel__empty">
          Arranca el motor para ajustar la corrección de tono.
        </p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="pitchcorrectionpanel">
        <p className="pitchcorrectionpanel__empty">
          El preset activo no incluye corrección de tono.
        </p>
      </div>
    );
  }

  const strengthPercent = Math.round(params.strength * 100);
  const mixPercent = Math.round(params.mix * 100);

  return (
    <div className="pitchcorrectionpanel">
      <div className="pitchcorrectionpanel__header">
        <span className="pitchcorrectionpanel__title">
          Corrección de tono
        </span>
        <span
          className={`pitchcorrectionpanel__state ${
            bypassed
              ? "pitchcorrectionpanel__state--off"
              : "pitchcorrectionpanel__state--on"
          }`}
        >
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="pitchcorrectionpanel__row">
        <div className="pitchcorrectionpanel__head">
          <span className="pitchcorrectionpanel__name">Escala</span>
        </div>
        <select
          className="pitchcorrectionpanel__select"
          value={params.scale}
          disabled={!enabled}
          onChange={(e) =>
            onSetPitchCorrection({
              ...params,
              scale: e.target.value as MusicalScale,
            })
          }
          aria-label="Escala musical"
        >
          {SCALE_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>

      <div className="pitchcorrectionpanel__row">
        <div className="pitchcorrectionpanel__head">
          <span className="pitchcorrectionpanel__name">Nota raíz</span>
        </div>
        <select
          className="pitchcorrectionpanel__select"
          value={params.root}
          disabled={!enabled}
          onChange={(e) =>
            onSetPitchCorrection({
              ...params,
              root: e.target.value as MusicalNote,
            })
          }
          aria-label="Nota raíz"
        >
          {NOTE_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>

      <div className="pitchcorrectionpanel__row">
        <div className="pitchcorrectionpanel__head">
          <span className="pitchcorrectionpanel__name">Intensidad</span>
          <span className="pitchcorrectionpanel__value">
            {strengthPercent}%
          </span>
        </div>
        <div className="pitchcorrectionpanel__track">
          <span className="pitchcorrectionpanel__scale">
            {STRENGTH_MIN * 100}%
          </span>
          <input
            type="range"
            className="pitchcorrectionpanel__slider"
            min={STRENGTH_MIN}
            max={STRENGTH_MAX}
            step={0.01}
            value={params.strength}
            disabled={!enabled}
            onChange={(e) =>
              onSetPitchCorrection({
                ...params,
                strength: Number(e.target.value),
              })
            }
            aria-label="Intensidad de corrección"
          />
          <span className="pitchcorrectionpanel__scale">
            {STRENGTH_MAX * 100}%
          </span>
        </div>
      </div>

      <div className="pitchcorrectionpanel__row">
        <div className="pitchcorrectionpanel__head">
          <span className="pitchcorrectionpanel__name">Mezcla</span>
          <span className="pitchcorrectionpanel__value">{mixPercent}%</span>
        </div>
        <div className="pitchcorrectionpanel__track">
          <span className="pitchcorrectionpanel__scale">
            {MIX_MIN * 100}%
          </span>
          <input
            type="range"
            className="pitchcorrectionpanel__slider"
            min={MIX_MIN}
            max={MIX_MAX}
            step={0.01}
            value={params.mix}
            disabled={!enabled}
            onChange={(e) =>
              onSetPitchCorrection({
                ...params,
                mix: Number(e.target.value),
              })
            }
            aria-label="Mezcla de corrección de tono"
          />
          <span className="pitchcorrectionpanel__scale">
            {MIX_MAX * 100}%
          </span>
        </div>
      </div>

      <div className="pitchcorrectionpanel__foot">
        <span className="pitchcorrectionpanel__hint">
          {strengthPercent === 0
            ? "Sin corrección de tono."
            : strengthPercent < 50
              ? "Corrección sutil — suaviza desafinaciones."
              : strengthPercent < 80
                ? "Corrección moderada — efecto Auto-Tune suave."
                : "Corrección fuerte — efecto Auto-Tune pronunciado."}
        </span>
      </div>
    </div>
  );
}
