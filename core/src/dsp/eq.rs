//! Ecualizador paramétrico: varias bandas (pico y shelving) en serie.

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::{EqBand, EqBandKind};

/// Ecualizador con `N` bandas biquad en serie.
///
/// Cada banda se diseña al crear el EQ (no en el hilo de audio). Los buffers
/// de trabajo se preasignan en la construcción para no asignar memoria en el
/// callback de audio.
#[derive(Debug, Clone)]
pub struct ParametricEq {
    /// Filtros activos, en orden de aplicación.
    filters: Vec<BiquadFilter>,
    /// Bandas que describen los filtros (para consulta/UI).
    bands: Vec<EqBand>,
    /// Buffer temporal A (capacidad `max_frames`).
    scratch_a: Vec<f32>,
    /// Buffer temporal B.
    scratch_b: Vec<f32>,
    /// Máximo de frames por bloque soportado sin reasignar.
    max_frames: usize,
}

impl ParametricEq {
    /// Crea un EQ con las bandas indicadas.
    ///
    /// `max_frames` es la mayor cantidad de muestras por bloque que se espera
    /// procesar (normalmente el tamaño de buffer del stream); se usa para
    /// preasignar los buffers internos.
    pub fn new(bands: Vec<EqBand>, sample_rate: u32, max_frames: usize) -> Self {
        let filters = bands
            .iter()
            .map(|band| {
                BiquadFilter::design(
                    BiquadParams {
                        kind: match band.kind {
                            EqBandKind::LowShelf => BiquadKind::LowShelf,
                            EqBandKind::Peaking => BiquadKind::Peaking,
                            EqBandKind::HighShelf => BiquadKind::HighShelf,
                        },
                        freq_hz: band.freq_hz,
                        gain_db: band.gain_db,
                        q: band.q,
                    },
                    sample_rate,
                )
            })
            .collect();
        let max_frames = max_frames.max(1);
        Self {
            filters,
            bands,
            scratch_a: vec![0.0; max_frames],
            scratch_b: vec![0.0; max_frames],
            max_frames,
        }
    }

    /// Bandas activas del ecualizador.
    pub fn bands(&self) -> &[EqBand] {
        &self.bands
    }
}

impl AudioProcessor for ParametricEq {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        // Sin bandas → paso directo.
        if self.filters.is_empty() {
            output[..frames].copy_from_slice(&input[..frames]);
            return ProcessResult { latency_ms: 0.0 };
        }

        // Si el bloque excede la capacidad preasignada (caso anómalo), se
        // redimensiona: asignación puntual que no ocurre en operación normal.
        if frames > self.max_frames {
            self.max_frames = frames;
            self.scratch_a.resize(frames, 0.0);
            self.scratch_b.resize(frames, 0.0);
        }

        self.scratch_a[..frames].copy_from_slice(&input[..frames]);

        // Aplicar los filtros alternando buffers; `scratch_a` queda con el
        // resultado al terminar.
        for filter in &mut self.filters {
            for i in 0..frames {
                self.scratch_b[i] = filter.process(self.scratch_a[i]);
            }
            std::mem::swap(&mut self.scratch_a, &mut self.scratch_b);
        }

        output[..frames].copy_from_slice(&self.scratch_a[..frames]);
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "eq"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn boosts_a_single_band() {
        let sr = 48_000;
        let mut eq = ParametricEq::new(
            vec![EqBand {
                kind: EqBandKind::Peaking,
                freq_hz: 1000.0,
                gain_db: 12.0,
                q: 4.0,
            }],
            sr,
            4096,
        );
        let n = 4096;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * PI * 1000.0 * i as f32 / sr as f32).sin() * 0.1)
            .collect();
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        eq.process(&input, &mut out, &info);
        let gain = rms(&out[n / 2..]) / rms(&input[n / 2..]);
        let expected = 10f32.powf(12.0 / 20.0);
        assert!((gain - expected).abs() < 0.4, "gain {gain:.2}");
    }

    #[test]
    fn empty_eq_passes_through() {
        let mut eq = ParametricEq::new(vec![], 48_000, 64);
        let mut out = [0.0; 3];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 3,
        };
        eq.process(&[0.2, -0.1, 0.5], &mut out, &info);
        assert_eq!(out, [0.2, -0.1, 0.5]);
    }

    #[test]
    fn block_larger_than_max_frames_still_works() {
        let mut eq = ParametricEq::new(
            vec![EqBand {
                kind: EqBandKind::HighShelf,
                freq_hz: 5000.0,
                gain_db: 3.0,
                q: 0.707,
            }],
            48_000,
            4,
        );
        let input = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        let mut out = [0.0; 8];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 8,
        };
        eq.process(&input, &mut out, &info);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }
}
