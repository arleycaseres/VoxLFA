//! Procesamiento de audio (DSP) puro: sin I/O ni dependencias de plataforma.
//!
//! El contrato base es el trait [`AudioProcessor`]: un transformador de bloques
//! `input → output`. Los procesadores concretos se combinan en una cadena
//! ([`chain::ChainProcessor`]) que modela la cabina de voz (EQ, compresor,
//! de-esser, saturación, delay, reverb, limiter).

pub mod biquad;
pub mod boomsuppressor;
pub mod chain;
pub mod compressor;
pub mod deesser;
pub mod delay;
#[cfg(feature = "rnnoise")]
pub mod denoise;
#[cfg(feature = "onnx")]
pub mod denoise_onnx;
pub mod eq;
pub mod feedback;
pub mod gain;
pub mod gate;
pub mod highpass;
pub mod level;
pub mod limiter;
pub mod notch;
pub mod passthrough;
pub mod pitch_correction;
pub mod presets;
pub mod processor;
pub mod reverb;
pub mod saturator;

pub use biquad::{BiquadFilter, BiquadKind, BiquadParams};
pub use boomsuppressor::BoomSuppressor;
pub use chain::{ChainProcessor, DspCommand, DspHandle};
pub use compressor::Compressor;
pub use deesser::DeEsser;
pub use delay::{Delay, DelayLine};
#[cfg(feature = "rnnoise")]
pub use denoise::RnnoiseDenoise;
#[cfg(feature = "onnx")]
pub use denoise_onnx::OnnxDenoise;
pub use eq::ParametricEq;
pub use feedback::FeedbackSuppressor;
pub use gain::Gain;
pub use gate::NoiseGate;
pub use highpass::HighPass;
pub use level::{LevelMeter, Levels};
pub use limiter::Limiter;
pub use notch::Notch;
pub use passthrough::PassThroughProcessor;
pub use pitch_correction::PitchCorrection;
pub use presets::PresetFactory;
pub use processor::{AudioProcessor, ProcessResult, ProcessingInfo};
pub use reverb::Reverb;
pub use saturator::Saturator;
