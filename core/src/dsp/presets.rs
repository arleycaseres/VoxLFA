//! Presets de la cabina: especificaciones de cadena DSP por preset.
//!
//! La especificación es declarativa (protocolo) y se convierte en la cadena
//! real mediante [`crate::dsp::chain::ChainProcessor`].

use crate::protocol::{
    DelayMode, DspModuleKind, DspModuleSpec, EqBand, EqBandKind, NoiseGateParams, PresetId,
    PresetInfo, ReverbMode, SaturatorMode,
};

/// Fábrica de presets: devuelve la especificación de cadena de cada uno.
pub struct PresetFactory;

impl PresetFactory {
    /// Metadatos de todos los presets disponibles (para la UI).
    pub fn all() -> Vec<PresetInfo> {
        vec![
            PresetInfo {
                id: PresetId::Dry,
                name: "Sin procesar".into(),
                description: "Paso directo de la señal, sin efectos.".into(),
            },
            PresetInfo {
                id: PresetId::VozLimpia,
                name: "Voz limpia".into(),
                description: "EQ suave y compresión transparente para canto.".into(),
            },
            PresetInfo {
                id: PresetId::Radio,
                name: "Radio".into(),
                description: "Carácter telefónico (banda estrecha + saturación).".into(),
            },
            PresetInfo {
                id: PresetId::Warm,
                name: "Warm".into(),
                description: "Bajos suaves y presencia vocal cálida.".into(),
            },
        ]
    }

    /// Especificación de la cadena para un preset (orden de procesamiento).
    pub fn specs(preset: PresetId) -> Vec<DspModuleSpec> {
        match preset {
            PresetId::Dry => vec![],
            PresetId::VozLimpia => voce_limpia(),
            PresetId::Radio => radio(),
            PresetId::Warm => warm(),
        }
    }

    /// Bandas por defecto del ecualizador de un preset (vacío si no tiene EQ).
    ///
    /// Se usa para restablecer el ajuste fino al aplicar un preset y para
    /// guardar los perfiles por dispositivo.
    pub fn eq_bands(preset: PresetId) -> Vec<EqBand> {
        Self::specs(preset)
            .into_iter()
            .find_map(|spec| match spec.kind {
                DspModuleKind::Eq { bands } => Some(bands),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Parámetros por defecto de la puerta de ruido de un preset, o `None` si
    /// no tiene puerta de ruido.
    ///
    /// Se usan para restablecer el ajuste en vivo al aplicar un preset y para
    /// guardar los perfiles por dispositivo.
    pub fn gate_params(preset: PresetId) -> Option<NoiseGateParams> {
        Self::specs(preset)
            .into_iter()
            .find_map(|spec| match spec.kind {
                DspModuleKind::NoiseGate {
                    threshold_db,
                    attack_ms,
                    release_ms,
                    hold_ms,
                    range_db,
                } => Some(NoiseGateParams {
                    threshold_db,
                    attack_ms,
                    release_ms,
                    hold_ms,
                    range_db,
                }),
                _ => None,
            })
    }
}

/// Voz limpia: pasa-altos, denoise, feedback, puerta de ruido, antifeedback
/// (boominess), EQ suave, de-esser, compresor transparente, slapback sutil,
/// plate reverb y limiter.
fn voce_limpia() -> Vec<DspModuleSpec> {
    vec![
        module(DspModuleKind::HighPass { cutoff_hz: 80.0 }),
        module(DspModuleKind::Denoise { mix: 1.0 }),
        module(DspModuleKind::FeedbackSuppressor {
            threshold_db: -30.0,
            q: 10.0,
        }),
        module(DspModuleKind::NoiseGate {
            threshold_db: -50.0,
            attack_ms: 2.0,
            release_ms: 100.0,
            hold_ms: 120.0,
            range_db: 40.0,
        }),
        module(DspModuleKind::BoomSuppressor {
            threshold_db: -30.0,
            freq_hz: 250.0,
            amount: 0.5,
        }),
        module(DspModuleKind::Eq {
            bands: vec![
                band(EqBandKind::LowShelf, 200.0, -2.0, 0.8),
                band(EqBandKind::Peaking, 3000.0, 2.0, 1.5),
                band(EqBandKind::HighShelf, 8000.0, 1.5, 0.8),
            ],
        }),
        module(DspModuleKind::DeEsser {
            threshold_db: -32.0,
            freq_hz: 6500.0,
            amount: 0.5,
        }),
        module(DspModuleKind::Compressor {
            threshold_db: -24.0,
            ratio: 3.0,
            attack_ms: 5.0,
            release_ms: 80.0,
            makeup_db: 3.0,
        }),
        module(DspModuleKind::Delay {
            mode: DelayMode::Slapback,
            time_ms: 65.0,
            feedback: 0.0,
            mix: 0.08,
            pre_delay_ms: 0.0,
            low_cut_hz: 200.0,
            high_cut_hz: 6000.0,
            tempo_bpm: 120.0,
            sync_enabled: false,
            duck_amount: 0.3,
        }),
        module(DspModuleKind::Reverb {
            mode: ReverbMode::Plate,
            room_size: 0.3,
            damping: 0.3,
            wet: 0.08,
            pre_delay_ms: 15.0,
            high_cut_hz: 7000.0,
            low_cut_hz: 200.0,
        }),
        module(DspModuleKind::Limiter {
            threshold_db: -1.0,
            lookahead_ms: 3.0,
            release_ms: 100.0,
        }),
    ]
}

/// Radio: banda estrecha (pasa-altos + shelf de agudos), denoise, feedback,
/// puerta de ruido, notch antifeedback, saturación, tape delay, reverb room
/// y comp.
fn radio() -> Vec<DspModuleSpec> {
    vec![
        module(DspModuleKind::HighPass { cutoff_hz: 250.0 }),
        module(DspModuleKind::Denoise { mix: 1.0 }),
        module(DspModuleKind::FeedbackSuppressor {
            threshold_db: -30.0,
            q: 10.0,
        }),
        module(DspModuleKind::NoiseGate {
            threshold_db: -45.0,
            attack_ms: 1.0,
            release_ms: 80.0,
            hold_ms: 120.0,
            range_db: 45.0,
        }),
        module(DspModuleKind::Notch {
            freq_hz: 1000.0,
            q: 8.0,
        }),
        module(DspModuleKind::Eq {
            bands: vec![
                band(EqBandKind::Peaking, 1000.0, 6.0, 1.2),
                band(EqBandKind::HighShelf, 3500.0, -18.0, 0.8),
            ],
        }),
        module(DspModuleKind::Saturator {
            mode: SaturatorMode::Tube,
            drive: 3.0,
            mix: 0.4,
        }),
        module(DspModuleKind::Compressor {
            threshold_db: -30.0,
            ratio: 4.0,
            attack_ms: 3.0,
            release_ms: 120.0,
            makeup_db: 6.0,
        }),
        module(DspModuleKind::Delay {
            mode: DelayMode::Tape,
            time_ms: 120.0,
            feedback: 0.3,
            mix: 0.15,
            pre_delay_ms: 0.0,
            low_cut_hz: 300.0,
            high_cut_hz: 4000.0,
            tempo_bpm: 120.0,
            sync_enabled: false,
            duck_amount: 0.2,
        }),
        module(DspModuleKind::Reverb {
            mode: ReverbMode::Room,
            room_size: 0.25,
            damping: 0.4,
            wet: 0.1,
            pre_delay_ms: 10.0,
            high_cut_hz: 5000.0,
            low_cut_hz: 300.0,
        }),
        module(DspModuleKind::Limiter {
            threshold_db: -1.0,
            lookahead_ms: 3.0,
            release_ms: 100.0,
        }),
    ]
}

/// Warm: bajos suaves con denoise, feedback, puerta de ruido y antifeedback
/// (boominess), presencia vocal, compresión ligera, digital delay y plate reverb.
fn warm() -> Vec<DspModuleSpec> {
    vec![
        module(DspModuleKind::HighPass { cutoff_hz: 70.0 }),
        module(DspModuleKind::Denoise { mix: 1.0 }),
        module(DspModuleKind::FeedbackSuppressor {
            threshold_db: -30.0,
            q: 10.0,
        }),
        module(DspModuleKind::NoiseGate {
            threshold_db: -48.0,
            attack_ms: 3.0,
            release_ms: 120.0,
            hold_ms: 120.0,
            range_db: 40.0,
        }),
        module(DspModuleKind::BoomSuppressor {
            threshold_db: -26.0,
            freq_hz: 200.0,
            amount: 0.5,
        }),
        module(DspModuleKind::Eq {
            bands: vec![
                band(EqBandKind::LowShelf, 120.0, 3.0, 0.8),
                band(EqBandKind::Peaking, 2500.0, 1.5, 1.5),
                band(EqBandKind::HighShelf, 7000.0, -2.0, 0.8),
            ],
        }),
        module(DspModuleKind::Compressor {
            threshold_db: -28.0,
            ratio: 2.5,
            attack_ms: 10.0,
            release_ms: 150.0,
            makeup_db: 4.0,
        }),
        module(DspModuleKind::Delay {
            mode: DelayMode::Digital,
            time_ms: 80.0,
            feedback: 0.25,
            mix: 0.1,
            pre_delay_ms: 0.0,
            low_cut_hz: 150.0,
            high_cut_hz: 8000.0,
            tempo_bpm: 120.0,
            sync_enabled: false,
            duck_amount: 0.4,
        }),
        module(DspModuleKind::Reverb {
            mode: ReverbMode::Plate,
            room_size: 0.4,
            damping: 0.3,
            wet: 0.12,
            pre_delay_ms: 20.0,
            high_cut_hz: 7000.0,
            low_cut_hz: 200.0,
        }),
        module(DspModuleKind::Limiter {
            threshold_db: -1.0,
            lookahead_ms: 3.0,
            release_ms: 100.0,
        }),
    ]
}

/// Envuelve un tipo de módulo en una especificación habilitada.
fn module(kind: DspModuleKind) -> DspModuleSpec {
    DspModuleSpec {
        kind,
        enabled: true,
    }
}

/// Crea una banda de EQ.
fn band(kind: EqBandKind, freq_hz: f32, gain_db: f32, q: f32) -> EqBand {
    EqBand {
        kind,
        freq_hz,
        gain_db,
        q,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_have_metadata() {
        let ids: Vec<PresetId> = PresetFactory::all().into_iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec![
                PresetId::Dry,
                PresetId::VozLimpia,
                PresetId::Radio,
                PresetId::Warm
            ]
        );
    }

    #[test]
    fn dry_has_no_links_and_others_have_some() {
        assert!(PresetFactory::specs(PresetId::Dry).is_empty());
        for preset in [PresetId::VozLimpia, PresetId::Radio, PresetId::Warm] {
            assert!(!PresetFactory::specs(preset).is_empty());
        }
    }

    #[test]
    fn all_specs_are_enabled_and_terminate_in_limiter() {
        for preset in [PresetId::VozLimpia, PresetId::Radio, PresetId::Warm] {
            let specs = PresetFactory::specs(preset);
            assert!(specs.iter().all(|s| s.enabled));
            let last = specs.last().expect("preset no vacío");
            assert!(
                matches!(last.kind, DspModuleKind::Limiter { .. }),
                "el último módulo de {preset:?} debería ser el limiter"
            );
        }
    }

    #[test]
    fn non_dry_presets_include_antifeedback() {
        for preset in [PresetId::VozLimpia, PresetId::Radio, PresetId::Warm] {
            let specs = PresetFactory::specs(preset);
            let has_antifeedback = specs.iter().any(|s| {
                matches!(
                    s.kind,
                    DspModuleKind::HighPass { .. }
                        | DspModuleKind::Notch { .. }
                        | DspModuleKind::BoomSuppressor { .. }
                )
            });
            assert!(
                has_antifeedback,
                "el preset {preset:?} no tiene antifeedback"
            );
        }
    }

    #[test]
    fn non_dry_presets_include_a_noise_gate() {
        for preset in [PresetId::VozLimpia, PresetId::Radio, PresetId::Warm] {
            let specs = PresetFactory::specs(preset);
            let has_gate = specs
                .iter()
                .any(|s| matches!(s.kind, DspModuleKind::NoiseGate { .. }));
            assert!(has_gate, "el preset {preset:?} no tiene puerta de ruido");
            assert!(
                PresetFactory::gate_params(preset).is_some(),
                "el preset {preset:?} no expone gate_params"
            );
        }
        assert_eq!(PresetFactory::gate_params(PresetId::Dry), None);
    }
}
