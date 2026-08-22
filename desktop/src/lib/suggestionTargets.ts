// Mapa de acciones de sugerencia a nombres legibles de la UI.
// Cada acción se traduce a panel → control → detalle para que
// el usuario sepa exactamente dónde está el ajuste en la interfaz.

/** Acción genérica de la UI (puede tener campos extra). */
type AnyAction = { type: string; [key: string]: unknown } | null | undefined;

export type ActionTarget = {
  /** Nombre del panel en la UI (ej: "Ecualizador"). */
  panel: string;
  /** Nombre del control específico (ej: "Banda 1"). */
  control: string;
  /** Detalle adicional (ej: "500 Hz"). */
  detail?: string;
};

/** Nombres de las bandas EQ por índice. */
const EQ_BANDS: Record<number, string> = {
  0: "Banda 1 (200 Hz)",
  1: "Banda 2 (500 Hz)",
  2: "Banda 3 (1 kHz)",
  3: "Banda 4 (2 kHz)",
  4: "Banda 5 (3 kHz)",
  5: "Banda 6 (5 kHz)",
  6: "Banda 7 (8 kHz)",
};

/** Nombres de presets en español. */
const PRESET_NAMES: Record<string, string> = {
  dry: "Seco",
  vozLimpia: "Voz Limpia",
  radio: "Radio",
  warm: "Calidez",
};

/** Extrae el destino legible de una acción de sugerencia. */
export function getActionTarget(action: AnyAction): ActionTarget | null {
  if (!action || action.type === "none") return null;

  switch (action.type) {
    case "applyPreset":
      return {
        panel: "Presets",
        control: PRESET_NAMES[(action as any).preset] ?? (action as any).preset,
      };
    case "setEqBand":
      return {
        panel: "Ecualizador",
        control: EQ_BANDS[(action as any).bandIndex] ?? `Banda ${(action as any).bandIndex + 1}`,
        detail: `${(action as any).gainDb >= 0 ? "+" : ""}${((action as any).gainDb as number).toFixed(1)} dB`,
      };
    case "setDenoise":
      return {
        panel: "Supresión de ruido",
        control: "Nivel de reducción",
        detail: `${Math.round((action as any).mix * 100)}%`,
      };
    case "setFeedback":
      return {
        panel: "Antifeedback",
        control: "Umbral / Q",
        detail: `${(action as any).thresholdDb} dB, Q=${(action as any).q}`,
      };
    case "setPitchCorrection":
      return {
        panel: "Corrección de tono",
        control: "Intensidad / Mezcla",
        detail: `${Math.round((action as any).strength * 100)}% / ${Math.round((action as any).mix * 100)}%`,
      };
    case "setNoiseGate":
      return {
        panel: "Puerta de ruido",
        control: "Umbral / Rango",
        detail: `${(action as any).thresholdDb} dB, rango ${(action as any).rangeDb} dB`,
      };
    case "setDelay":
      return {
        panel: "Delay",
        control: "Tiempo / Mezcla",
        detail: `${(action as any).timeMs} ms, ${Math.round((action as any).mix * 100)}%`,
      };
    case "setReverb":
      return {
        panel: "Reverb",
        control: "Mezcla / Tamaño",
        detail: `${Math.round((action as any).wet * 100)}%, tamaño ${Math.round((action as any).roomSize * 100)}%`,
      };
    case "setSaturator":
      return {
        panel: "Saturación",
        control: "Drive / Mezcla",
        detail: `${(action as any).drive}, ${Math.round((action as any).mix * 100)}%`,
      };
  }

  return null;
}

/** Devuelve un texto de una línea describiendo la ubicación del ajuste. */
export function getTargetSummary(action: AnyAction): string {
  const t = getActionTarget(action);
  if (!t) return "";
  return t.detail ? `${t.panel} → ${t.control} (${t.detail})` : `${t.panel} → ${t.control}`;
}
