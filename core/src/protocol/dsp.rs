//! Contrato DSP del protocolo: presets, módulos de la cadena y estado.
//!
//! Estos tipos son la **configuración** que la UI envía (preset aplicado,
//! bypass) y el **estado** que recibe (cadena activa con sus módulos). Son
//! puramente declarativos: la implementación del procesamiento vive en
//! `crate::dsp`. `dsp` depende de este módulo (config → procesamiento).

use serde::{Deserialize, Serialize};

/// Identificador de un preset de la cabina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PresetId {
    /// Sin procesamiento: paso directo de la señal.
    Dry,
    /// Voz limpia: EQ suave y compresión transparente.
    VozLimpia,
    /// Radio: carácter telefónico (banda estrecha + saturación).
    Radio,
    /// Warm: bajos suaves y presencia vocal.
    Warm,
}

/// Metadatos de un preset para mostrarlo en la cabina.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    /// Identificador único del preset.
    pub id: PresetId,
    /// Nombre legible (en español) para la UI.
    pub name: String,
    /// Descripción breve de una línea.
    pub description: String,
}

/// Banda de un ecualizador paramétrico.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBand {
    /// Tipo de la banda (shelving o pico).
    pub kind: EqBandKind,
    /// Frecuencia central (Hz).
    pub freq_hz: f32,
    /// Ganancia en dB (negativo = corte).
    pub gain_db: f32,
    /// Factor de calidad Q (solo relevante para bandas de pico).
    pub q: f32,
}

/// Tipo de banda del ecualizador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EqBandKind {
    /// Shelf de graves.
    LowShelf,
    /// Banda de pico (campana).
    Peaking,
    /// Shelf de agudos.
    HighShelf,
}

/// Tipo de módulo de la cadena DSP con sus parámetros.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DspModuleKind {
    /// Ganancia de salida (ganancia lineal en dB).
    Gain {
        /// Ganancia en dB (positivo = amplificar).
        gain_db: f32,
    },
    /// Filtro pasa-altos (recorta subgraves / boominess).
    HighPass {
        /// Frecuencia de corte (Hz).
        cutoff_hz: f32,
    },
    /// Filtro muesca (notch): mata una resonancia de feedback concreta.
    Notch {
        /// Frecuencia central de la muesca (Hz).
        freq_hz: f32,
        /// Factor de calidad (a mayor Q, muesca más estrecha).
        q: f32,
    },
    /// Supresión dinámica de "boominess" (graves medios).
    BoomSuppressor {
        /// Umbral (dBFS) de activación.
        threshold_db: f32,
        /// Frecuencia central de la banda baja-media (Hz).
        freq_hz: f32,
        /// Cantidad de reducción (0 = ninguno, 1 = máximo).
        amount: f32,
    },
    /// Ecualizador paramétrico con varias bandas.
    Eq {
        /// Bandas activas, en orden de aplicación.
        bands: Vec<EqBand>,
    },
    /// Compresor de dinámica.
    Compressor {
        /// Umbral (dBFS) a partir del cual comprime.
        threshold_db: f32,
        /// Relación de compresión (n:1).
        ratio: f32,
        /// Tiempo de ataque (ms).
        attack_ms: f32,
        /// Tiempo de liberación (ms).
        release_ms: f32,
        /// Ganancia de maquillaje aplicada tras comprimir (dB).
        makeup_db: f32,
    },
    /// De-esser: compresión dinámica de la banda sibilante.
    DeEsser {
        /// Umbral (dBFS) de activación.
        threshold_db: f32,
        /// Frecuencia central de la banda sibilante (Hz).
        freq_hz: f32,
        /// Cantidad de reducción (0 = ninguno, 1 = máximo).
        amount: f32,
    },
    /// Saturación / armónicos suaves.
    Saturator {
        /// Cantidad de "drive" (distorsión) pre-clipping.
        drive: f32,
        /// Mezcla seco/húmedo (0 = seco, 1 = saturado).
        mix: f32,
    },
    /// Delay (eco) con feedback.
    Delay {
        /// Tiempo del eco (ms).
        time_ms: f32,
        /// Feedback (0–1): cantidad del eco que vuelve a entrar.
        feedback: f32,
        /// Mezcla seco/húmedo (0 = seco, 1 = eco completo).
        mix: f32,
    },
    /// Reverberación (Schroeder).
    Reverb {
        /// Tamaño de la sala (0–1).
        room_size: f32,
        /// Amortiguación de la cola (0–1).
        damping: f32,
        /// Mezcla seco/húmedo (0 = seco, 1 = reverb completo).
        wet: f32,
    },
    /// Limitador de seguridad con lookahead (evita clipping).
    Limiter {
        /// Umbral (dBFS) que no se supera.
        threshold_db: f32,
        /// Tiempo de lookahead (ms): añade latencia pero evita picos.
        lookahead_ms: f32,
        /// Tiempo de recuperación (ms).
        release_ms: f32,
    },
}

/// Especificación de un módulo dentro de la cadena.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DspModuleSpec {
    /// Tipo de módulo con sus parámetros.
    pub kind: DspModuleKind,
    /// `true` si el módulo está activo en la cadena.
    pub enabled: bool,
}

/// Estado de un módulo dentro de la cadena activa (para la UI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DspLinkState {
    /// Nombre corto del módulo (identificador para el bypass).
    pub name: String,
    /// `true` si el módulo está en la cadena (habilitado en el preset).
    pub enabled: bool,
    /// `true` si está en bypass (se omite en tiempo real, sin reiniciar).
    pub bypass: bool,
    /// Bandas actuales del ecualizador si este módulo es el EQ; `None` en los
    /// demás módulos. Refleja los ajustes finos aplicados con `set_eq_band`.
    pub eq_bands: Option<Vec<EqBand>>,
}

/// Estado completo de la cadena DSP activa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DspState {
    /// Preset actualmente aplicado.
    pub preset: PresetId,
    /// `true` si toda la cadena está en bypass (paso directo).
    pub global_bypass: bool,
    /// Módulos de la cadena, en orden de procesamiento.
    pub links: Vec<DspLinkState>,
}
