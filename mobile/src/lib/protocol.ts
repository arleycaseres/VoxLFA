// Espejo TypeScript del protocolo de VoxLFA para la app móvil.
//
// Regla del proyecto: estos tipos coinciden 1:1 con `core/src/protocol/`
// (Rust/serde) y con `desktop/src/lib/types.ts`. Nunca cambies un nombre de
// campo sin actualizar los tres lados. Este archivo NO depende de Tauri.

/** Estados posibles del motor de audio. */
export type EngineState =
  | "stopped"
  | "starting"
  | "running"
  | "stopping"
  | "error";

/** Descripción de un dispositivo de audio del sistema. */
export interface AudioDeviceInfo {
  name: string;
  isDefault: boolean;
}

/** Instantánea del estado y configuración del motor. */
export interface EngineStatus {
  state: EngineState;
  sampleRate: number;
  bufferSize: number;
  latencyMs: number;
  inputDevice: string | null;
  outputDevice: string | null;
}

/** Muestra de nivel del audio capturado/procesado y latencia actual. */
export interface LevelSample {
  inputRmsDb: number;
  inputPeakDb: number;
  outputRmsDb: number;
  outputPeakDb: number;
  latencyMs: number;
  capturedAtMs: number;
}

/** Número fijo de bandas logarítmicas del espectro emitido por el motor. */
export const SPECTRUM_BIN_COUNT = 32;

/** Muestra del espectro de la entrada (FFT) emitida en vivo. */
export interface SpectrumSample {
  /** Nivel de cada banda logarítmica en dBFS (longitud fija). */
  binsDb: number[];
  /** Frecuencia de muestreo (Hz) de la captura; define los bordes de banda. */
  sampleRate: number;
  /** Tiempo monotónico (ms) de la captura. */
  capturedAtMs: number;
}

/** Identificador de un preset de la cabina. */
export type PresetId = "dry" | "vozLimpia" | "radio" | "warm";

/** Banda de un ecualizador paramétrico. */
export interface EqBand {
  kind: EqBandKind;
  freqHz: number;
  gainDb: number;
  q: number;
}

/** Tipo de banda del ecualizador. */
export type EqBandKind = "lowShelf" | "peaking" | "highShelf";

/** Parámetros de la puerta de ruido de la cadena. */
export interface NoiseGateParams {
  /** Umbral de cierre en dBFS. */
  thresholdDb: number;
  /** Tiempo de ataque (ms). */
  attackMs: number;
  /** Tiempo de liberación (ms). */
  releaseMs: number;
  /** Tiempo de retención de la apertura (ms). */
  holdMs: number;
  /** Atenuación máxima cuando la puerta está cerrada (dB). */
  rangeDb: number;
}

/** Estado de un módulo dentro de la cadena activa. */
export interface DspLinkState {
  name: string;
  enabled: boolean;
  bypass: boolean;
  /** Bandas actuales del EQ si este módulo es el ecualizador; si no, `null`. */
  eqBands: EqBand[] | null;
  /** Parámetros actuales de la puerta si este módulo es la puerta; si no, `null`. */
  gateParams: NoiseGateParams | null;
}

/** Estado completo de la cadena DSP activa. */
export interface DspState {
  preset: PresetId;
  globalBypass: boolean;
  links: DspLinkState[];
}

/** Listado de dispositivos de entrada/salida. */
export interface DeviceList {
  inputs: AudioDeviceInfo[];
  outputs: AudioDeviceInfo[];
}

/** Métricas de la voz sobre la ventana deslizante de análisis (dBFS). */
export interface VoiceMetrics {
  rmsDb: number;
  peakDb: number;
  dynamicRangeDb: number;
  crestDb: number;
  brightness: number;
  resonanceScore: number;
  fatigueScore: number;
  windowMs: number;
}

/** Área de la voz que motiva una sugerencia. */
export type SuggestionKind =
  | "timbre"
  | "dynamics"
  | "fatigue"
  | "resonance";

/** Acción confirmable que acompaña a una sugerencia. */
export type SuggestionAction =
  | { type: "none" }
  | { type: "applyPreset"; preset: PresetId };

/** Sugerencia generada por el asistente para la voz actual. */
export interface Suggestion {
  id: number;
  kind: SuggestionKind;
  severity: number;
  message: string;
  action: SuggestionAction;
}

/** Muestra de análisis emitida por el motor (métricas + sugerencias). */
export interface AnalysisSample {
  metrics: VoiceMetrics;
  suggestions: Suggestion[];
  capturedAtMs: number;
}

/** Resumen acumulado de la sesión de voz en curso. */
export interface SessionSummary {
  startedAtMs: number;
  durationMs: number;
  avgRmsDb: number;
  peakDb: number;
  dynamicRangeDb: number;
  avgBrightness: number;
  fatigueScore: number;
  loudTimeMs: number;
  suggestionsCount: number;
}

/**
 * Comando de control enviado por el móvil al escritorio (tag `type`,
 * campos camelCase). Espejo de `core/src/protocol/control.rs`.
 *
 * `start` NO se incluye a propósito: arrancar el motor solo se permite desde
 * la cabina de escritorio.
 */
export type ControlCommand =
  | { type: "stop" }
  | { type: "setPreset"; preset: PresetId }
  | { type: "setGlobalBypass"; bypass: boolean }
  | { type: "setLinkBypass"; link: string; bypass: boolean }
  | { type: "setEqBand"; bandIndex: number; gainDb: number };

/**
 * Evento emitido por el motor por el WebSocket (tag `type`, campos camelCase).
 */
export type EngineEvent =
  | (EngineStatus & { type: "status" })
  | (LevelSample & { type: "level" })
  | (SpectrumSample & { type: "spectrum" })
  | (DeviceList & { type: "devices" })
  | (DspState & { type: "dsp" })
  | (AnalysisSample & { type: "analysis" })
  | { type: "warning"; message: string };

/** Guard de tipos para mensajes recibidos del WebSocket. */
export function isEngineEvent(raw: unknown): raw is EngineEvent {
  if (typeof raw !== "object" || raw === null) return false;
  const event = raw as Record<string, unknown>;
  if (typeof event.type !== "string") return false;
  switch (event.type) {
    case "status":
      return typeof event.sampleRate === "number" && typeof event.latencyMs === "number";
    case "level":
      return (
        typeof event.inputRmsDb === "number" &&
        typeof event.inputPeakDb === "number" &&
        typeof event.outputRmsDb === "number" &&
        typeof event.outputPeakDb === "number"
      );
    case "spectrum":
      return (
        Array.isArray(event.binsDb) &&
        event.binsDb.every((value) => typeof value === "number") &&
        typeof event.sampleRate === "number" &&
        typeof event.capturedAtMs === "number"
      );
    case "devices":
      return Array.isArray(event.inputs) && Array.isArray(event.outputs);
    case "dsp":
      return (
        typeof event.preset === "string" &&
        typeof event.globalBypass === "boolean" &&
        Array.isArray(event.links)
      );
    case "analysis":
      return (
        typeof event.capturedAtMs === "number" &&
        typeof event.metrics === "object" &&
        event.metrics !== null &&
        Array.isArray(event.suggestions)
      );
    case "warning":
      return typeof event.message === "string";
    default:
      return false;
  }
}
