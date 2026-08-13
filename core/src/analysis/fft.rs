//! Análisis espectral en tiempo real con FFT (ventana Hann, 50 % de solapamiento).
//!
//! Calcula el espectro de magnitud de la entrada y lo reduce a bandas
//! logarítmicas para su visualización en la UI y el móvil (Fase 5 del plan de
//! producto). Los valores por banda se suavizan con un envolvente
//! ataque/release para que la vista sea estable y legible.
//!
//! Cumple la regla de los callbacks de audio: [`SpectrumAnalyzer::process`] **no
//! asigna memoria ni toma bloqueos**; todos los buffers (ventana, anillo, FFT y
//! scratch) se preasignan una sola vez en [`SpectrumAnalyzer::new`].

use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use crate::protocol::SPECTRUM_BIN_COUNT;

/// Tamaño de la FFT (puntos): ~23 Hz de resolución a 48 kHz.
pub const SPECTRUM_FFT_SIZE: usize = 2048;

/// Avance entre ventanas (muestras): 50 % de solapamiento.
pub const SPECTRUM_HOP_SIZE: usize = SPECTRUM_FFT_SIZE / 2;

/// Frecuencia de la primera banda (Hz).
const BAND_MIN_FREQ_HZ: f32 = 20.0;

/// Frecuencia máxima de la última banda (Hz); se acota al Nyquist real.
const BAND_MAX_FREQ_HZ: f32 = 20_000.0;

/// Constante de tiempo de ataque del suavizado (ms).
const ATTACK_TIME_MS: f32 = 10.0;

/// Velocidad de caída del release (dB por segundo).
///
/// En lugar de un one-pole, el release cae a tasa constante en el dominio dB
/// para que las barras del visualizador bajen a un ritmo visible y estable.
const RELEASE_DB_PER_SEC: f32 = 60.0;

/// Piso de dBFS reportado para bandas silenciosas.
const SILENCE_DB: f32 = -120.0;

/// Umbral de amplitud por debajo del cual se considera silencio.
const AMPLITUDE_EPSILON: f32 = 1e-6;

/// Analizador de espectro pensado para el callback de audio.
///
/// Uso: cada bloque de entrada se acumula en un anillo; cuando hay una ventana
/// completa se calcula la FFT, se reducen las magnitudes a bandas logarítmicas
/// (dBFS) y se actualizan los niveles suavizados. Después del cálculo se
/// conserva la última mitad de la ventana (solapamiento).
pub struct SpectrumAnalyzer {
    /// Plan de la FFT (reutilizado en cada cálculo).
    fft: Arc<dyn Fft<f32>>,
    /// Ventana Hann precalculada (longitud [`SPECTRUM_FFT_SIZE`]).
    window: Vec<f32>,
    /// Anillo de entrada que acumula muestras hasta completar la ventana.
    ring: Vec<f32>,
    /// Número de muestras válidas en `ring`.
    ring_len: usize,
    /// Buffer complejo de entrada/salida de la FFT (reutilizado).
    fft_buffer: Vec<Complex32>,
    /// Scratch de la FFT (preasignado para no asignar en el callback).
    scratch: Vec<Complex32>,
    /// Mapa de bin de FFT → banda logarítmica (longitud `FFT_SIZE/2 + 1`).
    band_map: Vec<usize>,
    /// Factor de normalización magnitud → amplitud de la señal original.
    magnitude_scale: f32,
    /// Coeficiente de ataque del suavizado.
    attack_alpha: f32,
    /// Caída del release por avance de ventana (dB).
    release_per_hop: f32,
    /// Nivel suavizado (dBFS) de cada banda.
    smoothed_db: [f32; SPECTRUM_BIN_COUNT],
}

impl SpectrumAnalyzer {
    /// Crea el analizador para una frecuencia de muestreo dada.
    ///
    /// Precalcula la ventana, los bordes de banda y los coeficientes de
    /// suavizado; todo lo que el callback necesita ya está listo.
    pub fn new(sample_rate: u32) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(SPECTRUM_FFT_SIZE);

        let window: Vec<f32> = (0..SPECTRUM_FFT_SIZE)
            .map(|n| {
                let angle =
                    2.0 * std::f32::consts::PI * n as f32 / (SPECTRUM_FFT_SIZE as f32 - 1.0);
                0.5 * (1.0 - angle.cos())
            })
            .collect();
        let window_sum: f32 = window.iter().sum();

        let band_edges = Self::band_edges(sample_rate);
        let band_map = Self::build_band_map(sample_rate, &band_edges);
        // Suavizado ataque/release: la subida es un one-pole rápido (ataque) y
        // la caída, una rampa lineal en dB (release), como en los medidores de
        // pico clásicos.
        let hop_secs = SPECTRUM_HOP_SIZE as f32 / sample_rate.max(1) as f32;
        let attack_alpha = 1.0 - (-hop_secs / (ATTACK_TIME_MS / 1000.0)).exp();
        let release_per_hop = RELEASE_DB_PER_SEC * hop_secs;

        Self {
            scratch: vec![Complex32::default(); fft.get_inplace_scratch_len()],
            fft,
            window,
            ring: vec![0.0; SPECTRUM_FFT_SIZE],
            ring_len: 0,
            fft_buffer: vec![Complex32::default(); SPECTRUM_FFT_SIZE],
            band_map,
            // El pico de un tono a amplitud A con ventana Hann vale `A·Σw/2`;
            // normalizar por eso recupera la amplitud de la señal original.
            magnitude_scale: 2.0 / window_sum,
            attack_alpha,
            release_per_hop,
            smoothed_db: [SILENCE_DB; SPECTRUM_BIN_COUNT],
        }
    }

    /// Acumula un bloque de entrada y, si se completó una ventana, devuelve las
    /// bandas suavizadas más recientes (dBFS).
    ///
    /// Devuelve `None` hasta que se acumulan [`SPECTRUM_FFT_SIZE`] muestras; a
    /// partir de ahí devuelve `Some` una vez por cada [`SPECTRUM_HOP_SIZE`]
    /// muestras nuevas (50 % de solapamiento).
    pub fn process(&mut self, input: &[f32]) -> Option<[f32; SPECTRUM_BIN_COUNT]> {
        let mut result = None;
        let mut idx = 0;
        while idx < input.len() {
            let space = SPECTRUM_FFT_SIZE - self.ring_len;
            let take = space.min(input.len() - idx);
            self.ring[self.ring_len..self.ring_len + take].copy_from_slice(&input[idx..idx + take]);
            self.ring_len += take;
            idx += take;

            if self.ring_len == SPECTRUM_FFT_SIZE {
                self.compute_fft();
                // Conservar la última mitad (solapamiento del 50 %).
                self.ring.copy_within(SPECTRUM_HOP_SIZE.., 0);
                self.ring_len = SPECTRUM_HOP_SIZE;
                result = Some(self.smoothed_db);
            }
        }
        result
    }

    /// Calcula la FFT de la ventana completa y actualiza las bandas suavizadas.
    fn compute_fft(&mut self) {
        for (i, &sample) in self.ring.iter().enumerate() {
            let v = sample * self.window[i];
            self.fft_buffer[i] = Complex32::new(v, 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.fft_buffer, &mut self.scratch);

        // Pico lineal de magnitud dentro de cada banda logarítmica.
        let mut band_peak = [0.0f32; SPECTRUM_BIN_COUNT];
        for (k, &band) in self.band_map.iter().enumerate() {
            let magnitude = self.fft_buffer[k].norm();
            if magnitude > band_peak[band] {
                band_peak[band] = magnitude;
            }
        }

        // Normalizar a amplitud, convertir a dBFS y aplicar ataque/release.
        for (i, &peak) in band_peak.iter().enumerate() {
            let amplitude = peak * self.magnitude_scale;
            let db = if amplitude > AMPLITUDE_EPSILON {
                20.0 * amplitude.log10()
            } else {
                SILENCE_DB
            };
            let prev = self.smoothed_db[i];
            self.smoothed_db[i] = if db > prev {
                // Ataque rápido (one-pole).
                prev + self.attack_alpha * (db - prev)
            } else {
                // Release: caída lineal en dB, sin bajar del valor objetivo.
                db.max(prev - self.release_per_hop)
            };
        }
    }

    /// Bordes (Hz) de las bandas logarítmicas entre ~20 Hz y el Nyquist real.
    fn band_edges(sample_rate: u32) -> Vec<f32> {
        let nyquist = sample_rate as f32 / 2.0;
        let max_freq = nyquist.min(BAND_MAX_FREQ_HZ);
        let min_freq = BAND_MIN_FREQ_HZ.min(max_freq);
        let ratio = (max_freq / min_freq).powf(1.0 / SPECTRUM_BIN_COUNT as f32);
        (0..=SPECTRUM_BIN_COUNT)
            .map(|i| min_freq * ratio.powi(i as i32))
            .collect()
    }

    /// Asigna cada bin de la FFT (0..=Nyquist) a su banda logarítmica.
    fn build_band_map(sample_rate: u32, edges: &[f32]) -> Vec<usize> {
        let nyquist_bins = SPECTRUM_FFT_SIZE / 2 + 1;
        let mut map = Vec::with_capacity(nyquist_bins);
        let mut band = 0;
        for k in 0..nyquist_bins {
            let freq = k as f32 * sample_rate as f32 / SPECTRUM_FFT_SIZE as f32;
            while band < SPECTRUM_BIN_COUNT - 1 && freq >= edges[band + 1] {
                band += 1;
            }
            map.push(band);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq_hz: f32, amplitude: f32, sample_rate: u32) -> impl FnMut() -> f32 {
        let mut phase = 0.0f32;
        let step = 2.0 * std::f32::consts::PI * freq_hz / sample_rate as f32;
        move || {
            let value = amplitude * phase.sin();
            phase += step;
            value
        }
    }

    /// Procesa una señal hasta obtener un espectro (múltiples bloques pequeños).
    fn run_until_spectrum(
        analyzer: &mut SpectrumAnalyzer,
        mut next: impl FnMut() -> f32,
        block_len: usize,
    ) -> [f32; SPECTRUM_BIN_COUNT] {
        let mut block = vec![0.0f32; block_len];
        let mut latest = [SILENCE_DB; SPECTRUM_BIN_COUNT];
        for _ in 0..(SPECTRUM_FFT_SIZE / block_len * 8) {
            for sample in block.iter_mut() {
                *sample = next();
            }
            if let Some(bins) = analyzer.process(&block) {
                latest = bins;
            }
        }
        latest
    }

    fn dominant_band(bins: &[f32; SPECTRUM_BIN_COUNT]) -> (usize, f32) {
        bins.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(i, &v)| (i, v))
            .expect("SPECTRUM_BIN_COUNT > 0")
    }

    #[test]
    fn silence_reports_silence_floor() {
        let mut analyzer = SpectrumAnalyzer::new(48_000);
        let bins = run_until_spectrum(&mut analyzer, || 0.0, 512);
        assert!(
            bins.iter().all(|&db| db <= -110.0),
            "silencio debe quedar en el piso: {bins:?}"
        );
    }

    #[test]
    fn low_frequency_tone_peaks_in_low_bands() {
        let mut analyzer = SpectrumAnalyzer::new(48_000);
        let bins = run_until_spectrum(&mut analyzer, sine(100.0, 0.5, 48_000), 512);
        let (band, _db) = dominant_band(&bins);
        assert!(
            band <= 11,
            "un tono de 100 Hz debe dominar las bandas graves (argmax {band})"
        );
    }

    #[test]
    fn high_frequency_tone_peaks_in_high_bands() {
        let mut analyzer = SpectrumAnalyzer::new(48_000);
        let bins = run_until_spectrum(&mut analyzer, sine(12_000.0, 0.5, 48_000), 512);
        let (band, _db) = dominant_band(&bins);
        assert!(
            band >= 26,
            "un tono de 12 kHz debe dominar las bandas agudas (argmax {band})"
        );
    }

    #[test]
    fn full_scale_tone_reports_about_zero_dbfs() {
        let mut analyzer = SpectrumAnalyzer::new(48_000);
        let bins = run_until_spectrum(&mut analyzer, sine(440.0, 1.0, 48_000), 512);
        let (_, db) = dominant_band(&bins);
        assert!(
            (db - 0.0).abs() < 4.0,
            "un tono a plena escala debe rondar los 0 dBFS (obtenido {db})"
        );
    }

    #[test]
    fn louder_tone_reports_higher_db() {
        let sr = 48_000;
        let mut quiet = SpectrumAnalyzer::new(sr);
        let mut loud = SpectrumAnalyzer::new(sr);
        let quiet_bins = run_until_spectrum(&mut quiet, sine(1000.0, 0.05, sr), 512);
        let loud_bins = run_until_spectrum(&mut loud, sine(1000.0, 0.5, sr), 512);
        let quiet_db = quiet_bins.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let loud_db = loud_bins.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            loud_db - quiet_db > 10.0,
            "el tono más fuerte debe verse más alto: quiet {quiet_db}, loud {loud_db}"
        );
    }

    #[test]
    fn process_returns_none_until_window_fills() {
        let mut analyzer = SpectrumAnalyzer::new(48_000);
        assert!(analyzer.process(&[0.0; 512]).is_none());
        assert!(analyzer.process(&[0.0; 512]).is_none());
        // 2048 acumuladas → primera ventana completa.
        assert!(analyzer.process(&[0.0; 1024]).is_some());
    }

    #[test]
    fn attack_rises_fast_and_release_decays_slowly() {
        let sr = 48_000;
        let mut analyzer = SpectrumAnalyzer::new(sr);
        let mut tone = sine(440.0, 1.0, sr);

        // Arrancar en silencio, luego un tono fuerte: el ataque es rápido.
        let mut block = [0.0f32; 1024];
        for _ in 0..(SPECTRUM_FFT_SIZE / 1024) {
            let _ = analyzer.process(&block);
        }
        let mut onset_db = SILENCE_DB;
        for _ in 0..(SPECTRUM_FFT_SIZE / 1024) {
            for sample in block.iter_mut() {
                *sample = tone();
            }
            if let Some(bins) = analyzer.process(&block) {
                onset_db = *bins.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
            }
        }
        assert!(
            onset_db > -20.0,
            "el ataque debe subir rápido tras el arranque (obtenido {onset_db})"
        );

        // Retirar el tono: tras la misma cantidad de ventanas, el release
        // (rampa lenta en dB) aún no debe haber llegado al piso.
        let mut quiet_blocks = 0;
        let mut decay_db = onset_db;
        for _ in 0..(SPECTRUM_FFT_SIZE / 1024 * 4) {
            block.fill(0.0);
            if let Some(bins) = analyzer.process(&block) {
                decay_db = *bins.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
                quiet_blocks += 1;
            }
        }
        // ~170 ms de silencio a 60 dB/s ≈ 10 dB de caída (ni nulo ni a piso).
        assert!(
            quiet_blocks >= 2 && decay_db > onset_db - 15.0 && decay_db < onset_db - 5.0,
            "el release debe decaer lento y estable: tras {quiet_blocks} ventanas \
             pasó de {onset_db} a {decay_db}"
        );
    }
}
