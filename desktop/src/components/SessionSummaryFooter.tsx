import type { SessionSummary } from "../lib/types";
import { useEffect, useRef, useState } from "react";

export function SessionSummaryFooter({ summary, onRefresh, onRequestAi: _onRequestAi, aiLoading: _aiLoading }: {
  summary: SessionSummary | null;
  onRefresh: () => void;
  onRequestAi: () => void;
  aiLoading: boolean;
}) {
  const [refreshing, setRefreshing] = useState(false);
  const lastStartedRef = useRef<number | null>(summary?.startedAtMs ?? null);

  useEffect(() => {
    // if summary changed (new startedAtMs), stop refreshing
    const started = summary?.startedAtMs ?? null;
    if (started && started !== lastStartedRef.current) {
      setRefreshing(false);
      lastStartedRef.current = started;
    }
  }, [summary]);

  const handleRefresh = () => {
    setRefreshing(true);
    try {
      onRefresh();
    } catch {
      setRefreshing(false);
    }
  };

  return (
    <div className="ai-footer">
      <div className="ai-footer__actions">
        <button className="btn btn--ghost" onClick={handleRefresh} disabled={refreshing} aria-busy={refreshing}>
          {refreshing ? "Actualizando…" : "Actualizar resumen"}
        </button>
      </div>
      <div className="ai-footer__summary" aria-live="polite">
        {summary ? (
          <div className="ai-footer__grid">
            <div className="ai-footer__row"><span>Duración</span><strong>{summary.durationMs != null ? `${Math.round(summary.durationMs / 1000)}s` : "—"}</strong></div>
            <div className="ai-footer__row"><span>Sugerencias</span><strong>{summary.suggestionsCount ?? 0}</strong></div>
          </div>
        ) : (
          <span className="ai-empty">Sin sesión todavía</span>
        )}
      </div>
    </div>
  );
}

export default SessionSummaryFooter;
