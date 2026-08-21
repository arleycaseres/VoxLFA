// Acceso tipado a Tauri. La UI NUNCA llama `invoke`/`listen` directamente:
// todo pasa por estas funciones para mantener el contrato en un solo sitio.
//
// Nota: los nombres de argumento van en camelCase porque Tauri v2 convierte
// los parámetros snake_case de Rust a camelCase en el lado JS por defecto.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import * as mock from "./mock";
import type {
  AnalysisSample,
  AppConfig,
  DelayParams,
  DenoiseParams,
  DeviceList,
  DspState,
  EngineEvent,
  EngineStatus,
  FeedbackSuppressorParams,
  HostList,
  LevelSample,
  ModelStatus,
  NoiseGateParams,
  PitchCorrectionParams,
  PresetId,
  PresetInfo,
  ReverbParams,
  SessionSummary,
  SpectrumSample,
  Suggestion,
} from "./types";

/** `true` dentro de la ventana real de Tauri; `false` en un navegador plano. */
function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Información de emparejamiento mostrada al usuario para conectar el móvil. */
export interface PairingInfo {
  /** Código de emparejamiento actual (6 caracteres). */
  code: string;
  /** Puerto del WebSocket del servidor de eventos. */
  port: number;
  /** Dirección LAN del equipo o `null` si no se pudo resolver. */
  lanAddress: string | null;
}

/** Estados que el frontend puede solicitar al backend. */
export interface DeviceListRequest {
  inputDevice: string | null;
  outputDevice: string | null;
}

/** Lista los dispositivos de audio disponibles en el sistema. */
export function listDevices(): Promise<DeviceList> {
  return inTauri() ? invoke<DeviceList>("list_devices") : mock.listDevices();
}

/** Lista los hosts de audio disponibles (ALSA, JACK, PipeWire, etc.). */
export function listAudioHosts(): Promise<HostList> {
  return inTauri()
    ? invoke<HostList>("list_audio_hosts")
    : mock.listAudioHosts();
}

/** Lista los dispositivos de un host específico. */
export function listDevicesForHost(hostId: string): Promise<DeviceList> {
  return inTauri()
    ? invoke<DeviceList>("list_devices_for_host", { hostId })
    : mock.listDevices();
}

/** Arranca el motor con los dispositivos dados (`null` = predeterminado).
 *  `bufferSize` (`null` = auto por heurística de dispositivo).
 *  `audioHost` (`null` = predeterminado del sistema; p. ej. `"jack"`, `"alsa"`). */
export function startEngine(
  inputDevice: string | null,
  outputDevice: string | null,
  bufferSize?: number | null,
  audioHost?: string | null,
): Promise<void> {
  return inTauri()
    ? invoke<void>("start_engine", { inputDevice, outputDevice, bufferSize, audioHost })
    : mock.startEngine(bufferSize);
}

/** Detiene el motor y libera el dispositivo de audio. */
export function stopEngine(): Promise<void> {
  return inTauri() ? invoke<void>("stop_engine") : mock.stopEngine();
}

/** Lee el último estado conocido del motor (o `null` si aún no arrancó). */
export function getEngineStatus(): Promise<EngineStatus | null> {
  return inTauri()
    ? invoke<EngineStatus | null>("get_engine_status")
    : mock.getEngineStatus();
}

/** Lee el código de emparejamiento y la dirección LAN actuales. */
export function getPairingInfo(): Promise<PairingInfo> {
  return inTauri()
    ? invoke<PairingInfo>("get_pairing_info")
    : mock.getPairingInfo();
}

/** Lee el último nivel medido (para el renderizado inicial de la UI). */
export function getLastLevel(): Promise<LevelSample | null> {
  return inTauri()
    ? invoke<LevelSample | null>("get_last_level")
    : mock.getLastLevel();
}

/** Lee el último espectro emitido (para el renderizado inicial de la UI). */
export function getLastSpectrum(): Promise<SpectrumSample | null> {
  return inTauri()
    ? invoke<SpectrumSample | null>("get_last_spectrum")
    : mock.getLastSpectrum();
}

/** Lista los presets de la cabina con sus metadatos. */
export function getPresets(): Promise<PresetInfo[]> {
  return inTauri() ? invoke<PresetInfo[]>("get_presets") : mock.getPresets();
}

/** Lee el último estado de la cadena DSP (o `null` si el motor no corre). */
export function getDspState(): Promise<DspState | null> {
  return inTauri()
    ? invoke<DspState | null>("get_dsp_state")
    : mock.getDspState();
}

/** Lee la configuración persistida (para precargar la cabina). */
export function getConfig(): Promise<AppConfig> {
  return inTauri() ? invoke<AppConfig>("get_config") : mock.getConfig();
}

/** Aplica un preset a la cadena DSP en vivo. */
export function applyPreset(preset: PresetId): Promise<void> {
  return inTauri()
    ? invoke<void>("apply_preset", { preset })
    : mock.applyPreset(preset);
}

/** Activa o desactiva el bypass global de la cadena DSP. */
export function setGlobalBypass(bypass: boolean): Promise<void> {
  return inTauri()
    ? invoke<void>("set_global_bypass", { bypass })
    : mock.setGlobalBypass(bypass);
}

/** Activa o desactiva el bypass de un módulo por su nombre. */
export function setLinkBypass(link: string, bypass: boolean): Promise<void> {
  return inTauri()
    ? invoke<void>("set_link_bypass", { link, bypass })
    : mock.setLinkBypass(link, bypass);
}

/** Ajusta la ganancia de una banda del EQ del preset activo en vivo. */
export function setEqBand(bandIndex: number, gainDb: number): Promise<void> {
  return inTauri()
    ? invoke<void>("set_eq_band", { bandIndex, gainDb })
    : mock.setEqBand(bandIndex, gainDb);
}

/** Ajusta los parámetros de la puerta de ruido del preset activo en vivo. */
export function setNoiseGate(params: NoiseGateParams): Promise<void> {
  return inTauri() ? invoke<void>("set_noise_gate", { params }) : mock.setNoiseGate(params);
}

/** Ajusta la mezcla seco/húmedo del denoise del preset activo en vivo. */
export function setDenoise(params: DenoiseParams): Promise<void> {
  return inTauri() ? invoke<void>("set_denoise", { params }) : mock.setDenoise(params);
}

/** Ajusta los parámetros del feedback suppressor en vivo. */
export function setFeedback(params: FeedbackSuppressorParams): Promise<void> {
  return inTauri() ? invoke<void>("set_feedback", { params }) : mock.setFeedback(params);
}

/** Ajusta los parámetros de corrección de tono en vivo. */
export function setPitchCorrection(params: PitchCorrectionParams): Promise<void> {
  return inTauri() ? invoke<void>("set_pitch_correction", { params }) : mock.setPitchCorrection(params);
}

/** Ajusta los parámetros del delay en vivo. */
export function setDelay(params: DelayParams): Promise<void> {
  return inTauri() ? invoke<void>("set_delay", { params }) : mock.setDelay(params);
}

/** Ajusta los parámetros del reverb en vivo. */
export function setReverb(params: ReverbParams): Promise<void> {
  return inTauri() ? invoke<void>("set_reverb", { params }) : mock.setReverb(params);
}

/** Lee la última muestra de análisis vocal (o `null` si no hay datos). */
export function getAnalysis(): Promise<AnalysisSample | null> {
  return inTauri()
    ? invoke<AnalysisSample | null>("get_analysis")
    : mock.getAnalysis();
}

/** Lee el resumen acumulado de la sesión actual (o `null`). */
export function getSessionSummary(): Promise<SessionSummary | null> {
  return inTauri()
    ? invoke<SessionSummary | null>("get_session_summary")
    : mock.getSessionSummary();
}

/** Aplica la acción de una sugerencia (con confirmación del usuario). */
export function applySuggestion(suggestionId: number): Promise<void> {
  return inTauri()
    ? invoke<void>("apply_suggestion", { suggestionId })
    : mock.applySuggestion(suggestionId);
}

/** Pide sugerencias al asesor de IA (Groq) con las métricas actuales. */
export function requestAiSuggestions(): Promise<Suggestion[]> {
  return inTauri()
    ? invoke<Suggestion[]>("request_ai_suggestions")
    : mock.requestAiSuggestions();
}

/** Devuelve las últimas sugerencias del asesor de IA. */
export function getAiSuggestions(): Promise<Suggestion[]> {
  return inTauri()
    ? invoke<Suggestion[]>("get_ai_suggestions")
    : mock.getAiSuggestions();
}

/** Devuelve el estado del consentimiento de telemetría. */
export function getTelemetryConsent(): Promise<boolean | null> {
  return inTauri()
    ? invoke<boolean | null>("get_telemetry_consent")
    : mock.getTelemetryConsent();
}

/** Establece el consentimiento de telemetría (opt-in / opt-out). */
export function setTelemetryConsent(enabled: boolean): Promise<void> {
  return inTauri()
    ? invoke<void>("set_telemetry_consent", { enabled })
    : mock.setTelemetryConsent(enabled);
}

export type { ModelStatus } from "./types";

/** Comprueba si los modelos ONNX están descargados. */
export function getModelStatus(): Promise<ModelStatus> {
  return inTauri()
    ? invoke<ModelStatus>("get_model_status")
    : mock.getModelStatus();
}

/** Descarga los modelos ONNX desde los assets de GitHub. */
export function downloadModels(): Promise<ModelStatus> {
  return inTauri()
    ? invoke<ModelStatus>("download_models")
    : mock.downloadModels();
}

/**
 * Se suscribe a los eventos del motor. Devuelve una función para cancelar la
 * suscripción.
 */
export async function onEngineEvent(
  handler: (event: EngineEvent) => void,
): Promise<UnlistenFn> {
  return inTauri()
    ? listen<EngineEvent>("engine-event", (event) => handler(event.payload))
    : (mock.onEngineEvent(handler) as UnlistenFn);
}

/**
 * Se suscribe a las rotaciones del código de emparejamiento: cuando el backend
 * rota el código por intentos fallidos, `handler` recibe el código nuevo.
 * Devuelve una función para cancelar la suscripción.
 */
export async function onPairingEvent(
  handler: (code: string) => void,
): Promise<UnlistenFn> {
  return inTauri()
    ? listen<string>("pairing-event", (event) => handler(event.payload))
    : (mock.onPairingEvent(handler) as UnlistenFn);
}

/** Se suscribe al progreso de descarga de modelos ONNX. */
export async function onModelDownloadProgress(
  handler: (progress: { step: number; total: number }) => void,
): Promise<UnlistenFn> {
  if (!inTauri()) {
    return () => {};
  }
  return listen<{ step: number; total: number }>(
    "model-download-progress",
    (event) => handler(event.payload),
  );
}
