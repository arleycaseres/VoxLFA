import type { UiSuggestion } from "../lib/uiTypes";
import SuggestionCard from "./SuggestionCard";
import { useState } from "react";

export default function SuggestionList({ suggestions, onApply, onDismiss }: {
  suggestions: UiSuggestion[];
  onApply: (id: number) => void;
  onDismiss: (id: number) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const maxVisible = 3;
  const visible = expanded ? suggestions : suggestions.slice(0, maxVisible);
  return (
    <div className="suggestion-list">
      {visible.map((s) => (
        <SuggestionCard key={s.id} suggestion={s} onApply={onApply} onDismiss={onDismiss} />
      ))}
      {suggestions.length > maxVisible && (
        <button className="btn btn--ghost" aria-expanded={expanded} onClick={() => setExpanded((v) => !v)}>
          {expanded ? `Ver menos` : `Ver todas (${suggestions.length})`}
        </button>
      )}
    </div>
  );
}
