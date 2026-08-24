import type { DspState, FeedbackMode, FeedbackSuppressorParams } from "../lib/types";

const THRESHOLD_MIN = -60;
const THRESHOLD_MAX = -10;
const Q_MIN = 2;
const Q_MAX = 30;
const MU_MIN = 0.01;
const MU_MAX = 0.5;
const FILTER_LEN_MIN = 64;
const FILTER_LEN_MAX = 4096;

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

  const setMode = (mode: FeedbackMode) =>
    onSetFeedback({ ...params, mode });

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
          <span className="feedbackpanel__name">Modo</span>
        </div>
        <div className="feedbackpanel__modes">
          <button
            className={`feedbackpanel__mode-btn ${
              params.mode === "notch" ? "feedbackpanel__mode-btn--active" : ""
            }`}
            disabled={!enabled}
            onClick={() => setMode("notch")}
          >
            Notch (clásico)
          </button>
          <button
            className={`feedbackpanel__mode-btn ${
              params.mode === "adaptive"
                ? "feedbackpanel__mode-btn--active"
                : ""
            }`}
            disabled={!enabled}
            onClick={() => setMode("adaptive")}
          >
            FIR adaptativo
          </button>
        </div>
      </div>

      {params.mode === "notch" && (
        <>
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
        </>
      )}

      {params.mode === "adaptive" && (
        <>
          <div className="feedbackpanel__row">
            <div className="feedbackpanel__head">
              <span className="feedbackpanel__name">Tasa aprendizaje</span>
              <span className="feedbackpanel__value">
                {params.mu.toFixed(2)}
              </span>
            </div>
            <div className="feedbackpanel__track">
              <span className="feedbackpanel__scale">{MU_MIN}</span>
              <input
                type="range"
                className="feedbackpanel__slider"
                min={MU_MIN}
                max={MU_MAX}
                step={0.01}
                value={params.mu}
                disabled={!enabled}
                onChange={(e) =>
                  onSetFeedback({
                    ...params,
                    mu: Number(e.target.value),
                  })
                }
                aria-label="Tasa de aprendizaje del filtro FIR"
              />
              <span className="feedbackpanel__scale">{MU_MAX}</span>
            </div>
          </div>

          <div className="feedbackpanel__row">
            <div className="feedbackpanel__head">
              <span className="feedbackpanel__name">Taps del filtro</span>
              <span className="feedbackpanel__value">{params.filterLen}</span>
            </div>
            <div className="feedbackpanel__track">
              <span className="feedbackpanel__scale">{FILTER_LEN_MIN}</span>
              <input
                type="range"
                className="feedbackpanel__slider"
                min={FILTER_LEN_MIN}
                max={FILTER_LEN_MAX}
                step={64}
                value={params.filterLen}
                disabled={!enabled}
                onChange={(e) =>
                  onSetFeedback({
                    ...params,
                    filterLen: Number(e.target.value),
                  })
                }
                aria-label="Longitud del filtro FIR adaptativo"
              />
              <span className="feedbackpanel__scale">{FILTER_LEN_MAX}</span>
            </div>
          </div>
        </>
      )}

      <div className="feedbackpanel__foot">
        <span className="feedbackpanel__hint">
          {params.mode === "notch"
            ? "Detecta resonancias de feedback mediante FFT y aplica filtros muesca adaptativos."
            : "Modela la ruta de feedback (altavoz → micrófono) con un filtro FIR adaptativo y la cancela en tiempo real."}
        </span>
      </div>
    </div>
  );
}
