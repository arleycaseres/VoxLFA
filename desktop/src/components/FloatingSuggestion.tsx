// Barra flotante de sugerencias de IA — overlay fijo que se ve siempre
// encima del contenido, con botón toggle para mostrar/ocultar.

import type { UiSuggestion } from "../lib/uiTypes";
import { getActionTarget } from "../lib/suggestionTargets";

interface FloatingSuggestionProps {
  suggestions: UiSuggestion[];
  onApply: (id: number) => void;
  onDismiss: (id: number) => void;
}

export function FloatingSuggestion({
  suggestions,
  onApply,
  onDismiss,
}: FloatingSuggestionProps) {
  if (suggestions.length === 0) return null;

  return (
    <div className="floating-suggestions">
      <div className="floating-suggestions__header">
        <span className="floating-suggestions__icon">💡</span>
        <span className="floating-suggestions__title">
          {suggestions.length === 1
            ? "1 ajuste recomendado"
            : `${suggestions.length} ajustes recomendados`}
        </span>
      </div>
      <div className="floating-suggestions__list">
        {suggestions.map((s) => {
          const target = getActionTarget(s.action);
          return (
            <div
              key={s.id}
              className={`floating-card floating-card--${s.severity}`}
            >
              <div className="floating-card__target">
                {target ? (
                  <>
                    <span className="floating-card__panel">{target.panel}</span>
                    <span className="floating-card__arrow">→</span>
                    <span className="floating-card__control">{target.control}</span>
                    {target.detail && (
                      <span className="floating-card__detail">{target.detail}</span>
                    )}
                  </>
                ) : (
                  <span className="floating-card__control">{s.consequence}</span>
                )}
              </div>
              <p className="floating-card__message">{s.consequence}</p>
              <div className="floating-card__actions">
                <button
                  className="btn btn--apply btn--tiny"
                  onClick={() => onApply(s.id)}
                >
                  Aplicar
                </button>
                <button
                  className="btn btn--ghost btn--tiny"
                  onClick={() => onDismiss(s.id)}
                >
                  Descartar
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default FloatingSuggestion;
