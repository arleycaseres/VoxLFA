//! Motor de audio en tiempo real (feature `audio`).
//!
//! Responsable de la **I/O de audio**: captura con `cpal`, paso a través de la
//! cadena DSP del crate y salida al dispositivo de reproducción, midiendo la
//! latencia real del pipeline (muestras en vuelo entre captura y salida).

mod engine;

pub use engine::{AudioEngine, AudioEngineConfig, EngineHandle};

// El mango de la cadena DSP vive en `dsp`, pero se re-exporta aquí porque el
// control en vivo del motor (presets/bypass) es parte de la API de audio.
pub use crate::dsp::DspHandle;
