//! Ganancia lineal simple (dB).

use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Aplica una ganancia constante a la señal.
#[derive(Debug, Clone)]
pub struct Gain {
    /// Ganancia en dB (positivo = amplificar, negativo = atenuar).
    gain_db: f32,
    /// Ganancia lineal precalculada (0 dB = 1.0).
    linear: f32,
}

impl Gain {
    /// Crea una etapa de ganancia con el valor dado en dB.
    pub fn new(gain_db: f32) -> Self {
        Self {
            gain_db,
            linear: 10f32.powf(gain_db / 20.0),
        }
    }

    /// Ganancia actual en dB.
    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }
}

impl Default for Gain {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl AudioProcessor for Gain {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        for (dst, &src) in output[..frames].iter_mut().zip(&input[..frames]) {
            *dst = src * self.linear;
        }
        ProcessResult { latency_ms: 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_gain_passes_through() {
        let mut gain = Gain::new(0.0);
        let mut out = [0.0; 4];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4,
        };
        gain.process(&[0.1, -0.2, 0.3, 0.0], &mut out, &info);
        assert_eq!(out, [0.1, -0.2, 0.3, 0.0]);
    }

    #[test]
    fn six_db_doubles_amplitude() {
        let mut gain = Gain::new(6.0);
        let mut out = [0.0; 2];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 2,
        };
        gain.process(&[0.5, 0.25], &mut out, &info);
        // 6 dB → ×10^(6/20) ≈ 1.9952.
        assert!((out[0] - 0.5 * 1.9952).abs() < 1e-3);
        assert!((out[1] - 0.25 * 1.9952).abs() < 1e-3);
    }
}
