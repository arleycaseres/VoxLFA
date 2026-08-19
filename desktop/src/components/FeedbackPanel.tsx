import type { DspState, FeedbackSuppressorParams } from "../lib/types";

const THRESHOLD_MIN = -60;
const THRESHOLD_MAX = -10;
const Q_MIN = 2;
const Q_MAX = 30;

interface FeedbackPanelProps {
  dsp: DspState | null;
  running: boolean;
  onSetFeedback: (params: FeedbackSuppressorParams) => void;
}

export function FeedbackPanel({
  dsp,
  running,
  onSetFeedback,
}: FeedbackPanelProps) {
  const feedbackLink =
    dsp?.links.find((link) => link.name === "feedback") ?? null;
  const params = feedbackLink?.feedbackParams ?? null;
  const bypassed =
    (feedbackLink?.bypass ?? false) || (dsp?.globalBypass ?? false);
  const enabled = running && params !== null && !bypassed;

  if (!running) {
    return (
      <div className="feedbackpanel">
        <p className="feedbackpanel__empty">
          Arranca el motor para ajustar la supresión de feedback.
        </p>
      </div>
    );
  }

  if (params === null) {
    return (
      <div className="feedbackpanel">
        <p className="feedbackpanel__empty">
          El preset activo no incluye supresión de feedback.
        </p>
      </div>
    );
  }

  return (
    <div className="feedbackpanel">
      <div className="feedbackpanel__header">
        <span className="feedbackpanel__title">
          Supresión de feedback
        </span>
        <span
          className={`feedbackpanel__state ${
            bypassed ? "feedbackpanel__state--off" : "feedbackpanel__state--on"
          }`}
        >
          {bypassed ? "en bypass" : "en vivo"}
        </span>
      </div>

      <div className="feedbackpanel__row">
        <div className="feedbackpanel__head">
          <span className="feedbackpanel__name">Umbral</span>
          <span className="feedbackpanel__value">
            {params.thresholdDb} dBFS
          </span>
        </div>
        <div className="feedbackpanel__track">
          <span className="feedbackpanel__scale">{THRESHOLD_MIN}</span>
          <input
            type="range"
            className="feedbackpanel__slider"
            min={THRESHOLD_MIN}
            max={THRESHOLD_MAX}
            step={1}
            value={params.thresholdDb}
            disabled={!enabled}
            onChange={(e) =>
              onSetFeedback({
                ...params,
                thresholdDb: Number(e.target.value),
              })
            }
            aria-label="Umbral de detección de feedback"
          />
          <span className="feedbackpanel__scale">{THRESHOLD_MAX}</span>
        </div>
      </div>

      <div className="feedbackpanel__row">
        <div className="feedbackpanel__head">
          <span className="feedbackpanel__name">Calidad (Q)</span>
          <span className="feedbackpanel__value">{params.q.toFixed(0)}</span>
        </div>
        <div className="feedbackpanel__track">
          <span className="feedbackpanel__scale">{Q_MIN}</span>
          <input
            type="range"
            className="feedbackpanel__slider"
            min={Q_MIN}
            max={Q_MAX}
            step={1}
            value={params.q}
            disabled={!enabled}
            onChange={(e) =>
              onSetFeedback({
                ...params,
                q: Number(e.target.value),
              })
            }
            aria-label="Factor de calidad del notch"
          />
          <span className="feedbackpanel__scale">{Q_MAX}</span>
        </div>
      </div>

      <div className="feedbackpanel__foot">
        <span className="feedbackpanel__hint">
          Detecta y suprime resonancias de feedback (micrófono → altavoz).
        </span>
      </div>
    </div>
  );
}
