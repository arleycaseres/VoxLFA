//! Análisis vocal: división en bandas, espectro (FFT), métricas, sugerencias y
//! resumen.
//!
//! Es la base de la Fase 2 (asistente vocal local) y de la Fase 5 (visualizador
//! de espectro). El camino de audio ([`crate::audio`]) ejecuta
//! [`bands::BandSplitter`] y [`fft::SpectrumAnalyzer`] (sin asignación); el
//! resto del análisis corre en un hilo dedicado que consume los marcos
//! resultantes y emite [`crate::protocol::AnalysisSample`] por el bus de
//! eventos, además de mantener un resumen de sesión consultable desde la UI
//! mediante [`handle::AnalysisHandle`].

mod analyzer;
mod bands;
mod fft;
mod handle;
mod suggest;

pub use analyzer::{SessionTracker, VoiceAnalyzer};
pub use bands::{BandSplitter, VoiceFrame, FRAMES_PER_FRAME};
pub use fft::{SpectrumAnalyzer, SPECTRUM_FFT_SIZE, SPECTRUM_HOP_SIZE};
pub use handle::{AnalysisHandle, AnalysisShared};
pub use suggest::SuggestionEngine;
