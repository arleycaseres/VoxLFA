import type { UiSuggestion } from "../lib/uiTypes";

export function SuggestionCard({ suggestion, onApply, onDismiss }: {
  suggestion: UiSuggestion;
  onApply: (id: number) => void;
  onDismiss: (id: number) => void;
}) {
  const sevText = suggestion.severity === "critical" ? "Crítico" : suggestion.severity === "recommended" ? "Recomendado" : "Opcional";
  return (
    <div className="suggestion-card" data-severity={suggestion.severity}>
      <div className={`suggestion-card__badge badge--${suggestion.severity}`}>
        <span className="suggestion-card__badge-text">{sevText}</span>
      </div>
      <div className="suggestion-card__main">
        <div className="suggestion-card__head">
          <div>
            <div className="suggestion-card__detected">
              {suggestion.detected.label}
              {suggestion.detected.value != null ? (
                <span className="suggestion-card__detected-value">: {suggestion.detected.value}{suggestion.detected.unit || ''}</span>
              ) : null}
            </div>
            <p className="suggestion-card__consequence">{suggestion.consequence}</p>
          </div>
          <div className="suggestion-card__rec">
            <span className="suggestion-card__rec-label">Qué hacer</span>
            <span className="suggestion-card__rec-action">{suggestion.recommendation.label}</span>
          </div>
        </div>
        <div className="suggestion-card__actions">
          <button className="btn btn--apply" onClick={() => onApply(suggestion.id)} aria-label={`Aplicar ${suggestion.recommendation.label}`}>Aplicar</button>
          <button className="btn btn--ghost" onClick={() => onDismiss(suggestion.id)} aria-label={`Descartar sugerencia ${suggestion.id}`}>Descartar</button>
        </div>
      </div>
    </div>
  );
}

export default SuggestionCard;
