//! Supresión de feedback adaptativa con dos modos de operación.
//!
//! **Modo Notch** (clásico): detecta resonancias de feedback mediante análisis
//! FFT del buffer y aplica filtros muesca adaptativos.
//!
//! **Modo Adaptive** (FIR NLMS): usa un filtro adaptativo FIR (Normalized
//! Least Mean Squares) que estima la ruta de feedback (altavoz → micrófono)
//! y la cancela sustrayendo la señal estimada de la entrada. Requiere la
//! señal de salida del bloque anterior como referencia.

use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::{FeedbackMode, FeedbackSuppressorParams};

// ── Constantes del modo Notch ──────────────────────────────────────────

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

// ── Constantes del modo Adaptive ───────────────────────────────────────

/// Longitud máxima del filtro FIR adaptativo (taps).
const MAX_FILTER_LEN: usize = 4096;

/// Factor de normalización mínimo para NLMS (evita división por cero).
const NLMS_EPSILON: f32 = 1e-10;

/// Energía mínima de la referencia para activar la adaptación del filtro.
const REF_ENERGY_MIN: f32 = 1e-8;

/// Límite de divergencia del filtro (norma máxima de los pesos).
const MAX_WEIGHT_NORM: f32 = 10.0;

// ── Modo Notch ─────────────────────────────────────────────────────────

/// Un notch adaptativo que sigue una frecuencia de feedback detectada.
struct AdaptiveNotch {
    filter: BiquadFilter,
    freq_hz: f32,
    depth: f32,
    target_depth: f32,
    q: f32,
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

    fn process_sample(&mut self, sample: f32) -> f32 {
        if self.depth < 0.001 {
            return sample;
        }
        let filtered = self.filter.process(sample);
        sample * (1.0 - self.depth) + filtered * self.depth
    }

    fn reset(&mut self) {
        self.filter.reset();
        self.depth = 0.0;
        self.target_depth = 0.0;
    }
}

// ── Modo Adaptive (NLMS FIR) ───────────────────────────────────────────

/// Filtro adaptativo FIR using Normalized Least Mean Squares (NLMS).
///
/// Modela la ruta de transferencia del sistema de feedback (altavoz → sala →
/// micrófono) y sustrae la señal estimada de la entrada para cancelar el
/// feedback sin degradar la señal vocal deseada.
struct AdaptiveFir {
    /// Pesos del filtro FIR (longitud variable).
    weights: Vec<f32>,
    /// Buffer circular de la referencia (salida anterior).
    ref_buf: Vec<f32>,
    /// Posición de escritura en el buffer de referencia.
    ref_pos: usize,
    /// Tasa de aprendizaje NLMS (0.01–0.5).
    mu: f32,
    /// Energía acumulada de la referencia (suavizada).
    ref_power: f32,
    /// Señal de error anterior (para suavizado).
    prev_error: f32,
}

impl AdaptiveFir {
    fn new(filter_len: usize, mu: f32) -> Self {
        let filter_len = filter_len.clamp(64, MAX_FILTER_LEN);
        Self {
            weights: vec![0.0; filter_len],
            ref_buf: vec![0.0; filter_len],
            ref_pos: 0,
            mu: mu.clamp(0.01, 0.5),
            ref_power: 0.0,
            prev_error: 0.0,
        }
    }

    fn reset(&mut self) {
        self.weights.fill(0.0);
        self.ref_buf.fill(0.0);
        self.ref_pos = 0;
        self.ref_power = 0.0;
        self.prev_error = 0.0;
    }

    /// Procesa una muestra: estima y cancela la señal de feedback.
    ///
    /// `input` es la señal del micrófono (con feedback potencial).
    /// `output_ref` es la señal que se envió al altavoz en el bloque anterior.
    fn process_sample(&mut self, input: f32) -> f32 {
        let filter_len = self.weights.len();

        // Calcular estimación de feedback: y_hat = w^T * x (convolución).
        let mut y_hat = 0.0;
        for i in 0..filter_len {
            let idx = (self.ref_pos + filter_len - 1 - i) % filter_len;
            y_hat += self.weights[i] * self.ref_buf[idx];
        }

        // Señal de error: entrada limpia (feedback cancelado).
        let error = input - y_hat;

        // NLMS: actualizar pesos normalizando por la energía de la referencia.
        let mut ref_energy = 0.0;
        for i in 0..filter_len {
            let idx = (self.ref_pos + filter_len - 1 - i) % filter_len;
            ref_energy += self.ref_buf[idx] * self.ref_buf[idx];
        }

        // Suavizado exponencial de la energía de referencia.
        self.ref_power = self.ref_power * 0.95 + ref_energy * 0.05;

        if self.ref_power > REF_ENERGY_MIN {
            let norm = self.mu / (self.ref_power + NLMS_EPSILON);
            // Clamp para evitar divergencia.
            let step = norm.clamp(0.0, 1.0);

            for i in 0..filter_len {
                let idx = (self.ref_pos + filter_len - 1 - i) % filter_len;
                self.weights[i] += step * error * self.ref_buf[idx];
            }

            // Verificar que los pesos no diverjan.
            let w_norm: f32 = self.weights.iter().map(|w| w * w).sum();
            if w_norm > MAX_WEIGHT_NORM * MAX_WEIGHT_NORM {
                let scale = MAX_WEIGHT_NORM / w_norm.sqrt();
                for w in &mut self.weights {
                    *w *= scale;
                }
            }
        }

        self.prev_error = error;
        error
    }

    /// Escribe una muestra en el buffer circular de referencia.
    fn push_reference(&mut self, sample: f32) {
        self.ref_buf[self.ref_pos] = sample;
        self.ref_pos = (self.ref_pos + 1) % self.ref_buf.len();
    }
}

// ── FeedbackSuppressor ─────────────────────────────────────────────────

/// Supresor de feedback adaptativa con dos modos de operación.
pub struct FeedbackSuppressor {
    mode: FeedbackMode,

    // ── Estado Notch ──
    threshold_db: f32,
    notches: [AdaptiveNotch; MAX_NOTCHES],
    ring: Vec<f32>,
    ring_pos: usize,
    samples_since_analysis: usize,
    hop_size: usize,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    fft_buf: Vec<Complex<f32>>,
    prev_magnitude: Vec<f32>,

    // ── Estado Adaptive ──
    adaptive: AdaptiveFir,
    /// Buffer de referencia del bloque anterior.
    prev_output: Vec<f32>,
    /// Longitud de la referencia almacenada.
    prev_output_len: usize,
}

impl FeedbackSuppressor {
    /// Crea un nuevo supresor de feedback con los parámetros indicados.
    pub fn from_params(params: FeedbackSuppressorParams, sample_rate: u32) -> Self {
        match params.mode {
            FeedbackMode::Notch => Self::new_notch(params.threshold_db, params.q, sample_rate),
            FeedbackMode::Adaptive => Self::new_adaptive(params.mu, params.filter_len, sample_rate),
        }
    }

    fn new_notch(threshold_db: f32, q: f32, sample_rate: u32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let hop_size = FFT_SIZE / 4;
        let n_bins = FFT_SIZE / 2 + 1;

        Self {
            mode: FeedbackMode::Notch,
            threshold_db,
            notches: std::array::from_fn(|_| AdaptiveNotch::new(sample_rate, q.max(2.0))),
            ring: vec![0.0; FFT_SIZE],
            ring_pos: 0,
            samples_since_analysis: 0,
            hop_size,
            fft,
            fft_buf: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            prev_magnitude: vec![0.0; n_bins],
            adaptive: AdaptiveFir::new(256, 0.1),
            prev_output: vec![0.0; MAX_FILTER_LEN],
            prev_output_len: 0,
        }
    }

    fn new_adaptive(mu: f32, filter_len: u32, sample_rate: u32) -> Self {
        let filter_len = (filter_len as usize).clamp(64, MAX_FILTER_LEN);
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let hop_size = FFT_SIZE / 4;
        let n_bins = FFT_SIZE / 2 + 1;

        Self {
            mode: FeedbackMode::Adaptive,
            threshold_db: -30.0,
            notches: std::array::from_fn(|_| AdaptiveNotch::new(sample_rate, 10.0)),
            ring: vec![0.0; FFT_SIZE],
            ring_pos: 0,
            samples_since_analysis: 0,
            hop_size,
            fft,
            fft_buf: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            prev_magnitude: vec![0.0; n_bins],
            adaptive: AdaptiveFir::new(filter_len, mu),
            prev_output: vec![0.0; MAX_FILTER_LEN],
            prev_output_len: 0,
        }
    }

    /// Retrocompatibilidad: crea con los parámetros legacy (solo Notch).
    pub fn new(threshold_db: f32, q: f32, sample_rate: u32) -> Self {
        Self::new_notch(threshold_db, q, sample_rate)
    }

    fn analyze_spectrum(&mut self, sample_rate: u32) {
        for i in 0..FFT_SIZE {
            let window = 0.5 * (1.0 - (2.0 * PI * i as f32 / FFT_SIZE as f32).cos());
            let idx = (self.ring_pos + i) % FFT_SIZE;
            self.fft_buf[i] = Complex::new(self.ring[idx] * window, 0.0);
        }

        self.fft.process(&mut self.fft_buf);

        let n_bins = FFT_SIZE / 2 + 1;
        let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
        let threshold_linear = 10f32.powf(self.threshold_db / 20.0);

        let mut peaks: Vec<(usize, f32)> = Vec::new();
        for i in 1..n_bins {
            let freq_hz = i as f32 * bin_hz;
            if freq_hz < FREQ_MIN_HZ {
                continue;
            }
            let mag = self.fft_buf[i].norm() / FFT_SIZE as f32;
            let smoothed = self.prev_magnitude[i] * 0.7 + mag * 0.3;
            self.prev_magnitude[i] = smoothed;

            if smoothed > threshold_linear {
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

        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        peaks.truncate(MAX_NOTCHES);

        let mut used_notches = [false; MAX_NOTCHES];
        for (bin, _mag) in &peaks {
            let freq_hz = *bin as f32 * bin_hz;
            let slot = self.notches.iter_mut().enumerate().find(|(i, n)| {
                !used_notches[*i] && ((n.freq_hz - freq_hz).abs() < bin_hz * 2.0 || n.depth < 0.01)
            });

            if let Some((idx, notch)) = slot {
                used_notches[idx] = true;
                notch.retune(freq_hz);
                notch.target_depth = 1.0;
            }
        }

        for (i, notch) in self.notches.iter_mut().enumerate() {
            if !used_notches[i] && notch.target_depth > 0.0 {
                notch.target_depth = 0.0;
            }
        }
    }

    fn process_notch(&mut self, input: &[f32], output: &mut [f32], info: &ProcessingInfo) {
        let frames = input.len().min(output.len());
        for i in 0..frames {
            self.ring[self.ring_pos] = input[i];
            self.ring_pos = (self.ring_pos + 1) % FFT_SIZE;

            let mut sample = input[i];
            for notch in &mut self.notches {
                sample = notch.process_sample(sample);
                notch.advance_depth();
            }
            output[i] = sample;

            self.samples_since_analysis += 1;
            if self.samples_since_analysis >= self.hop_size {
                self.samples_since_analysis = 0;
                self.analyze_spectrum(info.sample_rate);
            }
        }
    }

    fn process_adaptive(&mut self, input: &[f32], output: &mut [f32], _info: &ProcessingInfo) {
        let frames = input.len().min(output.len());

        // Cargar la referencia del bloque anterior en el buffer del filtro.
        for i in 0..frames {
            let ref_sample = if i < self.prev_output_len {
                self.prev_output[i]
            } else {
                0.0
            };
            self.adaptive.push_reference(ref_sample);
        }

        // Procesar cada muestra a través del filtro adaptativo.
        for i in 0..frames {
            output[i] = self.adaptive.process_sample(input[i]);
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
        match self.mode {
            FeedbackMode::Notch => self.process_notch(input, output, info),
            FeedbackMode::Adaptive => self.process_adaptive(input, output, info),
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "feedback"
    }

    fn reset(&mut self) {
        match self.mode {
            FeedbackMode::Notch => {
                self.ring.fill(0.0);
                self.ring_pos = 0;
                self.samples_since_analysis = 0;
                self.prev_magnitude.fill(0.0);
                for notch in &mut self.notches {
                    notch.reset();
                }
            }
            FeedbackMode::Adaptive => {
                self.adaptive.reset();
                self.prev_output.fill(0.0);
                self.prev_output_len = 0;
            }
        }
    }

    fn set_output_reference(&mut self, output: &[f32]) {
        if self.mode == FeedbackMode::Adaptive {
            let n = output.len().min(self.prev_output.len());
            self.prev_output[..n].copy_from_slice(&output[..n]);
            self.prev_output_len = n;
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

    fn params_notch() -> FeedbackSuppressorParams {
        FeedbackSuppressorParams {
            mode: FeedbackMode::Notch,
            threshold_db: -30.0,
            q: 10.0,
            mu: 0.1,
            filter_len: 256,
        }
    }

    fn params_adaptive() -> FeedbackSuppressorParams {
        FeedbackSuppressorParams {
            mode: FeedbackMode::Adaptive,
            threshold_db: -30.0,
            q: 10.0,
            mu: 0.15,
            filter_len: 256,
        }
    }

    #[test]
    fn notch_passes_quiet_signal() {
        let mut proc = FeedbackSuppressor::from_params(params_notch(), 48_000);
        let input = vec![0.001; 2048];
        let mut output = vec![0.0; 2048];
        proc.process(&input, &mut output, &info());

        let rms_in: f32 = input.iter().map(|x| x * x).sum::<f32>().sqrt() / input.len() as f32;
        let rms_out: f32 = output.iter().map(|x| x * x).sum::<f32>().sqrt() / output.len() as f32;
        let ratio = rms_out / rms_in;
        assert!(
            ratio > 0.8 && ratio < 1.2,
            "señal silenciosa alterada: ratio={ratio}"
        );
    }

    #[test]
    fn notch_detects_and_suppresses_tone() {
        let sr = 48_000u32;
        let mut proc = FeedbackSuppressor::from_params(params_notch(), sr);
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

        for chunk_start in (0..n).step_by(256) {
            let end = (chunk_start + 256).min(n);
            proc.process(
                &input[chunk_start..end],
                &mut output[chunk_start..end],
                &info,
            );
        }

        let second_half = n / 2;
        let rms_in = rms(&input[second_half..]);
        let rms_out = rms(&output[second_half..]);
        assert!(
            rms_out < rms_in * 0.8,
            "feedback no suprimido (notch): rms_in={rms_in:.4}, rms_out={rms_out:.4}"
        );
    }

    #[test]
    fn adaptive_passes_quiet_signal() {
        let mut proc = FeedbackSuppressor::from_params(params_adaptive(), 48_000);
        let input = vec![0.001; 2048];
        let mut output = vec![0.0; 2048];
        proc.set_output_reference(&vec![0.0; 2048]);
        proc.process(&input, &mut output, &info());

        let rms_in: f32 = input.iter().map(|x| x * x).sum::<f32>().sqrt() / input.len() as f32;
        let rms_out: f32 = output.iter().map(|x| x * x).sum::<f32>().sqrt() / output.len() as f32;
        let ratio = rms_out / rms_in;
        assert!(
            ratio > 0.5 && ratio < 1.5,
            "adaptive altera señal silenciosa: ratio={ratio}"
        );
    }

    #[test]
    fn adaptive_suppresses_simulated_feedback() {
        let sr = 48_000u32;
        let mut proc = FeedbackSuppressor::from_params(params_adaptive(), sr);
        let n = 16384;
        let frames = 256;

        // Simular una ruta de feedback: el altavoz emite un tono y el
        // micrófono lo recibe con un retardo y atenuación.
        let feedback_delay = 32; // samples
        let feedback_gain = 0.8;

        let mut prev_output = vec![0.0f32; n];
        let mut output = vec![0.0f32; n];

        // Generar señal de entrada (micrófono captando voz + feedback).
        let voice: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                0.1 * (2.0 * PI * 300.0 * t).sin() // voz a 300 Hz
            })
            .collect();

        for chunk_start in (0..n).step_by(frames) {
            let end = (chunk_start + frames).min(n);
            let chunk_len = end - chunk_start;

            // La entrada al sistema es voz + feedback retardado de la salida anterior.
            let mut input_chunk = vec![0.0f32; chunk_len];
            for (i, sample) in input_chunk.iter_mut().enumerate() {
                let idx = chunk_start + i;
                let fb_idx = idx.saturating_sub(feedback_delay);
                let feedback = prev_output[fb_idx] * feedback_gain;
                *sample = voice[idx] + feedback;
            }

            // Procesar.
            proc.set_output_reference(&prev_output[chunk_start..end]);
            proc.process(
                &input_chunk,
                &mut output[chunk_start..end],
                &ProcessingInfo {
                    sample_rate: sr,
                    frames: chunk_len,
                },
            );

            // Actualizar prev_output con la salida real (cierra el lazo de feedback).
            prev_output[chunk_start..end].copy_from_slice(&output[chunk_start..end]);
        }

        // Medir la energía de la voz pura vs la salida al final.
        let second_half = n / 2;
        let rms_voice = rms(&voice[second_half..]);
        let rms_out = rms(&output[second_half..]);

        // La salida debe tener energía razonable (no colapsada) pero con
        // el feedback reducido. Verificar que no colapsa.
        assert!(
            rms_out > rms_voice * 0.01,
            "adaptive colapsa la señal: rms_voice={rms_voice:.4}, rms_out={rms_out:.4}"
        );
    }

    #[test]
    fn reset_clears_state_notch() {
        let mut proc = FeedbackSuppressor::from_params(params_notch(), 48_000);
        let input = vec![0.5; 4096];
        let mut output = vec![0.0; 4096];
        proc.process(&input, &mut output, &info());

        proc.reset();
        assert_eq!(proc.ring_pos, 0);
        assert_eq!(proc.samples_since_analysis, 0);
    }

    #[test]
    fn reset_clears_state_adaptive() {
        let mut proc = FeedbackSuppressor::from_params(params_adaptive(), 48_000);
        let input = vec![0.5; 4096];
        let mut output = vec![0.0; 4096];
        proc.set_output_reference(&vec![0.5; 4096]);
        proc.process(&input, &mut output, &info());

        proc.reset();
        assert_eq!(proc.prev_output_len, 0);
        assert!(proc.adaptive.weights.iter().all(|&w| w == 0.0));
    }

    #[test]
    fn name_is_feedback() {
        let proc = FeedbackSuppressor::from_params(params_notch(), 48_000);
        assert_eq!(proc.name(), "feedback");
    }

    #[test]
    fn from_params_creates_correct_mode() {
        let p_notch = FeedbackSuppressor::from_params(params_notch(), 48_000);
        assert_eq!(p_notch.mode, FeedbackMode::Notch);

        let p_adaptive = FeedbackSuppressor::from_params(params_adaptive(), 48_000);
        assert_eq!(p_adaptive.mode, FeedbackMode::Adaptive);
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }
}
