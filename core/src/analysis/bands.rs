//! División de la señal en bandas para el análisis espectral ligero.
//!
//! En lugar de una FFT, se usan filtros biquad fijos para separar la señal en
//! bandas (graves, baja-media, media, agudos) y se acumulan sus energías junto
//! con la tasa de cruces por cero. Es O(n) y **no asigna memoria** en el
//! camino de audio: se construye una vez y solo acumula contadores.

use crate::dsp::{BiquadFilter, BiquadKind, BiquadParams};

/// Conteo de muestras en los que se basó una ventana de análisis.
pub const FRAMES_PER_FRAME: u32 = 4800;

/// Marco crudo de análisis producido por el divisor de bandas.
///
/// Solo contiene `f32`/enteros: se construye y envía desde el callback de audio
/// sin asignar memoria.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceFrame {
    /// Nivel RMS acumulado de la ventana (dBFS).
    pub rms_db: f32,
    /// Nivel pico de la ventana (dBFS).
    pub peak_db: f32,
    /// Razón de energía de la banda de graves (0–1).
    pub low_ratio: f32,
    /// Razón de energía de la banda baja-media / boom (0–1).
    pub lowmid_ratio: f32,
    /// Razón de energía de la banda media (0–1).
    pub mid_ratio: f32,
    /// Razón de energía de la banda de agudos (0–1).
    pub high_ratio: f32,
    /// Tasa de cruces por cero normalizada (0–1).
    pub zcr: f32,
}

/// Separador de bandas con acumuladores de energía y cruces por cero.
///
/// Uso en el hilo de audio:
/// 1. `process(bloque)` acumula energías de banda y picos.
/// 2. Cada intervalo de análisis, `frame()` extrae un [`VoiceFrame`] y
///    reinicia los acumuladores para la siguiente ventana.
pub struct BandSplitter {
    low: BiquadFilter,
    lowmid: BiquadFilter,
    mid: BiquadFilter,
    high: BiquadFilter,
    sum_low: f64,
    sum_lowmid: f64,
    sum_mid: f64,
    sum_high: f64,
    sum_total: f64,
    peak: f32,
    /// Cruces por cero en la ventana.
    crossings: u64,
    /// Muestras acumuladas en la ventana.
    samples: u64,
}

impl BandSplitter {
    /// Crea el divisor de bandas para una frecuencia de muestreo dada.
    pub fn new(sample_rate: u32) -> Self {
        let design = |kind: BiquadKind, freq_hz: f32, q: f32| {
            BiquadFilter::design(
                BiquadParams {
                    kind,
                    freq_hz,
                    gain_db: 0.0,
                    q,
                },
                sample_rate,
            )
        };
        Self {
            // Graves: lo que queda por debajo de ~200 Hz.
            low: design(BiquadKind::LowPass, 200.0, 0.707),
            // Baja-media: la zona de *boominess* (~250–350 Hz).
            lowmid: design(BiquadKind::Peaking, 300.0, 1.5),
            // Media: presencia vocal (~1.2 kHz).
            mid: design(BiquadKind::Peaking, 1200.0, 1.5),
            // Agudos: brillo / sibilancia (por encima de ~4 kHz).
            high: design(BiquadKind::HighPass, 4000.0, 0.707),
            sum_low: 0.0,
            sum_lowmid: 0.0,
            sum_mid: 0.0,
            sum_high: 0.0,
            sum_total: 0.0,
            peak: 0.0,
            crossings: 0,
            samples: 0,
        }
    }

    /// Acumula un bloque de audio en los contadores de la ventana.
    pub fn process(&mut self, input: &[f32]) {
        let mut prev = 0.0f32;
        for (i, &sample) in input.iter().enumerate() {
            let low = self.low.process(sample);
            let lowmid = self.lowmid.process(sample);
            let mid = self.mid.process(sample);
            let high = self.high.process(sample);

            self.sum_low += (low as f64) * (low as f64);
            self.sum_lowmid += (lowmid as f64) * (lowmid as f64);
            self.sum_mid += (mid as f64) * (mid as f64);
            self.sum_high += (high as f64) * (high as f64);
            self.sum_total += (sample as f64) * (sample as f64);

            if sample.abs() > self.peak {
                self.peak = sample.abs();
            }
            if i > 0 && sample.signum() != prev.signum() && prev != 0.0 {
                self.crossings += 1;
            }
            prev = sample;
        }
        self.samples += input.len() as u64;
    }

    /// Extrae el marco de análisis de la ventana acumulada y la reinicia.
    pub fn frame(&mut self) -> VoiceFrame {
        let n = self.samples.max(1) as f64;
        let total = self.sum_total.max(f64::EPSILON);

        let low_ratio = (self.sum_low / total) as f32;
        let lowmid_ratio = (self.sum_lowmid / total) as f32;
        let mid_ratio = (self.sum_mid / total) as f32;
        let high_ratio = (self.sum_high / total) as f32;

        let rms = (self.sum_total / n).sqrt();
        let rms_db = if rms > 1e-6 {
            20.0 * rms.log10()
        } else {
            -120.0
        };
        let peak_db = if self.peak > 1e-6 {
            20.0 * self.peak.log10()
        } else {
            -120.0
        };

        let zcr = (self.crossings as f32) / (n as f32).max(1.0);

        let frame = VoiceFrame {
            rms_db: rms_db as f32,
            peak_db,
            low_ratio,
            lowmid_ratio,
            mid_ratio,
            high_ratio,
            zcr,
        };

        self.sum_low = 0.0;
        self.sum_lowmid = 0.0;
        self.sum_mid = 0.0;
        self.sum_high = 0.0;
        self.sum_total = 0.0;
        self.peak = 0.0;
        self.crossings = 0;
        self.samples = 0;

        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq_hz: f32, sample_rate: u32, seconds: f32) -> Vec<f32> {
        let n = (sample_rate as f32 * seconds) as usize;
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin())
            .collect()
    }

    #[test]
    fn high_frequency_signal_has_more_high_ratio() {
        let sr = 48_000;
        let mut splitter = BandSplitter::new(sr);
        let block = sine(6000.0, sr, 0.2);
        splitter.process(&block);
        let frame = splitter.frame();
        assert!(frame.high_ratio > frame.low_ratio, "agudos deben dominar");
    }

    #[test]
    fn low_frequency_signal_has_more_low_ratio() {
        let sr = 48_000;
        let mut splitter = BandSplitter::new(sr);
        let block = sine(100.0, sr, 0.2);
        splitter.process(&block);
        let frame = splitter.frame();
        assert!(frame.low_ratio > frame.high_ratio, "graves deben dominar");
    }

    #[test]
    fn rms_of_full_scale_sine_is_about_minus_3db() {
        let sr = 48_000;
        let mut splitter = BandSplitter::new(sr);
        splitter.process(&sine(440.0, sr, 0.1));
        let frame = splitter.frame();
        assert!(
            (frame.rms_db - (-3.01)).abs() < 0.5,
            "RMS {:?}",
            frame.rms_db
        );
    }

    #[test]
    fn frame_resets_accumulators() {
        let sr = 48_000;
        let mut splitter = BandSplitter::new(sr);
        splitter.process(&[0.5; 64]);
        let first = splitter.frame();
        let second = splitter.frame();
        assert!(first.rms_db > -120.0);
        assert!(second.rms_db <= -120.0, "acumuladores deben reiniciarse");
    }
}
