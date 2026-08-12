//! Filtro muesca (notch) para antifeedback.
//!
//! Elimina una frecuencia concreta (típicamente la resonancia que realimenta
//! micrófono↔altavoz) sin tocar el resto de la señal. Es estático: los
//! coeficientes se calculan en la construcción y en tiempo real solo se aplica
//! la ecuación en diferencias del biquad.

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Filtro muesca con frecuencia central y factor de calidad configurables.
#[derive(Debug, Clone)]
pub struct Notch {
    /// Filtro biquad tipo notch.
    filter: BiquadFilter,
}

impl Notch {
    /// Crea una muesca en `freq_hz` con el ancho indicado por `q`.
    pub fn new(freq_hz: f32, q: f32, sample_rate: u32) -> Self {
        let filter = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::Notch,
                freq_hz,
                gain_db: 0.0,
                q: q.max(0.5),
            },
            sample_rate,
        );
        Self { filter }
    }
}

impl AudioProcessor for Notch {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        for i in 0..frames {
            output[i] = self.filter.process(input[i]);
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "notch"
    }

    fn reset(&mut self) {
        self.filter.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notch_attenuates_target_frequency() {
        let sr = 48_000;
        let mut notch = Notch::new(1000.0, 30.0, sr);
        let n = 8192;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin() * 0.2)
            .collect();
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        notch.process(&input, &mut out, &info);
        let rms_in = rms(&input[n / 2..]);
        let rms_out = rms(&out[n / 2..]);
        assert!(rms_out < rms_in * 0.05, "notch no atenúa 1 kHz");
    }

    #[test]
    fn notch_leaves_distant_frequency_mostly_intact() {
        let sr = 48_000;
        let mut notch = Notch::new(1000.0, 30.0, sr);
        let n = 8192;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 200.0 * i as f32 / sr as f32).sin() * 0.2)
            .collect();
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        notch.process(&input, &mut out, &info);
        let rms_in = rms(&input[n / 2..]);
        let rms_out = rms(&out[n / 2..]);
        assert!(rms_out > rms_in * 0.95, "notch altera frecuencias lejanas");
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }
}
