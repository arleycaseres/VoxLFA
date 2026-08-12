//! Saturación de armónicos suaves (drive + mezcla seco/húmedo).

use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Saturación tipo `tanh` con ganancia de entrada y mezcla.
#[derive(Debug, Clone)]
pub struct Saturator {
    drive: f32,
    mix: f32,
}

impl Saturator {
    /// Crea un saturador.
    ///
    /// - `drive`: ganancia previa al clipping (0 = lineal, > 0 = distorsión).
    /// - `mix`: proporción de señal saturada (0 = seco, 1 = saturado).
    pub fn new(drive: f32, mix: f32) -> Self {
        Self {
            drive: drive.max(0.0),
            mix: mix.clamp(0.0, 1.0),
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
            let saturated = (self.drive * input[i]).tanh();
            output[i] = dry * input[i] + self.mix * saturated;
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "saturator"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_mix_is_dry() {
        let mut sat = Saturator::new(4.0, 0.0);
        let input = [0.5, -0.2, 0.0, 0.1];
        let mut out = [0.0; 4];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4,
        };
        sat.process(&input, &mut out, &info);
        assert_eq!(out, input);
    }

    #[test]
    fn full_mix_limits_output_to_plus_minus_one() {
        let mut sat = Saturator::new(8.0, 1.0);
        let input = [2.0, -3.0, 10.0, -10.0];
        let mut out = [0.0; 4];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4,
        };
        sat.process(&input, &mut out, &info);
        for v in out {
            assert!(v.abs() <= 1.0, "saturator output {v} > 1");
        }
    }

    #[test]
    fn small_signal_stays_linear() {
        let mut sat = Saturator::new(1.0, 1.0);
        let input = [0.01, -0.02, 0.005];
        let mut out = [0.0; 3];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 3,
        };
        sat.process(&input, &mut out, &info);
        // tanh(x) ≈ x para x pequeño.
        for (o, &i) in out.iter().zip(&input) {
            assert!((o - i).abs() < 1e-4);
        }
    }
}
