//! Reverberación tipo Schroeder: 4 filtros comb + 2 allpass.
//!
//! Un único canal de reverb. Los retardos del Schroeder no son múltiplos entre
//! sí para que la densidad modal sea alta y el sonido sea natural.

use super::delay::DelayLine;
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Tamaños de los combs (en ms) — valores clásicos de Schroeder escalados.
const COMB_MS: [f32; 4] = [29.7, 37.1, 41.1, 43.7];
/// Tamaños de los allpass (en ms).
const ALLPASS_MS: [f32; 2] = [5.0, 1.7];
/// Número de retardos totales.
const N_LINES: usize = COMB_MS.len() + ALLPASS_MS.len();

/// Reverberación por combinación de combs + allpass (mono).
#[derive(Debug, Clone)]
pub struct Reverb {
    lines: [DelayLine; N_LINES],
    /// Retardos en muestras (recalculados con la frecuencia de muestreo).
    comb_delays: [usize; COMB_MS.len()],
    allpass_delays: [usize; ALLPASS_MS.len()],
    /// Ganancia de realimentación de cada comb (0–1).
    feedback: f32,
    /// Mezcla seco/húmedo (0–1).
    mix: f32,
    /// Amortiguación (0 = brillante, 1 = apagado) sobre el allpass.
    damping: f32,
}

impl Reverb {
    /// Crea una reverb con tiempo de cola, mezcla y amortiguación.
    ///
    /// - `room_ms`: controla la ganancia de realimentación (más largo = más
    ///   eco).
    /// - `mix`: proporción de señal reverberada (0 = seco, 1 = 100% wet).
    /// - `damping`: suavizado de la señal dentro de los allpass (0–1).
    pub fn new(room_ms: f32, mix: f32, damping: f32, sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        // Capacidad: el retardo máximo es el comb más largo (con margen).
        let max_comb = COMB_MS.iter().fold(0.0f32, |m, &c| m.max(c));
        let max_samples = (max_comb * sr / 1000.0).ceil() as usize + 2;

        let lines = std::array::from_fn(|_| DelayLine::new(max_samples));
        // Reutilizar los allpass sobre las mismas líneas (retardos cortos).
        // La capacidad del buffer cubre el peor caso; aquí solo cambia el
        // retardo de lectura, no la capacidad.
        let comb_delays = std::array::from_fn(|i| ms_to_samples(COMB_MS[i], sr));
        let allpass_delays = std::array::from_fn(|i| ms_to_samples(ALLPASS_MS[i], sr));
        // El feedback se aproxima a la cola deseada: g = exp(-3·t / room_ms),
        // donde t es el tiempo de cola (definimos t ≈ room_ms para -60 dB).
        let room = room_ms.max(10.0);
        let feedback = (-3.0 * max_comb / room).exp().clamp(0.0, 0.95);

        Self {
            lines,
            comb_delays,
            allpass_delays,
            feedback,
            mix: mix.clamp(0.0, 1.0),
            damping: damping.clamp(0.0, 0.5),
        }
    }
}

impl AudioProcessor for Reverb {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        let dry = 1.0 - self.mix;

        for i in 0..frames {
            // Combs en paralelo y allpass en serie sobre la entrada.
            let combs = self.comb_step(input[i]);
            let wet = self.allpass_step(combs);

            output[i] = input[i] * dry + wet * self.mix;
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "reverb"
    }
}

impl Reverb {
    /// Procesa los 4 combs en paralelo sobre `x` y devuelve su suma.
    ///
    /// Accede al buffer interno de las líneas para sumar las salidas antes de
    /// escribir el nuevo valor (algoritmo Schroeder clásico).
    fn comb_step(&mut self, x: f32) -> f32 {
        let mut acc = 0.0;
        for idx in 0..COMB_MS.len() {
            let delay = self.comb_delays[idx];
            let line = &mut self.lines[idx];
            let len = line.buffer.len();
            let delayed = line.buffer[(line.write + len - delay) % len];
            line.buffer[line.write] = x + delayed * self.feedback;
            line.write = (line.write + 1) % len;
            acc += delayed;
        }
        acc
    }

    /// Aplica los 2 allpass en serie (con damping) sobre `x`.
    fn allpass_step(&mut self, x: f32) -> f32 {
        let base = COMB_MS.len();
        let mut cur = x;
        for idx in 0..ALLPASS_MS.len() {
            let delay = self.allpass_delays[idx];
            let line = &mut self.lines[base + idx];
            let len = line.buffer.len();
            let delayed = line.buffer[(line.write + len - delay) % len];
            // Allpass clásico: out = -g·x + delayed.
            let g = 0.5 - self.damping; // damping 0.5 → g=0 (más apagado)
            let out = -g * cur + delayed;
            line.buffer[line.write] = cur + g * delayed;
            line.write = (line.write + 1) % len;
            cur = out;
        }
        cur
    }
}

/// Convierte un tiempo (ms) en muestras (redondeando hacia arriba).
fn ms_to_samples(ms: f32, sample_rate: f32) -> usize {
    (ms.max(0.0) * sample_rate / 1000.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverb_decays_toward_silence() {
        let sr = 48_000;
        let mut reverb = Reverb::new(200.0, 1.0, 0.0, sr);
        let n = sr as usize; // 1 s
        let mut input = vec![0.0; n];
        input[0] = 1.0; // impulso
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        reverb.process(&input, &mut out, &info);
        // La cola debe decaer: más energía al inicio que al final.
        let early: f32 = out[..n / 2].iter().map(|v| v * v).sum();
        let late: f32 = out[n / 2..].iter().map(|v| v * v).sum();
        assert!(early > late, "cola no decae (early={early}, late={late})");
        assert!(late < 1e-3, "cola demasiado larga (late={late})");
    }

    #[test]
    fn zero_mix_is_dry() {
        let mut reverb = Reverb::new(200.0, 0.0, 0.0, 48_000);
        let input = [0.1, 0.2, -0.3];
        let mut out = [0.0; 3];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 3,
        };
        reverb.process(&input, &mut out, &info);
        assert_eq!(out, input);
    }
}
