// Backend simulado (solo para desarrollo sin Tauri).
//
// Cuando la UI corre en un navegador normal (vite dev, fuera de la ventana de
// Tauri) no existen `invoke`/`listen`. Este módulo emula el backend para poder
// ver la cabina con señal simulada. NUNCA se usa dentro de la app de Tauri:
// `tauri.ts` lo activa solo si `window.__TAURI_INTERNALS__` no está presente.

import type {
  AnalysisSample,
  DeviceList,
  DspLinkState,
  DspState,
  EngineEvent,
  EngineState,
  EngineStatus,
  EqBand,
  LevelSample,
  PresetId,
  PresetInfo,
  SessionSummary,
  Suggestion,
  VoiceMetrics,
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
  vozLimpia: ["highpass", "boomsuppressor", "eq", "deesser", "compressor", "limiter"],
  radio: ["highpass", "notch", "eq", "saturator", "compressor", "limiter"],
  warm: ["highpass", "boomsuppressor", "eq", "compressor", "reverb", "limiter"],
};

/** Bandas del EQ por preset (espejo de los presets del core). */
const PRESET_EQ: Record<PresetId, EqBand[]> = {
  dry: [],
  vozLimpia: [
    { kind: "lowShelf", freqHz: 200, gainDb: -2, q: 0.8 },
    { kind: "peaking", freqHz: 3000, gainDb: 2, q: 1.5 },
    { kind: "highShelf", freqHz: 8000, gainDb: 1.5, q: 0.8 },
  ],
  radio: [
    { kind: "peaking", freqHz: 1000, gainDb: 6, q: 1.2 },
    { kind: "highShelf", freqHz: 3500, gainDb: -18, q: 0.8 },
  ],
  warm: [
    { kind: "lowShelf", freqHz: 120, gainDb: 3, q: 0.8 },
    { kind: "peaking", freqHz: 2500, gainDb: 1.5, q: 1.5 },
    { kind: "highShelf", freqHz: 7000, gainDb: -2, q: 0.8 },
  ],
};

/** Cada cuánto se emite una muestra de nivel (ms), igual que el core. */
const LEVEL_EMIT_INTERVAL_MS = 50;

/** Cada cuánto se emite una muestra de análisis (ms), igual que el core. */
const ANALYSIS_EMIT_INTERVAL_MS = 500;

/** Acumulador de la sesión de análisis simulada. */
interface SessionAccumulator {
  startedAtMs: number;
  frames: number;
  sumRms: number;
  minRms: number;
  maxRms: number;
  peak: number;
  sumBrightness: number;
  fatigueAcc: number;
  loudFrames: number;
  suggestionsCount: number;
}

/**
 * Heurística de buffer por dispositivo (espejo de la del core): USB → 128,
 * Bluetooth/HDMI → 1024, resto → 256.
 */
function heuristicBufferSize(): number {
  const names = [...FAKE_DEVICES.inputs, ...FAKE_DEVICES.outputs]
    .filter((d) => d.isDefault)
    .map((d) => d.name.toLowerCase())
    .join(" ");
  if (/(bluetooth|wireless|hdmi|displayport)/.test(names)) return 1024;
  if (/(usb|interface|scarlett|focusrite|steinberg|yamaha|presonus|rme)/.test(names)) {
    return 128;
  }
  return 256;
}

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

let hasRun = false;
let lastAnalysis: AnalysisSample | null = null;
let analysisPhase = 0;
let analysisTicker: ReturnType<typeof setInterval> | null = null;
let session: SessionAccumulator = newSession();

function newSession(): SessionAccumulator {
  return {
    startedAtMs: Date.now(),
    frames: 0,
    sumRms: 0,
    minRms: Number.POSITIVE_INFINITY,
    maxRms: Number.NEGATIVE_INFINITY,
    peak: Number.NEGATIVE_INFINITY,
    sumBrightness: 0,
    fatigueAcc: 0,
    loudFrames: 0,
    suggestionsCount: 0,
  };
}

function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

function buildDspState(preset: PresetId): DspState {
  const links: DspLinkState[] = PRESET_LINKS[preset].map((name) => ({
    name,
    enabled: true,
    bypass: false,
    eqBands: name === "eq" ? PRESET_EQ[preset] : null,
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

/** Acota un valor y lo devuelve. */
function suggest(
  id: number,
  kind: Suggestion["kind"],
  severity: number,
  message: string,
  action: Suggestion["action"],
): Suggestion {
  return { id, kind, severity: clamp01(severity), message, action };
}

/**
 * Genera métricas de voz simuladas y sus sugerencias (espejo de las reglas
 * del core: `suggest.rs`).
 */
function nextAnalysis(capturedAtMs: number): AnalysisSample {
  analysisPhase += 0.22;
  const loudness = clamp01((lastLevel?.inputRmsDb ?? -60) / 20 + 2);
  const brightness = clamp01(0.42 + Math.sin(analysisPhase * 0.7) * 0.28);
  const resonance = clamp01(0.3 + Math.cos(analysisPhase * 0.4) * 0.22);
  const dynamicRangeDb = 8 + 6 * Math.sin(analysisPhase * 0.3);
  const fatigue = clamp01(0.5 * loudness + 0.5 * brightness * loudness);
  const rmsDb = lastLevel?.inputRmsDb ?? -60;
  const peakDb = lastLevel?.inputPeakDb ?? -120;

  const metrics: VoiceMetrics = {
    rmsDb,
    peakDb,
    dynamicRangeDb: Math.max(0, dynamicRangeDb),
    crestDb: Math.max(0, peakDb - rmsDb),
    brightness,
    resonanceScore: resonance,
    fatigueScore: fatigue,
    windowMs: 2000,
  };

  const suggestions: Suggestion[] = [];
  if (resonance > 0.45) {
    suggestions.push(
      suggest(
        0,
        "resonance",
        (resonance - 0.45) / 0.25,
        "Se acumula energía en la zona baja-media (boominess). Aplica el preset 'Voz limpia' para reducir la banda de ~300 Hz.",
        { type: "applyPreset", preset: "vozLimpia" },
      ),
    );
  }
  if (brightness < 0.28) {
    suggestions.push(
      suggest(
        1,
        "timbre",
        (0.28 - brightness) / 0.12,
        "Timbre opaco: falta presencia en los agudos. Prueba el preset 'Voz limpia' para realzar la claridad.",
        { type: "applyPreset", preset: "vozLimpia" },
      ),
    );
  }
  if (brightness > 0.72) {
    suggestions.push(
      suggest(
        2,
        "timbre",
        (brightness - 0.72) / 0.15,
        "Timbre brillante/estridente. Suaviza los agudos con el preset 'Warm' para un tono más cálido.",
        { type: "applyPreset", preset: "warm" },
      ),
    );
  }
  if (dynamicRangeDb > 0 && dynamicRangeDb < 6) {
    suggestions.push(
      suggest(
        3,
        "dynamics",
        (6 - dynamicRangeDb) / 4,
        "La dinámica está muy comprimida. 'Warm' usa una compresión más ligera y deja respirar la voz.",
        { type: "applyPreset", preset: "warm" },
      ),
    );
  }
  if (dynamicRangeDb > 18) {
    suggestions.push(
      suggest(
        4,
        "dynamics",
        (dynamicRangeDb - 18) / 6,
        "Hay mucha variación de volumen. El preset 'Voz limpia' ayuda a controlar los picos sin sonar procesado.",
        { type: "applyPreset", preset: "vozLimpia" },
      ),
    );
  }
  if (fatigue > 0.55) {
    suggestions.push(
      suggest(
        5,
        "fatigue",
        (fatigue - 0.55) / 0.3,
        "Nivel alto sostenido: la voz muestra signos de fatiga. Considera pausas o reducir la ganancia de entrada.",
        { type: "none" },
      ),
    );
  }

  return { metrics, suggestions, capturedAtMs };
}

function updateSession(sample: AnalysisSample) {
  session.frames += 1;
  session.sumRms += sample.metrics.rmsDb;
  session.minRms = Math.min(session.minRms, sample.metrics.rmsDb);
  session.maxRms = Math.max(session.maxRms, sample.metrics.rmsDb);
  session.peak = Math.max(session.peak, sample.metrics.peakDb);
  session.sumBrightness += sample.metrics.brightness;
  session.fatigueAcc += sample.metrics.fatigueScore;
  if (sample.metrics.rmsDb > -20) session.loudFrames += 1;
  session.suggestionsCount += sample.suggestions.length;
}

function buildSessionSummary(): SessionSummary {
  const frames = Math.max(1, session.frames);
  return {
    startedAtMs: session.startedAtMs,
    durationMs: session.frames * ANALYSIS_EMIT_INTERVAL_MS,
    avgRmsDb: session.sumRms / frames,
    peakDb: session.peak,
    dynamicRangeDb: Math.max(0, session.maxRms - session.minRms),
    avgBrightness: session.sumBrightness / frames,
    fatigueScore: clamp01(session.fatigueAcc / frames),
    loudTimeMs: session.loudFrames * ANALYSIS_EMIT_INTERVAL_MS,
    suggestionsCount: session.suggestionsCount,
  };
}

function startAnalysisTicker() {
  stopAnalysisTicker();
  analysisPhase = 0;
  session = newSession();
  analysisTicker = setInterval(() => {
    const sample = nextAnalysis(Date.now());
    lastAnalysis = sample;
    updateSession(sample);
    emit({ type: "analysis", ...sample });
  }, ANALYSIS_EMIT_INTERVAL_MS);
}

function stopAnalysisTicker() {
  if (analysisTicker !== null) {
    clearInterval(analysisTicker);
    analysisTicker = null;
  }
}

export function listDevices(): Promise<DeviceList> {
  return Promise.resolve(FAKE_DEVICES);
}

export function startEngine(requested?: number | null): Promise<void> {
  return new Promise((resolve) => {
    state = "starting";
    bufferSize = requested ?? heuristicBufferSize();
    syncStatus();
    setTimeout(() => {
      state = "running";
      hasRun = true;
      syncStatus();
      syncDsp();
      startTicker();
      startAnalysisTicker();
      resolve();
    }, 650);
  });
}

export function stopEngine(): Promise<void> {
  return new Promise((resolve) => {
    stopTicker();
    stopAnalysisTicker();
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

export function setEqBand(bandIndex: number, gainDb: number): Promise<void> {
  dspState = {
    ...dspState,
    links: dspState.links.map((item) =>
      item.name === "eq" && item.eqBands
        ? {
            ...item,
            eqBands: item.eqBands.map((band, index) =>
              index === bandIndex ? { ...band, gainDb } : band,
            ),
          }
        : item,
    ),
  };
  syncDsp();
  return Promise.resolve();
}

export function getAnalysis(): Promise<AnalysisSample | null> {
  return Promise.resolve(hasRun ? lastAnalysis : null);
}

export function getSessionSummary(): Promise<SessionSummary | null> {
  return Promise.resolve(hasRun ? buildSessionSummary() : null);
}

export function applySuggestion(suggestionId: number): Promise<void> {
  const suggestion = lastAnalysis?.suggestions.find(
    (item) => item.id === suggestionId,
  );
  if (suggestion?.action.type === "applyPreset") {
    applyPreset(suggestion.action.preset);
  }
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
