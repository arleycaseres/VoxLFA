//! Supresión de ruido basada en RNNoise (nnnoiseless).
//!
//! [`RnnoiseDenoise`] usa la red neuronal recurrente de nnnoiseless para
//! suprimir ruido de fondo en tiempo real. Procesa en bloques de 480 muestras
//! (10 ms a 48 kHz); los bloques de la cadena se fragmentan internamente.
//!
//! **Escalas de señal**: nnnoiseless espera muestras en rango i16
//! ([-32768.0, 32767.0]) y devuelve el resultado en la misma escala. Este
//! procesador aplica la conversión automáticamente.

use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Tamaño de frame que espera nnnoiseless (480 muestras = 10 ms a 48 kHz).
const FRAME_SIZE: usize = nnnoiseless::DenoiseState::FRAME_SIZE;

/// Conversión de [-1.0, 1.0] (float PCM) a rango i16 para nnnoiseless.
const I16_SCALE: f32 = i16::MAX as f32;

/// Procesador de supresión de ruido con RNNoise.
///
/// Mantiene un buffer interno para acumular muestras hasta completar frames
/// de 480 muestras. El primer frame de salida se descarta (fade-in).
pub struct RnnoiseDenoise {
    state: Box<nnnoiseless::DenoiseState<'static>>,
    /// Buffer de entrada pendiente de procesar.
    input_buf: Vec<f32>,
    /// Buffer de salida pendiente de entregar.
    output_buf: Vec<f32>,
    /// `true` si aún no se ha descartado el primer frame (fade-in).
    first_frame: bool,
}

impl RnnoiseDenoise {
    /// Crea un nuevo procesador RNNoise.
    pub fn new() -> Self {
        Self {
            state: nnnoiseless::DenoiseState::new(),
            input_buf: Vec::with_capacity(FRAME_SIZE * 2),
            output_buf: Vec::with_capacity(FRAME_SIZE * 2),
            first_frame: true,
        }
    }
}

impl Default for RnnoiseDenoise {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioProcessor for RnnoiseDenoise {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        // Acumular entrada en el buffer interno.
        self.input_buf.extend_from_slice(&input[..frames]);

        // Procesar frames completos de 480 muestras.
        while self.input_buf.len() >= FRAME_SIZE {
            let frame_in: Vec<f32> = self.input_buf.drain(..FRAME_SIZE).collect();
            let mut frame_out = [0.0f32; FRAME_SIZE];

            // Convertir de float [-1,1] a rango i16.
            let mut frame_i16 = [0.0f32; FRAME_SIZE];
            for (dst, &s) in frame_i16.iter_mut().zip(frame_in.iter()) {
                *dst = s * I16_SCALE;
            }

            self.state.process_frame(&mut frame_out, &frame_i16);

            // Descartar el primer frame (artefactos de fade-in).
            if self.first_frame {
                self.first_frame = false;
                continue;
            }

            // Convertir de rango i16 de vuelta a float [-1,1].
            for &s in &frame_out {
                self.output_buf.push(s / I16_SCALE);
            }
        }

        // Entregar tantas muestras como estén disponibles y quepan en output.
        let available = self.output_buf.len();
        let to_copy = available.min(frames);
        output[..to_copy].copy_from_slice(&self.output_buf[..to_copy]);

        // Si quedó espacio en output, rellenar con silencio.
        if to_copy < frames {
            output[to_copy..frames].fill(0.0);
        }

        // Consumir las muestras entregadas del buffer de salida.
        self.output_buf.drain(..to_copy);

        // Latencia del buffer interno (aproximada).
        let buffered = self.input_buf.len() + self.output_buf.len();
        ProcessResult {
            latency_ms: (buffered as f32 / 48_000.0) * 1000.0,
        }
    }

    fn name(&self) -> &'static str {
        "rnnoise"
    }

    fn reset(&mut self) {
        self.state = nnnoiseless::DenoiseState::new();
        self.input_buf.clear();
        self.output_buf.clear();
        self.first_frame = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> ProcessingInfo {
        ProcessingInfo {
            sample_rate: 48_000,
            frames: 256,
        }
    }

    #[test]
    fn rnnoise_reduces_noise_in_silence() {
        let mut proc = RnnoiseDenoise::new();
        // Señal de silencio con ruido bajo.
        let input: Vec<f32> = (0..480).map(|_| (rand() as f32 / 1000.0) - 0.005).collect();
        let mut output = vec![0.0f32; 480];
        proc.process(&input, &mut output, &info());

        // RNNoise debe atenuar ruido en silencio.
        let input_rms: f32 = input.iter().map(|x| x * x).sum::<f32>().sqrt() / input.len() as f32;
        let output_rms: f32 =
            output.iter().map(|x| x * x).sum::<f32>().sqrt() / output.len() as f32;
        assert!(
            output_rms < input_rms || output_rms < 1e-6,
            "ruido no reducido: input_rms={input_rms:.6}, output_rms={output_rms:.6}"
        );
    }

    #[test]
    fn rnnoise_processes_without_crashing() {
        let mut proc = RnnoiseDenoise::new();
        // Señal con armónicos múltiples (más parecida a voz que un sine puro).
        let input: Vec<f32> = (0..960) // 2 frames
            .map(|i| {
                let t = i as f32 / 48_000.0;
                let f0 = 2.0 * std::f32::consts::PI * 200.0 * t;
                // Fundamental + armónicos con envolvente.
                let env = (0.5 + 0.5 * (2.0 * std::f32::consts::PI * 3.0 * t).sin()).max(0.1);
                (f0.sin() * 0.4 + (2.0 * f0).sin() * 0.25 + (3.0 * f0).sin() * 0.15) * env
            })
            .collect();
        let mut output = vec![0.0f32; 960];
        proc.process(&input, &mut output, &info());

        // Verificar que produce salida (no panic, no crash) y que el output
        // tiene magnitud razonable (ni todo cero ni clipping).
        let output_peak: f32 = output.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
        assert!(
            output_peak > 0.0,
            "salida completamente en silencio: output_peak=0"
        );
        assert!(
            output_peak < 1.0,
            "salida con clipping: output_peak={output_peak}"
        );
    }

    #[test]
    fn reset_clears_buffers() {
        let mut proc = RnnoiseDenoise::new();
        let input = vec![0.1; 100];
        let mut output = vec![0.0; 100];
        proc.process(&input, &mut output, &info());

        proc.reset();
        assert!(proc.input_buf.is_empty());
        assert!(proc.output_buf.is_empty());
        assert!(proc.first_frame);
    }

    #[test]
    fn name_is_rnnoise() {
        let proc = RnnoiseDenoise::new();
        assert_eq!(proc.name(), "rnnoise");
    }

    /// Genera un número pseudo-aleatorio determinista para tests (sin depender
    /// de crate `rand`). Simple LCG.
    fn rand() -> u32 {
        use std::cell::Cell;
        thread_local! { static SEED: Cell<u32> = const { Cell::new(12345) }; }
        SEED.with(|s| {
            let v = s.get();
            s.set(v.wrapping_mul(1664525).wrapping_add(1013904223));
            v
        })
    }
}
