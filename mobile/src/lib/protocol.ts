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

/** Identificador de un preset de la cabina. */
export type PresetId = "dry" | "vozLimpia" | "radio" | "warm";

/** Estado de un módulo dentro de la cadena activa. */
export interface DspLinkState {
  name: string;
  enabled: boolean;
  bypass: boolean;
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

/**
 * Evento emitido por el motor por el WebSocket (tag `type`, campos camelCase).
 */
export type EngineEvent =
  | (EngineStatus & { type: "status" })
  | (LevelSample & { type: "level" })
  | (DeviceList & { type: "devices" })
  | (DspState & { type: "dsp" })
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
    case "devices":
      return Array.isArray(event.inputs) && Array.isArray(event.outputs);
    case "dsp":
      return (
        typeof event.preset === "string" &&
        typeof event.globalBypass === "boolean" &&
        Array.isArray(event.links)
      );
    case "warning":
      return typeof event.message === "string";
    default:
      return false;
  }
}
