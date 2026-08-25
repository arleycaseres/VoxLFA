import { memo, useCallback } from "react";
// Panel Simple: 4 controles human-friendly que mapean a múltiples módulos DSP.
//
// Cada slider (0–100%) envía comandos DSP que ajustan varios parámetros a la
// vez. El valor del slider se deriva de los parámetros actuales de la cadena
// (lectura inversa de los módulos activos).

import type {
  DspState,
  DenoiseParams,
  NoiseGateParams,
  FeedbackSuppressorParams,
  CompressorParams,
  DeEsserParams,
  DynamicEqParams,
} from "../lib/types";

interface SimplePanelProps {
  dsp: DspState | null;
  running: boolean;
  onSetDenoise: (params: DenoiseParams) => void;
  onSetNoiseGate: (params: NoiseGateParams) => void;
  onSetFeedback: (params: FeedbackSuppressorParams) => void;
  onSetCompressor: (params: CompressorParams) => void;
  onSetDeEsser: (params: DeEsserParams) => void;
  onSetDynamicEq: (params: DynamicEqParams) => void;
  onSetEqBand: (bandIndex: number, gainDb: number) => void;
  onSetLinkBypass: (link: string, bypass: boolean) => void;
}

function findLink(dsp: DspState, name: string) {
  return dsp.links.find((l) => l.name === name) ?? null;
}

/** Clamp a value to [min, max]. */
function clamp(v: number, min: number, max: number) {
  return Math.min(max, Math.max(min, v));
}

/** Linear interpolation from a source range to [0, 1]. */
function toPercent(v: number, min: number, max: number) {
  return clamp((v - min) / (max - min), 0, 1);
}

/** Linear interpolation from [0, 1] to a source range. */
function fromPercent(p: number, min: number, max: number) {
  return p * (max - min) + min;
}

// ─── Claridad de voz ────────────────────────────────────────────────
// Slider 0–100%.
// 0%: de-esser off (amount=0), high-shelf 0 dB, dynamic eq ratio 1.
// 100%: de-esser max (amount=1), high-shelf -3 dB, dynamic eq ratio 4.

function clarityToPercent(dsp: DspState): number {
  const deesser = findLink(dsp, "deesser")?.deEsserParams;
  const eqLink = findLink(dsp, "eq");
  const dynEq = findLink(dsp, "dynamicEq")?.dynamicEqParams;

  const deesserScore = toPercent(deesser?.amount ?? 0, 0, 1);
  // High-shelf band (last EQ band typically) — more negative = more clarity
  const eqBands = eqLink?.eqBands;
  const hsGain = eqBands && eqBands.length > 0
    ? eqBands[eqBands.length - 1]?.gainDb ?? 0
    : 0;
  const eqScore = toPercent(-hsGain, 0, 3);
  // Dynamic eq ratio — higher = more clarity
  const dynRatio = dynEq?.bands?.[0]?.ratio ?? 1;
  const dynScore = toPercent(dynRatio, 1, 4);

  return clamp(Math.round((deesserScore * 0.5 + eqScore * 0.3 + dynScore * 0.2) * 100), 0, 100);
}

function percentToClarity(
  p: number,
  dsp: DspState,
  fns: Pick<SimplePanelProps, "onSetDeEsser" | "onSetEqBand" | "onSetDynamicEq">,
) {
  const amt = p;
  const deesserLink = findLink(dsp, "deesser");
  const eqLink = findLink(dsp, "eq");
  const dynEqLink = findLink(dsp, "dynamicEq");

  // De-esser
  if (deesserLink?.deEsserParams) {
    fns.onSetDeEsser({
      ...deesserLink.deEsserParams,
      amount: amt,
    });
  } else if (deesserLink) {
    fns.onSetDeEsser({ thresholdDb: -32, freqHz: 6500, amount: amt });
  }

  // High-shelf EQ (last band): 0 dB at 0%, -3 dB at 100%
  if (eqLink?.eqBands) {
    const bands = eqLink.eqBands;
    const lastIdx = bands.length - 1;
    if (lastIdx >= 0) {
      fns.onSetEqBand(lastIdx, -amt * 3);
    }
  }

  // Dynamic EQ ratio: 1 at 0%, 4 at 100%
  if (dynEqLink?.dynamicEqParams?.bands?.[0]) {
    const band0 = dynEqLink.dynamicEqParams.bands[0];
    fns.onSetDynamicEq({
      bands: [
        { ...band0, ratio: 1 + amt * 3 },
        ...dynEqLink.dynamicEqParams.bands.slice(1),
      ],
    });
  }
}

// ─── Reducir ruido ─────────────────────────────────────────────────
// 0%: denoise mix 0, gate threshold -80 (abierto), gate range 0.
// 100%: denoise mix 1, gate threshold -30 (cerrado), gate range 50.

function noiseToPercent(dsp: DspState): number {
  const denoise = findLink(dsp, "denoise")?.denoiseParams;
  const gate = findLink(dsp, "noisegate")?.gateParams;
  const denoiseScore = toPercent(denoise?.mix ?? 0, 0, 1);
  const gateScore = gate
    ? toPercent(gate.rangeDb, 0, 50)
    : 0;
  return clamp(Math.round((denoiseScore * 0.7 + gateScore * 0.3) * 100), 0, 100);
}

function percentToNoise(
  p: number,
  dsp: DspState,
  fns: Pick<SimplePanelProps, "onSetDenoise" | "onSetNoiseGate">,
) {
  const denoiseLink = findLink(dsp, "denoise");
  const gateLink = findLink(dsp, "noisegate");

  if (denoiseLink?.denoiseParams) {
    fns.onSetDenoise({ mix: p });
  } else if (denoiseLink) {
    fns.onSetDenoise({ mix: p });
  }

  if (gateLink?.gateParams) {
    fns.onSetNoiseGate({
      ...gateLink.gateParams,
      thresholdDb: fromPercent(p, -80, -30),
      rangeDb: p * 50,
    });
  } else if (gateLink) {
    fns.onSetNoiseGate({
      thresholdDb: fromPercent(p, -80, -30),
      attackMs: 2,
      releaseMs: 100,
      holdMs: 120,
      rangeDb: p * 50,
    });
  }
}

// ─── Evitar pitidos ────────────────────────────────────────────────
// 0%: threshold -10 (muy permisivo, casi off).
// 100%: threshold -40 (agresivo).

function feedbackToPercent(dsp: DspState): number {
  const fb = findLink(dsp, "feedback")?.feedbackParams;
  if (!fb) return 0;
  return clamp(Math.round(toPercent(-fb.thresholdDb, 10, 40) * 100), 0, 100);
}

function percentToFeedback(
  p: number,
  dsp: DspState,
  fns: Pick<SimplePanelProps, "onSetFeedback">,
) {
  const fbLink = findLink(dsp, "feedback");
  if (fbLink?.feedbackParams) {
    fns.onSetFeedback({
      ...fbLink.feedbackParams,
      thresholdDb: -fromPercent(p, 10, 40),
    });
  } else if (fbLink) {
    fns.onSetFeedback({
      mode: "notch",
      thresholdDb: -fromPercent(p, 10, 40),
      q: 10,
      mu: 0.15,
      filterLen: 256,
    });
  }
}

// ─── Volumen parejo ────────────────────────────────────────────────
// 0%: threshold -10 (no comprime), ratio 1, makeup 0.
// 100%: threshold -30 (comprime mucho), ratio 5, makeup 6.

function volumeToPercent(dsp: DspState): number {
  const comp = findLink(dsp, "compressor")?.compressorParams;
  if (!comp) return 0;
  const threshScore = toPercent(-comp.thresholdDb, 10, 30);
  const ratioScore = toPercent(comp.ratio, 1, 5);
  const makeupScore = toPercent(comp.makeupDb, 0, 6);
  return clamp(Math.round((threshScore * 0.5 + ratioScore * 0.3 + makeupScore * 0.2) * 100), 0, 100);
}

function percentToVolume(
  p: number,
  dsp: DspState,
  fns: Pick<SimplePanelProps, "onSetCompressor">,
) {
  const compLink = findLink(dsp, "compressor");
  if (compLink?.compressorParams) {
    fns.onSetCompressor({
      ...compLink.compressorParams,
      thresholdDb: -fromPercent(p, 10, 30),
      ratio: 1 + p * 4,
      makeupDb: p * 6,
    });
  } else if (compLink) {
    fns.onSetCompressor({
      thresholdDb: -fromPercent(p, 10, 30),
      ratio: 1 + p * 4,
      attackMs: 5,
      releaseMs: 80,
      makeupDb: p * 6,
    });
  }
}

// ─── UI ─────────────────────────────────────────────────────────────

const SimplePanel = memo(function SimplePanel({
  dsp,
  running,
  onSetDenoise,
  onSetNoiseGate,
  onSetFeedback,
  onSetCompressor,
  onSetDeEsser,
  onSetDynamicEq,
  onSetEqBand,
  onSetLinkBypass,
}: SimplePanelProps) {
  const fns = { onSetDenoise, onSetNoiseGate, onSetFeedback, onSetCompressor, onSetDeEsser, onSetDynamicEq, onSetEqBand, onSetLinkBypass };

  const clarityVal = dsp ? clarityToPercent(dsp) : 50;
  const noiseVal = dsp ? noiseToPercent(dsp) : 50;
  const feedbackVal = dsp ? feedbackToPercent(dsp) : 50;
  const volumeVal = dsp ? volumeToPercent(dsp) : 50;

  const onClarityChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      if (!dsp) return;
      const p = Number(e.target.value) / 100;
      percentToClarity(p, dsp, fns);
    },
    [dsp],
  );

  const onNoiseChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      if (!dsp) return;
      const p = Number(e.target.value) / 100;
      percentToNoise(p, dsp, fns);
    },
    [dsp],
  );

  const onFeedbackChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      if (!dsp) return;
      const p = Number(e.target.value) / 100;
      percentToFeedback(p, dsp, fns);
    },
    [dsp],
  );

  const onVolumeChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      if (!dsp) return;
      const p = Number(e.target.value) / 100;
      percentToVolume(p, dsp, fns);
    },
    [dsp],
  );

  if (!running) {
    return (
      <div className="simple-panel">
        <p className="simple-panel__empty">Arranca el motor para ajustar los controles simples.</p>
      </div>
    );
  }

  const disabled = !running || !dsp;

  const controls = [
    {
      id: "clarity",
      label: "Claridad de voz",
      description: "Reduce sibilancia y realza presencia.",
      value: clarityVal,
      onChange: onClarityChange,
    },
    {
      id: "noise",
      label: "Reducir ruido",
      description: "Suprime ruido de fondo y abre puerta.",
      value: noiseVal,
      onChange: onNoiseChange,
    },
    {
      id: "feedback",
      label: "Evitar pitidos",
      description: "Suprime retroalimentación del sistema.",
      value: feedbackVal,
      onChange: onFeedbackChange,
    },
    {
      id: "volume",
      label: "Volumen parejo",
      description: "Comprime para mantener nivel estable.",
      value: volumeVal,
      onChange: onVolumeChange,
    },
  ];

  return (
    <div className="simple-panel">
      {controls.map((c) => (
        <div key={c.id} className="simple-panel__control">
          <div className="simple-panel__control-header">
            <span className="simple-panel__label">{c.label}</span>
            <span className="simple-panel__value">{c.value}%</span>
          </div>
          <input
            type="range"
            className="simple-panel__slider"
            min={0}
            max={100}
            step={1}
            value={c.value}
            disabled={disabled}
            onChange={c.onChange}
            aria-label={c.label}
          />
          <span className="simple-panel__hint">{c.description}</span>
        </div>
      ))}
    </div>
  );
});

export { SimplePanel };
