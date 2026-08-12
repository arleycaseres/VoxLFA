//! Presets de la cabina: especificaciones de cadena DSP por preset.
//!
//! La especificación es declarativa (protocolo) y se convierte en la cadena
//! real mediante [`crate::dsp::chain::ChainProcessor`].

use crate::protocol::{DspModuleKind, DspModuleSpec, EqBand, EqBandKind, PresetId, PresetInfo};

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
}

/// Voz limpia: pasa-altos, EQ suave, de-esser, compresor transparente, limiter.
fn voce_limpia() -> Vec<DspModuleSpec> {
    vec![
        module(DspModuleKind::HighPass { cutoff_hz: 80.0 }),
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
        module(DspModuleKind::Limiter {
            threshold_db: -1.0,
            lookahead_ms: 3.0,
            release_ms: 100.0,
        }),
    ]
}

/// Radio: banda estrecha (pasa-altos + shelf de agudos), saturación y comp.
fn radio() -> Vec<DspModuleSpec> {
    vec![
        module(DspModuleKind::HighPass { cutoff_hz: 250.0 }),
        module(DspModuleKind::Eq {
            bands: vec![
                band(EqBandKind::Peaking, 1000.0, 6.0, 1.2),
                band(EqBandKind::HighShelf, 3500.0, -18.0, 0.8),
            ],
        }),
        module(DspModuleKind::Saturator {
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
        module(DspModuleKind::Limiter {
            threshold_db: -1.0,
            lookahead_ms: 3.0,
            release_ms: 100.0,
        }),
    ]
}

/// Warm: bajos suaves, presencia vocal, compresión ligera y toque de reverb.
fn warm() -> Vec<DspModuleSpec> {
    vec![
        module(DspModuleKind::HighPass { cutoff_hz: 70.0 }),
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
        module(DspModuleKind::Reverb {
            room_size: 0.35,
            damping: 0.3,
            wet: 0.12,
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
}
