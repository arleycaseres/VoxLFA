//! EQ dinámico: compresión por banda de frecuencia.
//!
//! Cada banda extrae una región del espectro con un filtro pasabanda, detecta
//! su envolvente y aplica compresión (ratio > 1) cuando supera el umbral.
//! La señal resultante se mezcla con la original: solo se modifica la banda
//! activa, preservando el resto del espectro intacto.

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::compressor::time_to_coef;
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::dsp::DynamicEqBandParams;

/// Una banda del EQ dinámico: filtro pasabanda (sidechain) + detector de
/// envolvente + computador de ganancia.
#[derive(Debug, Clone)]
struct DynamicEqBand {
    /// Filtro pasabanda para extraer la banda (sidechain de detección).
    bandpass: BiquadFilter,
    /// Umbral de compresión (dBFS).
    threshold_db: f32,
    /// Relación de compresión (> 1).
    ratio: f32,
    /// Coeficiente de suavizado de ataque.
    attack_coef: f32,
    /// Coeficiente de suavizado de liberación.
    release_coef: f32,
    /// Ganancia de maquillaje compensatoria (lineal).
    makeup_linear: f32,
    /// Envolvente detectada del bandpass (amplitud lineal).
    envelope: f32,
    /// Ganancia suavizada en dB (negativa = compresión).
    gr_smooth_db: f32,
}

impl DynamicEqBand {
    fn new(params: DynamicEqBandParams, sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        Self {
            bandpass: BiquadFilter::design(
                BiquadParams {
                    kind: BiquadKind::BandPass,
                    freq_hz: params.freq_hz,
                    gain_db: 0.0,
                    q: params.q,
                },
                sample_rate,
            ),
            threshold_db: params.threshold_db,
            ratio: params.ratio.max(1.0),
            attack_coef: time_to_coef(params.attack_ms, sr),
            release_coef: time_to_coef(params.release_ms, sr),
            makeup_linear: 10f32.powf(params.makeup_db / 20.0),
            envelope: 0.0,
            gr_smooth_db: 0.0,
        }
    }

    /// Procesa un solo sample: extrae banda, detecta envolvente, aplica
    /// compresión y devuelve la señal de la banda con ganancia aplicada.
    ///
    /// El resultado es la banda con su ganancia modificada; el caller suma
    /// `input + band * (gain - 1)` para reconstruir la señal.
    fn process_sample(&mut self, input: f32) -> f32 {
        // 1) Extraer la banda con el pasabanda.
        let band = self.bandpass.process(input);

        // 2) Detector de envolvente (pico con suavizado).
        let detected = band.abs();
        let coef = if detected > self.envelope {
            self.attack_coef
        } else {
            self.release_coef
        };
        self.envelope += coef * (detected - self.envelope);

        // 3) Reducción de ganancia por encima del umbral.
        let level_db = 20.0 * self.envelope.max(1e-20).log10();
        let over = level_db - self.threshold_db;
        let gr_target_db = if over > 0.0 {
            over * (1.0 - 1.0 / self.ratio)
        } else {
            0.0
        };

        // 4) Suavizado de ganancia.
        let coef = if gr_target_db > self.gr_smooth_db {
            self.attack_coef
        } else {
            self.release_coef
        };
        self.gr_smooth_db += coef * (gr_target_db - self.gr_smooth_db);

        // 5) Ganancia aplicada a la banda.
        let gain = 10f32.powf(-self.gr_smooth_db / 20.0) * self.makeup_linear;
        band * gain
    }

    fn reset(&mut self) {
        self.bandpass.reset();
        self.envelope = 0.0;
        self.gr_smooth_db = 0.0;
    }
}

/// EQ dinámico multibanda: compresión independiente por banda de frecuencia.
///
/// Implementación serial: cada banda procesa en secuencia sobre la salida de
/// la anterior, acumulando las modificaciones espectrales.
#[derive(Debug, Clone)]
pub struct DynamicEq {
    bands: Vec<DynamicEqBand>,
}

impl DynamicEq {
    /// Crea un EQ dinámico con las bandas indicadas.
    pub fn new(params: &[DynamicEqBandParams], sample_rate: u32) -> Self {
        Self {
            bands: params
                .iter()
                .map(|p| DynamicEqBand::new(*p, sample_rate))
                .collect(),
        }
    }

    /// Crea un EQ dinámico desde los parámetros del protocolo.
    pub fn from_params(params: &crate::protocol::dsp::DynamicEqParams, sample_rate: u32) -> Self {
        Self::new(&params.bands, sample_rate)
    }
}

impl AudioProcessor for DynamicEq {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        output[..frames].copy_from_slice(&input[..frames]);

        for band in &mut self.bands {
            for out_sample in output[..frames].iter_mut() {
                let dry = *out_sample;
                let wet_band = band.process_sample(dry);
                let band_original = band.bandpass.process(dry);
                *out_sample = dry + wet_band - band_original;
            }
        }

        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "dynamic_eq"
    }

    fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
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
    fn empty_bands_is_passthrough() {
        let mut deq = DynamicEq::new(&[], 48_000);
        let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut out = vec![0.0; 256];
        deq.process(&input, &mut out, &info());
        assert_eq!(out, input);
    }

    #[test]
    fn single_band_below_threshold_is_unchanged() {
        let params = vec![DynamicEqBandParams {
            freq_hz: 1000.0,
            q: 1.0,
            threshold_db: -6.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 100.0,
            makeup_db: 0.0,
        }];
        let mut deq = DynamicEq::new(&params, 48_000);
        // Señal muy baja: mucho por debajo del umbral.
        let input = vec![0.001; 256];
        let mut out = vec![0.0; 256];
        deq.process(&input, &mut out, &info());
        for (o, &i) in out.iter().zip(&input) {
            assert!((o - i).abs() < 1e-3, "got {o:.6}, expected {i:.6}");
        }
    }

    #[test]
    fn single_band_above_threshold_reduces() {
        let params = vec![DynamicEqBandParams {
            freq_hz: 1000.0,
            q: 0.7,
            threshold_db: -40.0,
            ratio: 4.0,
            attack_ms: 0.5,
            release_ms: 100.0,
            makeup_db: 0.0,
        }];
        let mut deq = DynamicEq::new(&params, 48_000);
        // Señal a 1 kHz por encima del umbral.
        let sample_rate = 48_000u32;
        let input: Vec<f32> = (0..4800)
            .map(|i| {
                0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate as f32).sin()
            })
            .collect();
        let mut out = vec![0.0; 4800];
        deq.process(&input, &mut out, &info());
        // La energía a 1 kHz debe reducirse.
        let rms_in: f32 = input.iter().map(|x| x * x).sum::<f32>().sqrt() / input.len() as f32;
        let rms_out: f32 = out.iter().map(|x| x * x).sum::<f32>().sqrt() / out.len() as f32;
        assert!(
            rms_out < rms_in,
            "dynamic EQ should reduce energy: rms_in={rms_in:.6}, rms_out={rms_out:.6}"
        );
    }

    #[test]
    fn makeup_gain_compensates() {
        let params = vec![DynamicEqBandParams {
            freq_hz: 1000.0,
            q: 0.7,
            threshold_db: -40.0,
            ratio: 4.0,
            attack_ms: 0.5,
            release_ms: 100.0,
            makeup_db: 6.0,
        }];
        let mut deq = DynamicEq::new(&params, 48_000);
        let sample_rate = 48_000u32;
        let input: Vec<f32> = (0..4800)
            .map(|i| {
                0.5 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate as f32).sin()
            })
            .collect();
        let mut out = vec![0.0; 4800];
        deq.process(&input, &mut out, &info());
        // Con makeup +6 dB la señal debe estar más loud que sin makeup.
        let params_no_makeup = vec![DynamicEqBandParams {
            makeup_db: 0.0,
            ..params[0]
        }];
        let mut deq2 = DynamicEq::new(&params_no_makeup, 48_000);
        let mut out2 = vec![0.0; 4800];
        deq2.process(&input, &mut out2, &info());
        let rms_with: f32 = out.iter().map(|x| x * x).sum::<f32>().sqrt() / out.len() as f32;
        let rms_without: f32 = out2.iter().map(|x| x * x).sum::<f32>().sqrt() / out2.len() as f32;
        assert!(
            rms_with > rms_without,
            "makeup should boost: with={rms_with:.6}, without={rms_without:.6}"
        );
    }
}
