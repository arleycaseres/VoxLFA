// VoxLFA — interfaz "cabina" de monitoreo y control del motor en vivo.
//
// Fase 2: asistente vocal local (análisis en vivo, sugerencias accionables con
// confirmación y resumen de sesión exportable) sobre la cabina de la Fase 1.

import { useEffect, useRef, useState } from "react";
import { useEngine } from "./hooks/useEngine";
import { Dial } from "./components/Dial";
import { Meter } from "./components/Meter";
import { DeviceSelector } from "./components/DeviceSelector";
import { BufferSelector } from "./components/BufferSelector";
import { StatusPill } from "./components/StatusPill";
import { PairingBadge } from "./components/PairingBadge";
import { PresetCard } from "./components/PresetCard";
import { DspChain } from "./components/DspChain";
import { EqPanel } from "./components/EqPanel";
import { SuggestionPanel } from "./components/SuggestionPanel";
import { formatLatency, formatSampleRate } from "./lib/format";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/global.css";
import "./App.css";

const IS_RUNNING = (state: string | null | undefined) => state === "running";

export default function App() {
  const engine = useEngine();
  const [inputName, setInputName] = useState<string | null>(null);
  const [outputName, setOutputName] = useState<string | null>(null);
  const [bufferSize, setBufferSize] = useState<number | null>(null);
  const prefilled = useRef(false);

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

  return (
    <div className="app">
      {/* Barra superior */}
      <header className="app__header">
        <div className="brand">
          <svg viewBox="0 0 32 32" className="brand__mark" aria-hidden="true">
            <circle cx="16" cy="16" r="13" fill="none" stroke="var(--color-border)" strokeWidth="3" />
            <line x1="16" y1="16" x2="24" y2="8" stroke="var(--color-accent)" strokeWidth="3" strokeLinecap="round" />
            <path d="M 9 21 A 10 10 0 0 1 23 21" fill="none" stroke="var(--color-cyan)" strokeWidth="2" />
          </svg>
          <span className="brand__name">Vox<span className="brand__accent">LFA</span></span>
          <span className="brand__tag">procesador vocal en vivo</span>
        </div>
        <div className="app__header-right">
          <StatusPill state={engine.status?.state ?? null} />
          <PairingBadge pairing={engine.pairing} />
        </div>
      </header>

      {/* Cuerpo en rejilla de tres columnas */}
      <main className="app__main">
        {/* Panel izquierdo: motor y dispositivos */}
        <aside className="panel panel--controls">
          <h2 className="panel__title">Motor</h2>

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
        <aside className="panel panel--side">
          <h2 className="panel__title">Presets</h2>
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
    </div>
  );
}
