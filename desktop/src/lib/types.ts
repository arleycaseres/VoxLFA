// Espejo TypeScript del protocolo de VoxLFA (ver docs/protocolo.md).
//
// Regla del proyecto: estos tipos coinciden 1:1 con `core/src/protocol/`
// (Rust/serde) y con `mobile/src/lib/protocol.ts`. Nunca cambies un nombre de
// campo sin actualizar los tres lados.

/** Estados posibles del motor de audio. */
export type EngineState =
  | "stopped"
  | "starting"
  | "running"
  | "stopping"
  | "error";

/** Descripción de un dispositivo de audio del sistema. */
export interface AudioDeviceInfo {
  /** Nombre único del dispositivo (identificador que usa el motor). */
  name: string;
  /** `true` si el sistema lo tiene como predeterminado. */
  isDefault: boolean;
}

/** Instantánea del estado y configuración del motor. */
export interface EngineStatus {
  state: EngineState;
  /** Frecuencia de muestreo (Hz). */
  sampleRate: number;
  /** Tamaño de buffer en muestras por callback. */
  bufferSize: number;
  /** Latencia medida captura→salida en ms. */
  latencyMs: number;
  /** Nombre del dispositivo de entrada en uso (si hay). */
  inputDevice: string | null;
  /** Nombre del dispositivo de salida en uso (si hay). */
  outputDevice: string | null;
}

/** Muestra de nivel del audio capturado/procesado y latencia actual. */
export interface LevelSample {
  /** Nivel RMS de la entrada en dBFS (silencio ≈ -120). */
  inputRmsDb: number;
  /** Nivel pico de la entrada en dBFS. */
  inputPeakDb: number;
  /** Nivel RMS de la salida (tras la cadena DSP) en dBFS. */
  outputRmsDb: number;
  /** Nivel pico de la salida en dBFS. */
  outputPeakDb: number;
  /** Latencia actual captura→salida en ms. */
  latencyMs: number;
  /** Tiempo monotónico (ms) de la captura; útil para gráficas. */
  capturedAtMs: number;
}

/** Identificador de un preset de la cabina. */
export type PresetId = "dry" | "vozLimpia" | "radio" | "warm";

/** Metadatos de un preset para mostrarlo en la cabina. */
export interface PresetInfo {
  id: PresetId;
  /** Nombre legible (en español) para la UI. */
  name: string;
  /** Descripción breve de una línea. */
  description: string;
}

/** Banda de un ecualizador paramétrico. */
export interface EqBand {
  /** Tipo de la banda (shelving o pico). */
  kind: EqBandKind;
  /** Frecuencia central (Hz). */
  freqHz: number;
  /** Ganancia en dB (negativo = corte). */
  gainDb: number;
  /** Factor de calidad Q (solo relevante para bandas de pico). */
  q: number;
}

/** Tipo de banda del ecualizador. */
export type EqBandKind = "lowShelf" | "peaking" | "highShelf";

/** Estado de un módulo dentro de la cadena activa. */
export interface DspLinkState {
  /** Nombre corto del módulo (identificador para el bypass). */
  name: string;
  /** `true` si el módulo está en la cadena (habilitado en el preset). */
  enabled: boolean;
  /** `true` si está en bypass (se omite en tiempo real). */
  bypass: boolean;
  /** Bandas actuales del EQ si este módulo es el ecualizador; si no, `null`. */
  eqBands: EqBand[] | null;
}

/** Estado completo de la cadena DSP activa. */
export interface DspState {
  /** Preset actualmente aplicado. */
  preset: PresetId;
  /** `true` si toda la cadena está en bypass (paso directo). */
  globalBypass: boolean;
  /** Módulos de la cadena, en orden de procesamiento. */
  links: DspLinkState[];
}

/** Listado de dispositivos de entrada/salida. */
export interface DeviceList {
  inputs: AudioDeviceInfo[];
  outputs: AudioDeviceInfo[];
}

/** Métricas de la voz sobre la ventana deslizante de análisis (dBFS). */
export interface VoiceMetrics {
  /** Nivel RMS medio de la ventana en dBFS. */
  rmsDb: number;
  /** Nivel pico de la ventana en dBFS. */
  peakDb: number;
  /** Rango dinámico de la ventana (dB). */
  dynamicRangeDb: number;
  /** Factor de cresta (dB entre pico y RMS). */
  crestDb: number;
  /** Brillo espectral (0–1): energía en agudos frente al total. */
  brightness: number;
  /** Resonancia baja-media (0–1): energía en la zona de boominess. */
  resonanceScore: number;
  /** Índice de fatiga vocal (0–1). */
  fatigueScore: number;
  /** Tamaño de la ventana de análisis en ms. */
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
  /** Identificador estable de la regla (para `applySuggestion`). */
  id: number;
  kind: SuggestionKind;
  /** Importancia (0–1). */
  severity: number;
  /** Mensaje legible en español. */
  message: string;
  action: SuggestionAction;
}

/** Muestra de análisis emitida por el motor (métricas + sugerencias). */
export interface AnalysisSample {
  metrics: VoiceMetrics;
  suggestions: Suggestion[];
  /** Tiempo monotónico (ms) de la captura. */
  capturedAtMs: number;
}

/** Resumen acumulado de la sesión de voz en curso. */
export interface SessionSummary {
  /** Tiempo (ms epoch) en el que arrancó la sesión. */
  startedAtMs: number;
  /** Duración de la sesión hasta ahora (ms). */
  durationMs: number;
  /** Nivel RMS medio de toda la sesión (dBFS). */
  avgRmsDb: number;
  /** Pico máximo de la sesión (dBFS). */
  peakDb: number;
  /** Rango dinámico observado (dB). */
  dynamicRangeDb: number;
  /** Brillo medio de la sesión (0–1). */
  avgBrightness: number;
  /** Índice de fatiga acumulado (0–1). */
  fatigueScore: number;
  /** Tiempo con nivel alto (RMS > -20 dBFS) en ms. */
  loudTimeMs: number;
  /** Número de sugerencias emitidas durante la sesión. */
  suggestionsCount: number;
}

/**
 * Evento emitido por el motor (tag `type`, campos en camelCase).
 * Los eventos `status`, `level`, `dsp` y `analysis` incluyen además todos los
 * campos de su respectiva estructura (serde los aplana).
 */
export type EngineEvent =
  | (EngineStatus & { type: "status" })
  | (LevelSample & { type: "level" })
  | (DeviceList & { type: "devices" })
  | (DspState & { type: "dsp" })
  | (AnalysisSample & { type: "analysis" })
  | { type: "warning"; message: string };
