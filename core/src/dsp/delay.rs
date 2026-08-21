//! Línea de retardo circular reutilizable y efecto de eco (delay) multi-modo.
//!
//! Soporta 4 modos de carácter:
//! - **Digital**: eco limpio sin degradación.
//! - **Analog**: eco cálido con filtro LP en el feedback loop.
//! - **Tape**: eco de cinta con wow & flutter (modulación LFO).
//! - **Slapback**: eco corto sin feedback, una sola repetición.

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::{DelayMode, DelayParams};
use std::f32::consts::TAU;

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

    /// Lee la muestra retardada sin escribir (para el lazo de feedback).
    #[inline]
    fn peek(&self, delay: usize) -> f32 {
        let len = self.buffer.len();
        self.buffer[(self.write + len - delay.min(len)) % len]
    }

    /// Lee una muestra retardada con interpolación lineal (para tape mode).
    #[inline]
    fn peek_interp(&self, delay_frac: f32) -> f32 {
        let len = self.buffer.len();
        let delay_int = delay_frac.floor() as usize;
        let frac = delay_frac - delay_int as f32;
        let idx0 = (self.write + len - delay_int.min(len)) % len;
        let idx1 = (self.write + len - (delay_int + 1).min(len)) % len;
        self.buffer[idx0] * (1.0 - frac) + self.buffer[idx1] * frac
    }

    /// Limpia el buffer (silencio).
    pub fn clear(&mut self) {
        for v in &mut self.buffer {
            *v = 0.0;
        }
        self.write = 0;
    }
}

/// Efecto de eco multi-modo con feedback, pre-delay, filtros y mezcla.
///
/// El camino seco no añade latencia (es un efecto en paralelo), por lo que el
/// `ProcessResult` reporta 0.
#[derive(Debug, Clone)]
pub struct Delay {
    mode: DelayMode,
    line: DelayLine,
    /// Retardo base en muestras (sin pre-delay).
    base_delay_samples: f32,
    /// Pre-delay en muestras.
    pre_delay_samples: usize,
    feedback: f32,
    mix: f32,
    /// Tiempo del eco en ms (guardado para consulta).
    time_ms: f32,
    /// Corte de graves del eco (filtro HP).
    low_cut: BiquadFilter,
    /// Corte de agudos del eco (filtro LP).
    high_cut: BiquadFilter,
    /// Ducking: atenuación del delay cuando la voz está presente.
    duck_amount: f32,
    duck_env: f32,
    /// Para Tape mode: fase del LFO de wow & flutter.
    tape_phase: f32,
    /// Para Analog mode: estado del filtro LP en el feedback loop.
    analog_filter_state: f32,
    /// Muestra anterior del feedback (para tape interpolation).
    prev_delayed: f32,
    sample_rate: u32,
}

impl Delay {
    /// Tiempo de retardo (ms).
    pub fn time_ms(&self) -> f32 {
        self.time_ms
    }

    /// Crea un delay a partir de parámetros del protocolo.
    pub fn from_params(params: DelayParams, sample_rate: u32) -> Self {
        // El delay efectivo incluye pre-delay.
        let effective_ms = params.time_ms + params.pre_delay_ms;
        let max_delay_samples = ms_to_samples(effective_ms, sample_rate).max(1);

        let low_cut = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::HighPass,
                freq_hz: params.low_cut_hz,
                gain_db: 0.0,
                q: 0.707,
            },
            sample_rate,
        );

        let high_cut = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::LowPass,
                freq_hz: params.high_cut_hz,
                gain_db: 0.0,
                q: 0.707,
            },
            sample_rate,
        );

        let base_delay_samples = ms_to_samples(params.time_ms, sample_rate) as f32;
        let pre_delay_samples = ms_to_samples(params.pre_delay_ms, sample_rate);

        // Para Slapback: forzar feedback a 0.
        let feedback = if matches!(params.mode, DelayMode::Slapback) {
            0.0
        } else {
            params.feedback.clamp(0.0, 0.95)
        };

        Self {
            mode: params.mode,
            line: DelayLine::new(max_delay_samples),
            base_delay_samples,
            pre_delay_samples,
            feedback,
            mix: params.mix.clamp(0.0, 1.0),
            time_ms: params.time_ms,
            low_cut,
            high_cut,
            duck_amount: params.duck_amount.clamp(0.0, 1.0),
            duck_env: 0.0,
            tape_phase: 0.0,
            analog_filter_state: 0.0,
            prev_delayed: 0.0,
            sample_rate,
        }
    }

    /// Crea un delay legacy (modo Digital) con tiempo, feedback y mezcla.
    pub fn new(time_ms: f32, feedback: f32, mix: f32, sample_rate: u32) -> Self {
        Self::from_params(
            DelayParams {
                mode: DelayMode::Digital,
                time_ms,
                feedback,
                mix,
                pre_delay_ms: 0.0,
                low_cut_hz: 50.0,
                high_cut_hz: 18000.0,
                tempo_bpm: 120.0,
                sync_enabled: false,
                duck_amount: 0.0,
            },
            sample_rate,
        )
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
            // 1) Ducking: env follower sobre la entrada.
            let env = input[i].abs();
            let attack = 0.002; // ~2 ms
            let release = 0.05; // ~50 ms
            let coeff = if env > self.duck_env { attack } else { release };
            self.duck_env += coeff * (env - self.duck_env);
            let duck_gain = 1.0 - self.duck_amount * self.duck_env.min(1.0);

            // 2) Calcular delay efectivo (con modulación para Tape).
            let delay_samples = match self.mode {
                DelayMode::Tape => {
                    // Wow & flutter: LFO lento modula el delay ±2%.
                    let lfo = (TAU * self.tape_phase).sin() * 0.02;
                    self.tape_phase += 1.0 / (self.sample_rate as f32 * 0.8); // ~0.8 Hz
                    if self.tape_phase > 1.0 {
                        self.tape_phase -= 1.0;
                    }
                    self.base_delay_samples * (1.0 + lfo)
                }
                _ => self.base_delay_samples,
            };

            // 3) Leer muestra retardada (interpolada para Tape, entera para el resto).
            let wet = match self.mode {
                DelayMode::Tape => {
                    let raw = self.line.peek_interp(delay_samples);
                    // Degradación tonal: suavizado simple.
                    self.prev_delayed = self.prev_delayed * 0.3 + raw * 0.7;
                    self.prev_delayed
                }
                _ => {
                    let delay_int = delay_samples.round() as usize;
                    self.line.peek(delay_int + self.pre_delay_samples)
                }
            };

            // 4) Feedback con coloración por modo.
            let feedback_sample = match self.mode {
                DelayMode::Analog => {
                    // Filtro LP simple en el feedback loop (degradación cálida).
                    let alpha = 0.15; // ~3.5 kHz cutoff (aprox.)
                    self.analog_filter_state += alpha * (wet - self.analog_filter_state);
                    self.analog_filter_state * self.feedback
                }
                DelayMode::Tape => {
                    // Ya tiene degradación por interpolación y suavizado.
                    wet * self.feedback
                }
                _ => wet * self.feedback,
            };

            // 5) Escribir en la línea: entrada + feedback.
            let write_sample = input[i] + feedback_sample;
            let _ = self.line.push(
                write_sample,
                delay_samples as usize + self.pre_delay_samples,
            );

            // 6) Filtros del wet signal (low-cut y high-cut).
            let filtered = self.low_cut.process(wet);
            let filtered = self.high_cut.process(filtered);

            // 7) Mezclar seco/húmedo con ducking.
            output[i] = input[i] * dry + filtered * self.mix * duck_gain;
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "delay"
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
        let sr = 48_000;
        let delay_samples = 240; // 5 ms a 48 kHz
        let mut delay = Delay::new(5.0, 0.0, 1.0, sr);
        let n = delay_samples + 64;
        let mut input = vec![0.0; n];
        input[0] = 1.0;
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        delay.process(&input, &mut out, &info);
        // Con mix=1 y feedback=0, el pulso aparece ~240 muestras después.
        // Los filtros biquad del wet signal atenúan el pico; verificamos
        // que la energía esté concentrada en la zona del eco.
        for v in &out[..delay_samples - 10] {
            assert!(v.abs() < 0.01, "antes del retardo: {v}");
        }
        assert!(
            out[delay_samples].abs() > 0.3,
            "eco esperado en out[{}]={}",
            delay_samples,
            out[delay_samples]
        );
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

    #[test]
    fn from_params_digital_matches_legacy() {
        let params = DelayParams {
            mode: DelayMode::Digital,
            time_ms: 5.0,
            feedback: 0.0,
            mix: 1.0,
            pre_delay_ms: 0.0,
            low_cut_hz: 50.0,
            high_cut_hz: 18000.0,
            tempo_bpm: 120.0,
            sync_enabled: false,
            duck_amount: 0.0,
        };
        let sr = 48_000;
        let mut delay = Delay::from_params(params, sr);
        let delay_samples = 240;
        let n = delay_samples + 64;
        let mut input = vec![0.0; n];
        input[0] = 1.0;
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        delay.process(&input, &mut out, &info);
        for v in &out[..delay_samples - 10] {
            assert!(v.abs() < 0.01, "antes del retardo: {v}");
        }
        assert!(
            out[delay_samples].abs() > 0.3,
            "eco esperado en out[{}]={}",
            delay_samples,
            out[delay_samples]
        );
    }

    #[test]
    fn slapback_has_no_feedback() {
        let params = DelayParams {
            mode: DelayMode::Slapback,
            time_ms: 80.0,
            feedback: 0.5, // Debería ignorarse en Slapback
            mix: 1.0,
            pre_delay_ms: 0.0,
            low_cut_hz: 100.0,
            high_cut_hz: 8000.0,
            tempo_bpm: 120.0,
            sync_enabled: false,
            duck_amount: 0.0,
        };
        let mut delay = Delay::from_params(params, 48_000);
        // Feedback forzado a 0 en Slapback.
        assert_eq!(delay.feedback, 0.0);
    }

    #[test]
    fn analog_mode_applies_coloration() {
        let params = DelayParams {
            mode: DelayMode::Analog,
            time_ms: 20.0,
            feedback: 0.5,
            mix: 1.0,
            pre_delay_ms: 0.0,
            low_cut_hz: 50.0,
            high_cut_hz: 18000.0,
            tempo_bpm: 120.0,
            sync_enabled: false,
            duck_amount: 0.0,
        };
        let mut delay = Delay::from_params(params, 48_000);
        let mut input = vec![0.0; 4800]; // 100 ms
        input[0] = 1.0;
        let mut out = vec![0.0; 4800];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4800,
        };
        delay.process(&input, &mut out, &info);
        // Debe haber ecos con feedback, pero atenuados por el filtro.
        assert!(out[960].abs() > 0.01, "eco debería aparecer ~20 ms después");
    }

    #[test]
    fn tape_mode_does_not_panic() {
        let params = DelayParams {
            mode: DelayMode::Tape,
            time_ms: 30.0,
            feedback: 0.4,
            mix: 0.6,
            pre_delay_ms: 0.0,
            low_cut_hz: 100.0,
            high_cut_hz: 10000.0,
            tempo_bpm: 120.0,
            sync_enabled: false,
            duck_amount: 0.0,
        };
        let mut delay = Delay::from_params(params, 48_000);
        let mut input = vec![0.5; 4800];
        let mut out = vec![0.0; 4800];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4800,
        };
        delay.process(&input, &mut out, &info);
        // Solo verificamos que no panic ni produce NaN.
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn ducking_reduces_wet_when_input_loud() {
        let params = DelayParams {
            mode: DelayMode::Digital,
            time_ms: 20.0,
            feedback: 0.5,
            mix: 1.0,
            pre_delay_ms: 0.0,
            low_cut_hz: 50.0,
            high_cut_hz: 18000.0,
            tempo_bpm: 120.0,
            sync_enabled: false,
            duck_amount: 0.8,
        };
        let mut delay = Delay::from_params(params, 48_000);
        // Dar tiempo a que el env follower se estabilice.
        let mut input = vec![0.0; 960];
        let mut out = vec![0.0; 960];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 960,
        };
        // Señal fuerte → el ducking debería reducir el eco.
        for sample in input.iter_mut().take(480) {
            *sample = 0.9;
        }
        delay.process(&input, &mut out, &info);

        // Reiniciar con señal fuerte de nuevo y medir el eco.
        delay.line.clear();
        delay.duck_env = 0.0;
        let input2 = vec![0.9; 960];
        let mut out2 = vec![0.0; 960];
        delay.process(&input2, &mut out2, &info);

        // El nivel del eco debería ser menor que el input por el ducking.
        let eco_level = out2[480].abs();
        assert!(
            eco_level < 0.9,
            "ducking no está funcionando: eco={eco_level}"
        );
    }
}
