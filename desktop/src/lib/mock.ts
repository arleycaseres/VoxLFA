// Backend simulado (solo para desarrollo sin Tauri).
//
// Cuando la UI corre en un navegador normal (vite dev, fuera de la ventana de
// Tauri) no existen `invoke`/`listen`. Este módulo emula el backend para poder
// ver la cabina con señal simulada. NUNCA se usa dentro de la app de Tauri:
// `tauri.ts` lo activa solo si `window.__TAURI_INTERNALS__` no está presente.

import type {
  DeviceList,
  DspLinkState,
  DspState,
  EngineEvent,
  EngineState,
  EngineStatus,
  LevelSample,
  PresetId,
  PresetInfo,
} from "./types";
import type { PairingInfo } from "./tauri";

/** Datos de emparejamiento simulados. */
const FAKE_PAIRING: PairingInfo = {
  code: "VX7K9Q",
  port: 4356,
  lanAddress: "192.168.1.24",
};

/** Dispositivos de audio simulados. */
const FAKE_DEVICES: DeviceList = {
  inputs: [
    { name: "Micrófono (USB Audio)", isDefault: true },
    { name: "Interfaz Scarlett 2i2", isDefault: false },
    { name: "BuiltIn Microphone", isDefault: false },
  ],
  outputs: [
    { name: "Altavoces (USB Audio)", isDefault: true },
    { name: "Monitor 01", isDefault: false },
    { name: "BuiltIn Output", isDefault: false },
  ],
};

/** Presets simulados (espejo de `PresetFactory::all()` en el core). */
const FAKE_PRESETS: PresetInfo[] = [
  {
    id: "dry",
    name: "Sin procesar",
    description: "Paso directo de la señal, sin efectos.",
  },
  {
    id: "vozLimpia",
    name: "Voz limpia",
    description: "EQ suave y compresión transparente para canto.",
  },
  {
    id: "radio",
    name: "Radio",
    description: "Carácter telefónico (banda estrecha + saturación).",
  },
  {
    id: "warm",
    name: "Warm",
    description: "Bajos suaves y presencia vocal cálida.",
  },
];

/** Nombres de módulo por preset (espejo de `PresetFactory::specs`). */
const PRESET_LINKS: Record<PresetId, string[]> = {
  dry: [],
  vozLimpia: ["highpass", "eq", "deesser", "compressor", "limiter"],
  radio: ["highpass", "eq", "saturator", "compressor", "limiter"],
  warm: ["highpass", "eq", "compressor", "reverb", "limiter"],
};

/** Cada cuánto se emite una muestra de nivel (ms), igual que el core. */
const LEVEL_EMIT_INTERVAL_MS = 50;

type Listener = (event: EngineEvent) => void;

const listeners = new Set<Listener>();
let ticker: ReturnType<typeof setInterval> | null = null;
let state: EngineState = "stopped";
let sampleRate = 48000;
let bufferSize = 256;
let phase = 0;
let lastLevel: LevelSample | null = null;
let lastStatus: EngineStatus | null = null;
let dspState: DspState = buildDspState("dry");

function buildDspState(preset: PresetId): DspState {
  const links: DspLinkState[] = PRESET_LINKS[preset].map((name) => ({
    name,
    enabled: true,
    bypass: false,
  }));
  return { preset, globalBypass: false, links };
}

function emit(event: EngineEvent) {
  for (const listener of listeners) listener(event);
}

/** Genera una señal vocal simulada (oscila y con "respiración"). */
function nextLevel(capturedAtMs: number): LevelSample {
  phase += 0.35;
  // Envolvente tipo habla: picos periódicos con silencios entre frases.
  const breath = (Math.sin(phase * 0.11) + 1) / 2;
  const rms = -26 + 14 * breath + Math.sin(phase) * 1.5;
  const peak = Math.min(rms + 6 + Math.random() * 2, -1);
  const latencyMs = 7.5 + Math.random() * 1.5;
  // La salida simula el efecto del preset: el limitador recorta el pico.
  const limited = Math.min(peak + 2, -1);
  const outRms = dspState.globalBypass
    ? rms
    : Math.max(rms - 2 + (dspState.links.length ? 3 : 0), -60);
  return {
    inputRmsDb: rms,
    inputPeakDb: peak,
    outputRmsDb: outRms,
    outputPeakDb: limited,
    latencyMs,
    capturedAtMs,
  };
}

function buildStatus(): EngineStatus {
  return {
    state,
    sampleRate,
    bufferSize,
    latencyMs: lastLevel?.latencyMs ?? 8,
    inputDevice: state === "stopped" ? null : FAKE_DEVICES.inputs[0].name,
    outputDevice: state === "stopped" ? null : FAKE_DEVICES.outputs[0].name,
  };
}

function syncStatus() {
  lastStatus = buildStatus();
  emit({ type: "status", ...lastStatus });
}

function syncDsp() {
  emit({ type: "dsp", ...dspState });
}

function startTicker() {
  stopTicker();
  phase = 0;
  ticker = setInterval(() => {
    lastLevel = nextLevel(Date.now());
    emit({ type: "level", ...lastLevel });
  }, LEVEL_EMIT_INTERVAL_MS);
}

function stopTicker() {
  if (ticker !== null) {
    clearInterval(ticker);
    ticker = null;
  }
}

export function listDevices(): Promise<DeviceList> {
  return Promise.resolve(FAKE_DEVICES);
}

export function startEngine(): Promise<void> {
  return new Promise((resolve) => {
    state = "starting";
    syncStatus();
    setTimeout(() => {
      state = "running";
      syncStatus();
      syncDsp();
      startTicker();
      resolve();
    }, 650);
  });
}

export function stopEngine(): Promise<void> {
  return new Promise((resolve) => {
    stopTicker();
    state = "stopping";
    syncStatus();
    setTimeout(() => {
      state = "stopped";
      syncStatus();
      resolve();
    }, 250);
  });
}

export function getEngineStatus(): Promise<EngineStatus | null> {
  return Promise.resolve(lastStatus);
}

export function getLastLevel(): Promise<LevelSample | null> {
  return Promise.resolve(lastLevel);
}

export function getPresets(): Promise<PresetInfo[]> {
  return Promise.resolve(FAKE_PRESETS);
}

export function getDspState(): Promise<DspState | null> {
  return Promise.resolve(state === "stopped" ? null : dspState);
}

export function applyPreset(preset: PresetId): Promise<void> {
  dspState = buildDspState(preset);
  syncDsp();
  return Promise.resolve();
}

export function setGlobalBypass(bypass: boolean): Promise<void> {
  dspState = { ...dspState, globalBypass: bypass };
  syncDsp();
  return Promise.resolve();
}

export function setLinkBypass(link: string, bypass: boolean): Promise<void> {
  dspState = {
    ...dspState,
    links: dspState.links.map((item) =>
      item.name === link ? { ...item, bypass } : item,
    ),
  };
  syncDsp();
  return Promise.resolve();
}

export function getPairingInfo(): Promise<PairingInfo> {
  return Promise.resolve(FAKE_PAIRING);
}

export function onEngineEvent(handler: Listener): () => void {
  listeners.add(handler);
  return () => {
    listeners.delete(handler);
  };
}
