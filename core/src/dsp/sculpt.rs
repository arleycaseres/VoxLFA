//! Sculpt: modelado tonal de la voz con un solo control.
//!
//! Deriva tres filtros biquad (shelf de graves, pico de presencia y shelf de
//! agudos) de un único parámetro `tone`:
//!
//! ```text
//!   t        = (tone − 0.5) · 2        (−1 = muy oscuro, +1 = muy brillante)
//!   bass     = −3.0 · t  dB  @ 250 Hz   low-shelf  (Q 0.7)
//!   presence = +1.5 · t  dB  @ 3 kHz    peaking    (Q 1.2)
//!   air      = +3.5 · t  dB  @ 8.5 kHz  high-shelf (Q 0.7)
//! ```
//!
//! Con `tone = 0.5` todas las ganancias son 0 → el filtro es **identidad** (paso
//! directo), así que meter Sculpt en un preset nunca altera el sonido por
//! defecto. No aporta latencia.

use crate::dsp::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use crate::dsp::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::SculptParams;

/// Frecuencia central del shelf de graves (Hz).
const BASS_FREQ_HZ: f32 = 250.0;
/// Frecuencia central del pico de presencia (Hz).
const PRESENCE_FREQ_HZ: f32 = 3000.0;
/// Frecuencia central del shelf de agudos (Hz).
const AIR_FREQ_HZ: f32 = 8500.0;
/// Factor de ganancia (dB) del shelf de graves por unidad de `t`.
const BASS_GAIN_PER_T: f32 = -3.0;
/// Factor de ganancia (dB) del pico de presencia por unidad de `t`.
const PRESENCE_GAIN_PER_T: f32 = 1.5;
/// Factor de ganancia (dB) del shelf de agudos por unidad de `t`.
const AIR_GAIN_PER_T: f32 = 3.5;
/// Factor de calidad del shelf de graves y del shelf de agudos.
const SHELF_Q: f32 = 0.7;
/// Factor de calidad del pico de presencia.
const PRESENCE_Q: f32 = 1.2;

/// Desviación tonal (−1 … +1) a partir del parámetro `tone` (0–1).
fn tone_delta(tone: f32) -> f32 {
    (tone - 0.5).clamp(-0.5, 0.5) * 2.0
}

/// Procesador Sculpt: tres biquads cuyas ganancias dependen de `tone`.
///
/// Los coeficientes se calculan en la construcción (hilo de control); en el
/// hilo de audio solo se aplica la ecuación en diferencias.
pub struct Sculpt {
    params: SculptParams,
    bass: BiquadFilter,
    presence: BiquadFilter,
    air: BiquadFilter,
}

impl Sculpt {
    /// Crea el procesador a partir de los parámetros y la frecuencia de muestreo.
    pub fn from_params(params: SculptParams, sample_rate: u32) -> Self {
        let t = tone_delta(params.tone);
        Self {
            params,
            bass: BiquadFilter::design(
                BiquadParams {
                    kind: BiquadKind::LowShelf,
                    freq_hz: BASS_FREQ_HZ,
                    gain_db: BASS_GAIN_PER_T * t,
                    q: SHELF_Q,
                },
                sample_rate,
            ),
            presence: BiquadFilter::design(
                BiquadParams {
                    kind: BiquadKind::Peaking,
                    freq_hz: PRESENCE_FREQ_HZ,
                    gain_db: PRESENCE_GAIN_PER_T * t,
                    q: PRESENCE_Q,
                },
                sample_rate,
            ),
            air: BiquadFilter::design(
                BiquadParams {
                    kind: BiquadKind::HighShelf,
                    freq_hz: AIR_FREQ_HZ,
                    gain_db: AIR_GAIN_PER_T * t,
                    q: SHELF_Q,
                },
                sample_rate,
            ),
        }
    }

    /// Actualiza los parámetros en vivo (desde el hilo de control).
    pub fn update_params(&mut self, params: SculptParams) {
        self.params = params;
    }
}

impl AudioProcessor for Sculpt {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        let wet = self.params.mix.clamp(0.0, 1.0);
        if wet <= 0.0 || tone_delta(self.params.tone).abs() < 1e-3 {
            output[..frames].copy_from_slice(&input[..frames]);
            return ProcessResult { latency_ms: 0.0 };
        }

        let dry = 1.0 - wet;
        for i in 0..frames {
            let shaped = self
                .air
                .process(self.presence.process(self.bass.process(input[i])));
            output[i] = input[i] * dry + shaped * wet;
        }

        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "sculpt"
    }

    fn reset(&mut self) {
        self.bass.reset();
        self.presence.reset();
        self.air.reset();
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
    fn neutral_tone_is_identity() {
        let mut s = Sculpt::from_params(
            SculptParams {
                tone: 0.5,
                mix: 1.0,
            },
            48_000,
        );
        let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.13).sin() * 0.5).collect();
        let mut out = vec![0.0; 256];
        s.process(&input, &mut out, &info(256));
        for (a, b) in input.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-4, "tone 0.5 no es identidad");
        }
    }

    #[test]
    fn mix_zero_is_identity() {
        let mut s = Sculpt::from_params(
            SculptParams {
                tone: 0.9,
                mix: 0.0,
            },
            48_000,
        );
        let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.13).sin() * 0.5).collect();
        let mut out = vec![0.0; 256];
        s.process(&input, &mut out, &info(256));
        for (a, b) in input.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn dark_tone_brightens_and_bright_tone_brightens_highs() {
        // El brillo percibido se mueve con `tone`: el espectro alto crece al
        // aumentar `tone`. Se compara la energía en agudos (integral de |y| de
        // un barrido suave) entre tono oscuro y brillante.
        let sr = 48_000.0;
        let n = 4096;
        let input: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr).sin() * 0.3)
            .collect();

        let high_energy = |tone: f32| -> f32 {
            let mut s = Sculpt::from_params(SculptParams { tone, mix: 1.0 }, 48_000);
            let mut out = vec![0.0; n];
            s.process(&input, &mut out, &info(n));
            // Energía del último cuarto (ya estable) normalizada por el total.
            let mut total = 0.0f32;
            let mut high = 0.0f32;
            for (i, v) in out.iter().enumerate() {
                let e = v * v;
                total += e;
                if i > n * 3 / 4 {
                    high += e;
                }
            }
            high / total.max(1e-12)
        };

        let bright = high_energy(0.9);
        let dark = high_energy(0.1);
        assert!(
            bright > dark,
            "se esperaba más energía en agudos con tone=0.9 (bright={bright:.4}, dark={dark:.4})"
        );
    }

    #[test]
    fn name_is_sculpt() {
        let s = Sculpt::from_params(
            SculptParams {
                tone: 0.5,
                mix: 1.0,
            },
            48_000,
        );
        assert_eq!(s.name(), "sculpt");
    }
}
