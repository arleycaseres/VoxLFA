//! Mango de control del análisis vocal (hilo de UI/control).
//!
//! Expone a la UI el último análisis y el resumen de sesión, y permite aplicar
//! una sugerencia con confirmación. El estado compartido se escribe desde el
//! hilo de análisis; el mango solo lo lee y, al aplicar una sugerencia,
//! delega en [`crate::dsp::DspHandle`] (mismo canal de reconfiguración en vivo).

use std::sync::{Arc, Mutex};

use crate::dsp::DspHandle;
use crate::error::Error;
use crate::protocol::{
    AnalysisSample, DelayParams, DenoiseParams, FeedbackSuppressorParams, PitchCorrectionParams,
    ReverbParams, SaturatorParams, SessionSummary, SuggestionAction,
};
use crate::Result;

/// Estado de análisis compartido entre el hilo de análisis y la UI.
#[derive(Debug, Default)]
pub struct AnalysisShared {
    /// Última muestra de análisis emitida.
    pub last_sample: Option<AnalysisSample>,
    /// Resumen acumulado de la sesión en curso.
    pub session: Option<SessionSummary>,
}

/// Mango de control del análisis vocal.
#[derive(Clone)]
pub struct AnalysisHandle {
    shared: Arc<Mutex<AnalysisShared>>,
    dsp: DspHandle,
}

impl AnalysisHandle {
    /// Crea un mango ligado al estado compartido y al control de la cadena DSP.
    pub fn new(shared: Arc<Mutex<AnalysisShared>>, dsp: DspHandle) -> Self {
        Self { shared, dsp }
    }

    /// Última muestra de análisis emitida (o `None` si aún no hay datos).
    pub fn get_analysis(&self) -> Result<Option<AnalysisSample>> {
        self.shared
            .lock()
            .map(|guard| guard.last_sample.clone())
            .map_err(|_| Error::analysis("analysis state lock poisoned"))
    }

    /// Resumen acumulado de la sesión en curso (o `None` si aún no arrancó).
    pub fn get_session_summary(&self) -> Result<Option<SessionSummary>> {
        self.shared
            .lock()
            .map(|guard| guard.session.clone())
            .map_err(|_| Error::analysis("analysis state lock poisoned"))
    }

    /// Aplica la acción de una sugerencia (con confirmación del usuario).
    ///
    /// Busca la sugerencia por su `id` en la última muestra emitida; si su
    /// acción es aplicar un preset, reconfigura la cadena en vivo.
    pub fn apply_suggestion(&self, id: u8) -> Result<()> {
        let shared = self
            .shared
            .lock()
            .map_err(|_| Error::analysis("analysis state lock poisoned"))?;
        let sample = shared
            .last_sample
            .as_ref()
            .ok_or_else(|| Error::analysis("no hay análisis disponible aún"))?;
        let suggestion = sample
            .suggestions
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| Error::analysis(format!("sugerencia no encontrada: {id}")))?;
        match &suggestion.action {
            SuggestionAction::None => Ok(()),
            SuggestionAction::ApplyPreset { preset } => self.dsp.apply_preset(*preset),
            SuggestionAction::SetEqBand {
                band_index,
                gain_db,
            } => self.dsp.set_eq_band(*band_index as usize, *gain_db),
            SuggestionAction::SetDenoise { mix } => {
                self.dsp.set_denoise(DenoiseParams { mix: *mix })
            }
            SuggestionAction::SetFeedback { threshold_db, q } => {
                self.dsp.set_feedback(FeedbackSuppressorParams {
                    threshold_db: *threshold_db,
                    q: *q,
                })
            }
            SuggestionAction::SetPitchCorrection { strength, mix } => {
                self.dsp.set_pitch_correction(PitchCorrectionParams {
                    scale: crate::protocol::MusicalScale::Chromatic,
                    root: crate::protocol::MusicalNote::C,
                    strength: *strength,
                    mix: *mix,
                })
            }
            SuggestionAction::SetNoiseGate {
                threshold_db,
                range_db,
            } => self.dsp.set_noise_gate(crate::protocol::NoiseGateParams {
                threshold_db: *threshold_db,
                attack_ms: 2.0,
                release_ms: 100.0,
                hold_ms: 25.0,
                range_db: *range_db,
            }),
            SuggestionAction::SetDelay { time_ms, mix } => self.dsp.set_delay(DelayParams {
                mode: crate::protocol::DelayMode::Digital,
                time_ms: *time_ms,
                feedback: 0.35,
                mix: *mix,
                pre_delay_ms: 0.0,
                low_cut_hz: 100.0,
                high_cut_hz: 8000.0,
                tempo_bpm: 120.0,
                sync_enabled: false,
                duck_amount: 0.0,
            }),
            SuggestionAction::SetReverb { wet, room_size } => self.dsp.set_reverb(ReverbParams {
                mode: crate::protocol::ReverbMode::Plate,
                room_size: *room_size,
                damping: 0.3,
                wet: *wet,
                pre_delay_ms: 15.0,
                high_cut_hz: 8000.0,
                low_cut_hz: 200.0,
            }),
            SuggestionAction::SetSaturator { drive, mix } => {
                self.dsp.set_saturator(SaturatorParams {
                    mode: crate::protocol::SaturatorMode::Tube,
                    drive: *drive,
                    mix: *mix,
                })
            }
        }
    }
}
