// VoxLFA — interfaz "cabina" de monitoreo y control del motor en vivo.
//
// Fase 2: asistente vocal local (análisis en vivo, sugerencias accionables con
// confirmación y resumen de sesión exportable) sobre la cabina de la Fase 1.

import { useCallback, useEffect, useRef, useState } from "react";
import { useEngine } from "./hooks/useEngine";
import type { UiSuggestion } from "./lib/uiTypes";
import type { Suggestion as RawSuggestion } from "./lib/types";
import { Dial } from "./components/Dial";
import { Meter } from "./components/Meter";
import { DeviceSelector } from "./components/DeviceSelector";
import { BufferSelector } from "./components/BufferSelector";
import { StatusPill } from "./components/StatusPill";
import { PairingBadge } from "./components/PairingBadge";
import { PresetCard } from "./components/PresetCard";
import { SimplePanel } from "./components/SimplePanel";
import { AdvancedDspPanel } from "./components/AdvancedDspPanel";
import { SuggestionPanel } from "./components/SuggestionPanel";
import { FloatingSuggestion } from "./components/FloatingSuggestion";
import { SpectrumView } from "./components/SpectrumView";
import { SetupGuide } from "./components/SetupGuide";
import { formatLatency, formatSampleRate } from "./lib/format";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/global.css";
import "./App.css";
import brandMark from "./assets/brand/brand_mark.png";
import brandSecondary from "./assets/brand/brand_secondary.png";

/** Categorías del acordeón en modo Avanzado. */
type AccordionGroup = "basic" | "dynamics" | "effects" | "creative";

const IS_RUNNING = (state: string | null | undefined) => state === "running";

export default function App() {
  const engine = useEngine();
  const [inputName, setInputName] = useState<string | null>(null);
  const [outputName, setOutputName] = useState<string | null>(null);
  const [bufferSize, setBufferSize] = useState<number | null>(null);
  const prefilled = useRef(false);

  // Estado compartido de sugerencias descartadas (persistido en sessionStorage).
  const dismissedKey = "voxlfa:dismissedSuggestions";
  const [dismissed, setDismissed] = useState<number[]>(() => {
    try {
      const raw = sessionStorage.getItem(dismissedKey);
      return raw ? JSON.parse(raw) : [];
    } catch {
      return [];
    }
  });
  useEffect(() => {
    try {
      sessionStorage.setItem(dismissedKey, JSON.stringify(dismissed));
    } catch {}
  }, [dismissed]);
  const dismissSuggestion = (id: number) =>
    setDismissed((prev) => Array.from(new Set([...prev, id])));

  // Mapea una sugerencia raw a UiSuggestion para FloatingSuggestion.
  function mapToUi(s: RawSuggestion): UiSuggestion {
    const sev: UiSuggestion["severity"] =
      s.severity >= 0.75 ? "critical" : s.severity >= 0.4 ? "recommended" : "optional";
    return {
      id: s.id,
      kind: s.kind,
      detected: { label: s.kind },
      consequence: s.message,
      recommendation: { label: s.message },
      severity: sev,
      action: s.action ?? null,
    };
  }

  // Estado de visibilidad de sugerencias flotantes.
  const [showFloating, setShowFloating] = useState(() => {
    return localStorage.getItem("voxlfa:floatingVisible") !== "0";
  });
  const toggleFloating = () => {
    const next = !showFloating;
    setShowFloating(next);
    localStorage.setItem("voxlfa:floatingVisible", next ? "1" : "0");
  };

  // Sugerencias activas (heurísticas + IA) sin descartadas, para FloatingSuggestion.
  const activeSuggestions: UiSuggestion[] = [
    ...(engine.analysis?.suggestions ?? []),
    ...(engine.aiSuggestions ?? []),
  ]
    .map(mapToUi)
    .filter((s) => !dismissed.includes(s.id));

  // Muestra la guía de configuración solo en el primer arranque.
  const [showGuide, setShowGuide] = useState(() => {
    return localStorage.getItem("voxlfa:guideSeen") !== "1";
  });

  // Modo de UI: "simple" (4 sliders) o "advanced" (panel completo).
  const [uiMode, setUiMode] = useState<"simple" | "advanced">(() => {
    const stored = localStorage.getItem("voxlfa:uiMode");
    return stored === "advanced" ? "advanced" : "simple";
  });
  const toggleUiMode = () => {
    const next = uiMode === "simple" ? "advanced" : "simple";
    setUiMode(next);
    localStorage.setItem("voxlfa:uiMode", next);
  };

  // Estado del acordeón en modo avanzado (una categoría abierta a la vez).
  const [openAccordion, setOpenAccordion] = useState<AccordionGroup | null>(() => {
    const stored = localStorage.getItem("voxlfa:accordionGroup");
    return (stored as AccordionGroup) ?? "basic";
  });
  const toggleAccordion = (group: AccordionGroup) => {
    const next = openAccordion === group ? null : group;
    setOpenAccordion(next);
    if (next) localStorage.setItem("voxlfa:accordionGroup", next);
    else localStorage.removeItem("voxlfa:accordionGroup");
  };

  // Precarga los selectores con la última selección persistida (si el
  // dispositivo sigue conectado). Solo la primera vez que hay config y lista.
  useEffect(() => {
    if (prefilled.current || !engine.config || !engine.devices) return;
    const config = engine.config;
    const inputs = engine.devices?.inputs ?? [];
    const outputs = engine.devices?.outputs ?? [];
    if (
      config.defaultInput &&
      inputs.some((device) => device.name === config.defaultInput)
    ) {
      setInputName(config.defaultInput);
    }
    if (
      config.defaultOutput &&
      outputs.some((device) => device.name === config.defaultOutput)
    ) {
      setOutputName(config.defaultOutput);
    }
    if (config.bufferSize != null) {
      setBufferSize(config.bufferSize);
    }
    prefilled.current = true;
  }, [engine.config, engine.devices]);

  const running = IS_RUNNING(engine.status?.state);
  const busy = engine.busy;
  const level = engine.level;
  const [leftCollapsed, setLeftCollapsed] = useState<boolean>(
    localStorage.getItem("voxlfa:leftCollapsed") === "1"
  );
  const [rightCollapsed, setRightCollapsed] = useState<boolean>(
    localStorage.getItem("voxlfa:rightCollapsed") === "1"
  );
  const [leftWidth, _setLeftWidth] = useState<number>(() =>
    parseInt(localStorage.getItem("voxlfa:leftCol") ?? "300", 10)
  );
  const [rightWidth, _setRightWidth] = useState<number>(() =>
    parseInt(localStorage.getItem("voxlfa:rightCol") ?? "320", 10)
  );

  const toggleLeft = () => {
    const next = !leftCollapsed;
    setLeftCollapsed(next);
    localStorage.setItem("voxlfa:leftCollapsed", next ? "1" : "0");
  };

  const toggleRight = () => {
    const next = !rightCollapsed;
    setRightCollapsed(next);
    localStorage.setItem("voxlfa:rightCollapsed", next ? "1" : "0");
  };

  const closeGuide = () => {
    setShowGuide(false);
    localStorage.setItem("voxlfa:guideSeen", "1");
  };

  // Callbacks estables para componentes memoizados (evitan re-renders).
  const handleGlobalBypass = useCallback(
    (bypass: boolean) => void engine.setGlobalBypass(bypass),
    [engine.setGlobalBypass],
  );
  const handleLinkBypass = useCallback(
    (link: string, bypass: boolean) => void engine.setLinkBypass(link, bypass),
    [engine.setLinkBypass],
  );
  const handleSetEqBand = useCallback(
    (index: number, gainDb: number) => void engine.setEqBand(index, gainDb),
    [engine.setEqBand],
  );
  const handleSetNoiseGate = useCallback(
    (params: Parameters<typeof engine.setNoiseGate>[0]) =>
      void engine.setNoiseGate(params),
    [engine.setNoiseGate],
  );
  const handleSetDenoise = useCallback(
    (params: Parameters<typeof engine.setDenoise>[0]) =>
      void engine.setDenoise(params),
    [engine.setDenoise],
  );
  const handleSetFeedback = useCallback(
    (params: Parameters<typeof engine.setFeedback>[0]) =>
      void engine.setFeedback(params),
    [engine.setFeedback],
  );
  const handleSetPitchCorrection = useCallback(
    (params: Parameters<typeof engine.setPitchCorrection>[0]) =>
      void engine.setPitchCorrection(params),
    [engine.setPitchCorrection],
  );
  const handleSetDelay = useCallback(
    (params: Parameters<typeof engine.setDelay>[0]) =>
      void engine.setDelay(params),
    [engine.setDelay],
  );
  const handleSetReverb = useCallback(
    (params: Parameters<typeof engine.setReverb>[0]) =>
      void engine.setReverb(params),
    [engine.setReverb],
  );
  const handleSetSaturator = useCallback(
    (params: Parameters<typeof engine.setSaturator>[0]) =>
      void engine.setSaturator(params),
    [engine.setSaturator],
  );
  const handleSetDynamicEq = useCallback(
    (params: Parameters<typeof engine.setDynamicEq>[0]) =>
      void engine.setDynamicEq(params),
    [engine.setDynamicEq],
  );
  const handleSetHarmonizer = useCallback(
    (params: Parameters<typeof engine.setHarmonizer>[0]) =>
      void engine.setHarmonizer(params),
    [engine.setHarmonizer],
  );
  const handleSetCompressor = useCallback(
    (params: Parameters<typeof engine.setCompressor>[0]) =>
      void engine.setCompressor(params),
    [engine.setCompressor],
  );
  const handleSetDeEsser = useCallback(
    (params: Parameters<typeof engine.setDeEsser>[0]) =>
      void engine.setDeEsser(params),
    [engine.setDeEsser],
  );
  const handleApplyPreset = useCallback(
    (id: string) => void engine.applyPreset(id as import("./lib/types").PresetId),
    [engine.applyPreset],
  );
  const handleApplySuggestion = useCallback(
    (id: number) => void engine.applySuggestion(id),
    [engine.applySuggestion],
  );

  return (
    <div className="app">
      {/* Barra superior */}
      <header className="app__header">
        <div className="brand">
          <img src={brandMark} alt="VoxLFA" className="brand__mark" />
          <div className="brand__names">
            <img src={brandSecondary} alt="VoxLFA secondary" className="brand__secondary" />
            <div className="brand__meta">
              <span className="brand__name">Vox<span className="brand__accent">LFA</span></span>
              <span className="brand__tag">procesador vocal en vivo</span>
            </div>
          </div>
        </div>
        <div className="app__header-right">
          <StatusPill state={engine.status?.state ?? null} />
          <PairingBadge pairing={engine.pairing} />
          <button
            type="button"
            className={`btn btn--ghost btn--small ui-mode-toggle ${uiMode === "advanced" ? "ui-mode-toggle--active" : ""}`}
            onClick={toggleUiMode}
            aria-label={uiMode === "simple" ? "Cambiar a modo avanzado" : "Cambiar a modo simple"}
          >
            {uiMode === "simple" ? "Avanzado" : "Simple"}
          </button>
          <button
            className="help-trigger"
            onClick={() => setShowGuide(true)}
            aria-label="Abrir guía de configuración"
          >
            ?
          </button>
        </div>
      </header>

      {/* Cuerpo en rejilla de tres columnas */}
      <main
        className={`app__main`}
        style={{
          ["--left-col" as any]: leftCollapsed ? "40px" : `${leftWidth}px`,
          ["--right-col" as any]: rightCollapsed ? "40px" : `${rightWidth}px`,
          ["--resizer-left" as any]: leftCollapsed ? "0px" : "8px",
          ["--resizer-right" as any]: rightCollapsed ? "0px" : "8px",
        }}
      >
        {/* Panel izquierdo: motor y dispositivos */}
        <aside
          className={`panel panel--controls ${leftCollapsed ? "is-collapsed" : ""}`}
        >
          <h2 className="panel__title">
            Motor
            <button
              type="button"
              className="panel__collapse"
              aria-label={leftCollapsed ? "Expandir panel izquierdo" : "Colapsar panel izquierdo"}
              onClick={toggleLeft}
            >
              {leftCollapsed ? "›" : "‹"}
            </button>
          </h2>

          {/* Toggle de sugerencias flotantes */}
          {activeSuggestions.length > 0 && (
            <button
              type="button"
              className={`btn btn--ghost btn--small floating-toggle ${showFloating ? "floating-toggle--active" : ""}`}
              onClick={toggleFloating}
            >
              💡 Sugerencias ({activeSuggestions.length})
            </button>
          )}

          {showFloating && (
            <FloatingSuggestion
              suggestions={activeSuggestions}
              onApply={(id) => void engine.applySuggestion(id)}
              onDismiss={dismissSuggestion}
            />
          )}

          <DeviceSelector
            label="Entrada"
            devices={engine.devices?.inputs ?? []}
            value={inputName}
            onChange={setInputName}
            disabled={running || busy}
          />
          <DeviceSelector
            label="Salida"
            devices={engine.devices?.outputs ?? []}
            value={outputName}
            onChange={setOutputName}
            disabled={running || busy}
          />
          <BufferSelector
            value={bufferSize}
            onChange={setBufferSize}
            disabled={running || busy}
          />

          <div className="controls__actions">
            <button
              type="button"
              className={`btn ${running ? "btn--stop" : "btn--start"}`}
              disabled={busy}
              onClick={() =>
                running
                  ? engine.stop()
                  : engine.start(inputName, outputName, bufferSize)
              }
            >
              {!running && busy
                ? "Abriendo…"
                : running && busy
                  ? "Deteniendo…"
                  : running
                    ? "Detener"
                    : "Arrancar"}
            </button>
            <button
              type="button"
              className="btn btn--ghost"
              disabled={running || busy}
              onClick={() => void engine.refreshDevices()}
            >
              Detectar
            </button>
          </div>

          {uiMode === "simple" ? (
            <SimplePanel
              dsp={engine.dsp}
              running={running}
              onSetDenoise={handleSetDenoise}
              onSetNoiseGate={handleSetNoiseGate}
              onSetFeedback={handleSetFeedback}
              onSetCompressor={handleSetCompressor}
              onSetDeEsser={handleSetDeEsser}
              onSetDynamicEq={handleSetDynamicEq}
              onSetEqBand={handleSetEqBand}
              onSetLinkBypass={handleLinkBypass}
            />
          ) : (
            <AdvancedDspPanel
              dsp={engine.dsp}
              running={running}
              openAccordion={openAccordion}
              onToggleAccordion={toggleAccordion}
              onGlobalBypass={handleGlobalBypass}
              onLinkBypass={handleLinkBypass}
              onSetEqBand={handleSetEqBand}
              onSetNoiseGate={handleSetNoiseGate}
              onSetDenoise={handleSetDenoise}
              onSetFeedback={handleSetFeedback}
              onSetDynamicEq={handleSetDynamicEq}
              onSetPitchCorrection={handleSetPitchCorrection}
              onSetDelay={handleSetDelay}
              onSetReverb={handleSetReverb}
              onSetSaturator={handleSetSaturator}
              onSetHarmonizer={handleSetHarmonizer}
            />
          )}

          {engine.deviceStuckError && (
            <div className="controls__stuck-banner">
              <p>
                <strong>{engine.deviceStuckError.device ?? "Dispositivo"}</strong>{" "}
                no respondió la última vez y puede seguir ocupado.
                {engine.deviceStuckError.stuckCount
                  ? ` (Intentos sin resolver: ${engine.deviceStuckError.stuckCount})`
                  : ""}
              </p>
              <div className="controls__stuck-actions">
                <button
                  type="button"
                  className="btn btn--ghost"
                  onClick={() => engine.dismissDeviceStuck()}
                >
                  Cancelar
                </button>
                <button
                  type="button"
                  className="btn btn--start"
                  disabled={engine.busy}
                  onClick={() => engine.retryWithForce()}
                >
                  Reintentar
                </button>
              </div>
            </div>
          )}
          {engine.error && <p className="controls__error">{engine.error}</p>}
          {engine.warning && <p className="controls__warning">{engine.warning}</p>}
        </aside>

        {/* Centro: instrumento principal */}
        <section className="app__center">
          <div className="gauges">
            <div className="panel panel--gauge">
              <h2 className="panel__title">Señal de entrada</h2>
              <Dial
                peakDb={level?.inputPeakDb ?? -100}
                rmsDb={level?.inputRmsDb ?? undefined}
                label="dBFS"
                size={260}
              />
              <div className="gauge__meters">
                <Meter label="RMS" valueDb={level?.inputRmsDb ?? -100} peakDb={level?.inputPeakDb ?? -100} />
                <Meter label="PICO" valueDb={level?.inputPeakDb ?? -100} />
              </div>
            </div>

            <div className="panel panel--gauge">
              <h2 className="panel__title">Señal de salida</h2>
              <Dial
                peakDb={level?.outputPeakDb ?? -100}
                rmsDb={level?.outputRmsDb ?? undefined}
                label="dBFS"
                size={260}
              />
              <div className="gauge__meters">
                <Meter label="RMS" valueDb={level?.outputRmsDb ?? -100} peakDb={level?.outputPeakDb ?? -100} />
                <Meter label="PICO" valueDb={level?.outputPeakDb ?? -100} />
              </div>
            </div>
          </div>

          <div className="panel panel--spectrum">
            <h2 className="panel__title">Espectro de entrada</h2>
            <SpectrumView spectrum={engine.spectrum} />
          </div>

          <div className="panel panel--telemetry">
            <div className="telemetry">
              <span className="telemetry__label">Latencia</span>
              <span className="telemetry__value">
                {level?.latencyMs !== undefined ? formatLatency(level.latencyMs) : "—"}
              </span>
            </div>
            <div className="telemetry">
              <span className="telemetry__label">Muestreo</span>
              <span className="telemetry__value">
                {engine.status ? formatSampleRate(engine.status.sampleRate) : "—"}
              </span>
            </div>
            <div className="telemetry">
              <span className="telemetry__label">Buffer</span>
              <span className="telemetry__value">
                {engine.status ? `${engine.status.bufferSize} smp` : "—"}
              </span>
            </div>
          </div>
        </section>

        {/* Panel derecho: presets y sugerencias de IA */}
        <aside className={`panel panel--side ${rightCollapsed ? "is-collapsed" : ""}`}>
          <h2 className="panel__title">
            Presets
            <button
              type="button"
              className="panel__collapse panel__collapse--right"
              aria-label={rightCollapsed ? "Expandir panel derecho" : "Colapsar panel derecho"}
              onClick={toggleRight}
            >
              {rightCollapsed ? "‹" : "›"}
            </button>
          </h2>
          <div className="preset__list">
            {(engine.presets ?? []).map((preset) => (
              <PresetCard
                key={preset.id}
                id={preset.id}
                name={preset.name}
                description={preset.description}
                accent={preset.id === "radio" ? "orange" : "cyan"}
                active={engine.dsp?.preset === preset.id}
                disabled={!running}
                onSelect={() => void handleApplyPreset(preset.id)}
              />
            ))}
          </div>

          <h2 className="panel__title panel__title--spaced">Sugerencias de IA</h2>
          <SuggestionPanel
            analysis={engine.analysis}
            running={running}
            sessionSummary={engine.sessionSummary}
            onApplySuggestion={handleApplySuggestion}
            onRefreshSummary={() => void engine.refreshSessionSummary()}
            aiSuggestions={engine.aiSuggestions}
            aiLoading={engine.aiLoading}
            aiError={engine.aiError}
            onRequestAi={() => void engine.requestAi()}
            dismissed={dismissed}
            onDismiss={dismissSuggestion}
          />
        </aside>
      </main>

      {/* Barra de estado inferior */}
      <footer className="app__footer">
        <span className="app__footer-item">
          Estado: <strong>{engine.status?.state ?? "stopped"}</strong>
        </span>
        <span className="app__footer-item">
          Entrada: <strong>{engine.status?.inputDevice ?? "—"}</strong>
        </span>
        <span className="app__footer-item">
          Salida: <strong>{engine.status?.outputDevice ?? "—"}</strong>
        </span>
        <span className="app__footer-item app__footer-item--mono">
          v0.4.0 · ajustes guardados por dispositivo
        </span>
      </footer>

      {/* Guía de configuración (se muestra en el primer arranque) */}
      {showGuide && <SetupGuide onClose={closeGuide} />}
    </div>
  );
}
