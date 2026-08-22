import type { DspState, SaturatorMode, SaturatorParams } from "../lib/types";

const DRIVE_MIN = 0;
const DRIVE_MAX = 1;
const MIX_MIN = 0;
const MIX_MAX = 1;

const SATURATOR_MODES: { value: SaturatorMode; label: string }[] = [
  { value: "tube", label: "Tube" },
  { value: "tape", label: "Tape" },
  { value: "tubeTape", label: "Tube + Tape" },
];

const MODE_HINTS: Record<SaturatorMode, string> = {
  tube: "Saturación de válvula: armónicos pares cálidos y musicales.",
  tape: "Saturación de cinta: compresión suave y rolloff natural.",
  tubeTape: "Valvular + cinta en cascada: riqueza armónica controlada.",
};

interface SaturatorPanelProps {
  dsp: DspState | null;
  running: boolean;
  onSetSaturator: (params: SaturatorParams) => void;
}

export function SaturatorPanel({ dsp, running, onSetSaturator }: SaturatorPanelProps) {
  const link = dsp?.links.find((l) => l.name === "saturator") ?? null;
  const params = link?.saturatorParams ?? null;
  const bypassed = (link?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="saturatorpanel">
        <p className="saturatorpanel__empty">Arranca el motor para ajustar la saturación.</p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="saturatorpanel">
        <p className="saturatorpanel__empty">El preset activo no incluye saturación.</p>
      </div>
    );
  }

  const update = (patch: Partial<SaturatorParams>) => {
    onSetSaturator({ ...params, ...patch });
  };

  const drivePercent = Math.round(params.drive * 100);
  const mixPercent = Math.round(params.mix * 100);

  return (
    <div className="saturatorpanel">
      <div className="saturatorpanel__header">
        <span className="saturatorpanel__title">Saturación</span>
        <span className={`saturatorpanel__state ${bypassed ? "saturatorpanel__state--off" : "saturatorpanel__state--on"}`}>
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="saturatorpanel__row">
        <div className="saturatorpanel__head">
          <span className="saturatorpanel__name">Modo</span>
        </div>
        <div className="saturatorpanel__modes">
          {SATURATOR_MODES.map((m) => (
            <button
              key={m.value}
              className={`saturatorpanel__mode ${params.mode === m.value ? "saturatorpanel__mode--active" : ""}`}
              disabled={!enabled}
              onClick={() => update({ mode: m.value })}
            >
              {m.label}
            </button>
          ))}
        </div>
      </div>

      <div className="saturatorpanel__row">
        <div className="saturatorpanel__head">
          <span className="saturatorpanel__name">Drive</span>
          <span className="saturatorpanel__value">{drivePercent}%</span>
        </div>
        <div className="saturatorpanel__track">
          <span className="saturatorpanel__scale">{DRIVE_MIN * 100}%</span>
          <input
            type="range"
            className="saturatorpanel__slider"
            min={DRIVE_MIN}
            max={DRIVE_MAX}
            step={0.01}
            value={params.drive}
            disabled={!enabled}
            onChange={(e) => update({ drive: Number(e.target.value) })}
            aria-label="Drive de saturación"
          />
          <span className="saturatorpanel__scale">{DRIVE_MAX * 100}%</span>
        </div>
      </div>

      <div className="saturatorpanel__row">
        <div className="saturatorpanel__head">
          <span className="saturatorpanel__name">Mezcla</span>
          <span className="saturatorpanel__value">{mixPercent}%</span>
        </div>
        <div className="saturatorpanel__track">
          <span className="saturatorpanel__scale">{MIX_MIN * 100}%</span>
          <input
            type="range"
            className="saturatorpanel__slider"
            min={MIX_MIN}
            max={MIX_MAX}
            step={0.01}
            value={params.mix}
            disabled={!enabled}
            onChange={(e) => update({ mix: Number(e.target.value) })}
            aria-label="Mezcla de saturación"
          />
          <span className="saturatorpanel__scale">{MIX_MAX * 100}%</span>
        </div>
      </div>

      <div className="saturatorpanel__foot">
        <span className="saturatorpanel__hint">
          {MODE_HINTS[params.mode]}
        </span>
      </div>
    </div>
  );
}
