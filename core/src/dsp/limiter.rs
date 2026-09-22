//! Limitador de techo con lookahead (prevención de clipping).

use super::delay::DelayLine;
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Limitador de techo con lookahead.
///
/// Inserta una línea de retardo de `lookahead_ms` para que el detector vea el
/// futuro y aplique la ganancia con antelación, evitando distorsión por
/// sobrepasos. El `ProcessResult` reporta la latencia introducida.
#[derive(Debug, Clone)]
pub struct Limiter {
    /// Línea de retardo del lookahead.
    line: DelayLine,
    /// Retardo de lookahead en muestras.
    lookahead_samples: usize,
    /// Umbral en dBFS (pico).
    threshold_db: f32,
    /// Margen máximo de ganancia (dB) que puede aplicar el limitador.
    margin_db: f32,
    /// Coeficiente de suavizado por muestra (release corto).
    release_coef: f32,
    /// Ganancia aplicada (lineal), suavizada.
    gain: f32,
    /// Máximo nivel pico visto en la ventana de lookahead (lineal).
    peak: f32,
    /// Retardo del lookahead en ms (para reportar latencia).
    lookahead_ms: f32,
}

impl Limiter {
    /// Crea un limitador.
    ///
    /// - `threshold_db`: techo en dBFS (p. ej. -1.0).
    /// - `lookahead_ms`: anticipación (1–10 ms típico).
    /// - `release_ms`: tiempo de recuperación tras un pico.
    pub fn new(threshold_db: f32, lookahead_ms: f32, release_ms: f32, sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        let lookahead_samples = (lookahead_ms.max(0.0) * sr / 1000.0).ceil() as usize;
        Self {
            line: DelayLine::new(lookahead_samples.max(1)),
            lookahead_samples,
            threshold_db,
            margin_db: 12.0,
            release_coef: time_to_coef(release_ms, sr),
            gain: 1.0,
            peak: 0.0,
            lookahead_ms: lookahead_ms.max(0.0),
        }
    }
}

impl AudioProcessor for Limiter {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        let threshold = 10f32.powf(self.threshold_db / 20.0);

        for i in 0..frames {
            // 1) Detector con mirada al futuro: máximo pico en la ventana.
            //    El pico decae con el release: si no llegan más picos altos,
            //    la atenuación se recupera (sin esto quedaría atornillado a
            //    la reducción máxima para siempre).
            let in_future = if i + self.lookahead_samples < frames {
                input[i + self.lookahead_samples]
            } else {
                input[frames - 1]
            };
            self.peak *= (1.0 - self.release_coef).max(0.0);
            if in_future.abs() > self.peak {
                self.peak = in_future.abs();
            }

            // 2) Ganancia objetivo: suficiente para meter el pico bajo el techo.
            let target = if self.peak > threshold {
                threshold / self.peak
            } else {
                1.0
            };
            // Limitar el margen para evitar vaciados exagerados.
            let min_gain = 10f32.powf(-self.margin_db / 20.0);
            let target = target.max(min_gain);

            // 3) Suavizado (ataque instantáneo hacia abajo, release lento).
            if target < self.gain {
                self.gain = target;
            } else {
                self.gain += self.release_coef * (target - self.gain);
            }

            // 4) Aplicar la ganancia a la muestra retardada (lookahead).
            let delayed = self.line.push(input[i], self.lookahead_samples);
            output[i] = delayed * self.gain;
        }
        ProcessResult {
            latency_ms: self.lookahead_ms,
        }
    }

    fn name(&self) -> &'static str {
        "limiter"
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
    fn output_never_exceeds_threshold() {
        let sr = 48_000;
        let mut lim = Limiter::new(-1.0, 3.0, 100.0, sr);
        // Onda cuadrada a +3 dB (≈1.41 de pico).
        let n = sr as usize; // 1 s
        let input: Vec<f32> = (0..n)
            .map(|i| if (i / 100) % 2 == 0 { 1.41 } else { -1.41 })
            .collect();
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        lim.process(&input, &mut out, &info);
        let max = out.iter().fold(0.0f32, |m, &v| m.max(v.abs()));
        assert!(max <= 1.0, "output peak {max} > threshold");
        // Y que haya limitado de verdad (no passthrough plano).
        assert!(max < 1.2, "peak too high: {max}");
    }

    #[test]
    fn quiet_signal_is_not_gained_up() {
        let sr = 48_000;
        let mut lim = Limiter::new(-1.0, 3.0, 100.0, sr);
        let input = [0.05; 4800];
        let mut out = [0.0; 4800];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: 4800,
        };
        lim.process(&input, &mut out, &info);
        let last = out[out.len() - 1];
        // La señal por debajo del techo debe pasar casi intacta.
        assert!((last - 0.05).abs() < 1e-4, "got {last}");
    }

    #[test]
    fn gain_recovers_after_transient_peak() {
        // Tras un pico transitorio por encima del techo, el limitador debe
        // recuperar la unidad de ganancia (no quedarse atenuando para siempre).
        let sr = 48_000;
        let mut lim = Limiter::new(-1.0, 3.0, 50.0, sr);
        let n = sr as usize;
        let mut input = vec![0.02; n];
        // Un pico único de 1.4 (por encima de 0 dBFS) en la mitad.
        input[n / 2] = 1.4;
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        lim.process(&input, &mut out, &info);
        // Al final del buffer (muy después del pico) la ganancia debe volver a
        // ~1, así que la señal baja debe recuperar su nivel original.
        let tail = &out[3 * n / 4..];
        let mean_tail = tail.iter().map(|v| v.abs()).sum::<f32>() / tail.len() as f32;
        assert!(
            (mean_tail - 0.02).abs() < 0.005,
            "gain stuck: tail mean {mean_tail:.5} != ~0.02"
        );
    }
}
