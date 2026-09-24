//! Aislamiento de voz en tiempo real (comb armónico guiado por YIN).
//!
//! Detecta la frecuencia fundamental (F0) de la voz con el mismo detector YIN
//! que la corrección de tono y aplica un comb armónico desplazado exactamente
//! un periodo de voz:
//!
//! ```text
//!   y[n] = (x[n] + a · x[n − D]) / (1 + a)
//!   D    = fs / F0            (suavizado, con interpolación fraccional)
//!   a    = strength · voiced  (envuelta que activa solo cuando hay voz)
//! ```
//!
//! La corrección es causal y usa solo muestras pasadas: **no añade latencia**
//! y no asigna memoria en el callback (ring preasignado). Cuando `F0 = 0`
//! (silencios / pasajes no vocales) la intensidad cae a 0 y el módulo se
//! comporta como paso directo.

use crate::dsp::pitch_correction::PitchDetector;
use crate::dsp::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::VocalIsolationParams;

/// Frecuencia mínima esperada de la voz (Hz) para dimensionar el delay máximo.
const MIN_F0_HZ: f32 = 60.0;
/// Frecuencia máxima esperada de la voz (Hz).
const MAX_F0_HZ: f32 = 1100.0;
/// Velocidad de ataque de la envuelta de voz (0–1 por corrección).
const VOICE_ATTACK: f32 = 0.5;
/// Velocidad de liberación de la envuelta de voz (0–1 por corrección).
const VOICE_RELEASE: f32 = 0.08;
/// Velocidad de seguimiento del retardo suavizado (0–1 por corrección).
const DELAY_SMOOTH: f32 = 0.2;
/// Intensidad máxima del comb (0–1) para evitar sobre-amplificación.
const MAX_COMB_GAIN: f32 = 0.9;

/// Tamaño del frame del detector (debe coincidir con el de corrección de tono).
const DETECT_FRAME: usize = 2048;

/// Línea de retardo circular con lectura fraccional (interpolada).
struct FractionalDelay {
    /// Buffer circular preasignado.
    buffer: Vec<f32>,
    /// Posición de escritura.
    pos: usize,
}

impl FractionalDelay {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            pos: 0,
        }
    }

    /// Escribe una muestra y devuelve la muestra retardada `delay` muestras
    /// atrás (con interpolación lineal para retardos fraccionales).
    fn process(&mut self, sample: f32, delay: f32) -> f32 {
        self.buffer[self.pos] = sample;

        let max_delay = (self.buffer.len() - 1) as f32;
        let d = delay.clamp(1.0, max_delay);

        // Índice de lectura con interpolación lineal.
        let read = self.pos as f32 - d;
        let i0 = read.floor() as isize;
        let frac = read - i0 as f32;
        let len = self.buffer.len() as isize;

        // Posiciones envueltas (0..len).
        let a_idx = if i0 >= 0 {
            i0 as usize
        } else {
            (i0 + len) as usize
        };
        let a = a_idx % self.buffer.len();
        let b = (a + 1) % self.buffer.len();
        let va = self.buffer[a];
        let vb = self.buffer[b];

        self.pos = (self.pos + 1) % self.buffer.len();
        va + (vb - va) * frac
    }
}

/// Procesador de aislamiento de voz en tiempo real.
pub struct VocalIsolation {
    params: VocalIsolationParams,
    /// Detector YIN de la frecuencia fundamental.
    detector: PitchDetector,
    /// Línea de retardo del comb.
    delay_line: FractionalDelay,
    /// Última F0 detectada (Hz); 0 = no hay voz.
    last_f0: f32,
    /// Retardo suavizado en muestras (para evitar saltos bruscos).
    smooth_delay: f32,
    /// Envuelta de presencia de voz (0–1).
    voiced_env: f32,
    /// Frecuencia de muestreo del pipeline.
    sample_rate: u32,
}

impl VocalIsolation {
    /// Crea el procesador a partir de los parámetros y la frecuencia de muestreo.
    pub fn from_params(params: VocalIsolationParams, sample_rate: u32) -> Self {
        // Capacidad suficiente para el retardo máximo: fs / F0 mín.
        let capacity = ((sample_rate as f32 / MIN_F0_HZ).ceil() as usize + 4).max(2);
        Self {
            params,
            detector: PitchDetector::new(DETECT_FRAME),
            delay_line: FractionalDelay::new(capacity),
            last_f0: 0.0,
            smooth_delay: (sample_rate as f32 / 200.0).max(1.0),
            voiced_env: 0.0,
            sample_rate,
        }
    }

    /// Actualiza los parámetros en vivo (desde el hilo de control).
    pub fn update_params(&mut self, params: VocalIsolationParams) {
        self.params = params;
    }

    /// Calcula la intensidad del comb a partir de la envuelta de voz.
    fn comb_gain(&self) -> f32 {
        (self.params.strength.clamp(0.0, 1.0) * self.voiced_env).min(MAX_COMB_GAIN)
    }
}

impl AudioProcessor for VocalIsolation {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        // Params no finitos (p. ej. de la red): degradar a passthrough. Con
        // NaN, `clamp(0.0, 1.0)` devuelve NaN y `<= 0.0` es `false` → NaN se
        // propagaría por `comb_gain()`.
        if !self.params.mix.is_finite() || !self.params.strength.is_finite() {
            output[..frames].copy_from_slice(&input[..frames]);
            return ProcessResult { latency_ms: 0.0 };
        }
        let wet = self.params.mix.clamp(0.0, 1.0);
        let strength = self.params.strength.clamp(0.0, 1.0);
        if wet <= 0.0 || strength <= 0.0 {
            output[..frames].copy_from_slice(&input[..frames]);
            return ProcessResult { latency_ms: 0.0 };
        }

        // Actualizar F0 y la envuelta de voz una vez por bloque.
        if self.sample_rate == 0 {
            self.sample_rate = info.sample_rate;
        }
        if let Some(f0) = self.detector.feed_and_detect(input) {
            self.last_f0 = f0;
            if f0 > 0.0 {
                let target = self.sample_rate as f32 / f0;
                let min_tau = (self.sample_rate as f32 / MAX_F0_HZ).max(1.0);
                let max_tau = (self.sample_rate as f32 / MIN_F0_HZ).ceil();
                let target = target.clamp(min_tau, max_tau);
                self.smooth_delay += (target - self.smooth_delay) * DELAY_SMOOTH;
                self.voiced_env += (1.0 - self.voiced_env) * VOICE_ATTACK;
            } else {
                self.voiced_env *= 1.0 - VOICE_RELEASE;
            }
        } else {
            // Sin frame completo: liberar suavemente la envuelta si se dejó de
            // detectar voz (p. ej. silencios largos entre bloques).
            if self.last_f0 <= 0.0 {
                self.voiced_env *= 1.0 - VOICE_RELEASE;
            }
        }

        let a = self.comb_gain();
        let inv = 1.0 / (1.0 + a);
        let delay = self.smooth_delay;
        let dry = 1.0 - wet;

        for i in 0..frames {
            let delayed = self.delay_line.process(input[i], delay);
            let combed = (input[i] + a * delayed) * inv;
            output[i] = input[i] * dry + combed * wet;
        }

        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "vocal_isolation"
    }

    fn reset(&mut self) {
        self.delay_line.buffer.fill(0.0);
        self.delay_line.pos = 0;
        self.detector = PitchDetector::new(DETECT_FRAME);
        self.last_f0 = 0.0;
        self.voiced_env = 0.0;
        self.smooth_delay = (self.sample_rate as f32 / 200.0).max(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(frames: usize) -> ProcessingInfo {
        ProcessingInfo {
            sample_rate: 48_000,
            frames,
        }
    }

    #[test]
    fn mix_zero_is_identity() {
        let mut v = VocalIsolation::from_params(
            VocalIsolationParams {
                strength: 1.0,
                mix: 0.0,
            },
            48_000,
        );
        let input: Vec<f32> = (0..512).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let mut out = vec![0.0; 512];
        v.process(&input, &mut out, &info(512));
        for (a, b) in input.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn strength_zero_is_identity() {
        let mut v = VocalIsolation::from_params(
            VocalIsolationParams {
                strength: 0.0,
                mix: 1.0,
            },
            48_000,
        );
        let input: Vec<f32> = (0..512).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let mut out = vec![0.0; 512];
        v.process(&input, &mut out, &info(512));
        for (a, b) in input.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn no_latency_added() {
        let mut v = VocalIsolation::from_params(
            VocalIsolationParams {
                strength: 0.8,
                mix: 0.6,
            },
            48_000,
        );
        let input: Vec<f32> = (0..512).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let mut out = vec![0.0; 512];
        let result = v.process(&input, &mut out, &info(512));
        assert_eq!(result.latency_ms, 0.0);
    }

    #[test]
    fn voiced_signal_produces_output_change_from_input() {
        // Un seno estable a 220 Hz debe activar el comb una llenado el frame.
        let sr = 48_000.0;
        let n = 4096;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr).sin() * 0.3)
            .collect();

        let mut v = VocalIsolation::from_params(
            VocalIsolationParams {
                strength: 1.0,
                mix: 1.0,
            },
            48_000,
        );
        let mut out = vec![0.0; n];
        v.process(&input, &mut out, &info(n));

        // La señal cambia (comb activo) en la zona estable (tras el frame Lleno).
        let changed = out[3000..]
            .iter()
            .zip(input[3000..].iter())
            .any(|(a, b)| (a - b).abs() > 1e-3);
        assert!(changed, "el comb no alteró la señal con voz detectada");
    }

    #[test]
    fn nan_params_degrade_to_passthrough() {
        // Params de red no finitos: con el bug, `mix.clamp()` devuelve NaN y
        // `wet <= 0.0` es false con NaN → `comb_gain()` computa NaN → salida
        // con NaN.
        for mix in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 2.0] {
            for strength in [f32::NAN, 1.0] {
                let mut v =
                    VocalIsolation::from_params(VocalIsolationParams { strength, mix }, 48_000);
                let input: Vec<f32> = (0..512).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
                let mut out = vec![0.0; 512];
                v.process(&input, &mut out, &info(512));
                assert!(
                    out.iter().all(|s| s.is_finite()),
                    "salida NaN con mix={mix} strength={strength}"
                );
                for (a, b) in input.iter().zip(out.iter()) {
                    assert!((a - b).abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn name_is_vocal_isolation() {
        let v = VocalIsolation::from_params(
            VocalIsolationParams {
                strength: 0.5,
                mix: 0.5,
            },
            48_000,
        );
        assert_eq!(v.name(), "vocal_isolation");
    }
}
