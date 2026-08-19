// Tarjeta de preset de efectos. Se pulsa para aplicar el preset a la cadena
// DSP en vivo; el preset activo se resalta.

interface PresetCardProps {
  /** Identificador del preset. */
  id: string;
  /** Nombre del preset. */
  name: string;
  /** Descripción breve (una línea). */
  description: string;
  /** Color de acento del preset. */
  accent: "cyan" | "orange";
  /** `true` si es el preset aplicado actualmente. */
  active: boolean;
  /** `true` si el control está deshabilitado (motor parado). */
  disabled: boolean;
  /** Se invoca al pulsar la tarjeta. */
  onSelect: () => void;
}

export function PresetCard({
  id,
  name,
  description,
  accent,
  active,
  disabled,
  onSelect,
}: PresetCardProps) {
  return (
    <button
      type="button"
      className={`preset preset--${accent} ${active ? "preset--active" : ""} ${
        disabled ? "preset--disabled" : ""
      }`}
      disabled={disabled}
      onClick={onSelect}
      aria-pressed={active}
      aria-label={`${name} preset ${active ? "activo" : ""}`}
    >
      <div className="preset__left">
        <div className="preset__name" title={name}>{name}</div>
        <div className="preset__desc" title={description}>{description}</div>
      </div>
      <div className="preset__right">
        {active && <span className="preset__badge">activo</span>}
        <div className="preset__id">{id}</div>
      </div>
    </button>
  );
}
