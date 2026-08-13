// Visualizador de espectro en tiempo real (canvas, Fase 5 del plan de producto).
//
// Dibuja las bandas logarítmicas emitidas por el motor (`EngineEvent::Spectrum`)
// como barras sobre una escala de frecuencia logarítmica y de amplitud en dB.
// El motor ya suaviza los niveles (ataque rápido / release en dB); aquí solo se
// refleja la última muestra en cada fotograma.

import { useEffect, useRef } from "react";
import type { SpectrumSample } from "../lib/types";
import { SPECTRUM_BIN_COUNT } from "../lib/types";
import "./SpectrumView.css";

/** Frecuencias (Hz) de referencia para las marcas del eje inferior. */
const FREQ_MARKS = [100, 1000, 10000];
/** Frecuencia mínima de la escala (Hz). */
const FREQ_MIN = 20;
/** Niveles (dBFS) de las líneas de la rejilla horizontal. */
const DB_MARKS = [0, -12, -24, -36, -48, -60];
/** Límite superior e inferior de la escala de amplitud (dBFS). */
const DB_TOP = 0;
const DB_BOTTOM = -60;

/** Colores de la cabina (espejo de `tokens.css`). */
const COLOR_GRID = "rgba(42, 46, 52, 0.9)";
const COLOR_GRID_LABEL = "#8a929c";
const COLOR_BAR = "#4fd8ff";
const COLOR_BAR_HOT = "#ff4a1f";

/** Posición horizontal (px) de una frecuencia dentro de la escala logarítmica. */
function xFor(freqHz: number, freqMax: number, width: number): number {
  const t = Math.log(freqHz / FREQ_MIN) / Math.log(freqMax / FREQ_MIN);
  return t * width;
}

/** Posición vertical (px) de un nivel dBFS dentro de la escala. */
function yFor(dbfs: number, height: number): number {
  const t = (DB_TOP - dbfs) / (DB_TOP - DB_BOTTOM);
  return t * height;
}

interface SpectrumViewProps {
  /** Última muestra de espectro del motor (o `null` si no hay datos). */
  spectrum: SpectrumSample | null;
}

export function SpectrumView({ spectrum }: SpectrumViewProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const spectrumRef = useRef<SpectrumSample | null>(spectrum);
  spectrumRef.current = spectrum;

  useEffect(() => {
    const container = containerRef.current;
    const canvas = canvasRef.current;
    if (!container || !canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let width = 0;
    let height = 0;
    let lastDrawn: { at: number; width: number; height: number } | null = null;

    /** Ajusta el tamaño del lienzo a su contenedor (con DPI del monitor). */
    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = container.getBoundingClientRect();
      width = rect.width;
      height = rect.height;
      canvas.width = Math.max(1, Math.round(width * dpr));
      canvas.height = Math.max(1, Math.round(height * dpr));
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };

    /** Dibuja rejilla, etiquetas y barras del espectro actual. */
    const draw = () => {
      const sample = spectrumRef.current;
      const signature = {
        at: sample?.capturedAtMs ?? 0,
        width,
        height,
      };
      // No redibujar si nada cambió desde el último fotograma.
      if (lastDrawn && lastDrawn.at === signature.at) {
        if (
          lastDrawn.width === signature.width &&
          lastDrawn.height === signature.height
        ) {
          return;
        }
      }
      lastDrawn = signature;

      ctx.clearRect(0, 0, width, height);
      if (width < 4 || height < 4) return;

      // Límite superior de frecuencia (Nyquist o 20 kHz, como en el core).
      const freqMax = Math.min((sample?.sampleRate ?? 48000) / 2, 20000);

      // Rejilla horizontal (niveles dB).
      ctx.font = "9px 'IBM Plex Mono', ui-monospace, monospace";
      ctx.textBaseline = "middle";
      ctx.fillStyle = COLOR_GRID_LABEL;
      for (const db of DB_MARKS) {
        const y = yFor(db, height);
        ctx.strokeStyle = COLOR_GRID;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
        ctx.fillText(`${db}`, 4, y);
      }

      // Barras: una por banda logarítmica.
      if (sample) {
        const bins = sample.binsDb;
        const ratio = Math.pow(freqMax / FREQ_MIN, 1 / SPECTRUM_BIN_COUNT);
        const gap = Math.max(1, width / SPECTRUM_BIN_COUNT * 0.18);
        for (let i = 0; i < Math.min(SPECTRUM_BIN_COUNT, bins.length); i += 1) {
          const leftHz = FREQ_MIN * Math.pow(ratio, i);
          const rightHz = FREQ_MIN * Math.pow(ratio, i + 1);
          const x0 = xFor(leftHz, freqMax, width);
          const x1 = xFor(rightHz, freqMax, width);
          const db = bins[i];
          const yTop = yFor(db, height);
          const x = x0 + gap / 2;
          const w = Math.max(1, x1 - x0 - gap);

          // Relleno degradado vertical (más brillante cerca del pico).
          const gradient = ctx.createLinearGradient(0, yTop, 0, height);
          gradient.addColorStop(0, db >= -6 ? COLOR_BAR_HOT : COLOR_BAR);
          gradient.addColorStop(1, "rgba(79, 216, 255, 0.06)");
          ctx.fillStyle = gradient;
          ctx.fillRect(x, yTop, w, height - yTop);
        }
      }

      // Etiquetas de frecuencia en la base.
      ctx.textBaseline = "alphabetic";
      for (const freq of FREQ_MARKS) {
        if (freq > freqMax) break;
        const x = xFor(freq, freqMax, width);
        ctx.strokeStyle = COLOR_GRID;
        ctx.beginPath();
        ctx.moveTo(x, height - 2);
        ctx.lineTo(x, height - 6);
        ctx.stroke();
        ctx.fillText(freq >= 1000 ? `${freq / 1000}k` : `${freq}`, x - 6, height - 8);
      }
    };

    resize();
    const observer = new ResizeObserver(() => {
      resize();
      draw();
    });
    observer.observe(container);

    let rafId = 0;
    const loop = () => {
      draw();
      rafId = requestAnimationFrame(loop);
    };
    rafId = requestAnimationFrame(loop);

    return () => {
      cancelAnimationFrame(rafId);
      observer.disconnect();
    };
  }, []);

  return (
    <div className="spectrum" ref={containerRef}>
      <canvas className="spectrum__canvas" ref={canvasRef} />
    </div>
  );
}
