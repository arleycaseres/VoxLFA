// Conexión WebSocket con el escritorio VoxLFA para monitoreo remoto.
//
// El protocolo exige el código de emparejamiento en la URL (`?token=...`);
// sin él el servidor rechaza la conexión (401). La app reconecta sola con
// retroceso exponencial acotado.

import { useCallback, useEffect, useRef, useState } from "react";
import {
  isEngineEvent,
  type AnalysisSample,
  type DeviceList,
  type DspState,
  type EngineEvent,
  type EngineStatus,
  type LevelSample,
} from "../lib/protocol";

/** Estado de la conexión con el escritorio. */
export type ConnectionState = "idle" | "connecting" | "connected" | "reconnecting" | "error";

/** Máximo de reintentos automáticos antes de rendirse. */
const MAX_RETRIES = 5;
/** Retardo base del retroceso exponencial (ms). */
const RETRY_BASE_MS = 1000;

export interface RemoteEngine {
  connState: ConnectionState;
  /** Mensaje del último error de conexión (o `null`). */
  error: string | null;
  status: EngineStatus | null;
  level: LevelSample | null;
  devices: DeviceList | null;
  /** Último estado de la cadena DSP del escritorio. */
  dsp: DspState | null;
  /** Última muestra de análisis vocal (métricas + sugerencias del escritorio). */
  analysis: AnalysisSample | null;
  /** Conecta al escritorio en `host:port` usando el código de emparejamiento. */
  connect: (host: string, port: number, code: string) => void;
  /** Cierra la conexión de forma voluntaria. */
  disconnect: () => void;
}

export function useRemoteEngine(): RemoteEngine {
  const [connState, setConnState] = useState<ConnectionState>("idle");
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<EngineStatus | null>(null);
  const [level, setLevel] = useState<LevelSample | null>(null);
  const [devices, setDevices] = useState<DeviceList | null>(null);
  const [dsp, setDsp] = useState<DspState | null>(null);
  const [analysis, setAnalysis] = useState<AnalysisSample | null>(null);

  const socketRef = useRef<WebSocket | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const attemptsRef = useRef(0);
  const closedIntentionallyRef = useRef(false);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  /** Cierra el socket y cancela reintentos pendientes. */
  const teardown = useCallback(() => {
    closedIntentionallyRef.current = true;
    clearTimer();
    const ws = socketRef.current;
    socketRef.current = null;
    if (ws) {
      ws.onclose = null;
      ws.onerror = null;
      ws.onmessage = null;
      ws.close();
    }
  }, [clearTimer]);

  const disconnect = useCallback(() => {
    teardown();
    attemptsRef.current = 0;
    setConnState("idle");
  }, [teardown]);

  const openSocket = useCallback(
    (target: { host: string; port: number; code: string }, onReconnect: () => void) => {
      closedIntentionallyRef.current = false;
      const ws = new WebSocket(`ws://${target.host}:${target.port}/?token=${target.code}`);
      socketRef.current = ws;

      ws.onopen = () => {
        attemptsRef.current = 0;
        setConnState("connected");
        setError(null);
      };

      ws.onmessage = (message) => {
        let parsed: unknown;
        try {
          parsed = JSON.parse(String(message.data));
        } catch {
          // Mensaje malformado: se ignora y se sigue esperando.
          return;
        }
        if (!isEngineEvent(parsed)) return;
        applyEvent(parsed);
      };

      ws.onerror = () => {
        // `onclose` siempre le sigue; el error se muestra ahí.
      };

      ws.onclose = () => {
        if (socketRef.current !== ws) return; // Descartar cierre de un socket viejo.
        socketRef.current = null;
        if (closedIntentionallyRef.current) return;

        // Reintentar con retroceso exponencial acotado.
        const attempt = attemptsRef.current;
        attemptsRef.current += 1;
        if (attempt >= MAX_RETRIES) {
          setConnState("error");
          setError("No se pudo mantener la conexión con el escritorio.");
          return;
        }
        const delay = Math.min(RETRY_BASE_MS * 2 ** attempt, 8000);
        setConnState("reconnecting");
        setError(`Conexión perdida. Reintentando en ${Math.round(delay / 1000)} s…`);
        timerRef.current = setTimeout(onReconnect, delay);
      };

      function applyEvent(event: EngineEvent) {
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
            setError(event.message);
            break;
        }
      }
    },
    [],
  );

  const connect = useCallback(
    (host: string, port: number, code: string) => {
      const trimmedHost = host.trim();
      const trimmedCode = code.trim();
      if (!trimmedHost || !trimmedCode || port < 1 || port > 65535) {
        setError("Introduce una IP, un puerto y el código de emparejamiento.");
        return;
      }
      teardown();
      attemptsRef.current = 0;
      const target = { host: trimmedHost, port, code: trimmedCode };
      setConnState("connecting");
      setError(null);
      const reconnect = () => openSocket(target, reconnect);
      openSocket(target, reconnect);
    },
    [teardown, openSocket],
  );

  // Limpieza al desmontar el componente.
  useEffect(() => {
    return () => {
      teardown();
    };
  }, [teardown]);

  return { connState, error, status, level, devices, dsp, analysis, connect, disconnect };
}
