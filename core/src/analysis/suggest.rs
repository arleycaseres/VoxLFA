//! Motor de sugerencias: traduce las métricas de voz en recomendaciones.
//!
//! Son reglas heurísticas (IA "consejera" local, sin nube): cada regla evalúa
//! una métrica contra un umbral y, si se dispara, produce una [`Suggestion`]
//! con severidad proporcional a cuánto se excede el umbral y una acción
//! confirmable (p. ej. aplicar un preset).

use crate::protocol::{PresetId, Suggestion, SuggestionAction, SuggestionKind, VoiceMetrics};

/// Motor de reglas para generar sugerencias de ajuste vocal.
#[derive(Debug, Clone, Copy)]
pub struct SuggestionEngine;

impl SuggestionEngine {
    /// Evalúa las métricas y devuelve las sugerencias activas (sin orden).
    pub fn evaluate(&self, metrics: &VoiceMetrics) -> Vec<Suggestion> {
        let mut out = Vec::new();

        // Regla 0: resonancia / boominess en la zona baja-media.
        if metrics.resonance_score > 0.45 {
            out.push(suggestion(
                0,
                SuggestionKind::Resonance,
                clamp((metrics.resonance_score - 0.45) / 0.25),
                "Se acumula energía en la zona baja-media (boominess). Aplica el preset \
                 'Voz limpia' para reducir la banda de ~300 Hz."
                    .into(),
                SuggestionAction::ApplyPreset {
                    preset: PresetId::VozLimpia,
                },
            ));
        }

        // Regla 1: timbre opaco (falta de presencia en agudos).
        if metrics.brightness < 0.28 {
            out.push(suggestion(
                1,
                SuggestionKind::Timbre,
                clamp((0.28 - metrics.brightness) / 0.12),
                "Timbre opaco: falta presencia en los agudos. Prueba el preset \
                 'Voz limpia' para realzar la claridad."
                    .into(),
                SuggestionAction::ApplyPreset {
                    preset: PresetId::VozLimpia,
                },
            ));
        }

        // Regla 2: timbre estridente (demasiada energía en agudos).
        if metrics.brightness > 0.72 {
            out.push(suggestion(
                2,
                SuggestionKind::Timbre,
                clamp((metrics.brightness - 0.72) / 0.15),
                "Timbre brillante/estridente. Suaviza los agudos con el preset \
                 'Warm' para un tono más cálido."
                    .into(),
                SuggestionAction::ApplyPreset {
                    preset: PresetId::Warm,
                },
            ));
        }

        // Regla 3: dinámica muy plana (sobrecomprimida).
        if metrics.dynamic_range_db > 0.0 && metrics.dynamic_range_db < 6.0 {
            out.push(suggestion(
                3,
                SuggestionKind::Dynamics,
                clamp((6.0 - metrics.dynamic_range_db) / 4.0),
                "La dinámica está muy comprimida. 'Warm' usa una compresión más \
                 ligera y deja respirar la voz."
                    .into(),
                SuggestionAction::ApplyPreset {
                    preset: PresetId::Warm,
                },
            ));
        }

        // Regla 4: dinámica demasiado amplia (picos difíciles de controlar).
        if metrics.dynamic_range_db > 18.0 {
            out.push(suggestion(
                4,
                SuggestionKind::Dynamics,
                clamp((metrics.dynamic_range_db - 18.0) / 6.0),
                "Hay mucha variación de volumen. El preset 'Voz limpia' ayuda a \
                 controlar los picos sin sonar procesado."
                    .into(),
                SuggestionAction::ApplyPreset {
                    preset: PresetId::VozLimpia,
                },
            ));
        }

        // Regla 5: fatiga vocal (nivel alto sostenido). Informativa.
        if metrics.fatigue_score > 0.55 {
            out.push(suggestion(
                5,
                SuggestionKind::Fatigue,
                clamp((metrics.fatigue_score - 0.55) / 0.3),
                "Nivel alto sostenido: la voz muestra signos de fatiga. Considera \
                 pausas o reducir la ganancia de entrada."
                    .into(),
                SuggestionAction::None,
            ));
        }

        out
    }
}

/// Construye una sugerencia con sus campos.
fn suggestion(
    id: u8,
    kind: SuggestionKind,
    severity: f32,
    message: String,
    action: SuggestionAction,
) -> Suggestion {
    Suggestion {
        id,
        kind,
        severity: severity.clamp(0.0, 1.0),
        message,
        action,
    }
}

/// Acota un valor a 0–1.
fn clamp(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics() -> VoiceMetrics {
        VoiceMetrics {
            rms_db: -24.0,
            peak_db: -10.0,
            dynamic_range_db: 12.0,
            crest_db: 14.0,
            brightness: 0.5,
            resonance_score: 0.2,
            fatigue_score: 0.2,
            window_ms: 2000,
        }
    }

    #[test]
    fn healthy_voice_produces_no_suggestions() {
        let out = SuggestionEngine.evaluate(&metrics());
        assert!(out.is_empty());
    }

    #[test]
    fn boomy_voice_suggests_voz_limpia() {
        let mut m = metrics();
        m.resonance_score = 0.7;
        let out = SuggestionEngine.evaluate(&m);
        assert!(out.iter().any(|s| s.kind == SuggestionKind::Resonance));
        let resonance = out.iter().find(|s| s.id == 0).unwrap();
        assert_eq!(
            resonance.action,
            SuggestionAction::ApplyPreset {
                preset: PresetId::VozLimpia
            }
        );
    }

    #[test]
    fn dull_timbre_suggests_presence() {
        let mut m = metrics();
        m.brightness = 0.1;
        let out = SuggestionEngine.evaluate(&m);
        assert!(out
            .iter()
            .any(|s| s.id == 1 && s.kind == SuggestionKind::Timbre));
    }

    #[test]
    fn fatigue_is_informational_only() {
        let mut m = metrics();
        m.fatigue_score = 0.9;
        let out = SuggestionEngine.evaluate(&m);
        let fatigue = out.iter().find(|s| s.id == 5).unwrap();
        assert_eq!(fatigue.action, SuggestionAction::None);
    }
}
