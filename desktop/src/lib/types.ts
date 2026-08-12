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

/** Estado de un módulo dentro de la cadena activa. */
export interface DspLinkState {
  /** Nombre corto del módulo (identificador para el bypass). */
  name: string;
  /** `true` si el módulo está en la cadena (habilitado en el preset). */
  enabled: boolean;
  /** `true` si está en bypass (se omite en tiempo real). */
  bypass: boolean;
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

/**
 * Evento emitido por el motor (tag `type`, campos en camelCase).
 * Los eventos `status`, `level` y `dsp` incluyen además todos los campos de su
 * respectiva estructura (serde los aplana).
 */
export type EngineEvent =
  | (EngineStatus & { type: "status" })
  | (LevelSample & { type: "level" })
  | (DeviceList & { type: "devices" })
  | (DspState & { type: "dsp" })
  | { type: "warning"; message: string };
