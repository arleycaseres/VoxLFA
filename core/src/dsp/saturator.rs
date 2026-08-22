//! Saturación de armónicos multi-modo (Tube, Tape, Tube+Tape).
//!
//! Cada modo tiene un carácter distinto:
//! - **Tube**: saturación suave con armónicos pares dominantes (calidez musical).
//! - **Tape**: saturación con compresión suave y rolloff de agudos (vintage).
//! - **TubeTape**: dos etapas en cascada para un carácter más rico.

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::dsp::SaturatorMode;

/// Función de saturación tipo tubo: soft clipping asimétrico con armónicos pares.
///
/// `drive` controla la ganancia antes de la función. Valores altos producen
/// más armónicos pares (calidez musical).
fn tube_saturate(sample: f32, drive: f32) -> f32 {
    let x = drive * sample;
    // Asimetría suave: armónicos pares dominantes.
    let neg = (-x).exp();
    let pos = x.exp();
    (pos - neg) / (pos + neg)
}

/// Función de saturación tipo cinta: compresión suave con soft knee.
///
/// La cinta tiene unAtPath diferente: compresión simétrica más suave que el
/// tubo, con un ligero rolloff de agudos (modelado con un filtro LP simple).
fn tape_saturate(sample: f32, drive: f32) -> f32 {
    let x = drive * sample;
    // Soft clipping suave con soften knee.
    x / (1.0 + x.abs().powf(1.5))
}

/// Mezcla dos funciones de saturación en cascada (tube + tape).
fn tubetape_saturate(sample: f32, drive: f32) -> f32 {
    let after_tube = tube_saturate(sample, drive);
    tape_saturate(after_tube, drive * 0.7)
}

/// Saturación multi-modo con drive, modo y mezcla seco/húmedo.
#[derive(Debug, Clone)]
pub struct Saturator {
    /// Modo de saturación.
    mode: SaturatorMode,
    /// Ganancia previa al clipping (0 = lineal, > 0 = distorsión).
    drive: f32,
    /// Mezcla seco/húmedo (0 = seco, 1 = saturado).
    mix: f32,
    /// Filtro LP para el modo Tape ( rolloff de agudos).
    tape_filter: Option<BiquadFilter>,
}

impl Saturator {
    /// Crea un saturador multi-modo.
    pub fn new(mode: SaturatorMode, drive: f32, mix: f32, sample_rate: u32) -> Self {
        let tape_filter = if matches!(mode, SaturatorMode::Tape | SaturatorMode::TubeTape) {
            Some(BiquadFilter::design(
                BiquadParams {
                    kind: BiquadKind::LowPass,
                    freq_hz: 8000.0,
                    gain_db: 0.0,
                    q: 0.707,
                },
                sample_rate,
            ))
        } else {
            None
        };

        Self {
            mode,
            drive: drive.max(0.0),
            mix: mix.clamp(0.0, 1.0),
            tape_filter,
        }
    }

    /// Crea un saturador con parámetros desde el protocolo.
    pub fn from_params(mode: SaturatorMode, drive: f32, mix: f32, sample_rate: u32) -> Self {
        Self::new(mode, drive, mix, sample_rate)
    }

    /// Crea un saturador legacy (compatibilidad con presets antiguos).
    pub fn new_legacy(drive: f32, mix: f32) -> Self {
        Self {
            mode: SaturatorMode::Tube,
            drive: drive.max(0.0),
            mix: mix.clamp(0.0, 1.0),
            tape_filter: None,
        }
    }
}

impl AudioProcessor for Saturator {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        let dry = 1.0 - self.mix;
        for i in 0..frames {
            let saturated = match self.mode {
                SaturatorMode::Tube => tube_saturate(input[i], self.drive),
                SaturatorMode::Tape => {
                    let s = tape_saturate(input[i], self.drive);
                    if let Some(ref mut filter) = self.tape_filter {
                        filter.process(s)
                    } else {
                        s
                    }
                }
                SaturatorMode::TubeTape => {
                    let s = tubetape_saturate(input[i], self.drive);
                    if let Some(ref mut filter) = self.tape_filter {
                        filter.process(s)
                    } else {
                        s
                    }
                }
            };
            output[i] = dry * input[i] + self.mix * saturated;
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "saturator"
    }

    fn reset(&mut self) {
        if let Some(ref mut filter) = self.tape_filter {
            filter.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> ProcessingInfo {
        ProcessingInfo {
            sample_rate: 48_000,
            frames: 256,
        }
    }

    #[test]
    fn zero_mix_is_dry() {
        let mut sat = Saturator::new(SaturatorMode::Tube, 4.0, 0.0, 48_000);
        let input = [0.5, -0.2, 0.0, 0.1];
        let mut out = [0.0; 4];
        sat.process(&input, &mut out, &info());
        assert_eq!(out, input);
    }

    #[test]
    fn full_mix_limits_output() {
        let mut sat = Saturator::new(SaturatorMode::Tube, 8.0, 1.0, 48_000);
        let input = [2.0, -3.0, 10.0, -10.0];
        let mut out = [0.0; 4];
        sat.process(&input, &mut out, &info());
        for v in out {
            assert!(v.abs() <= 1.0, "saturator output {v} > 1");
        }
    }

    #[test]
    fn small_signal_stays_linear() {
        let mut sat = Saturator::new(SaturatorMode::Tube, 1.0, 1.0, 48_000);
        let input = [0.01, -0.02, 0.005];
        let mut out = [0.0; 3];
        sat.process(&input, &mut out, &info());
        for (o, &i) in out.iter().zip(&input) {
            assert!((o - i).abs() < 1e-3);
        }
    }

    #[test]
    fn tape_mode_preserves_energy() {
        let mut sat = Saturator::new(SaturatorMode::Tape, 4.0, 0.5, 48_000);
        let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut out = vec![0.0; 256];
        sat.process(&input, &mut out, &info());
        let rms_in: f32 = input.iter().map(|x| x * x).sum::<f32>().sqrt() / input.len() as f32;
        let rms_out: f32 = out.iter().map(|x| x * x).sum::<f32>().sqrt() / out.len() as f32;
        assert!(
            rms_out > 0.3 * rms_in && rms_out < 2.0 * rms_in,
            "tape mode altered energy: rms_in={rms_in:.4}, rms_out={rms_out:.4}"
        );
    }

    #[test]
    fn tubetape_mode_limits_output() {
        let mut sat = Saturator::new(SaturatorMode::TubeTape, 8.0, 1.0, 48_000);
        let input = [2.0, -3.0, 10.0, -10.0];
        let mut out = [0.0; 4];
        sat.process(&input, &mut out, &info());
        for v in out {
            assert!(v.abs() <= 1.0, "tubetape output {v} > 1");
        }
    }

    #[test]
    fn legacy_mode_works() {
        let mut sat = Saturator::new_legacy(4.0, 0.5);
        let input = [0.5, -0.3, 0.1];
        let mut out = [0.0; 3];
        sat.process(&input, &mut out, &info());
        // Must produce some output (not silent).
        assert!(out[0].abs() > 0.01, "legacy saturator produced silence");
    }

    #[test]
    fn from_params_matches_new() {
        let mut sat1 = Saturator::from_params(SaturatorMode::Tube, 4.0, 0.5, 48_000);
        let mut sat2 = Saturator::new(SaturatorMode::Tube, 4.0, 0.5, 48_000);
        let input = [0.3, -0.2, 0.1, 0.0];
        let mut out1 = [0.0; 4];
        let mut out2 = [0.0; 4];
        sat1.process(&input, &mut out1, &info());
        sat2.process(&input, &mut out2, &info());
        assert_eq!(out1, out2);
    }
}
