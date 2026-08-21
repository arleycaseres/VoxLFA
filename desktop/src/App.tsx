// VoxLFA — interfaz "cabina" de monitoreo y control del motor en vivo.
//
// Fase 2: asistente vocal local (análisis en vivo, sugerencias accionables con
// confirmación y resumen de sesión exportable) sobre la cabina de la Fase 1.

import { useEffect, useRef, useState } from "react";
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
import { DspChain } from "./components/DspChain";
import { EqPanel } from "./components/EqPanel";
import { GatePanel } from "./components/GatePanel";
import { DenoisePanel } from "./components/DenoisePanel";
import { FeedbackPanel } from "./components/FeedbackPanel";
import { PitchCorrectionPanel } from "./components/PitchCorrectionPanel";
import { DelayPanel } from "./components/DelayPanel";
import { ReverbPanel } from "./components/ReverbPanel";
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
              {busy ? "…" : running ? "Detener" : "Arrancar"}
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

          <h2 className="panel__title panel__title--spaced">Cadena DSP</h2>
          <DspChain
            dsp={engine.dsp}
            onGlobalBypass={(bypass) => void engine.setGlobalBypass(bypass)}
            onLinkBypass={(link, bypass) => void engine.setLinkBypass(link, bypass)}
          />

          <h2 className="panel__title panel__title--spaced">Ecualizador</h2>
          <EqPanel
            dsp={engine.dsp}
            running={running}
            onSetEqBand={(index, gainDb) => void engine.setEqBand(index, gainDb)}
          />

          <h2 className="panel__title panel__title--spaced">Puerta de ruido</h2>
          <GatePanel
            dsp={engine.dsp}
            running={running}
            onSetNoiseGate={(params) => void engine.setNoiseGate(params)}
          />

          <h2 className="panel__title panel__title--spaced">Supresión de ruido</h2>
          <DenoisePanel
            dsp={engine.dsp}
            running={running}
            onSetDenoise={(params) => void engine.setDenoise(params)}
          />

          <h2 className="panel__title panel__title--spaced">Antifeedback</h2>
          <FeedbackPanel
            dsp={engine.dsp}
            running={running}
            onSetFeedback={(params) => void engine.setFeedback(params)}
          />

          <h2 className="panel__title panel__title--spaced">Corrección tono</h2>
          <PitchCorrectionPanel
            dsp={engine.dsp}
            running={running}
            onSetPitchCorrection={(params) => void engine.setPitchCorrection(params)}
          />

          <h2 className="panel__title panel__title--spaced">Delay</h2>
          <DelayPanel
            dsp={engine.dsp}
            running={running}
            onSetDelay={(params) => void engine.setDelay(params)}
          />

          <h2 className="panel__title panel__title--spaced">Reverb</h2>
          <ReverbPanel
            dsp={engine.dsp}
            running={running}
            onSetReverb={(params) => void engine.setReverb(params)}
          />

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
                onSelect={() => void engine.applyPreset(preset.id)}
              />
            ))}
          </div>

          <h2 className="panel__title panel__title--spaced">Sugerencias de IA</h2>
          <SuggestionPanel
            analysis={engine.analysis}
            running={running}
            sessionSummary={engine.sessionSummary}
            onApplySuggestion={(id) => void engine.applySuggestion(id)}
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
