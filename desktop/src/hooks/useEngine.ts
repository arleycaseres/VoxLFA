// Hook de estado del motor de audio para la UI.
//
// Conecta la UI con el backend de Tauri: escucha los eventos del motor
// (estado, niveles, dispositivos, avisos) y expone acciones tipadas.

import { useCallback, useEffect, useState } from "react";
import type {
  AnalysisSample,
  DeviceList,
  DspState,
  EngineEvent,
  EngineStatus,
  LevelSample,
  PresetId,
  PresetInfo,
  SessionSummary,
} from "../lib/types";
import {
  applyPreset,
  applySuggestion,
  getAnalysis,
  getDspState,
  getEngineStatus,
  getLastLevel,
  getPairingInfo,
  getPresets,
  getSessionSummary,
  listDevices,
  onEngineEvent,
  setGlobalBypass,
  setLinkBypass,
  startEngine,
  stopEngine,
  type PairingInfo,
} from "../lib/tauri";

export interface EngineController {
  /** Estado más reciente del motor. */
  status: EngineStatus | null;
  /** Última muestra de nivel (para diales y medidores). */
  level: LevelSample | null;
  /** Dispositivos de audio detectados. */
  devices: DeviceList | null;
  /** Datos de emparejamiento (móvil ↔ escritorio). */
  pairing: PairingInfo | null;
  /** Presets de la cabina disponibles. */
  presets: PresetInfo[] | null;
  /** Último estado de la cadena DSP (o `null` si el motor no corre). */
  dsp: DspState | null;
  /** Última muestra de análisis vocal (métricas + sugerencias de IA). */
  analysis: AnalysisSample | null;
  /** Resumen acumulado de la sesión en curso (o `null`). */
  sessionSummary: SessionSummary | null;
  /** Mensaje del último aviso (o `null`). */
  warning: string | null;
  /** Error de la última operación (o `null`). */
  error: string | null;
  /** `true` mientras una operación de arranque/parada está en curso. */
  busy: boolean;
  /** Arranca el motor con los dispositivos indicados (`null` = default).
   *  `bufferSize` (`null` = auto por heurística de dispositivo). */
  start: (
    input?: string | null,
    output?: string | null,
    bufferSize?: number | null,
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
  /** Aplica la acción de una sugerencia (con confirmación del usuario). */
  applySuggestion: (suggestionId: number) => Promise<void>;
  /** Refresca el resumen acumulado de la sesión (tras detener el motor). */
  refreshSessionSummary: () => Promise<void>;
}

/**
 * Suscribe el componente a los eventos del motor y expone el control.
 * Úsese una sola vez en la raíz de la app.
 */
export function useEngine(): EngineController {
  const [status, setStatus] = useState<EngineStatus | null>(null);
  const [level, setLevel] = useState<LevelSample | null>(null);
  const [devices, setDevices] = useState<DeviceList | null>(null);
  const [pairing, setPairing] = useState<PairingInfo | null>(null);
  const [presets, setPresets] = useState<PresetInfo[] | null>(null);
  const [dsp, setDsp] = useState<DspState | null>(null);
  const [analysis, setAnalysis] = useState<AnalysisSample | null>(null);
  const [sessionSummary, setSessionSummary] = useState<SessionSummary | null>(
    null,
  );
  const [warning, setWarning] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refreshDevices = useCallback(async () => {
    try {
      setDevices(await listDevices());
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const start = useCallback(
    async (input?: string | null, output?: string | null, bufferSize?: number | null) => {
      setBusy(true);
      setError(null);
      try {
        await startEngine(input ?? null, output ?? null, bufferSize ?? null);
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

  const applySuggestionAction = useCallback(async (suggestionId: number) => {
    try {
      await applySuggestion(suggestionId);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const refreshSessionSummary = useCallback(async () => {
    try {
      const summary = await getSessionSummary();
      if (summary) setSessionSummary(summary);
    } catch (err) {
      setError(String(err));
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

    (async () => {
      const [status, level, pairing, presets, dsp, analysis] = await Promise.allSettled([
        getEngineStatus(),
        getLastLevel(),
        getPairingInfo(),
        getPresets(),
        getDspState(),
        getAnalysis(),
      ]);
      if (cancelled) return;
      if (status.status === "fulfilled") setStatus(status.value);
      if (level.status === "fulfilled" && level.value) setLevel(level.value);
      if (pairing.status === "fulfilled") setPairing(pairing.value);
      if (presets.status === "fulfilled" && presets.value)
        setPresets(presets.value);
      if (dsp.status === "fulfilled" && dsp.value) setDsp(dsp.value);
      if (analysis.status === "fulfilled" && analysis.value)
        setAnalysis(analysis.value);
    })();

    refreshDevices();

    return () => {
      cancelled = true;
      unsubscribe.then((fn) => fn?.());
    };
  }, [refreshDevices]);

  return {
    status,
    level,
    devices,
    pairing,
    presets,
    dsp,
    analysis,
    sessionSummary,
    warning,
    error,
    busy,
    start,
    stop,
    refreshDevices,
    applyPreset: applyPresetAction,
    setGlobalBypass: setGlobalBypassAction,
    setLinkBypass: setLinkBypassAction,
    applySuggestion: applySuggestionAction,
    refreshSessionSummary,
  };
}
