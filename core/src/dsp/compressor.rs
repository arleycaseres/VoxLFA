//! Compresor de dinámica vocal: detector de envolvente + ganancia suavizada.

use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Compresor con ataque/liberación configurables.
///
/// Detecta la envolvente del nivel (pico con suavizado), calcula la reducción
/// de ganancia por encima del umbral y suaviza la ganancia en el dominio dB
/// para evitar "zipper noise". El makeup se aplica al final.
#[derive(Debug, Clone)]
pub struct Compressor {
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    makeup_db: f32,

    /// Coeficiente de suavizado de ataque (0–1, por muestra).
    attack_coef: f32,
    /// Coeficiente de suavizado de liberación (0–1, por muestra).
    release_coef: f32,
    /// Envolvente detectada (amplitud lineal).
    envelope: f32,
    /// Reducción de ganancia suavizada (dB).
    gr_smooth_db: f32,
}

impl Compressor {
    /// Crea un compresor con los parámetros dados.
    ///
    /// Los tiempos se convierten a coeficientes por muestra según la frecuencia
    /// de muestreo; los valores inválidos (ms ≤ 0, ratio < 1) se sanan.
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_db: f32,
        sample_rate: u32,
    ) -> Self {
        let sr = sample_rate.max(1) as f32;
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            attack_ms,
            release_ms,
            makeup_db,
            attack_coef: time_to_coef(attack_ms, sr),
            release_coef: time_to_coef(release_ms, sr),
            envelope: 0.0,
            gr_smooth_db: 0.0,
        }
    }

    /// Umbral en dBFS.
    pub fn threshold_db(&self) -> f32 {
        self.threshold_db
    }

    /// Tiempo de ataque (ms).
    pub fn attack_ms(&self) -> f32 {
        self.attack_ms
    }

    /// Tiempo de liberación (ms).
    pub fn release_ms(&self) -> f32 {
        self.release_ms
    }
}

impl AudioProcessor for Compressor {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        for i in 0..frames {
            let sample = input[i];

            // 1) Detector de envolvente (ataque rápido, liberación lenta).
            let detected = sample.abs();
            let coef = if detected > self.envelope {
                self.attack_coef
            } else {
                self.release_coef
            };
            self.envelope += coef * (detected - self.envelope);

            // 2) Reducción de ganancia por encima del umbral (dominio dB).
            let level_db = 20.0 * self.envelope.log10().max(-120.0);
            let over = level_db - self.threshold_db;
            let gr_target_db = if over > 0.0 {
                over * (1.0 - 1.0 / self.ratio)
            } else {
                0.0
            };

            // 3) Suavizado de la ganancia (evita zipper noise).
            let coef = if gr_target_db > self.gr_smooth_db {
                self.attack_coef
            } else {
                self.release_coef
            };
            self.gr_smooth_db += coef * (gr_target_db - self.gr_smooth_db);

            // 4) Aplicar ganancia (reducción + makeup).
            let gain = 10f32.powf((self.makeup_db - self.gr_smooth_db) / 20.0);
            output[i] = sample * gain;
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "compressor"
    }
}

/// Convierte un tiempo (ms) en el coeficiente de suavizado por muestra:
/// `coef = 1 - exp(-1 / (τ·fs))`, con `τ = ms/1000`.
fn time_to_coef(ms: f32, sample_rate: f32) -> f32 {
    let tau = ms.max(0.001) / 1000.0;
    (1.0 - (-1.0 / (tau * sample_rate)).exp()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_threshold_is_unity_gain() {
        let mut comp = Compressor::new(-40.0, 4.0, 5.0, 100.0, 0.0, 48_000);
        // Señal muy baja: por debajo del umbral, sin reducción.
        let input = [0.001, -0.001, 0.0005, -0.0005];
        let mut out = [0.0; 4];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4,
        };
        comp.process(&input, &mut out, &info);
        for (o, &i) in out.iter().zip(&input) {
            assert!((o - i).abs() < 1e-6, "got {o}, expected {i}");
        }
    }

    #[test]
    fn loud_signal_is_reduced_below_unity() {
        let mut comp = Compressor::new(-20.0, 4.0, 1.0, 200.0, 0.0, 48_000);
        let input = [1.0; 4800]; // 100 ms de señal a escala completa
        let mut out = [0.0; 4800];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4800,
        };
        comp.process(&input, &mut out, &info);
        let last = out[out.len() - 1];
        // En régimen permanente: gr ≈ over*(1-1/ratio) = (0+20)*(0.75) = 15 dB.
        let expected = 10f32.powf(-15.0 / 20.0);
        assert!(
            (last - expected).abs() < 0.02,
            "steady gain {last:.4} != {expected:.4}"
        );
    }

    #[test]
    fn makeup_boosts_after_compression() {
        let mut comp = Compressor::new(-20.0, 4.0, 1.0, 200.0, 15.0, 48_000);
        let input = [1.0; 4800];
        let mut out = [0.0; 4800];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4800,
        };
        comp.process(&input, &mut out, &info);
        // gr 15 dB → ganancia neta = makeup - gr = 0 dB → salida ≈ 1.0.
        assert!((out[out.len() - 1] - 1.0).abs() < 0.02);
    }
}
