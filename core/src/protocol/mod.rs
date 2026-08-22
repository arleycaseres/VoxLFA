//! Protocolo de comunicación entre el core y sus consumidores (UI desktop y
//! app móvil).
//!
//! Define el **contrato de datos**: eventos que el motor emite (niveles,
//! estado, dispositivos) y comandos que la UI/envía (arrancar, detener).
//!
//! - Los tipos se serializan a JSON con serde (`tag = "type"` en los enums).
//! - Los nombres de campo usan `camelCase` para coincidir con los tipos
//!   TypeScript (`desktop/src/lib/types.ts`, `mobile/src/lib/protocol.ts`).
//!
//! **Regla del proyecto:** nunca cambies un nombre de campo sin actualizar los
//! tres lados (Rust, TS desktop, TS móvil). Ver `docs/protocolo.md`.

pub mod analysis;
pub mod control;
pub mod dsp;
pub mod event;

pub use analysis::{
    AnalysisSample, SessionSummary, Suggestion, SuggestionAction, SuggestionKind, VoiceMetrics,
};
pub use control::ControlCommand;
pub use dsp::{
    DelayMode, DelayParams, DenoiseParams, DspLinkState, DspModuleKind, DspModuleSpec, DspState,
    EqBand, EqBandKind, FeedbackSuppressorParams, MusicalNote, MusicalScale, NoiseGateParams,
    PitchCorrectionParams, PresetId, PresetInfo, ReverbMode, ReverbParams, SaturatorMode,
    SaturatorParams,
};
pub use event::{
    AudioDeviceInfo, AudioHostInfo, EngineEvent, EngineState, EngineStatus, LevelSample,
    SpectrumSample, SPECTRUM_BIN_COUNT,
};
