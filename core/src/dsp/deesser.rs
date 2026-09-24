//! De-esser: compresión dinámica de la banda sibilante.
//!
//! Detecta la energía de la banda sibilante (paso de banda centrado en
//! `freq_hz`), calcula una reducción de ganancia dinámica y la aplica
//! únicamente a esa banda mediante sustracción: `out = x - (1 - g)·band`.

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// De-esser con umbral, frecuencia sibilante y cantidad configurables.
#[derive(Debug, Clone)]
pub struct DeEsser {
    /// Paso de banda para detección y reducción.
    band: BiquadFilter,
    threshold_db: f32,
    amount: f32,
    /// Envolvente detectada (lineal).
    envelope: f32,
    /// Coeficiente de ataque por muestra.
    attack_coef: f32,
    /// Coeficiente de liberación por muestra.
    release_coef: f32,
}

impl DeEsser {
    /// Crea un de-esser.
    ///
    /// `amount` (0–1) escala la reducción máxima. `sample_rate` define los
    /// coeficientes de tiempo (ataque ~1 ms, liberación ~80 ms fijos).
    pub fn new(threshold_db: f32, freq_hz: f32, amount: f32, sample_rate: u32) -> Self {
        let band = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::BandPass,
                freq_hz,
                gain_db: 0.0,
                q: 1.0,
            },
            sample_rate,
        );
        let sr = sample_rate.max(1) as f32;
        Self {
            band,
            threshold_db,
            // `amount` llega de la red: NaN/±inf → 0 (sin reducción) en vez de
            // contaminar `gr_db` y la salida con NaN.
            amount: if amount.is_finite() {
                amount.clamp(0.0, 1.0)
            } else {
                0.0
            },
            envelope: 0.0,
            attack_coef: time_to_coef(1.0, sr),
            release_coef: time_to_coef(80.0, sr),
        }
    }
}

impl AudioProcessor for DeEsser {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        for i in 0..frames {
            let sample = input[i];

            // 1) Banda sibilante.
            let band_sample = self.band.process(sample);

            // 2) Envolvente de la banda (pico suavizado).
            let detected = band_sample.abs();
            let coef = if detected > self.envelope {
                self.attack_coef
            } else {
                self.release_coef
            };
            self.envelope += coef * (detected - self.envelope);

            // 3) Reducción dinámica por encima del umbral.
            let level_db = 20.0 * self.envelope.log10().max(-120.0);
            let over_db = (level_db - self.threshold_db).max(0.0);
            // Relación 3:1 y escala por `amount`.
            let gr_db = (over_db * (1.0 - 1.0 / 3.0)) * self.amount;
            let g = 10f32.powf(-gr_db / 20.0);

            // 4) Reducir la banda: out = x - (1 - g)·band.
            output[i] = sample - band_sample * (1.0 - g);
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "deesser"
    }
}

/// Convierte un tiempo (ms) en coeficiente de suavizado por muestra.
fn time_to_coef(ms: f32, sample_rate: f32) -> f32 {
    let tau = ms.max(0.001) / 1000.0;
    (1.0 - (-1.0 / (tau * sample_rate)).exp()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_signal_passes_unchanged() {
        let mut de = DeEsser::new(-40.0, 6000.0, 1.0, 48_000);
        let input = [0.0001, -0.0002, 0.0001];
        let mut out = [0.0; 3];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 3,
        };
        de.process(&input, &mut out, &info);
        for (o, &i) in out.iter().zip(&input) {
            assert!((o - i).abs() < 1e-6);
        }
    }

    #[test]
    fn loud_sibilance_is_reduced() {
        // Un seno fuerte en la banda sibilante debe atenuarse.
        let sr = 48_000;
        let mut de = DeEsser::new(-20.0, 6000.0, 1.0, sr);
        let n = 8192;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 6000.0 * i as f32 / sr as f32).sin())
            .collect();
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        de.process(&input, &mut out, &info);
        let rms_in = rms(&input[n / 2..]);
        let rms_out = rms(&out[n / 2..]);
        assert!(rms_out < rms_in * 0.7, "sibilance not reduced");
    }

    #[test]
    fn off_band_signal_is_not_affected() {
        // Un seno fuera de la banda sibilante NO debe atenuarse. Con un
        // filtro de banda real (BandPass) el de-esser deja intacto el resto
        // del espectro; con la "banda" identidad previa reducía todo.
        let sr = 48_000;
        let mut de = DeEsser::new(-20.0, 6000.0, 1.0, sr);
        let n = 8192;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 300.0 * i as f32 / sr as f32).sin())
            .collect();
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        de.process(&input, &mut out, &info);
        let rms_in = rms(&input[n / 2..]);
        let rms_out = rms(&out[n / 2..]);
        assert!(
            (rms_out - rms_in).abs() < rms_in * 0.2,
            "off-band signal altered: in={rms_in:.6}, out={rms_out:.6}"
        );
    }

    #[test]
    fn nan_amount_degrades_to_passthrough() {
        // `amount` no finito (de la red) con el bug producía `gr_db` NaN →
        // salida NaN para siempre.
        for amount in [f32::NAN, f32::INFINITY, 1e6] {
            let mut de = DeEsser::new(-20.0, 6000.0, amount, 48_000);
            let mut out = vec![0.0; 8192];
            let info = ProcessingInfo {
                sample_rate: 48_000,
                frames: 8192,
            };
            de.process(&vec![0.5; 8192], &mut out, &info);
            assert!(
                out.iter().all(|v| v.is_finite()),
                "salida NaN con amount={amount}"
            );
        }
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }
}
