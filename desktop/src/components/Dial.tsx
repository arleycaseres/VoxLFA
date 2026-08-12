// Dial central estilo "cabina": instrumento analógico que muestra el nivel de
// la señal en dBFS, con escala -60..0, aguja, arco de nivel y lectura digital.
//
// Ángulos medidos en grados en sentido horario desde las 12. La aguja barre
// 270° desde abajo-izquierda (225°, -60 dB) hasta abajo-derecha (135°, 0 dB).

import { useMemo } from "react";
import { levelColor } from "../lib/format";
import "./Dial.css";

/** Nivel mínimo de la escala (dBFS). */
const DB_MIN = -60;
/** Nivel máximo de la escala (dBFS). */
const DB_MAX = 0;
/** Ángulo inicial de la escala (abajo-izquierda). */
const ANGLE_START = 225;
/** Barrido total de la escala (270°, 3/4 de círculo). */
const ANGLE_SWEEP = 270;

/** Convierte un nivel dBFS en ángulo de la aguja (grados, sentido horario). */
function dbToAngle(dbfs: number): number {
  const f = Math.min(1, Math.max(0, (dbfs - DB_MIN) / (DB_MAX - DB_MIN)));
  return ANGLE_START + ANGLE_SWEEP * f;
}

/** Coordenadas de un punto de la escala dados ángulo y radio. */
function polar(cx: number, cy: number, r: number, angleDeg: number) {
  const rad = (angleDeg * Math.PI) / 180;
  return {
    x: cx + r * Math.sin(rad),
    y: cy - r * Math.cos(rad),
  };
}

/** Aplica un degradado lineal entre dos posiciones de la escala. */
function gradientStops(offset1: number, color1: string, color2: string) {
  return (
    <linearGradient id="dialGradient" x1="0" y1="0" x2="1" y2="0">
      <stop offset={`${offset1}%`} stopColor={color1} />
      <stop offset="100%" stopColor={color2} />
    </linearGradient>
  );
}

interface DialProps {
  /** Nivel pico de la señal en dBFS. */
  peakDb: number;
  /** Nivel RMS de la señal en dBFS (secundario). */
  rmsDb?: number;
  /** Etiqueta pequeña bajo la lectura digital. */
  label?: string;
  /** Diámetro del dial en píxeles. */
  size?: number;
}

export function Dial({ peakDb, rmsDb, label = "dBFS", size = 320 }: DialProps) {
  const cx = size / 2;
  const cy = size / 2;
  const r = size / 2 - 10;

  const { needleAngle, color, readout } = useMemo(() => {
    const angle = dbToAngle(peakDb);
    return {
      needleAngle: angle,
      color: levelColor(peakDb),
      readout: peakDb <= -100 ? "-inf" : peakDb.toFixed(1),
    };
  }, [peakDb]);

  // Líneas de la escala: mayor cada 5 dB, menor cada 1 dB.
  const ticks = useMemo(() => {
    const out: { x1: number; y1: number; x2: number; y2: number; major: boolean }[] = [];
    for (let db = DB_MIN; db <= DB_MAX; db += 1) {
      const major = db % 5 === 0;
      const rOuter = r - (major ? 6 : 12);
      const a = dbToAngle(db);
      const p1 = polar(cx, cy, rOuter, a);
      const p2 = polar(cx, cy, r - 2, a);
      out.push({
        x1: p1.x,
        y1: p1.y,
        x2: p2.x,
        y2: p2.y,
        major,
      });
    }
    return out;
  }, [cx, cy, r]);

  const needleTip = useMemo(() => polar(cx, cy, r - 18, needleAngle), [cx, cy, r, needleAngle]);
  const fillArcEnd = useMemo(() => polar(cx, cy, r - 4, needleAngle), [cx, cy, r, needleAngle]);
  const arcStart = polar(cx, cy, r - 4, ANGLE_START);
  const arcEnd = polar(cx, cy, r - 4, ANGLE_START + ANGLE_SWEEP);

  const colorClass = `dial--${color}`;

  return (
    <div className={`dial ${colorClass}`} style={{ width: size, height: size }}>
      <svg viewBox={`0 0 ${size} ${size}`} className="dial__svg" aria-hidden="true">
        {gradientStops(70, "#2E3944", "#4FD8FF")}

        {/* Cara del dial */}
        <circle cx={cx} cy={cy} r={r} className="dial__face" />
        <circle cx={cx} cy={cy} r={r} className="dial__rim" fill="none" />

        {/* Arco base (escala completa) */}
        <path
          d={`M ${arcStart.x} ${arcStart.y} A ${r - 4} ${r - 4} 0 1 1 ${arcEnd.x} ${arcEnd.y}`}
          className="dial__arc"
          fill="none"
        />

        {/* Arco de nivel actual */}
        <path
          d={`M ${arcStart.x} ${arcStart.y} A ${r - 4} ${r - 4} 0 1 1 ${fillArcEnd.x} ${fillArcEnd.y}`}
          className="dial__arc-fill"
          fill="none"
          stroke="url(#dialGradient)"
        />

        {/* Marcas de escala */}
        {ticks.map((tick, i) => (
          <line
            key={i}
            x1={tick.x1}
            y1={tick.y1}
            x2={tick.x2}
            y2={tick.y2}
            className={tick.major ? "dial__tick dial__tick--major" : "dial__tick"}
          />
        ))}

        {/* Aguja */}
        <line x1={cx} y1={cy} x2={needleTip.x} y2={needleTip.y} className="dial__needle" />
        <circle cx={cx} cy={cy} r={6} className="dial__hub" />
      </svg>

      {/* Lectura digital central */}
      <div className="dial__readout">
        <span className="dial__value">{readout}</span>
        <span className="dial__unit">{label}</span>
        {rmsDb !== undefined && (
          <span className="dial__rms">RMS {rmsDb <= -100 ? "-inf" : rmsDb.toFixed(1)}</span>
        )}
      </div>
    </div>
  );
}
