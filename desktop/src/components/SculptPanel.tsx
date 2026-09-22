import { memo } from "react";
import type { DspState, SculptParams } from "../lib/types";

const TONE_MIN = 0;
const TONE_MAX = 1;
const MIX_MIN = 0;
const MIX_MAX = 1;

interface SculptPanelProps {
  dsp: DspState | null;
  running: boolean;
  onSetSculpt: (params: SculptParams) => void;
}

const SculptPanel = memo(function SculptPanel({ dsp, running, onSetSculpt }: SculptPanelProps) {
  const link = dsp?.links.find((l) => l.name === "sculpt") ?? null;
  const params = link?.sculptParams ?? null;
  const bypassed = (link?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="sculptpanel">
        <p className="sculptpanel__empty">Arranca el motor para ajustar Sculpt.</p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="sculptpanel">
        <p className="sculptpanel__empty">El preset activo no incluye Sculpt.</p>
      </div>
    );
  }

  const update = (patch: Partial<SculptParams>) => {
    onSetSculpt({ ...params, ...patch });
  };

  const tonePercent = Math.round(params.tone * 100);
  const mixPercent = Math.round(params.mix * 100);

  return (
    <div className="sculptpanel">
      <div className="sculptpanel__header">
        <span className="sculptpanel__title">Sculpt</span>
        <span className={`sculptpanel__state ${bypassed ? "sculptpanel__state--off" : "sculptpanel__state--on"}`}>
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="sculptpanel__row">
        <div className="sculptpanel__head">
          <span className="sculptpanel__name">Tono</span>
          <span className="sculptpanel__value">{tonePercent}%</span>
        </div>
        <div className="sculptpanel__track">
          <span className="sculptpanel__scale">{TONE_MIN * 100}%</span>
          <input
            type="range"
            className="sculptpanel__slider"
            min={TONE_MIN}
            max={TONE_MAX}
            step={0.01}
            value={params.tone}
            disabled={!enabled}
            onChange={(e) => update({ tone: Number(e.target.value) })}
            aria-label="Tono de Sculpt"
          />
          <span className="sculptpanel__scale">{TONE_MAX * 100}%</span>
        </div>
      </div>

      <div className="sculptpanel__row">
        <div className="sculptpanel__head">
          <span className="sculptpanel__name">Mezcla</span>
          <span className="sculptpanel__value">{mixPercent}%</span>
        </div>
        <div className="sculptpanel__track">
          <span className="sculptpanel__scale">{MIX_MIN * 100}%</span>
          <input
            type="range"
            className="sculptpanel__slider"
            min={MIX_MIN}
            max={MIX_MAX}
            step={0.01}
            value={params.mix}
            disabled={!enabled}
            onChange={(e) => update({ mix: Number(e.target.value) })}
            aria-label="Mezcla de Sculpt"
          />
          <span className="sculptpanel__scale">{MIX_MAX * 100}%</span>
        </div>
      </div>

      <div className="sculptpanel__foot">
        <span className="sculptpanel__hint">
          {params.tone < 0.48
            ? "Tono oscuro: redondea graves y suaviza agudos."
            : params.tone > 0.52
              ? "Tono brillante: añade presencia y aire."
              : "Tono neutro: el preset suena como está diseñado."}
        </span>
      </div>
    </div>
  );
});

export { SculptPanel };