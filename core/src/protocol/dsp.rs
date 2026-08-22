//! Contrato DSP del protocolo: presets, módulos de la cadena y estado.
//!
//! Estos tipos son la **configuración** que la UI envía (preset aplicado,
//! bypass) y el **estado** que recibe (cadena activa con sus módulos). Son
//! puramente declarativos: la implementación del procesamiento vive en
//! `crate::dsp`. `dsp` depende de este módulo (config → procesamiento).

use serde::{Deserialize, Serialize};

/// Identificador de un preset de la cabina.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PresetId {
    /// Sin procesamiento: paso directo de la señal.
    #[default]
    Dry,
    /// Voz limpia: EQ suave y compresión transparente.
    VozLimpia,
    /// Radio: carácter telefónico (banda estrecha + saturación).
    Radio,
    /// Warm: bajos suaves y presencia vocal.
    Warm,
}

impl std::fmt::Display for PresetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PresetId::Dry => write!(f, "dry"),
            PresetId::VozLimpia => write!(f, "vozLimpia"),
            PresetId::Radio => write!(f, "radio"),
            PresetId::Warm => write!(f, "warm"),
        }
    }
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

/// Parámetros de la puerta de ruido (espejo de `dsp::gate::NoiseGate`).
///
/// Se transportan en `DspModuleKind::NoiseGate` (configuración del preset) y en
/// `DspLinkState::gate_params` (estado actual, tras los ajustes en vivo).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseGateParams {
    /// Umbral (dBFS) a partir del cual se abre la puerta.
    pub threshold_db: f32,
    /// Tiempo de ataque (ms): qué rápido se abre.
    pub attack_ms: f32,
    /// Tiempo de liberación (ms): qué rápido se cierra.
    pub release_ms: f32,
    /// Tiempo que permanece abierta tras caer bajo el umbral (ms).
    pub hold_ms: f32,
    /// Atenuación máxima aplicada al cerrar (dB).
    pub range_db: f32,
}

/// Parámetros de supresión de ruido (espejo de `DspModuleKind::Denoise`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DenoiseParams {
    /// Mezcla seco/húmedo (0 = sin denoise, 1 = denoise completo).
    pub mix: f32,
}

/// Parámetros de supresión de feedback adaptativa (espejo de
/// `DspModuleKind::FeedbackSuppressor`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackSuppressorParams {
    /// Umbral de detección en dBFS.
    pub threshold_db: f32,
    /// Factor de calidad de los filtros notch (mayor = más estrecho).
    pub q: f32,
}

/// Modo de saturación (tipo de distorsión armónica).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SaturatorMode {
    /// Tubo: saturación suave, armónicos pares dominantes (calidez musical).
    Tube,
    /// Cinta: saturación con compresión suave y rolloff de agudos (vintage).
    Tape,
    /// Tubo + Cinta: dos etapas en cascada para un carácter más rico.
    TubeTape,
}

/// Parámetros de saturación (espejo de `DspModuleKind::Saturator`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaturatorParams {
    /// Modo de saturación.
    pub mode: SaturatorMode,
    /// Ganancia previa al clipping (0 = lineal, > 0 = distorsión).
    pub drive: f32,
    /// Mezcla seco/húmedo (0 = seco, 1 = saturado).
    pub mix: f32,
}

/// Modo del delay (tipo de carácter del eco).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DelayMode {
    /// Eco digital limpio (sin degradación).
    Digital,
    /// Eco analógico con degradación tonal (filtro LP en feedback).
    Analog,
    /// Eco de cinta con wow & flutter (modulación LFO en delay).
    Tape,
    /// Slapback: eco corto (60-120 ms) sin feedback, una sola repetición.
    Slapback,
}

/// Modo del reverb (tipo de algoritmo de reverberación).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReverbMode {
    /// Reverb de placa metálica (estándar para vocales en vivo).
    Plate,
    /// Reverb de sala de concierto grande.
    Hall,
    /// Reverb de sala pequeña-mediana (presencia natural).
    Room,
}

/// Parámetros de delay (espejo de `dsp::delay::Delay`).
///
/// Se transportan en `DspModuleKind::Delay` y en `DspLinkState::delay_params`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelayParams {
    /// Modo del delay.
    pub mode: DelayMode,
    /// Tiempo del eco en ms.
    pub time_ms: f32,
    /// Feedback (0–0.95): cantidad del eco que vuelve a entrar.
    pub feedback: f32,
    /// Mezcla seco/húmedo (0 = seco, 1 = eco completo).
    pub mix: f32,
    /// Pre-delay en ms (separación del eco respecto a la señal seca).
    pub pre_delay_ms: f32,
    /// Corte de graves del eco en Hz (50–500).
    pub low_cut_hz: f32,
    /// Corte de agudos del eco en Hz (1000–20000).
    pub high_cut_hz: f32,
    /// Tempo BPM para sincronizar el delay (solo relevante si `sync_enabled`).
    pub tempo_bpm: f32,
    /// `true` para sincronizar el tiempo del delay al tempo.
    pub sync_enabled: bool,
    /// Cantidad de ducking (0–1): atenuación del delay cuando la voz habla.
    pub duck_amount: f32,
}

/// Parámetros de reverb (espejo de `dsp::reverb::Reverb`).
///
/// Se transportan en `DspModuleKind::Reverb` y en `DspLinkState::reverb_params`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverbParams {
    /// Modo del reverb.
    pub mode: ReverbMode,
    /// Tamaño de la sala (0–1).
    pub room_size: f32,
    /// Amortiguación de agudos (0–1).
    pub damping: f32,
    /// Mezcla seco/húmedo (0 = seco, 1 = 100% wet).
    pub wet: f32,
    /// Pre-delay en ms (separa la señal seca de la reverberada).
    pub pre_delay_ms: f32,
    /// Corte de agudos de la cola del reverb en Hz (500–20000).
    pub high_cut_hz: f32,
    /// Corte de graves del return del reverb en Hz (50–500).
    /// Clave para evitar "mud" en mezcla en vivo.
    pub low_cut_hz: f32,
}

/// Nota musical raíz para la escala de corrección de tono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MusicalNote {
    /// Do (C).
    C,
    /// Do sostenido (C#).
    Cs,
    /// Re (D).
    D,
    /// Re sostenido (D#).
    Ds,
    /// Mi (E).
    E,
    /// Fa (F).
    F,
    /// Fa sostenido (F#).
    Fs,
    /// Sol (G).
    G,
    /// Sol sostenido (G#).
    Gs,
    /// La (A).
    A,
    /// La sostenido (A#).
    As,
    /// Si (B).
    B,
}

impl MusicalNote {
    /// Semitonos desde C (0–11).
    pub fn semitones(self) -> u8 {
        match self {
            Self::C => 0,
            Self::Cs => 1,
            Self::D => 2,
            Self::Ds => 3,
            Self::E => 4,
            Self::F => 5,
            Self::Fs => 6,
            Self::G => 7,
            Self::Gs => 8,
            Self::A => 9,
            Self::As => 10,
            Self::B => 11,
        }
    }
}

/// Escala musical para la corrección de tono.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MusicalScale {
    /// Cromática: corrige a la nota más cercana (cualquier semitono).
    Chromatic,
    /// Mayor: I–II–III–IV–V–VI–VII.
    Major,
    /// Menor natural: I–II–III–IV–V–VI–VII.
    MinorNatural,
    /// Menor armónica: I–II–III–IV–V–VI–VII↑.
    MinorHarmonic,
    /// Pentatónica mayor: I–II–III–V–VI.
    PentatonicMajor,
    /// Pentatónica menor: I–III–IV–V–VII.
    PentatonicMinor,
    /// Blues: I–III–IV–V–VII.
    Blues,
}

/// Parámetros de corrección de tono (espejo de `DspModuleKind::PitchCorrection`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PitchCorrectionParams {
    /// Escala musical objetivo.
    pub scale: MusicalScale,
    /// Nota raíz de la escala.
    pub root: MusicalNote,
    /// Intensidad de la corrección (0 = desactivada, 1 = corrección completa).
    pub strength: f32,
    /// Mezcla seco/húmedo (0 = seco, 1 = señal corregida completa).
    pub mix: f32,
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
    /// Puerta de ruido: atenúa la señal bajo el umbral.
    NoiseGate {
        /// Umbral (dBFS) a partir del cual se abre la puerta.
        threshold_db: f32,
        /// Tiempo de ataque (ms).
        attack_ms: f32,
        /// Tiempo de liberación (ms).
        release_ms: f32,
        /// Tiempo que permanece abierta tras caer bajo el umbral (ms).
        hold_ms: f32,
        /// Atenuación máxima aplicada al cerrar (dB).
        range_db: f32,
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
        /// Modo de saturación.
        mode: SaturatorMode,
        /// Cantidad de "drive" (distorsión) pre-clipping.
        drive: f32,
        /// Mezcla seco/húmedo (0 = seco, 1 = saturado).
        mix: f32,
    },
    /// Delay (eco) con feedback y múltiples modos.
    Delay {
        /// Modo del delay.
        mode: DelayMode,
        /// Tiempo del eco (ms).
        time_ms: f32,
        /// Feedback (0–0.95): cantidad del eco que vuelve a entrar.
        feedback: f32,
        /// Mezcla seco/húmedo (0 = seco, 1 = eco completo).
        mix: f32,
        /// Pre-delay en ms (separación del eco respecto a la señal seca).
        pre_delay_ms: f32,
        /// Corte de graves del eco en Hz.
        low_cut_hz: f32,
        /// Corte de agudos del eco en Hz.
        high_cut_hz: f32,
        /// Tempo BPM para sincronizar el delay.
        tempo_bpm: f32,
        /// `true` para sincronizar el tiempo del delay al tempo.
        sync_enabled: bool,
        /// Cantidad de ducking (0–1): atenuación del delay cuando la voz habla.
        duck_amount: f32,
    },
    /// Reverberación con múltiples algoritmos.
    Reverb {
        /// Modo del reverb.
        mode: ReverbMode,
        /// Tamaño de la sala (0–1).
        room_size: f32,
        /// Amortiguación de agudos (0–1).
        damping: f32,
        /// Mezcla seco/húmedo (0 = seco, 1 = reverb completo).
        wet: f32,
        /// Pre-delay en ms (separa la señal seca de la reverberada).
        pre_delay_ms: f32,
        /// Corte de agudos de la cola del reverb en Hz.
        high_cut_hz: f32,
        /// Corte de graves del return del reverb en Hz.
        low_cut_hz: f32,
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
    /// Supresión de ruido (RNNoise / ONNX).
    Denoise {
        /// Mezcla seco/húmedo (0 = sin denoise, 1 = denoise completo).
        mix: f32,
    },
    /// Supresión de feedback adaptativa (FFT + notch adaptativo).
    FeedbackSuppressor {
        /// Umbral de detección (dBFS): picos por encima se consideran feedback.
        threshold_db: f32,
        /// Factor de calidad de los filtros notch (mayor = más estrecho).
        q: f32,
    },
    /// Corrección de tono en tiempo real (Auto-Tune / pitch correction).
    PitchCorrection {
        /// Escala musical objetivo.
        scale: MusicalScale,
        /// Nota raíz de la escala.
        root: MusicalNote,
        /// Intensidad de la corrección (0 = desactivada, 1 = corrección completa).
        strength: f32,
        /// Mezcla seco/húmedo (0 = seco, 1 = señal corregida completa).
        mix: f32,
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
    /// Parámetros actuales de la puerta de ruido si este módulo es el gate;
    /// `None` en los demás. Refleja los ajustes en vivo con `set_noise_gate`.
    pub gate_params: Option<NoiseGateParams>,
    /// Parámetros actuales de denoise si este módulo es denoise; `None` en los
    /// demás. Refleja los ajustes en vivo con `set_denoise`.
    pub denoise_params: Option<DenoiseParams>,
    /// Parámetros actuales de feedback suppressor si este módulo es feedback;
    /// `None` en los demás. Refleja los ajustes en vivo con `set_feedback`.
    pub feedback_params: Option<FeedbackSuppressorParams>,
    /// Parámetros actuales de corrección de tono si este módulo es pitch
    /// correction; `None` en los demás. Refleja los ajustes en vivo con
    /// `set_pitch_correction`.
    pub pitch_correction_params: Option<PitchCorrectionParams>,
    /// Parámetros actuales de delay si este módulo es delay; `None` en los
    /// demás. Refleja los ajustes en vivo con `set_delay`.
    pub delay_params: Option<DelayParams>,
    /// Parámetros actuales de reverb si este módulo es reverb; `None` en los
    /// demás. Refleja los ajustes en vivo con `set_reverb`.
    pub reverb_params: Option<ReverbParams>,
    /// Parámetros actuales de saturación si este módulo es saturator; `None` en
    /// los demás. Refleja los ajustes en vivo con `set_saturator`.
    pub saturator_params: Option<SaturatorParams>,
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
