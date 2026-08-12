// Utilidades de formato para la UI. Estilo "cabina": monoespaciado, decimales
// controlados y unidades explícitas.

/** Umbral por debajo del cual el nivel se muestra como "-inf". */
const SILENCE_DB = -100;

/** Formatea un nivel dBFS con 1 decimal, o "-inf" si es silencio. */
export function formatDb(dbfs: number): string {
  if (dbfs <= SILENCE_DB || !Number.isFinite(dbfs)) return "-inf";
  return `${dbfs.toFixed(1)}`;
}

/** Formatea una frecuencia de muestreo en kHz legible. */
export function formatSampleRate(hz: number): string {
  if (hz >= 1000) return `${(hz / 1000).toFixed(1)} kHz`;
  return `${hz} Hz`;
}

/** Formatea una latencia en ms (enteros para valores ≥ 10 ms). */
export function formatLatency(ms: number): string {
  if (ms < 10) return `${ms.toFixed(1)} ms`;
  return `${Math.round(ms)} ms`;
}

/** Nombre corto de un dispositivo (recorta prefijos verbosos del sistema). */
export function shortDeviceName(name: string): string {
  const cleaned = name.replace(
    /^(Default (Audio|Output|Input)|BuiltIn (Output|Input)|麦克风|耳麦)\s*/i,
    "",
  );
  return cleaned.length > 42 ? `${cleaned.slice(0, 39)}…` : cleaned;
}

/** Color del indicador según nivel dBFS (verdes/cian normal, naranja crítico). */
export function levelColor(dbfs: number): "safe" | "warn" | "hot" {
  if (dbfs >= -6) return "hot";
  if (dbfs >= -18) return "warn";
  return "safe";
}
