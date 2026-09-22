//! Harmonizer vocal en tiempo real.
//!
//! Genera copias de la voz desplazadas por intervalos musicales utilizando
//! un pitch shifter basado en delay-lines con crossfade triangular.
//!
//! El algoritmo:
//! 1. Para cada intervalo (semitonos), calcula el factor de pitch ratio.
//! 2. Cada voice usa un delay-line circular con crossfade para cambiar el
//!    pitch sin alterar la duración.
//! 3. Las voces se mezclan con la señal seca según el parámetro `mix`.

use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::HarmonizerParams;

/// Número máximo de intervalos soportados.
const MAX_INTERVALS: usize = 8;

/// Número máximo de copias por intervalo.
const MAX_VOICES_PER_INTERVAL: usize = 4;

/// Rango máximo de desplazamiento en semitonos (±4 octavas). Valores fuera de
/// este rango no tienen sentido musical y producen tasas de lectura que pueden
/// desbordar `f32` a infinito, colgando el hilo de audio en el bucle de avance.
const MAX_SEMITONES: i32 = 48;

/// Longitud del buffer del pitch shifter (muestras). Debe cubrir al menos
/// 50 ms a 48 kHz para evitar artefactos en notas graves.
const SHIFTER_BUF_LEN: usize = 4096;

/// Pitch shifter individual basado en delay-line con crossfade triangular.
///
/// Implementa el algoritmo clásico de pitch shifting por delay-line:
/// dos líneas de retardo leen la señal de entrada a diferentes tasas y se
/// cruzan con una envolvente triangular para producir una salida con pitch
/// alterado sin cambiar la duración.
struct PitchShifter {
    /// Buffer circular de entrada.
    buf: Vec<f32>,
    /// Posición de escritura en el buffer.
    write_pos: usize,
    /// Tasa de lectura relativa (1.0 = sin cambio de pitch).
    read_rate: f32,
    /// Posición de lectura (fraccionaria).
    read_pos: f32,
    /// Offset entre las dos líneas de delay (mitad del periodo de crossfade).
    delay_offset: f32,
    /// Fase del crossfade triangular (0..1).
    crossfade_phase: f32,
    /// Tasa de avance del crossfade por muestra.
    crossfade_inc: f32,
}

impl PitchShifter {
    fn new(semitones: i32, sample_rate: u32) -> Self {
        let ratio = 2.0f32.powf(semitones as f32 / 12.0);
        let delay_offset = SHIFTER_BUF_LEN as f32 / 2.0;
        // Periodo de crossfade: ~20 ms a 48 kHz ≈ 960 muestras.
        let crossfade_period = sample_rate as f32 * 0.02;
        Self {
            buf: vec![0.0; SHIFTER_BUF_LEN],
            write_pos: 0,
            read_rate: ratio,
            read_pos: 0.0,
            delay_offset,
            crossfade_phase: 0.0,
            crossfade_inc: 1.0 / crossfade_period,
        }
    }

    fn process_sample(&mut self, input: f32) -> f32 {
        let len = self.buf.len() as f32;

        // Defensa en profundidad: un read_rate no finito (p. ej. por NaN
        // propagado) colgaría el callback en el bucle de avance de `read_pos`.
        if !self.read_rate.is_finite() {
            return 0.0;
        }

        // Escribir entrada en el buffer circular.
        self.buf[self.write_pos] = input;
        self.write_pos = (self.write_pos + 1) % self.buf.len();

        // Calcular las dos posiciones de lectura con crossfade triangular.
        let read_a = self.read_pos;
        let read_b = (self.read_pos + self.delay_offset) % len;

        // Interpolación lineal para cada posición.
        let sample_a = self.interpolate(read_a);
        let sample_b = self.interpolate(read_b);

        // Envolvente triangular: asciende de 0→1 y desciende de 1→0.
        let tri = if self.crossfade_phase < 0.5 {
            self.crossfade_phase * 2.0
        } else {
            2.0 - self.crossfade_phase * 2.0
        };

        // Mezcla las dos líneas de delay.
        let output = sample_a * (1.0 - tri) + sample_b * tri;

        // Avanzar la posición de lectura y el crossfade.
        self.read_pos += self.read_rate;
        while self.read_pos >= len {
            self.read_pos -= len;
        }
        while self.read_pos < 0.0 {
            self.read_pos += len;
        }

        self.crossfade_phase += self.crossfade_inc;
        if self.crossfade_phase >= 1.0 {
            self.crossfade_phase -= 1.0;
        }

        output
    }

    /// Interpola linealmente entre dos muestras del buffer.
    fn interpolate(&self, pos: f32) -> f32 {
        let len = self.buf.len();
        let idx = pos as usize;
        let frac = pos - idx as f32;
        let a = self.buf[idx % len];
        let b = self.buf[(idx + 1) % len];
        a + (b - a) * frac
    }

    fn reset(&mut self) {
        self.buf.fill(0.0);
        self.write_pos = 0;
        self.read_pos = 0.0;
        self.crossfade_phase = 0.0;
    }
}

/// Una voz de harmonía: intervalo + nivel + pitch shifter.
struct HarmonyVoice {
    _semitones: i32,
    level: f32,
    shifter: PitchShifter,
}

/// Harmonizer vocal: genera múltiples voces de harmonía.
pub struct Harmonizer {
    /// Voces activas (hasta MAX_INTERVALS × MAX_VOICES_PER_INTERVAL).
    voices: Vec<HarmonyVoice>,
    /// Mezcla seco/húmedo.
    mix: f32,
    /// Sample rate.
    _sample_rate: u32,
    /// Buffer reutilizado para acumular la señal de harmonía (evita
    /// asignaciones en el callback de audio).
    harmony_buf: Vec<f32>,
}

impl Harmonizer {
    /// Crea un nuevo harmonizer con los parámetros indicados.
    pub fn from_params(params: &HarmonizerParams, sample_rate: u32) -> Self {
        let mut voices = Vec::new();
        let voices_per = params
            .voices_per_interval
            .clamp(1, MAX_VOICES_PER_INTERVAL as u32);

        for &semitones in &params.intervals {
            if voices.len() >= MAX_INTERVALS * MAX_VOICES_PER_INTERVAL {
                break;
            }
            // Clamp de seguridad: evita read_rate = Inf y el bucle infinito en
            // el callback de audio si llega un intervalo descontrolado por red.
            let semitones = semitones.clamp(-MAX_SEMITONES, MAX_SEMITONES);
            for v in 0..voices_per {
                // Pequeño detuning entre copias para crear efecto de coro (±5 cents).
                let detune_cents = if voices_per > 1 {
                    ((v as f32 / (voices_per - 1) as f32) - 0.5) * 10.0
                } else {
                    0.0
                };
                let detune_semitones = semitones as f32 + detune_cents / 100.0;
                let level = 1.0 / voices_per as f32;
                voices.push(HarmonyVoice {
                    _semitones: detune_semitones as i32,
                    level,
                    shifter: PitchShifter::new(detune_semitones as i32, sample_rate),
                });
            }
        }

        Self {
            voices,
            mix: params.mix,
            _sample_rate: sample_rate,
            harmony_buf: Vec::new(),
        }
    }

    /// Retrocompatibilidad: crea con parámetros legacy.
    pub fn new(intervals: Vec<i32>, mix: f32, sample_rate: u32) -> Self {
        let params = HarmonizerParams {
            intervals,
            mix,
            voices_per_interval: 1,
        };
        Self::from_params(&params, sample_rate)
    }
}

impl AudioProcessor for Harmonizer {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        let n_voices = self.voices.len();

        if n_voices == 0 || self.mix <= 0.0 {
            output[..frames].copy_from_slice(&input[..frames]);
            return ProcessResult { latency_ms: 0.0 };
        }

        // Señal de harmonía acumulada (reutiliza buffer preasignado).
        self.harmony_buf.clear();
        self.harmony_buf.resize(frames, 0.0);
        let harmony = &mut self.harmony_buf;

        for voice in &mut self.voices {
            for i in 0..frames {
                harmony[i] += voice.shifter.process_sample(input[i]) * voice.level;
            }
        }

        // Normalizar la harmonía por número de voces.
        let norm = 1.0 / n_voices as f32;

        // Mezclar seco + harmonía.
        let dry = 1.0 - self.mix;
        for i in 0..frames {
            output[i] = input[i] * dry + harmony[i] * norm * self.mix;
        }

        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "harmonizer"
    }

    fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.shifter.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn info() -> ProcessingInfo {
        ProcessingInfo {
            sample_rate: 48_000,
            frames: 256,
        }
    }

    #[test]
    fn harmonizer_passes_quiet_signal() {
        let params = HarmonizerParams {
            intervals: vec![0],
            mix: 0.5,
            voices_per_interval: 1,
        };
        let mut proc = Harmonizer::from_params(&params, 48_000);
        let input = vec![0.001; 2048];
        let mut output = vec![0.0; 2048];
        proc.process(&input, &mut output, &info());

        let rms_in: f32 = input.iter().map(|x| x * x).sum::<f32>().sqrt() / input.len() as f32;
        let rms_out: f32 = output.iter().map(|x| x * x).sum::<f32>().sqrt() / output.len() as f32;
        let ratio = rms_out / rms_in;
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "señal silenciosa alterada: ratio={ratio}"
        );
    }

    #[test]
    fn harmonizer_mix_zero_is_passthrough() {
        let params = HarmonizerParams {
            intervals: vec![-12, 12],
            mix: 0.0,
            voices_per_interval: 1,
        };
        let mut proc = Harmonizer::from_params(&params, 48_000);
        let input = vec![0.3; 2048];
        let mut output = vec![0.0; 2048];
        proc.process(&input, &mut output, &info());

        for (a, b) in input.iter().zip(output.iter()) {
            assert!((a - b).abs() < 1e-6, "mix=0 debería ser passthrough");
        }
    }

    #[test]
    fn harmonizer_no_intervals_is_passthrough() {
        let params = HarmonizerParams {
            intervals: vec![],
            mix: 0.5,
            voices_per_interval: 1,
        };
        let mut proc = Harmonizer::from_params(&params, 48_000);
        let input = vec![0.3; 2048];
        let mut output = vec![0.0; 2048];
        proc.process(&input, &mut output, &info());

        for (a, b) in input.iter().zip(output.iter()) {
            assert!(
                (a - b).abs() < 1e-6,
                "sin intervalos debería ser passthrough"
            );
        }
    }

    #[test]
    fn harmonizer_produces_output() {
        let params = HarmonizerParams {
            intervals: vec![-12, -7, 0, 5, 7, 12],
            mix: 0.3,
            voices_per_interval: 1,
        };
        let mut proc = Harmonizer::from_params(&params, 48_000);

        // Generar un tono de prueba (440 Hz).
        let sr = 48_000u32;
        let n = 8192;
        let input: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                0.5 * (2.0 * PI * 440.0 * t).sin()
            })
            .collect();
        let mut output = vec![0.0; n];

        for chunk_start in (0..n).step_by(256) {
            let end = (chunk_start + 256).min(n);
            proc.process(
                &input[chunk_start..end],
                &mut output[chunk_start..end],
                &info(),
            );
        }

        // La salida debe tener energía (no es silenciosa).
        let rms_out: f32 = output.iter().map(|x| x * x).sum::<f32>().sqrt() / n as f32;
        assert!(
            rms_out > 0.001,
            "harmonizer produce silencio: rms={rms_out}"
        );
    }

    #[test]
    fn harmonizer_modifies_signal() {
        let sr = 48_000u32;
        let n = 4096;
        let input: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                0.5 * (2.0 * PI * 440.0 * t).sin()
            })
            .collect();

        let params = HarmonizerParams {
            intervals: vec![-12, 12],
            mix: 0.5,
            voices_per_interval: 1,
        };
        let mut proc = Harmonizer::from_params(&params, sr);
        let mut output = vec![0.0; n];
        proc.process(&input, &mut output, &info());

        // La salida difiere de la entrada (las voces modifican la señal).
        let diff: f32 = input
            .iter()
            .zip(output.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / n as f32;
        assert!(
            diff > 0.001,
            "la salida debería diferir de la entrada: diff={diff}"
        );

        // La salida no es silenciosa.
        let rms_out: f32 = output.iter().map(|x| x * x).sum::<f32>().sqrt() / n as f32;
        assert!(
            rms_out > 0.001,
            "harmonizer produce silencio: rms={rms_out}"
        );
    }

    #[test]
    fn extreme_intervals_do_not_hang_or_panic() {
        // Antes del clamp este caso producía read_rate = Inf y un bucle
        // infinito en el callback de audio (DoS).
        let params = HarmonizerParams {
            intervals: vec![i32::MAX, i32::MIN],
            mix: 0.5,
            voices_per_interval: 4,
        };
        let mut proc = Harmonizer::from_params(&params, 48_000);
        let input = vec![0.1; 2048];
        let mut output = vec![0.0; 2048];
        proc.process(&input, &mut output, &info());

        for (a, b) in input.iter().zip(output.iter()) {
            assert!(a.is_finite() && b.is_finite(), "salida no finita");
        }
    }

    #[test]
    fn name_is_harmonizer() {
        let params = HarmonizerParams::default();
        let proc = Harmonizer::from_params(&params, 48_000);
        assert_eq!(proc.name(), "harmonizer");
    }

    #[test]
    fn reset_clears_state() {
        let params = HarmonizerParams {
            intervals: vec![-12, 12],
            mix: 0.5,
            voices_per_interval: 1,
        };
        let mut proc = Harmonizer::from_params(&params, 48_000);
        let input = vec![0.5; 4096];
        let mut output = vec![0.0; 4096];
        proc.process(&input, &mut output, &info());

        proc.reset();
        for voice in &mut proc.voices {
            assert!(voice.shifter.buf.iter().all(|&v| v == 0.0));
        }
    }
}
