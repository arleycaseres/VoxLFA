// Panel del asistente de IA (Fase 2): métricas de voz en vivo, sugerencias
// accionables con confirmación y resumen de sesión exportable a JSON.

import type {
  AnalysisSample,
  PresetId,
  SessionSummary,
  Suggestion,
} from "../lib/types";
import { formatDb } from "../lib/format";

interface SuggestionPanelProps {
  /** Última muestra de análisis del motor (o `null` si no corre). */
  analysis: AnalysisSample | null;
  /** `true` si el motor está corriendo (el asistente solo aplica en vivo). */
  running: boolean;
  /** Resumen acumulado de la sesión (o `null` si aún no hay). */
  sessionSummary: SessionSummary | null;
  /** Aplica la acción de una sugerencia (con confirmación). */
  onApplySuggestion: (id: number) => void;
  /** Refresca el resumen de sesión desde el backend. */
  onRefreshSummary: () => void;
}

/** Nombre legible de cada área de la voz (en español). */
const KIND_LABELS: Record<Suggestion["kind"], string> = {
  timbre: "Timbre",
  dynamics: "Dinámica",
  fatigue: "Fatiga",
  resonance: "Resonancia",
};

const KIND_ICONS: Record<Suggestion["kind"], string> = {
  timbre: "✦",
  dynamics: "⇉",
  fatigue: "◌",
  resonance: "≈",
};

const KIND_COLORS: Record<Suggestion["kind"], string> = {
  timbre: "var(--color-cyan)",
  dynamics: "var(--color-cyan)",
  fatigue: "var(--color-accent)",
  resonance: "var(--color-accent)",
};

const PRESET_NAMES: Record<PresetId, string> = {
  dry: "Sin procesar",
  vozLimpia: "Voz limpia",
  radio: "Radio",
  warm: "Warm",
};

/** Barra de métrica 0–1 con etiqueta y valor porcentual. */
function MetricBar({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color: string;
}) {
  const pct = Math.min(100, Math.max(0, value * 100));
  return (
    <div className="ai-metric">
      <div className="ai-metric__row">
        <span className="ai-metric__label">{label}</span>
        <span className="ai-metric__value">{Math.round(pct)}%</span>
      </div>
      <div className="ai-metric__track">
        <div
          className="ai-metric__fill"
          style={{ width: `${pct}%`, background: color }}
        />
      </div>
    </div>
  );
}

/** Formatea una duración en mm:ss. */
function formatDuration(ms: number): string {
  const totalSeconds = Math.round(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

/** Descarga el resumen como archivo JSON. */
function exportJson(summary: SessionSummary) {
  const blob = new Blob([JSON.stringify(summary, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `voxlfa-sesion-${new Date().toISOString().slice(0, 19)}.json`;
  anchor.click();
  URL.revokeObjectURL(url);
}

export function SuggestionPanel({
  analysis,
  running,
  sessionSummary,
  onApplySuggestion,
  onRefreshSummary,
}: SuggestionPanelProps) {
  const metrics = analysis?.metrics;

  return (
    <div className="ai">
      {/* Métricas de voz en vivo */}
      <div className="ai__metrics">
        {!metrics ? (
          <p className="ai-empty">
            {running
              ? "Analizando la voz… (se necesitan ~2 s de señal)."
              : "Arranca el motor para ver el análisis de la voz."}
          </p>
        ) : (
          <>
            <MetricBar
              label="Brillo"
              value={metrics.brightness}
              color="var(--color-cyan)"
            />
            <MetricBar
              label="Resonancia"
              value={metrics.resonanceScore}
              color="var(--color-cyan)"
            />
            <MetricBar
              label="Fatiga"
              value={metrics.fatigueScore}
              color="var(--color-accent)"
            />
            <div className="ai-metric ai-metric--plain">
              <div className="ai-metric__row">
                <span className="ai-metric__label">Dinámica</span>
                <span className="ai-metric__value">
                  {metrics.dynamicRangeDb.toFixed(1)} dB
                </span>
              </div>
            </div>
          </>
        )}
      </div>

      {/* Sugerencias accionables */}
      <h3 className="ai__subtitle">Sugerencias</h3>
      {!metrics ? (
        <p className="ai-empty">Aún no hay sugerencias.</p>
      ) : analysis.suggestions.length === 0 ? (
        <p className="ai-empty">Voz equilibrada: sin sugerencias por ahora.</p>
      ) : (
        <div className="ai__list">
          {analysis.suggestions.map((suggestion) => (
            <div
              key={suggestion.id}
              className="ai-suggestion"
              style={{
                borderLeftColor: KIND_COLORS[suggestion.kind],
              }}
            >
              <span
                className="ai-suggestion__icon"
                style={{ color: KIND_COLORS[suggestion.kind] }}
              >
                {KIND_ICONS[suggestion.kind]}
              </span>
              <div className="ai-suggestion__body">
                <div className="ai-suggestion__head">
                  <span className="ai-suggestion__kind">
                    {KIND_LABELS[suggestion.kind]}
                  </span>
                  <span className="ai-suggestion__sev">
                    {Math.round(suggestion.severity * 100)}%
                  </span>
                </div>
                <p className="ai-suggestion__message">{suggestion.message}</p>
                {suggestion.action.type === "applyPreset" && (
                  <button
                    type="button"
                    className="btn btn--apply"
                    disabled={!running}
                    onClick={() => onApplySuggestion(suggestion.id)}
                  >
                    Aplicar {PRESET_NAMES[suggestion.action.preset]}
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Resumen de sesión */}
      <h3 className="ai__subtitle">Resumen de sesión</h3>
      <div className="ai-session">
        <button
          type="button"
          className="btn btn--ghost btn--small"
          onClick={onRefreshSummary}
        >
          Actualizar resumen
        </button>
        {!sessionSummary ? (
          <p className="ai-empty">
            {running
              ? "Acumulando la sesión…"
              : "Sin sesión registrada todavía."}
          </p>
        ) : (
          <>
            <div className="ai-session__grid">
              <span>Duración</span>
              <strong>{formatDuration(sessionSummary.durationMs)}</strong>
              <span>RMS medio</span>
              <strong>{formatDb(sessionSummary.avgRmsDb)} dBFS</strong>
              <span>Pico</span>
              <strong>{formatDb(sessionSummary.peakDb)} dBFS</strong>
              <span>Dinámica</span>
              <strong>{sessionSummary.dynamicRangeDb.toFixed(1)} dB</strong>
              <span>Brillo</span>
              <strong>{Math.round(sessionSummary.avgBrightness * 100)}%</strong>
              <span>Fatiga</span>
              <strong>{Math.round(sessionSummary.fatigueScore * 100)}%</strong>
              <span>Voz alta</span>
              <strong>{formatDuration(sessionSummary.loudTimeMs)}</strong>
              <span>Sugerencias</span>
              <strong>{sessionSummary.suggestionsCount}</strong>
            </div>
            <button
              type="button"
              className="btn btn--export"
              onClick={() => exportJson(sessionSummary)}
            >
              Exportar JSON
            </button>
          </>
        )}
      </div>
    </div>
  );
}
