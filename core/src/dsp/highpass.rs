//! Filtro pasa-altos para la cadena vocal (recorta subgraves / boominess).

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Filtro pasa-altos con pendiente de 12 dB/oct (biquad).
#[derive(Debug, Clone)]
pub struct HighPass {
    filter: BiquadFilter,
    cutoff_hz: f32,
}

impl HighPass {
    /// Crea un pasa-altos con la frecuencia de corte indicada.
    pub fn new(cutoff_hz: f32, sample_rate: u32) -> Self {
        let filter = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::HighPass,
                freq_hz: cutoff_hz,
                gain_db: 0.0,
                q: 0.707,
            },
            sample_rate,
        );
        Self { filter, cutoff_hz }
    }

    /// Frecuencia de corte (Hz).
    pub fn cutoff_hz(&self) -> f32 {
        self.cutoff_hz
    }
}

impl AudioProcessor for HighPass {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        for (dst, &src) in output[..frames].iter_mut().zip(&input[..frames]) {
            *dst = self.filter.process(src);
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "highpass"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_dc_offset() {
        let mut hp = HighPass::new(80.0, 48_000);
        let mut out = [0.0; 1024];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 1024,
        };
        hp.process(&[1.0; 1024], &mut out, &info);
        assert!(out[out.len() - 1].abs() < 1e-2);
    }
}
