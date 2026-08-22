//! Contrato de análisis vocal del protocolo: métricas, sugerencias y resumen.
//!
//! El motor de análisis (`crate::analysis`) calcula métricas de la voz en vivo
//! (timbre, dinámica, fatiga, resonancia) y genera sugerencias en español para
//! la UI. Este módulo define solo los tipos que viajan por el protocolo.

use serde::{Deserialize, Serialize};

use super::dsp::PresetId;

/// Métricas de la voz calculadas sobre una ventana deslizante de audio.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceMetrics {
    /// Nivel RMS medio de la ventana (dBFS).
    pub rms_db: f32,
    /// Nivel pico de la ventana (dBFS).
    pub peak_db: f32,
    /// Rango dinámico de la ventana (dB entre la frase más floja y la más fuerte).
    pub dynamic_range_db: f32,
    /// Factor de cresta (dB entre pico y RMS): indica picos o compresión.
    pub crest_db: f32,
    /// Brillo espectral (0–1): energía en agudos frente al total.
    pub brightness: f32,
    /// Resonancia baja-media (0–1): energía en la zona de *boominess*.
    pub resonance_score: f32,
    /// Índice de fatiga vocal (0–1): esfuerzo sostenido de la voz.
    pub fatigue_score: f32,
    /// Tamaño de la ventana de análisis (ms).
    pub window_ms: u32,
}

/// Área de la voz que motiva una sugerencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuggestionKind {
    /// Timbre: brillo / opacidad / presencia.
    Timbre,
    /// Dinámica: compresión o exceso de rango.
    Dynamics,
    /// Fatiga vocal.
    Fatigue,
    /// Resonancia / *boominess*.
    Resonance,
    /// Sugerencia del asesor de IA (LLM).
    AiAdvisor,
}

/// Acción que el usuario puede confirmar desde la UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SuggestionAction {
    /// Solo informativo: no hay nada que aplicar.
    None,
    /// Aplicar un preset generado a partir de la sugerencia (con confirmación).
    ApplyPreset {
        /// Preset sugerido.
        preset: PresetId,
    },
    /// Ajustar la ganancia de una banda del ecualizador.
    SetEqBand {
        /// Índice de la banda (0–6).
        band_index: u8,
        /// Ganancia objetivo en dB (−18 … +18).
        gain_db: f32,
    },
    /// Ajustar la mezcla del denoise.
    SetDenoise {
        /// Mezcla seco/húmedo (0–1).
        mix: f32,
    },
    /// Ajustar los parámetros del feedback suppressor.
    SetFeedback {
        /// Umbral de detección en dBFS.
        threshold_db: f32,
        /// Factor de calidad Q.
        q: f32,
    },
    /// Ajustar la corrección de tono.
    SetPitchCorrection {
        /// Intensidad de la corrección (0–1).
        strength: f32,
        /// Mezcla seco/húmedo (0–1).
        mix: f32,
    },
    /// Ajustar la puerta de ruido.
    SetNoiseGate {
        /// Umbral (dBFS) a partir del cual se abre la puerta.
        threshold_db: f32,
        /// Atenuación máxima al cerrar (dB).
        range_db: f32,
    },
    /// Ajustar los parámetros del delay.
    SetDelay {
        /// Tiempo del eco en ms.
        time_ms: f32,
        /// Mezcla seco/húmedo (0–1).
        mix: f32,
    },
    /// Ajustar los parámetros del reverb.
    SetReverb {
        /// Mezcla seco/húmedo (0–1).
        wet: f32,
        /// Tamaño de la sala (0–1).
        room_size: f32,
    },
    /// Ajustar los parámetros de saturación.
    SetSaturator {
        /// Ganancia previa al clipping (0–16).
        drive: f32,
        /// Mezcla seco/húmedo (0–1).
        mix: f32,
    },
}

/// Sugerencia generada por el motor de análisis para la voz actual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    /// Identificador estable de la regla (permite `apply_suggestion`).
    pub id: u8,
    /// Área de la voz a la que se refiere.
    pub kind: SuggestionKind,
    /// Importancia (0–1): a mayor valor, más relevante.
    pub severity: f32,
    /// Mensaje legible en español para la UI.
    pub message: String,
    /// Acción confirmable que acompaña a la sugerencia.
    pub action: SuggestionAction,
}

/// Muestra de análisis emitida por el motor (métricas + sugerencias).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisSample {
    /// Métricas de la ventana actual.
    pub metrics: VoiceMetrics,
    /// Sugerencias activas para esta ventana (reglas disparadas).
    pub suggestions: Vec<Suggestion>,
    /// Tiempo monotónico (ms) de la captura.
    pub captured_at_ms: u64,
}

/// Resumen acumulado de la sesión de audio en curso (exportable a JSON).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    /// Tiempo (ms epoch) en el que arrancó la sesión.
    pub started_at_ms: u64,
    /// Duración de la sesión hasta ahora (ms).
    pub duration_ms: u64,
    /// Nivel RMS medio de toda la sesión (dBFS).
    pub avg_rms_db: f32,
    /// Pico máximo de la sesión (dBFS).
    pub peak_db: f32,
    /// Rango dinámico observado (dB).
    pub dynamic_range_db: f32,
    /// Brillo medio de la sesión (0–1).
    pub avg_brightness: f32,
    /// Índice de fatiga acumulado de la sesión (0–1).
    pub fatigue_score: f32,
    /// Tiempo con nivel alto (RMS > -20 dBFS) en ms.
    pub loud_time_ms: u64,
    /// Número de sugerencias emitidas durante la sesión.
    pub suggestions_count: u32,
}
