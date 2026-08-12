//! Supresión de "boominess": reducción dinámica de la banda de graves medios.
//!
//! Cuando la señal acumula demasiada energía en la banda baja-media (típico
//! de micrófonos cerca de altavoces o salas con resonancias ~200–300 Hz), se
//! reduce solo esa banda de forma dinámica: `out = x - (1 - g)·band`. Es el
//! análogo en graves del de-esser: detecta la banda, calcula una ganancia de
//! reducción y la sustrae, dejando intacto el resto de la señal.

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Supresor de boominess con umbral, frecuencia y cantidad configurables.
#[derive(Debug, Clone)]
pub struct BoomSuppressor {
    /// Banda baja-media usada para detectar y reducir.
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

impl BoomSuppressor {
    /// Crea un supresor de boominess.
    ///
    /// `freq_hz` suele estar entre 150 y 400 Hz (la zona de "caja"/resonancia).
    /// `amount` (0–1) escala la reducción máxima. `sample_rate` define los
    /// coeficientes de tiempo (ataque ~5 ms, liberación ~120 ms fijos).
    pub fn new(threshold_db: f32, freq_hz: f32, amount: f32, sample_rate: u32) -> Self {
        let band = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::Peaking,
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
            amount: amount.clamp(0.0, 1.0),
            envelope: 0.0,
            attack_coef: time_to_coef(5.0, sr),
            release_coef: time_to_coef(120.0, sr),
        }
    }
}

impl AudioProcessor for BoomSuppressor {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        for i in 0..frames {
            let sample = input[i];

            // 1) Banda baja-media.
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
        "boomsuppressor"
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
        let mut boom = BoomSuppressor::new(-40.0, 250.0, 1.0, 48_000);
        let input = [0.0001, -0.0002, 0.0001];
        let mut out = [0.0; 3];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 3,
        };
        boom.process(&input, &mut out, &info);
        for (o, &i) in out.iter().zip(&input) {
            assert!((o - i).abs() < 1e-6);
        }
    }

    #[test]
    fn loud_low_mid_band_is_reduced() {
        // Un seno fuerte en la banda baja-media debe atenuarse.
        let sr = 48_000;
        let mut boom = BoomSuppressor::new(-20.0, 250.0, 1.0, sr);
        let n = 8192;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 250.0 * i as f32 / sr as f32).sin())
            .collect();
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        boom.process(&input, &mut out, &info);
        let rms_in = rms(&input[n / 2..]);
        let rms_out = rms(&out[n / 2..]);
        assert!(rms_out < rms_in * 0.7, "boominess not reduced");
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }
}
