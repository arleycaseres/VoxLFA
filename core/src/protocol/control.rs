//! Comandos de control que la UI envía al motor.
//!
//! Los comandos viajan por dos canales distintos:
//!   * la UI de escritorio los invoca como comandos Tauri (`start_engine`,
//!     `apply_preset`, …), y
//!   * la app móvil los envía por el WebSocket como JSON con `tag = "type"`.
//!
//! `Start` **no** se acepta desde el móvil: arrancar el motor exige el callback
//! de eventos hacia la ventana, que solo existe en el flujo Tauri.

use serde::{Deserialize, Serialize};

use super::dsp::{
    DelayParams, DenoiseParams, FeedbackSuppressorParams, NoiseGateParams, PitchCorrectionParams,
    PresetId, ReverbParams, SaturatorParams,
};

/// Comando de control del motor, enviado por la UI (o el móvil) hacia el core.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ControlCommand {
    /// Arranca el pipeline con los dispositivos indicados.
    ///
    /// Si un dispositivo es `None`, se usa el predeterminado del sistema.
    /// Solo lo invoca la UI de escritorio vía Tauri; el WebSocket lo rechaza.
    Start {
        /// Nombre del dispositivo de entrada (micrófono).
        input_device: Option<String>,
        /// Nombre del dispositivo de salida (altavoces/interfaz).
        output_device: Option<String>,
    },
    /// Detiene el pipeline de forma controlada.
    Stop,
    /// Aplica un preset a la cadena en vivo (requiere el motor corriendo).
    SetPreset {
        /// Identificador del preset a aplicar.
        preset: PresetId,
    },
    /// Activa o desactiva el bypass global de la cadena (motor corriendo).
    SetGlobalBypass {
        /// `true` para puentear toda la cadena (paso directo).
        bypass: bool,
    },
    /// Activa o desactiva el bypass de un módulo por su nombre.
    SetLinkBypass {
        /// Nombre corto del módulo (p. ej. `"eq"`, `"compressor"`).
        link: String,
        /// `true` para puentear el módulo.
        bypass: bool,
    },
    /// Ajusta la ganancia de una banda del EQ del preset activo (motor corriendo).
    SetEqBand {
        /// Índice de la banda dentro del ecualizador (0 = primera).
        band_index: usize,
        /// Ganancia en dB (se acota a `[-18, 18]` en la entrada de red).
        gain_db: f32,
    },
    /// Ajusta los parámetros de la puerta de ruido del preset activo (motor corriendo).
    SetNoiseGate {
        /// Nuevos parámetros de la puerta.
        params: NoiseGateParams,
    },
    /// Ajusta la mezcla seco/húmedo del denoise del preset activo (motor corriendo).
    SetDenoise {
        /// Nuevos parámetros de denoise.
        params: DenoiseParams,
    },
    /// Ajusta los parámetros del feedback suppressor del preset activo (motor corriendo).
    SetFeedback {
        /// Nuevos parámetros del supresor de feedback.
        params: FeedbackSuppressorParams,
    },
    /// Ajusta los parámetros de corrección de tono del preset activo (motor corriendo).
    SetPitchCorrection {
        /// Nuevos parámetros de corrección de tono.
        params: PitchCorrectionParams,
    },
    /// Ajusta los parámetros del delay del preset activo (motor corriendo).
    SetDelay {
        /// Nuevos parámetros del delay.
        params: DelayParams,
    },
    /// Ajusta los parámetros del reverb del preset activo (motor corriendo).
    SetReverb {
        /// Nuevos parámetros del reverb.
        params: ReverbParams,
    },
    /// Ajusta los parámetros de saturación del preset activo (motor corriendo).
    SetSaturator {
        /// Nuevos parámetros de saturación.
        params: SaturatorParams,
    },
}
