//! Línea de retardo circular reutilizable y efecto de eco (delay).

use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Línea de retardo circular (buffer de una sola muestra de canal).
///
/// Usada por el efecto `Delay` y por el `Limiter` (lookahead). No asigna
/// memoria en tiempo real: el buffer se preasigna al construir.
#[derive(Debug, Clone)]
pub struct DelayLine {
    pub(crate) buffer: Vec<f32>,
    /// Posición de escritura (índice de la muestra más reciente).
    pub(crate) write: usize,
}

impl DelayLine {
    /// Crea una línea con capacidad para `max_delay` muestras.
    pub fn new(max_delay: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay.max(1)],
            write: 0,
        }
    }

    /// Escribe la muestra actual y devuelve la muestra de `delay` muestras
    /// atrás (leyendo por la rama invertida del buffer circular).
    pub fn push(&mut self, sample: f32, delay: usize) -> f32 {
        let len = self.buffer.len();
        let delayed = self.buffer[(self.write + len - delay.min(len)) % len];
        self.buffer[self.write] = sample;
        self.write = (self.write + 1) % len;
        delayed
    }

    /// Limpia el buffer (silencio).
    pub fn clear(&mut self) {
        for v in &mut self.buffer {
            *v = 0.0;
        }
        self.write = 0;
    }
}

/// Efecto de eco con feedback y mezcla seco/húmedo.
///
/// El camino seco no añade latencia (es un efecto en paralelo), por lo que el
/// `ProcessResult` reporta 0.
#[derive(Debug, Clone)]
pub struct Delay {
    line: DelayLine,
    /// Retardo en muestras (calculado a partir de `time_ms`).
    delay_samples: usize,
    feedback: f32,
    mix: f32,
    /// Retardo en ms (guardado para consulta).
    time_ms: f32,
}

impl Delay {
    /// Tiempo de retardo (ms).
    pub fn time_ms(&self) -> f32 {
        self.time_ms
    }

    /// Crea un delay con el tiempo, feedback y mezcla indicados.
    ///
    /// `max_time_ms` define la capacidad interna de la línea (por defecto igual
    /// a `time_ms`); si luego se aumenta el tiempo se reasigna el buffer.
    pub fn new(time_ms: f32, feedback: f32, mix: f32, sample_rate: u32) -> Self {
        let max_delay = ms_to_samples(time_ms, sample_rate).max(1);
        Self {
            line: DelayLine::new(max_delay),
            delay_samples: max_delay,
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
            time_ms: time_ms.max(0.0),
        }
    }
}

impl AudioProcessor for Delay {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        let dry = 1.0 - self.mix;
        for i in 0..frames {
            let wet = self.line.push(
                input[i] + self.line_delayed() * self.feedback,
                self.delay_samples,
            );
            // Re-escribir el valor con feedback... la línea ya guardó la mezcla;
            // la muestra retardada se obtiene de nuevo.
            output[i] = input[i] * dry + wet * self.mix;
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "delay"
    }
}

impl Delay {
    /// Última muestra retardada disponible (para el lazo de feedback).
    fn line_delayed(&self) -> f32 {
        let len = self.line.buffer.len();
        self.line.buffer[(self.line.write + len - self.delay_samples.min(len)) % len]
    }
}

/// Convierte un tiempo en ms a muestras (redondeando hacia arriba).
fn ms_to_samples(ms: f32, sample_rate: u32) -> usize {
    (ms.max(0.0) * sample_rate as f32 / 1000.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_an_impulse_by_the_right_amount() {
        let sr = 1000; // 1 kHz → 1 muestra = 1 ms
        let mut delay = Delay::new(5.0, 0.0, 1.0, sr);
        let mut input = [0.0; 8];
        input[0] = 1.0;
        let mut out = [0.0; 8];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: 8,
        };
        delay.process(&input, &mut out, &info);
        // Con mix=1 y feedback=0, el pulso aparece 5 ms (5 muestras) después.
        assert_eq!(out[4], 0.0, "antes del retardo");
        assert!(
            (out[5] - 1.0).abs() < 1e-6,
            "impulso en out[5] = {}",
            out[5]
        );
        assert_eq!(out[6], 0.0);
    }

    #[test]
    fn zero_mix_is_dry() {
        let mut delay = Delay::new(10.0, 0.5, 0.0, 48_000);
        let input = [0.1, 0.2, 0.3];
        let mut out = [0.0; 3];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 3,
        };
        delay.process(&input, &mut out, &info);
        assert_eq!(out, input);
    }
}
