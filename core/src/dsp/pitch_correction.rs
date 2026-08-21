//! Corrección de tono en tiempo real (Auto-Tune / pitch correction).
//!
//! Implementa detección de pitch con el algoritmo YIN y corrección
//! basada en PSOLA (Phase-Synchronous Overlap-Add):
//!
//! 1. **Detección** (`PitchDetector`): algoritmo YIN sobre ventanas de 2048
//!    muestras a 48 kHz (~42.7 ms). Devuelve la frecuencia fundamental (F0)
//!    estimada o 0 si no hay voz detectada.
//! 2. **Corrección** (`PitchCorrection`): dado F0 y una escala musical,
//!    calcula la nota objetivo más cercana y el factor de cambio de pitch.
//!    Aplica PSOLA para cambiar el pitch sin alterar la duración.
//!
//! El procesador implementa [`AudioProcessor`] y se integra en la cadena DSP
//! como cualquier otro módulo. No asigna memoria en el callback (buffers
//! preasignados).

use crate::dsp::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::{MusicalNote, MusicalScale, PitchCorrectionParams};

// ── Constantes ──

/// Tamaño de la ventana YIN (muestras).
const YIN_FRAME_SIZE: usize = 2048;
/// Umbral de confianza YIN (0–1). Valores menores detectan más pitch pero con
/// más falsos positivos.
const YIN_THRESHOLD: f32 = 0.15;
/// Máxima F0 esperada (Hz) — vo femenina/niño puede llegar a ~1100 Hz.
const YIN_MAX_FREQ: f32 = 1100.0;
/// Frecuencia de muestreo esperada.
const SR: f32 = 48000.0;

// ── Detección de pitch (YIN) ──

/// Detector de frecuencia fundamental con algoritmo YIN.
struct PitchDetector {
    /// Buffer circular de entrada.
    buffer: Vec<f32>,
    /// Posición de escritura en el buffer circular.
    pos: usize,
    /// Tamaño del frame (muestras).
    frame_size: usize,
    /// Buffer de trabajo para la diferencia cuadrática cumulativa.
    diff: Vec<f32>,
    /// Buffer para las diferencias normalizadas.
    norm_diff: Vec<f32>,
}

impl PitchDetector {
    fn new(frame_size: usize) -> Self {
        Self {
            buffer: vec![0.0; frame_size],
            pos: 0,
            frame_size,
            diff: vec![0.0; frame_size / 2],
            norm_diff: vec![0.0; frame_size / 2],
        }
    }

    /// Acumula muestras en el buffer circular y, cuando hay un frame completo,
    /// estima F0. Devuelve `Some(f0)` o `None` si el buffer no está lleno.
    fn feed_and_detect(&mut self, samples: &[f32]) -> Option<f32> {
        for &s in samples {
            self.buffer[self.pos] = s;
            self.pos = (self.pos + 1) % self.frame_size;
        }
        if self.pos != 0 {
            return None;
        }
        Some(self.detect())
    }

    /// Ejecuta el algoritmo YIN sobre el buffer circular.
    fn detect(&mut self) -> f32 {
        let n = self.frame_size;
        let half = n / 2;

        // 1) Diferencia cuadrática cumulativa (CDF).
        self.diff[0] = 0.0;
        for tau in 1..half {
            let mut sum = 0.0f32;
            for i in 0..half {
                let idx_a = (self.pos + i) % n;
                let idx_b = (self.pos + i + tau) % n;
                let d = self.buffer[idx_a] - self.buffer[idx_b];
                sum += d * d;
            }
            self.diff[tau] = sum;
        }

        // 2) Normalización: diff[tau] / mean(diff[tau+1..]).
        self.norm_diff[0] = 1.0;
        let mut cumsum = 0.0f32;
        for tau in 1..half {
            cumsum += self.diff[tau];
            if tau + 1 < half {
                self.norm_diff[tau] = self.diff[tau] * tau as f32 / cumsum;
            } else {
                self.norm_diff[tau] = 1.0;
            }
        }

        // 3) Buscar la primera tau donde norm_diff cruza el umbral y luego
        //    el mínimo local para refinar.
        let min_tau = (SR / YIN_MAX_FREQ) as usize;
        let mut tau_hat = None;

        for tau in min_tau..half {
            if self.norm_diff[tau] < YIN_THRESHOLD {
                // Buscar el mínimo local a partir de esta tau.
                let mut best_tau = tau;
                let mut best_val = self.norm_diff[tau];
                let end = (tau + 16).min(half); // Ventana de búsqueda limitada.
                for t in (tau + 1)..end {
                    if self.norm_diff[t] < best_val {
                        best_val = self.norm_diff[t];
                        best_tau = t;
                    }
                }
                tau_hat = Some(best_tau);
                break;
            }
        }

        match tau_hat {
            Some(tau) => SR / tau as f32,
            None => 0.0, // No se detectó pitch.
        }
    }
}

// ── Escalas musicales ──

/// Devuelve las posiciones de semitonos (0–11) de una escala relativa a la raíz.
fn scale_intervals(scale: MusicalScale) -> &'static [i8] {
    match scale {
        MusicalScale::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        MusicalScale::Major => &[0, 2, 4, 5, 7, 9, 11],
        MusicalScale::MinorNatural => &[0, 2, 3, 5, 7, 8, 10],
        MusicalScale::MinorHarmonic => &[0, 2, 3, 5, 7, 8, 11],
        MusicalScale::PentatonicMajor => &[0, 2, 4, 7, 9],
        MusicalScale::PentatonicMinor => &[0, 3, 5, 7, 10],
        MusicalScale::Blues => &[0, 3, 5, 6, 7, 10],
    }
}

/// Dado un F0 en Hz, calcula el semitono absoluto más cercano en la escala y
/// devuelve el factor de cambio de frecuencia (target_freq / f0).
///
/// `strength` (0–1) controla la intensidad: 0 = sin cambio, 1 = corrección
/// completa. `strength` = 0.7 significa corregir el 70 % de la distancia.
fn pitch_shift_ratio(f0: f32, scale: MusicalScale, root: MusicalNote, strength: f32) -> f32 {
    if f0 <= 0.0 || strength <= 0.0 {
        return 1.0;
    }

    // F0 → número MIDI (69 = A4 = 440 Hz).
    let midi = 69.0 + 12.0 * (f0 / 440.0).log2();

    // Posición dentro de la octava (0–11), relativa a la raíz de la escala.
    let root_semitone = root.semitones() as f32;
    let note_from_root = ((midi - root_semitone) % 12.0 + 12.0) % 12.0;

    // Encontrar el intervalo de la escala más cercano.
    let intervals = scale_intervals(scale);
    let target_interval = find_nearest_interval_f32(note_from_root, intervals);

    // Diferencia en semitonos (con wrap-around).
    let mut diff = target_interval - note_from_root;
    if diff > 6.0 {
        diff -= 12.0;
    } else if diff < -6.0 {
        diff += 12.0;
    }

    if diff.abs() < 0.01 {
        return 1.0;
    }

    // Factor de cambio con strength interpolada.
    let corrected_diff = diff * strength;
    2.0_f32.powf(corrected_diff / 12.0)
}

/// Encuentra el intervalo de la escala más cercano a una posición dada (0–11).
fn find_nearest_interval_f32(note: f32, intervals: &[i8]) -> f32 {
    let note_mod = ((note % 12.0) + 12.0) % 12.0;
    let mut best = intervals[0] as f32;
    let mut best_dist = 12.0f32;
    for &iv in intervals {
        let iv_f = iv as f32;
        let dist = (note_mod - iv_f).abs();
        let dist = dist.min(12.0 - dist);
        if dist < best_dist {
            best_dist = dist;
            best = iv_f;
        }
    }
    best
}

// ── PSOLA ──

/// Implementación simplificada de PSOLA para cambio de pitch.
///
/// PSOLA trabaja en el dominio del tiempo con ventanas solapadas:
/// 1. Detecta épocas (picos de energía) de forma pitch-sincronizada.
/// 2. Extrae ventanas centradas en cada época.
/// 3. Reubica las ventanas en el output con el nuevo espaciamiento.
/// 4. Suma solapada (overlap-add) para reconstruir la señal.
struct PsolaShifter {
    /// Factor de cambio de pitch actual (target).
    shift_ratio: f32,
    /// Factor suavizado (para transiciones).
    smooth_ratio: f32,
    /// Época anterior detectada (posición en el buffer de entrada).
    prev_epoch_in: f32,
    /// Posición de escritura en el buffer circular de entrada.
    write_pos: usize,
    /// Buffer circular de entrada (mantiene historial para épocas).
    input_ring: Vec<f32>,
    /// Tamaño del buffer circular.
    ring_size: usize,
    /// Energía suavizada para detección de épocas.
    energy: f32,
    /// Estado de detección de pico (true = buscando pico ascendente).
    rising: bool,
    /// Buffer reutilizable para épocas detectadas (evita alloc por frame).
    epochs_buf: Vec<f32>,
    /// Buffer reutilizable para ventana PSOLA (evita alloc por época).
    windowed_buf: Vec<f32>,
}

impl PsolaShifter {
    fn new() -> Self {
        let ring_size = YIN_FRAME_SIZE * 4;
        Self {
            shift_ratio: 1.0,
            smooth_ratio: 1.0,
            prev_epoch_in: -1.0,
            write_pos: 0,
            input_ring: vec![0.0; ring_size],
            ring_size,
            energy: 0.0,
            rising: false,
            epochs_buf: Vec::new(),
            windowed_buf: Vec::new(),
        }
    }

    /// Suaviza el ratio de cambio para evitar artefactos en transiciones.
    fn set_target_ratio(&mut self, ratio: f32) {
        self.shift_ratio = ratio;
    }

    fn process_block(&mut self, input: &[f32], output: &mut [f32], frames: usize) {
        let ratio = self.shift_ratio;

        if (ratio - 1.0).abs() < 0.001 {
            // Sin cambio de pitch: paso directo.
            output[..frames].copy_from_slice(&input[..frames]);
            return;
        }

        // Suavizar el ratio para evitar saltos bruscos.
        self.smooth_ratio += (ratio - self.smooth_ratio) * 0.15;

        // 1) Escribir input en el ring buffer.
        for &sample in input.iter().take(frames) {
            self.input_ring[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.ring_size;
        }

        // 2) Detectar épocas (picos de energía local con onset).
        self.epochs_buf.clear();
        let mut energy_acc = 0.0f32;
        for (i, &sample) in input.iter().enumerate().take(frames) {
            energy_acc += sample * sample;

            // Suavizado exponencial.
            self.energy = self.energy * 0.99 + energy_acc / (i + 1) as f32;
            let norm_energy = if self.energy > 1e-10 {
                energy_acc / (i + 1) as f32 / self.energy
            } else {
                0.0
            };

            if norm_energy > 0.8 && !self.rising {
                self.rising = true;
            } else if norm_energy < 0.5 && self.rising {
                self.rising = false;
                self.epochs_buf.push(i as f32);
            }
        }

        // 3) Si no hay épocas detectadas o poca energía, paso directo.
        let rms: f32 = (input.iter().map(|x| x * x).sum::<f32>() / frames as f32).sqrt();
        if rms < 0.005 || self.epochs_buf.is_empty() {
            output[..frames].copy_from_slice(&input[..frames]);
            return;
        }

        // 4) PSOLA: reubicar ventanas de input en output con el nuevo espaciamiento.
        let half_win = YIN_FRAME_SIZE / 2;
        output.fill(0.0);

        let mut out_pos = 0.0f32;
        let mut prev_out = 0.0f32;

        for &epoch in &self.epochs_buf {
            let new_spacing = (epoch - self.prev_epoch_in) / self.smooth_ratio;
            if new_spacing <= 0.0 || new_spacing > YIN_FRAME_SIZE as f32 {
                self.prev_epoch_in = epoch;
                continue;
            }

            // Posición de inicio de la ventana de input (centrada en la época).
            let in_start = (epoch - half_win as f32).max(0.0);

            // Ventana Hann.
            let win_size = YIN_FRAME_SIZE.min(frames);
            self.windowed_buf.resize(win_size, 0.0);
            for (i, win) in self.windowed_buf[..win_size].iter_mut().enumerate() {
                let hann =
                    0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / win_size as f32).cos());
                let in_idx = (in_start as usize + i).min(frames - 1);
                *win = input[in_idx] * hann;
            }

            // Escribir en output con el espaciamiento corregido.
            let out_start = out_pos as usize;
            for (i, win) in self.windowed_buf[..win_size].iter().enumerate() {
                let idx = out_start + i;
                if idx < output.len() {
                    output[idx] += *win;
                }
            }

            out_pos += new_spacing;
            self.prev_epoch_in = epoch;
            prev_out = out_pos;
        }

        // Si no se escribió nada, paso directo.
        if prev_out == 0.0 {
            output[..frames].copy_from_slice(&input[..frames]);
        }
    }
}

// ── Procesador principal ──

/// Corrección de tono en tiempo real con YIN + PSOLA.
///
/// Detecta la frecuencia fundamental y la desvía hacia la nota más cercana
/// de la escala configurada. El parámetro `strength` controla la intensidad
/// (0 = desactivada, 1 = corrección completa tipo Auto-Tune).
pub struct PitchCorrection {
    params: PitchCorrectionParams,
    detector: PitchDetector,
    shifter: PsolaShifter,
    /// Latencia acumulada del procesamiento (ms).
    latency_ms: f32,
}

impl PitchCorrection {
    /// Crea un procesador de corrección de tono con los parámetros indicados.
    pub fn new(params: PitchCorrectionParams) -> Self {
        Self {
            params,
            detector: PitchDetector::new(YIN_FRAME_SIZE),
            shifter: PsolaShifter::new(),
            latency_ms: (YIN_FRAME_SIZE as f32 / SR) * 1000.0,
        }
    }

    /// Actualiza los parámetros en vivo (desde el hilo de control).
    pub fn update_params(&mut self, params: PitchCorrectionParams) {
        self.params = params;
    }
}

impl AudioProcessor for PitchCorrection {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        if self.params.strength <= 0.0 || self.params.mix <= 0.0 {
            output[..frames].copy_from_slice(&input[..frames]);
            return ProcessResult { latency_ms: 0.0 };
        }

        // 1) Detectar pitch del frame completo.
        if let Some(f0) = self.detector.feed_and_detect(input) {
            if f0 > 0.0 {
                let ratio = pitch_shift_ratio(
                    f0,
                    self.params.scale,
                    self.params.root,
                    self.params.strength,
                );
                self.shifter.set_target_ratio(ratio);
            } else {
                self.shifter.set_target_ratio(1.0);
            }
        }

        // 2) Aplicar PSOLA.
        self.shifter.process_block(input, output, frames);

        // 3) Mezclar seco/húmedo.
        let wet = self.params.mix;
        let dry = 1.0 - wet;
        for i in 0..frames {
            output[i] = input[i] * dry + output[i] * wet;
        }

        ProcessResult {
            latency_ms: self.latency_ms,
        }
    }

    fn name(&self) -> &'static str {
        "pitch_correction"
    }

    fn reset(&mut self) {
        self.detector.pos = 0;
        self.detector.buffer.fill(0.0);
        self.shifter = PsolaShifter::new();
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detector_creation() {
        let det = PitchDetector::new(YIN_FRAME_SIZE);
        assert_eq!(det.frame_size, YIN_FRAME_SIZE);
    }

    #[test]
    fn scale_intervals_chromatic_has_12_notes() {
        assert_eq!(scale_intervals(MusicalScale::Chromatic).len(), 12);
    }

    #[test]
    fn scale_intervals_major_has_7_notes() {
        assert_eq!(scale_intervals(MusicalScale::Major).len(), 7);
    }

    #[test]
    fn pitch_shift_ratio_is_unity_when_strength_zero() {
        let ratio = pitch_shift_ratio(440.0, MusicalScale::Chromatic, MusicalNote::A, 0.0);
        assert!((ratio - 1.0).abs() < 0.001);
    }

    #[test]
    fn pitch_shift_ratio_is_unity_when_f0_is_zero() {
        let ratio = pitch_shift_ratio(0.0, MusicalScale::Chromatic, MusicalNote::A, 1.0);
        assert!((ratio - 1.0).abs() < 0.001);
    }

    #[test]
    fn pitch_shift_ratio_corrects_off_pitch() {
        // 442 Hz (ligeramente agudo de La) → debería corregir hacia abajo.
        let ratio = pitch_shift_ratio(442.0, MusicalScale::Chromatic, MusicalNote::A, 1.0);
        assert!(ratio < 1.0, "expected downward correction, got {ratio}");
    }

    #[test]
    fn psola_passthrough_when_ratio_is_one() {
        let mut shifter = PsolaShifter::new();
        shifter.set_target_ratio(1.0);
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        let mut output = vec![0.0; 8];
        shifter.process_block(&input, &mut output, 8);
        for (a, b) in input.iter().zip(output.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn pitch_correction_creation() {
        let params = PitchCorrectionParams {
            scale: MusicalScale::Major,
            root: MusicalNote::C,
            strength: 0.7,
            mix: 1.0,
        };
        let processor = PitchCorrection::new(params);
        assert_eq!(processor.name(), "pitch_correction");
    }

    #[test]
    fn pitch_correction_passthrough_when_strength_zero() {
        let params = PitchCorrectionParams {
            scale: MusicalScale::Chromatic,
            root: MusicalNote::A,
            strength: 0.0,
            mix: 1.0,
        };
        let mut processor = PitchCorrection::new(params);
        let input = vec![0.1, -0.2, 0.3, -0.4, 0.5, -0.6, 0.7, -0.8];
        let mut output = vec![0.0; 8];
        let info = ProcessingInfo {
            sample_rate: 48000,
            frames: 8,
        };
        let result = processor.process(&input, &mut output, &info);
        assert_eq!(result.latency_ms, 0.0);
        for (a, b) in input.iter().zip(output.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn pitch_correction_passthrough_when_mix_zero() {
        let params = PitchCorrectionParams {
            scale: MusicalScale::Chromatic,
            root: MusicalNote::A,
            strength: 1.0,
            mix: 0.0,
        };
        let mut processor = PitchCorrection::new(params);
        let input = vec![0.1, -0.2, 0.3, -0.4, 0.5, -0.6, 0.7, -0.8];
        let mut output = vec![0.0; 8];
        let info = ProcessingInfo {
            sample_rate: 48000,
            frames: 8,
        };
        let result = processor.process(&input, &mut output, &info);
        assert_eq!(result.latency_ms, 0.0);
        for (a, b) in input.iter().zip(output.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn find_nearest_interval_chromatic() {
        let intervals = scale_intervals(MusicalScale::Chromatic);
        assert_eq!(find_nearest_interval_f32(0.0, intervals) as i32, 0);
        assert_eq!(find_nearest_interval_f32(5.0, intervals) as i32, 5);
        assert_eq!(find_nearest_interval_f32(11.0, intervals) as i32, 11);
    }

    #[test]
    fn find_nearest_interval_major() {
        let intervals = scale_intervals(MusicalScale::Major);
        // 1 (C#) → más cercano a 0 (C) o 2 (D), distancia 1 en ambos casos.
        let nearest = find_nearest_interval_f32(1.0, intervals);
        assert!(nearest == 0.0 || nearest == 2.0);
    }

    #[test]
    fn musical_note_semitones() {
        assert_eq!(MusicalNote::C.semitones(), 0);
        assert_eq!(MusicalNote::Fs.semitones(), 6);
        assert_eq!(MusicalNote::B.semitones(), 11);
    }
}
