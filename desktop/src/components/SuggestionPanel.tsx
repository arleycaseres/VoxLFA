// Panel del asistente de IA: métricas de voz en vivo, sugerencias
// accionables con confirmación, asesor IA (Groq) y resumen de sesión
// exportable a JSON.

import type {
  AnalysisSample,
  SessionSummary,
  Suggestion as RawSuggestion,
} from "../lib/types";
import type { UiSuggestion } from "../lib/uiTypes";
import { useEffect, useState } from "react";
// formatDb no se usa en este panel; se mantiene en otros componentes si es necesario
import SuggestionList from "./SuggestionList";
import SessionSummaryFooter from "./SessionSummaryFooter";

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
  /** Sugerencias generadas por el asesor de IA (Groq). */
  aiSuggestions: RawSuggestion[];
  /** Estado de carga del asesor IA. */
  aiLoading: boolean;
  /** Error del asesor IA (o vacío). */
  aiError: string;
  /** Solicita sugerencias al asesor de IA. */
  onRequestAi: () => void;
}

/** Nombre legible de cada área de la voz (en español). */

// preset display names are defined in other modules; not used here

/** Descripción legible de una acción de sugerencia para el botón. */

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
// helper formatDuration kept in SessionSummaryFooter if needed

export function SuggestionPanel({
  analysis,
  running,
  sessionSummary,
  onApplySuggestion,
  onRefreshSummary,
  aiSuggestions,
  aiLoading,
  aiError,
  onRequestAi,
}: SuggestionPanelProps) {
  const metrics = analysis?.metrics;

  // dismissed suggestions persisted in sessionStorage; keep local state to avoid mutating `analysis`
  const dismissedKey = "voxlfa:dismissedSuggestions";
  const [dismissed, setDismissed] = useState<number[]>(() => {
    try {
      const raw = sessionStorage.getItem(dismissedKey);
      return raw ? JSON.parse(raw) : [];
    } catch {
      return [];
    }
  });
  useEffect(() => {
    try {
      sessionStorage.setItem(dismissedKey, JSON.stringify(dismissed));
    } catch {}
  }, [dismissed]);

  // collapsed state for metrics (compact by default)
  const [metricsCollapsed, setMetricsCollapsed] = useState<boolean>(true);
  const toggleMetrics = () => setMetricsCollapsed((v) => !v);

  // map raw Suggestion -> UiSuggestion with enforced structure
  function mapToUi(s: RawSuggestion): UiSuggestion {
    const sev: UiSuggestion["severity"] = s.severity >= 0.75 ? "critical" : s.severity >= 0.4 ? "recommended" : "optional";
    // best-effort extraction for numeric detected value
    const numMatch = String(s.message).match(/([0-9]+(?:\.[0-9]+)?)(?:\s*)(ms|s|db|dB|%|:1)?/i);
    const detected = {
      label: s.kind === "dynamics" ? "Compresión" : s.kind === "fatigue" ? "Fatiga" : s.kind === "resonance" ? "Resonancia" : s.kind === "timbre" ? "Brillo" : "Medida",
      value: numMatch ? Number(numMatch[1]) : undefined,
      unit: numMatch && numMatch[2] ? numMatch[2] : undefined,
    };
    // try to split message into consequence + recommendation if structured
    const consequence = s.message || "";
    const recommendationLabel = s.action && s.action.type === "applyPreset" ? `Aplicar preset ${s.action.payload?.presetId ?? ""}` : s.suggestion ?? s.message ?? "Aplicar ajuste recomendado";
    return {
      id: s.id,
      kind: s.kind,
      detected,
      consequence,
      recommendation: { label: recommendationLabel, payload: s.action?.payload ?? null },
      severity: sev,
      action: s.action ?? null,
    };
  }

  const dismiss = (id: number) => setDismissed((prev) => Array.from(new Set([...prev, id])));

  return (
    <div className="ai">
      {/* Métricas de voz en vivo */}
      <div className={`ai__metrics ${metricsCollapsed ? "ai__metrics--collapsed" : ""}`}>
        {!metrics ? (
          <p className="ai-empty">
            {running
              ? "Analizando la voz… (se necesitan ~2 s de señal)."
              : "Arranca el motor para ver el análisis de la voz."}
          </p>
        ) : (
          <>
            <div className="ai__metrics__header">
              <button className="btn btn--ghost btn--small" onClick={toggleMetrics} aria-expanded={!metricsCollapsed}>
                {metricsCollapsed ? "Mostrar estado" : "Ocultar estado"}
              </button>
            </div>
            <div className="ai__metrics__body">
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
            </div>
          </>
        )}
      </div>
      {/* Sugerencias activas (heurísticas + IA) */}
      <h3 className="ai__subtitle">Sugerencias activas</h3>
      {!(analysis?.suggestions || aiSuggestions)?.length ? (
        <p className="ai-empty">Aún no hay sugerencias.</p>
      ) : (
        (() => {
          const raw: RawSuggestion[] = [
            ...(analysis?.suggestions ?? []),
            ...(aiSuggestions ?? []),
          ];
          const uiSugs: UiSuggestion[] = raw.map(mapToUi).filter((s) => !dismissed.includes(s.id));
          if (uiSugs.length === 0) return <p className="ai-empty">Voz equilibrada: sin sugerencias por ahora.</p>;
          // order by severity: critical, recommended, optional
          const rank = (sev: UiSuggestion["severity"]) => (sev === "critical" ? 0 : sev === "recommended" ? 1 : 2);
          uiSugs.sort((a, b) => rank(a.severity) - rank(b.severity) || ((b.detected.value ?? 0) - (a.detected.value ?? 0)));
          return (
            <div className="ai__list">
              <SuggestionList
                suggestions={uiSugs}
                onApply={(id) => onApplySuggestion(id)}
                onDismiss={(id) => dismiss(id)}
              />
            </div>
          );
        })()
      )}

      {/* Asesor IA (Groq) */}
      <h3 className="ai__subtitle">Asesor de IA</h3>
      <div className="ai-llm">
        <button
          type="button"
          className="btn btn--ghost btn--small"
          disabled={!running || aiLoading}
          onClick={onRequestAi}
        >
          {aiLoading ? "Consultando IA…" : "Pedir consejo a la IA"}
        </button>
        {aiError && <p className="ai-llm__error">{aiError}</p>}
        {(aiSuggestions ?? []).length > 0 && (
          <div className="ai__list">
            <SuggestionList
              suggestions={(aiSuggestions ?? []).map(mapToUi)}
              onApply={(id) => onApplySuggestion(id)}
              onDismiss={(id) => dismiss(id)}
            />
          </div>
        )}
      </div>

      {/* Footer: session summary + IA request */}
      {/* Rendered by SessionSummaryFooter component */}
      <SessionSummaryFooter
        summary={sessionSummary}
        onRefresh={onRefreshSummary}
        onRequestAi={onRequestAi}
        aiLoading={aiLoading}
      />
    </div>
  );
}
