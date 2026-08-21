// Hook de estado del motor de audio para la UI.
//
// Conecta la UI con el backend de Tauri: escucha los eventos del motor
// (estado, niveles, dispositivos, avisos) y expone acciones tipadas.

import { useCallback, useEffect, useState } from "react";
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
} from "../lib/types";
import {
  applyPreset,
  applySuggestion,
  downloadModels,
  getAnalysis,
  getConfig,
  getDspState,
  getEngineStatus,
  getLastLevel,
  getLastSpectrum,
  getModelStatus,
  getPairingInfo,
  getPresets,
  getSessionSummary,
  listDevices,
  onEngineEvent,
  onModelDownloadProgress,
  onPairingEvent,
  requestAiSuggestions,
  setDenoise,
  setEqBand,
  setFeedback,
  setGlobalBypass,
  setLinkBypass,
  setNoiseGate,
  setPitchCorrection,
  setDelay,
  setReverb,
  setTelemetryConsent,
  startEngine,
  stopEngine,
  type PairingInfo,
} from "../lib/tauri";

export interface EngineController {
  /** Estado más reciente del motor. */
  status: EngineStatus | null;
  /** Última muestra de nivel (para diales y medidores). */
  level: LevelSample | null;
  /** Último espectro de la entrada (bandas logarítmicas, dBFS). */
  spectrum: SpectrumSample | null;
  /** Dispositivos de audio detectados. */
  devices: DeviceList | null;
  /** Datos de emparejamiento (móvil ↔ escritorio). */
  pairing: PairingInfo | null;
  /** Presets de la cabina disponibles. */
  presets: PresetInfo[] | null;
  /** Configuración persistida (para precargar selectores y perfiles). */
  config: AppConfig | null;
  /** Último estado de la cadena DSP (o `null` si el motor no corre). */
  dsp: DspState | null;
  /** Última muestra de análisis vocal (métricas + sugerencias de IA). */
  analysis: AnalysisSample | null;
  /** Resumen acumulado de la sesión en curso (o `null`). */
  sessionSummary: SessionSummary | null;
  /** Estado de los modelos ONNX en disco. */
  modelStatus: ModelStatus | null;
  /** Progreso de descarga de modelos (`null` = no está descargando). */
  modelDownloadProgress: { step: number; total: number } | null;
  /** Mensaje del último aviso (o `null`). */
  warning: string | null;
  /** Error de la última operación (o `null`). */
  error: string | null;
  /** `true` mientras una operación de arranque/parada está en curso. */
  busy: boolean;
  /** Arranca el motor con los dispositivos indicados (`null` = default).
   *  `bufferSize` (`null` = auto por heurística de dispositivo).
   *  `audioHost` (`null` = predeterminado del sistema; p. ej. `"jack"`, `"alsa"`). */
  start: (
    input?: string | null,
    output?: string | null,
    bufferSize?: number | null,
    audioHost?: string | null,
  ) => Promise<void>;
  /** Detiene el motor. */
  stop: () => Promise<void>;
  /** Vuelve a detectar dispositivos de audio. */
  refreshDevices: () => Promise<void>;
  /** Aplica un preset a la cadena DSP. */
  applyPreset: (preset: PresetId) => Promise<void>;
  /** Cambia el bypass global de la cadena. */
  setGlobalBypass: (bypass: boolean) => Promise<void>;
  /** Cambia el bypass de un módulo por su nombre. */
  setLinkBypass: (link: string, bypass: boolean) => Promise<void>;
  /** Ajusta la ganancia de una banda del EQ del preset activo en vivo. */
  setEqBand: (bandIndex: number, gainDb: number) => Promise<void>;
  /** Ajusta los parámetros de la puerta de ruido del preset activo en vivo. */
  setNoiseGate: (params: NoiseGateParams) => Promise<void>;
  /** Ajusta la mezcla seco/húmedo del denoise del preset activo en vivo. */
  setDenoise: (params: DenoiseParams) => Promise<void>;
  /** Ajusta los parámetros del feedback suppressor del preset activo en vivo. */
  setFeedback: (params: FeedbackSuppressorParams) => Promise<void>;
  /** Ajusta los parámetros de corrección de tono del preset activo en vivo. */
  setPitchCorrection: (params: PitchCorrectionParams) => Promise<void>;
  /** Ajusta los parámetros de delay del preset activo en vivo. */
  setDelay: (params: DelayParams) => Promise<void>;
  /** Ajusta los parámetros de reverb del preset activo en vivo. */
  setReverb: (params: ReverbParams) => Promise<void>;
  /** Aplica la acción de una sugerencia (con confirmación del usuario). */
  applySuggestion: (suggestionId: number) => Promise<void>;
  /** Refresca el resumen acumulado de la sesión (tras detener el motor). */
  refreshSessionSummary: () => Promise<void>;
  /** Establece el consentimiento de telemetría (opt-in / opt-out). */
  setTelemetryConsent: (enabled: boolean) => Promise<void>;
  /** Comprueba si los modelos ONNX están descargados. */
  checkModelStatus: () => Promise<void>;
  /** Descarga los modelos ONNX desde los assets de GitHub. */
  downloadModel: () => Promise<void>;
  /** Sugerencias generadas por el asesor de IA (Groq). */
  aiSuggestions: Suggestion[];
  /** `true` mientras se consultan las sugerencias de IA. */
  aiLoading: boolean;
  /** Error de la última consulta al asesor IA (o vacío). */
  aiError: string;
  /** Solicita sugerencias al asesor de IA con las métricas actuales. */
  requestAi: () => Promise<void>;
}

/**
 * Suscribe el componente a los eventos del motor y expone el control.
 * Úsese una sola vez en la raíz de la app.
 */
export function useEngine(): EngineController {
  const [status, setStatus] = useState<EngineStatus | null>(null);
  const [level, setLevel] = useState<LevelSample | null>(null);
  const [spectrum, setSpectrum] = useState<SpectrumSample | null>(null);
  const [devices, setDevices] = useState<DeviceList | null>(null);
  const [pairing, setPairing] = useState<PairingInfo | null>(null);
  const [presets, setPresets] = useState<PresetInfo[] | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [dsp, setDsp] = useState<DspState | null>(null);
  const [analysis, setAnalysis] = useState<AnalysisSample | null>(null);
  const [sessionSummary, setSessionSummary] = useState<SessionSummary | null>(
    null,
  );
  const [warning, setWarning] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [modelStatus, setModelStatus] = useState<ModelStatus | null>(null);
  const [modelDownloadProgress, setModelDownloadProgress] = useState<{
    step: number;
    total: number;
  } | null>(null);
  const [aiSuggestions, setAiSuggestions] = useState<Suggestion[]>([]);
  const [aiLoading, setAiLoading] = useState(false);
  const [aiError, setAiError] = useState("");

  const refreshDevices = useCallback(async () => {
    try {
      setDevices(await listDevices());
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const start = useCallback(
    async (
      input?: string | null,
      output?: string | null,
      bufferSize?: number | null,
      audioHost?: string | null,
    ) => {
      setBusy(true);
      setError(null);
      try {
        await startEngine(input ?? null, output ?? null, bufferSize ?? null, audioHost ?? null);
      } catch (err) {
        setError(String(err));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const stop = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await stopEngine();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, []);

  const applyPresetAction = useCallback(async (preset: PresetId) => {
    try {
      await applyPreset(preset);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const setGlobalBypassAction = useCallback(async (bypass: boolean) => {
    try {
      await setGlobalBypass(bypass);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const setLinkBypassAction = useCallback(async (link: string, bypass: boolean) => {
    try {
      await setLinkBypass(link, bypass);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const setEqBandAction = useCallback(async (bandIndex: number, gainDb: number) => {
    try {
      await setEqBand(bandIndex, gainDb);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const setNoiseGateAction = useCallback(async (params: NoiseGateParams) => {
    try {
      await setNoiseGate(params);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const setDenoiseAction = useCallback(async (params: DenoiseParams) => {
    try {
      await setDenoise(params);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const setFeedbackAction = useCallback(async (params: FeedbackSuppressorParams) => {
    try {
      await setFeedback(params);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const setPitchCorrectionAction = useCallback(async (params: PitchCorrectionParams) => {
    try {
      await setPitchCorrection(params);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const setDelayAction = useCallback(async (params: DelayParams) => {
    try {
      await setDelay(params);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const setReverbAction = useCallback(async (params: ReverbParams) => {
    try {
      await setReverb(params);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const applySuggestionAction = useCallback(async (suggestionId: number) => {
    try {
      await applySuggestion(suggestionId);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const refreshSessionSummary = useCallback(async () => {
    const summary = await getSessionSummary();
    if (summary) setSessionSummary(summary);
  }, []);

  const setTelemetryConsentAction = useCallback(async (enabled: boolean) => {
    try {
      await setTelemetryConsent(enabled);
      setConfig((prev) => (prev ? { ...prev, telemetryEnabled: enabled } : prev));
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const checkModelStatusAction = useCallback(async () => {
    try {
      setModelStatus(await getModelStatus());
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const downloadModelAction = useCallback(async () => {
    try {
      setModelDownloadProgress({ step: 0, total: 1 });
      const status = await downloadModels();
      setModelStatus(status);
    } catch (err) {
      setError(String(err));
    } finally {
      setModelDownloadProgress(null);
    }
  }, []);

  const requestAiAction = useCallback(async () => {
    setAiLoading(true);
    setAiError("");
    try {
      const suggestions = await requestAiSuggestions();
      setAiSuggestions(suggestions);
    } catch (err) {
      setAiError(String(err));
    } finally {
      setAiLoading(false);
    }
  }, []);

  // Carga inicial + suscripción de eventos.
  useEffect(() => {
    let cancelled = false;

    const applyEvent = (event: EngineEvent) => {
      switch (event.type) {
        case "status":
          setStatus(event);
          break;
        case "level":
          setLevel(event);
          break;
        case "spectrum":
          setSpectrum({
            binsDb: event.binsDb,
            sampleRate: event.sampleRate,
            capturedAtMs: event.capturedAtMs,
          });
          break;
        case "devices":
          setDevices({ inputs: event.inputs, outputs: event.outputs });
          break;
        case "dsp":
          setDsp({
            preset: event.preset,
            globalBypass: event.globalBypass,
            links: event.links,
          });
          break;
        case "analysis":
          setAnalysis({
            metrics: event.metrics,
            suggestions: event.suggestions,
            capturedAtMs: event.capturedAtMs,
          });
          break;
        case "warning":
          setWarning(event.message);
          break;
      }
    };

    const unsubscribe = onEngineEvent(applyEvent).catch(() => undefined);

    // Rotación del código de emparejamiento (por fallos consecutivos en el
    // handshake del móvil): refresca el código mostrado en la cabina.
    const unsubscribePairing = onPairingEvent((code) => {
      setPairing((previous) => (previous ? { ...previous, code } : previous));
    }).catch(() => undefined);

    // Escuchar progreso de descarga de modelos ONNX.
    const unsubscribeProgress = onModelDownloadProgress((progress) => {
      setModelDownloadProgress(progress);
    }).catch(() => undefined);

    (async () => {
      const [status, level, spectrum, pairing, presets, config, dsp, analysis, models] =
        await Promise.allSettled([
          getEngineStatus(),
          getLastLevel(),
          getLastSpectrum(),
          getPairingInfo(),
          getPresets(),
          getConfig(),
          getDspState(),
          getAnalysis(),
          getModelStatus(),
        ]);
      if (cancelled) return;
      if (status.status === "fulfilled") setStatus(status.value);
      if (level.status === "fulfilled" && level.value) setLevel(level.value);
      if (spectrum.status === "fulfilled" && spectrum.value)
        setSpectrum(spectrum.value);
      if (pairing.status === "fulfilled") setPairing(pairing.value);
      if (presets.status === "fulfilled" && presets.value)
        setPresets(presets.value);
      if (config.status === "fulfilled") setConfig(config.value);
      if (dsp.status === "fulfilled" && dsp.value) setDsp(dsp.value);
      if (analysis.status === "fulfilled" && analysis.value)
        setAnalysis(analysis.value);
      if (models.status === "fulfilled") setModelStatus(models.value);
    })();

    refreshDevices();

    return () => {
      cancelled = true;
      unsubscribe.then((fn) => fn?.());
      unsubscribePairing.then((fn) => fn?.());
      unsubscribeProgress.then((fn) => fn?.());
    };
  }, [refreshDevices]);

  return {
    status,
    level,
    spectrum,
    devices,
    pairing,
    presets,
    config,
    dsp,
    analysis,
    sessionSummary,
    warning,
    error,
    busy,
    modelStatus,
    modelDownloadProgress,
    start,
    stop,
    refreshDevices,
    applyPreset: applyPresetAction,
    setGlobalBypass: setGlobalBypassAction,
    setLinkBypass: setLinkBypassAction,
    setEqBand: setEqBandAction,
    setNoiseGate: setNoiseGateAction,
    setDenoise: setDenoiseAction,
    setFeedback: setFeedbackAction,
    setPitchCorrection: setPitchCorrectionAction,
    setDelay: setDelayAction,
    setReverb: setReverbAction,
    applySuggestion: applySuggestionAction,
    refreshSessionSummary,
    setTelemetryConsent: setTelemetryConsentAction,
    checkModelStatus: checkModelStatusAction,
    downloadModel: downloadModelAction,
    aiSuggestions,
    aiLoading,
    aiError,
    requestAi: requestAiAction,
  };
}
