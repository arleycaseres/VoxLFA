// Panel DSP en modo Avanzado, memoizado.
//
// Se aísla en un componente propio para que las actualizaciones de alta
// frecuencia (medidores de nivel/espectro) no fuerzen el re-render de toda la
// cadena DSP: este panel solo depende de `dsp`, `running` y del estado local
// del acordeón, no de las muestras de nivel. Con `React.memo` y callbacks
// estables, se salta el re-render cuando únicamente cambian `level`/`spectrum`.

import { memo } from "react";
import type { DspState } from "../lib/types";
import { DspChain } from "./DspChain";
import { EqPanel } from "./EqPanel";
import { GatePanel } from "./GatePanel";
import { DenoisePanel } from "./DenoisePanel";
import { FeedbackPanel } from "./FeedbackPanel";
import { DynamicEqPanel } from "./DynamicEqPanel";
import { PitchCorrectionPanel } from "./PitchCorrectionPanel";
import { DelayPanel } from "./DelayPanel";
import { ReverbPanel } from "./ReverbPanel";
import { SaturatorPanel } from "./SaturatorPanel";
import { HarmonizerPanel } from "./HarmonizerPanel";
import { SculptPanel } from "./SculptPanel";
import { VocalIsolationPanel } from "./VocalIsolationPanel";

type AccordionGroup = "basic" | "dynamics" | "effects" | "creative";

const ACCORDION_GROUPS: { key: AccordionGroup; label: string }[] = [
  { key: "basic", label: "Procesamiento básico" },
  { key: "dynamics", label: "Dinámica" },
  { key: "effects", label: "Efectos" },
  { key: "creative", label: "Creativos" },
];

interface Props {
  dsp: DspState | null;
  running: boolean;
  openAccordion: AccordionGroup | null;
  onToggleAccordion: (group: AccordionGroup) => void;
  onGlobalBypass: (bypass: boolean) => void;
  onLinkBypass: (link: string, bypass: boolean) => void;
  onSetEqBand: (index: number, gainDb: number) => void;
  onSetNoiseGate: (params: Parameters<import("../hooks/useEngine").EngineController["setNoiseGate"]>[0]) => void;
  onSetDenoise: (params: Parameters<import("../hooks/useEngine").EngineController["setDenoise"]>[0]) => void;
  onSetFeedback: (params: Parameters<import("../hooks/useEngine").EngineController["setFeedback"]>[0]) => void;
  onSetDynamicEq: (params: Parameters<import("../hooks/useEngine").EngineController["setDynamicEq"]>[0]) => void;
  onSetPitchCorrection: (params: Parameters<import("../hooks/useEngine").EngineController["setPitchCorrection"]>[0]) => void;
  onSetDelay: (params: Parameters<import("../hooks/useEngine").EngineController["setDelay"]>[0]) => void;
  onSetReverb: (params: Parameters<import("../hooks/useEngine").EngineController["setReverb"]>[0]) => void;
  onSetSaturator: (params: Parameters<import("../hooks/useEngine").EngineController["setSaturator"]>[0]) => void;
  onSetHarmonizer: (params: Parameters<import("../hooks/useEngine").EngineController["setHarmonizer"]>[0]) => void;
  onSetSculpt: (params: Parameters<import("../hooks/useEngine").EngineController["setSculpt"]>[0]) => void;
  onSetVocalIsolation: (params: Parameters<import("../hooks/useEngine").EngineController["setVocalIsolation"]>[0]) => void;
}

function AdvancedDspPanelInner({
  dsp,
  running,
  openAccordion,
  onToggleAccordion,
  onGlobalBypass,
  onLinkBypass,
  onSetEqBand,
  onSetNoiseGate,
  onSetDenoise,
  onSetFeedback,
  onSetDynamicEq,
  onSetPitchCorrection,
  onSetDelay,
  onSetReverb,
  onSetSaturator,
  onSetHarmonizer,
  onSetSculpt,
  onSetVocalIsolation,
}: Props) {
  return (
    <>
      <h2 className="panel__title panel__title--spaced">Cadena DSP</h2>
      <DspChain
        dsp={dsp}
        onGlobalBypass={onGlobalBypass}
        onLinkBypass={onLinkBypass}
      />

      {ACCORDION_GROUPS.map(({ key, label }) => (
        <div key={key} className="accordion">
          <button
            type="button"
            className={`accordion__header ${openAccordion === key ? "accordion__header--open" : ""}`}
            onClick={() => onToggleAccordion(key)}
            aria-expanded={openAccordion === key}
          >
            <span>{label}</span>
            <span className="accordion__chevron">{openAccordion === key ? "▾" : "▸"}</span>
          </button>
          {openAccordion === key && (
            <div className="accordion__body">
              {key === "basic" && (
                <>
                  <h3 className="panel__subtitle">Ecualizador</h3>
                  <EqPanel dsp={dsp} running={running} onSetEqBand={onSetEqBand} />
                  <h3 className="panel__subtitle">Puerta de ruido</h3>
                  <GatePanel dsp={dsp} running={running} onSetNoiseGate={onSetNoiseGate} />
                  <h3 className="panel__subtitle">Supresión de ruido</h3>
                  <DenoisePanel dsp={dsp} running={running} onSetDenoise={onSetDenoise} />
                  <h3 className="panel__subtitle">Antifeedback</h3>
                  <FeedbackPanel dsp={dsp} running={running} onSetFeedback={onSetFeedback} />
                  <h3 className="panel__subtitle">Aislamiento de voz</h3>
                  <VocalIsolationPanel dsp={dsp} running={running} onSetVocalIsolation={onSetVocalIsolation} />
                  <h3 className="panel__subtitle">Sculpt</h3>
                  <SculptPanel dsp={dsp} running={running} onSetSculpt={onSetSculpt} />
                </>
              )}
              {key === "dynamics" && (
                <>
                  <h3 className="panel__subtitle">EQ Dinámico</h3>
                  <DynamicEqPanel dsp={dsp} running={running} onSetDynamicEq={onSetDynamicEq} />
                  <h3 className="panel__subtitle">Corrección tono</h3>
                  <PitchCorrectionPanel dsp={dsp} running={running} onSetPitchCorrection={onSetPitchCorrection} />
                </>
              )}
              {key === "effects" && (
                <>
                  <h3 className="panel__subtitle">Delay</h3>
                  <DelayPanel dsp={dsp} running={running} onSetDelay={onSetDelay} />
                  <h3 className="panel__subtitle">Reverb</h3>
                  <ReverbPanel dsp={dsp} running={running} onSetReverb={onSetReverb} />
                  <h3 className="panel__subtitle">Saturación</h3>
                  <SaturatorPanel dsp={dsp} running={running} onSetSaturator={onSetSaturator} />
                </>
              )}
              {key === "creative" && (
                <>
                  <h3 className="panel__subtitle">Harmonizer</h3>
                  <HarmonizerPanel dsp={dsp} running={running} onSetHarmonizer={onSetHarmonizer} />
                </>
              )}
            </div>
          )}
        </div>
      ))}
    </>
  );
}

export const AdvancedDspPanel = memo(AdvancedDspPanelInner);
