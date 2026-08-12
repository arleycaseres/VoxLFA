// Medidor de barra vertical para niveles de audio (estilo cabina).
//
// Muestra un nivel dBFS con una barra segmentada y una marca flotante para el
// pico. La zona caliente (≥ -6 dB) se ilumina en naranja de advertencia.

import "./Meter.css";

/** Nivel correspondiente al 100% de la barra. */
const DB_FULL = 0;
/** Nivel correspondiente al 0% de la barra. */
const DB_EMPTY = -48;

/** Altura en % para un nivel dBFS dado. */
function fillPercent(dbfs: number): number {
  const f = Math.min(1, Math.max(0, (dbfs - DB_EMPTY) / (DB_FULL - DB_EMPTY)));
  return Math.round(f * 100);
}

interface MeterProps {
  /** Nivel RMS actual (dBFS). */
  valueDb: number;
  /** Nivel pico mostrado (dBFS). */
  peakDb?: number;
  /** Etiqueta bajo la barra. */
  label: string;
}

export function Meter({ valueDb, peakDb, label }: MeterProps) {
  const fill = fillPercent(valueDb);
  const hot = valueDb >= -6;

  return (
    <div className={`meter ${hot ? "meter--hot" : ""}`}>
      <div className="meter__track">
        <div className="meter__fill" style={{ height: `${fill}%` }} />
        {/* Marca flotante del pico */}
        {peakDb !== undefined && (
          <div className="meter__peak" style={{ bottom: `${fillPercent(peakDb)}%` }} />
        )}
      </div>
      <span className="meter__label">{label}</span>
      <span className="meter__value">{valueDb <= -100 ? "-inf" : valueDb.toFixed(0)}</span>
    </div>
  );
}
