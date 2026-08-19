//! Supresión de feedback adaptativa con análisis espectral.
//!
//! Detecta resonancias de feedback (micrófono ↔ altavoz) mediante análisis
//! FFT del buffer de salida y aplica filtros muesca adaptativos para atenuar
//! las frecuencias problemáticas en tiempo real.
//!
//! El algoritmo:
//! 1. Acuma muestras en un buffer circular.
//! 2. Cada `hop_size` muestras, calcula la FFT y busca picos que superen
//!    el umbral de detección.
//! 3. Aplica filtros notch adaptativos con ataque lento y liberación rápida
//!    en las frecuencias detectadas.
//! 4. Los filtros se actualizan suavemente para evitar artefactos.

use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Número máximo de filtros notch adaptativos simultáneos.
const MAX_NOTCHES: usize = 4;

/// Tamaño del FFT para detección de feedback (potencia de 2).
const FFT_SIZE: usize = 2048;

/// Frecuencia mínima a escanear para feedback (Hz).
const FREQ_MIN_HZ: f32 = 100.0;

/// Factor de suavizado para el ataque del notch (0–1, menor = más lento).
const ATTACK_ALPHA: f32 = 0.02;

/// Factor de suavizado para la liberación del notch (0–1, menor = más lento).
const RELEASE_ALPHA: f32 = 0.05;

/// Un notch adaptativo que sigue una frecuencia de feedback detectada.
struct AdaptiveNotch {
    /// Filtro biquad subyacente.
    filter: BiquadFilter,
    /// Frecuencia central actual (Hz).
    freq_hz: f32,
    /// Atenuación actual (0 = sin atenuación, 1 = máximo).
    depth: f32,
    /// Profundidad objetivo (0 o 1).
    target_depth: f32,
    /// Factor de calidad del notch.
    q: f32,
    /// Sample rate.
    sample_rate: u32,
}

impl AdaptiveNotch {
    fn new(sample_rate: u32, q: f32) -> Self {
        let filter = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::Notch,
                freq_hz: 1000.0,
                gain_db: 0.0,
                q,
            },
            sample_rate,
        );
        Self {
            filter,
            freq_hz: 1000.0,
            depth: 0.0,
            target_depth: 0.0,
            q,
            sample_rate,
        }
    }

    /// Actualiza la frecuencia objetivo y reconstruye el filtro.
    fn retune(&mut self, freq_hz: f32) {
        if (freq_hz - self.freq_hz).abs() < 1.0 {
            return;
        }
        self.freq_hz = freq_hz;
        self.filter = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::Notch,
                freq_hz,
                gain_db: 0.0,
                q: self.q,
            },
            self.sample_rate,
        );
    }

    /// Avanza la profundidad hacia el objetivo (suavizado).
    fn advance_depth(&mut self) {
        let alpha = if self.target_depth > self.depth {
            ATTACK_ALPHA
        } else {
            RELEASE_ALPHA
        };
        self.depth += (self.target_depth - self.depth) * alpha;
        if self.depth < 0.001 {
            self.depth = 0.0;
            self.target_depth = 0.0;
        }
    }

    /// Procesa una muestra aplicando la atenuación.
    fn process_sample(&mut self, sample: f32) -> f32 {
        if self.depth < 0.001 {
            return sample;
        }
        let filtered = self.filter.process(sample);
        // Mezcla seco/húmedo según la profundidad.
        sample * (1.0 - self.depth) + filtered * self.depth
    }

    fn reset(&mut self) {
        self.filter.reset();
        self.depth = 0.0;
        self.target_depth = 0.0;
    }
}

/// Supresor de feedback adaptativa con análisis espectral.
pub struct FeedbackSuppressor {
    /// Umbral de detección en dB.
    threshold_db: f32,
    /// Filtros notch adaptativos.
    notches: [AdaptiveNotch; MAX_NOTCHES],
    /// Buffer circular para el FFT.
    ring: Vec<f32>,
    /// Posición de escritura en el ring.
    ring_pos: usize,
    /// Contador de muestras desde el último análisis FFT.
    samples_since_analysis: usize,
    /// Hop size para el análisis FFT.
    hop_size: usize,
    /// Planificador FFT.
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    /// Buffer de trabajo complejo para FFT.
    fft_buf: Vec<Complex<f32>>,
    /// Magnitudes del espectro anterior (para suavizado).
    prev_magnitude: Vec<f32>,
}

impl FeedbackSuppressor {
    /// Crea un nuevo supresor de feedback.
    pub fn new(threshold_db: f32, q: f32, sample_rate: u32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let hop_size = FFT_SIZE / 4; // 75% de overlap
        let n_bins = FFT_SIZE / 2 + 1;

        Self {
            threshold_db,
            notches: std::array::from_fn(|_| AdaptiveNotch::new(sample_rate, q.max(2.0))),
            ring: vec![0.0; FFT_SIZE],
            ring_pos: 0,
            samples_since_analysis: 0,
            hop_size,
            fft,
            fft_buf: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            prev_magnitude: vec![0.0; n_bins],
        }
    }

    /// Analiza el espectro y detecta picos de feedback.
    fn analyze_spectrum(&mut self, sample_rate: u32) {
        // Copiar ring al buffer FFT con ventana de Hann.
        for i in 0..FFT_SIZE {
            let window = 0.5 * (1.0 - (2.0 * PI * i as f32 / FFT_SIZE as f32).cos());
            let idx = (self.ring_pos + i) % FFT_SIZE;
            self.fft_buf[i] = Complex::new(self.ring[idx] * window, 0.0);
        }

        self.fft.process(&mut self.fft_buf);

        // Calcular magnitudes y frecuencias.
        let n_bins = FFT_SIZE / 2 + 1;
        let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
        let threshold_linear = 10f32.powf(self.threshold_db / 20.0);

        let mut peaks: Vec<(usize, f32)> = Vec::new(); // (bin, magnitude)
        for i in 1..n_bins {
            let freq_hz = i as f32 * bin_hz;
            if freq_hz < FREQ_MIN_HZ {
                continue;
            }
            let mag = self.fft_buf[i].norm() / FFT_SIZE as f32;
            // Suavizado temporal.
            let smoothed = self.prev_magnitude[i] * 0.7 + mag * 0.3;
            self.prev_magnitude[i] = smoothed;

            if smoothed > threshold_linear {
                // Verificar que es un pico local (mayor que sus vecinos).
                let prev = self.prev_magnitude[i - 1];
                let next = if i + 1 < n_bins {
                    self.prev_magnitude[i + 1]
                } else {
                    0.0
                };
                if smoothed > prev && smoothed > next {
                    peaks.push((i, smoothed));
                }
            }
        }

        // Ordenar por magnitud (mayor primero) y tomar los MAX_NOTCHES.
        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        peaks.truncate(MAX_NOTCHES);

        // Asignar picos a los notches disponibles.
        let mut used_notches = [false; MAX_NOTCHES];
        for (bin, _mag) in &peaks {
            let freq_hz = *bin as f32 * bin_hz;
            // Buscar un notch que ya siga esta frecuencia o uno libre.
            let slot = self.notches.iter_mut().enumerate().find(|(i, n)| {
                !used_notches[*i] && ((n.freq_hz - freq_hz).abs() < bin_hz * 2.0 || n.depth < 0.01)
            });

            if let Some((idx, notch)) = slot {
                used_notches[idx] = true;
                notch.retune(freq_hz);
                notch.target_depth = 1.0;
            }
        }

        // Desactivar notches que no detectaron feedback.
        for (i, notch) in self.notches.iter_mut().enumerate() {
            if !used_notches[i] && notch.target_depth > 0.0 {
                notch.target_depth = 0.0;
            }
        }
    }
}

impl AudioProcessor for FeedbackSuppressor {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        for i in 0..frames {
            // Escribir en el ring buffer.
            self.ring[self.ring_pos] = input[i];
            self.ring_pos = (self.ring_pos + 1) % FFT_SIZE;

            // Procesar a través de los notches adaptativos.
            let mut sample = input[i];
            for notch in &mut self.notches {
                sample = notch.process_sample(sample);
                notch.advance_depth();
            }
            output[i] = sample;

            // Análisis periódico.
            self.samples_since_analysis += 1;
            if self.samples_since_analysis >= self.hop_size {
                self.samples_since_analysis = 0;
                self.analyze_spectrum(info.sample_rate);
            }
        }

        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "feedback"
    }

    fn reset(&mut self) {
        self.ring.fill(0.0);
        self.ring_pos = 0;
        self.samples_since_analysis = 0;
        self.prev_magnitude.fill(0.0);
        for notch in &mut self.notches {
            notch.reset();
        }
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
    fn feedback_suppressor_passes_quiet_signal() {
        let mut proc = FeedbackSuppressor::new(-30.0, 10.0, 48_000);
        let input = vec![0.001; 2048];
        let mut output = vec![0.0; 2048];
        proc.process(&input, &mut output, &info());

        // Señal silenciosa no debe ser alterada significativamente.
        let rms_in: f32 = input.iter().map(|x| x * x).sum::<f32>().sqrt() / input.len() as f32;
        let rms_out: f32 = output.iter().map(|x| x * x).sum::<f32>().sqrt() / output.len() as f32;
        let ratio = rms_out / rms_in;
        assert!(
            ratio > 0.8 && ratio < 1.2,
            "señal silenciosa alterada: ratio={ratio}"
        );
    }

    #[test]
    fn feedback_suppressor_detects_and_suppresses_tone() {
        let sr = 48_000u32;
        let mut proc = FeedbackSuppressor::new(-20.0, 10.0, sr);
        // Generar una señal con un tono fuerte a 1000 Hz (simula feedback).
        let n = 8192;
        let input: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                0.5 * (2.0 * PI * 1000.0 * t).sin()
            })
            .collect();
        let mut output = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: 256,
        };

        // Procesar en bloques de 256.
        for chunk_start in (0..n).step_by(256) {
            let end = (chunk_start + 256).min(n);
            proc.process(
                &input[chunk_start..end],
                &mut output[chunk_start..end],
                &info,
            );
        }

        // Tras suficiente procesamiento, el tono debe estar atenuado.
        // Medimos la segunda mitad (dar tiempo a que los notches se activen).
        let second_half = n / 2;
        let rms_in = rms(&input[second_half..]);
        let rms_out = rms(&output[second_half..]);
        assert!(
            rms_out < rms_in * 0.8,
            "feedback no suprimido: rms_in={rms_in:.4}, rms_out={rms_out:.4}"
        );
    }

    #[test]
    fn reset_clears_state() {
        let mut proc = FeedbackSuppressor::new(-30.0, 10.0, 48_000);
        let input = vec![0.5; 4096];
        let mut output = vec![0.0; 4096];
        proc.process(&input, &mut output, &info());

        proc.reset();
        assert_eq!(proc.ring_pos, 0);
        assert_eq!(proc.samples_since_analysis, 0);
    }

    #[test]
    fn name_is_feedback() {
        let proc = FeedbackSuppressor::new(-30.0, 10.0, 48_000);
        assert_eq!(proc.name(), "feedback");
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }
}
