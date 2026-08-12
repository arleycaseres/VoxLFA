//! Filtros biquad (RBJ Audio EQ Cookbook): LP, HP, pico y shelving.
//!
//! Implementan la función de transferencia estándar
//! `H(z) = (b0 + b1 z⁻¹ + b2 z⁻²) / (a0 + a1 z⁻¹ + a2 z⁻²)` en forma
//! directa 1. Se usan como bloques base del EQ y del de-esser.
//!
//! Nota: los coeficientes se calculan en la construcción (no en el hilo de
//! audio); en tiempo real solo se aplica la diferencia de la ecuación.

use std::f32::consts::PI;

/// Tipo de filtro que se quiere construir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiquadKind {
    /// Paso bajos (corta las frecuencias altas).
    LowPass,
    /// Paso altos (corta las frecuencias bajas).
    HighPass,
    /// Banda de pico (campana) con ganancia ajustable.
    Peaking,
    /// Shelf de graves.
    LowShelf,
    /// Shelf de agudos.
    HighShelf,
}

/// Parámetros de diseño de un filtro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiquadParams {
    /// Tipo de filtro.
    pub kind: BiquadKind,
    /// Frecuencia de corte o central (Hz).
    pub freq_hz: f32,
    /// Ganancia (dB) para pico y shelving.
    pub gain_db: f32,
    /// Factor de calidad Q.
    pub q: f32,
}

/// Coeficientes del filtro normalizados (a0 = 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BiquadCoeffs {
    /// Coeficiente del término b0 del numerador.
    pub b0: f32,
    /// Coeficiente del término b1 del numerador.
    pub b1: f32,
    /// Coeficiente del término b2 del numerador.
    pub b2: f32,
    /// Coeficiente del término a1 del denominador (a0 = 1).
    pub a1: f32,
    /// Coeficiente del término a2 del denominador (a0 = 1).
    pub a2: f32,
}

impl BiquadCoeffs {
    /// Calcula los coeficientes a partir del tipo, frecuencia y Q (cookbook).
    ///
    /// `sample_rate` debe ser > 0 y `freq_hz` en `(0, Nyquist)`; de lo
    /// contrario el filtro se degrada a paso directo (resultado seguro).
    pub fn design(params: BiquadParams, sample_rate: u32) -> Self {
        if sample_rate == 0 || !params.freq_hz.is_finite() || params.freq_hz <= 0.0 {
            return Self::passthrough();
        }
        let nyquist = sample_rate as f32 * 0.5;
        let f0 = params.freq_hz.min(nyquist * 0.99).max(1.0);
        let q = params.q.max(0.01);

        let w0 = 2.0 * PI * f0 / sample_rate as f32;
        let cos = w0.cos();
        let sin = w0.sin();
        let alpha = sin / (2.0 * q);
        let a = 10f32.powf(params.gain_db / 40.0);
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let (mut b0, mut b1, mut b2, a0, mut a1, mut a2) = match params.kind {
            BiquadKind::LowPass => {
                let b0 = (1.0 - cos) * 0.5;
                (b0, 1.0 - cos, b0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            BiquadKind::HighPass => {
                let b0 = (1.0 + cos) * 0.5;
                (b0, -(1.0 + cos), b0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
            BiquadKind::Peaking => (
                1.0 + alpha * a,
                -2.0 * cos,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos,
                1.0 - alpha / a,
            ),
            BiquadKind::LowShelf => {
                let b0 = a * ((a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha);
                let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos);
                let b2 = a * ((a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha);
                let a0 = (a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha;
                let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos);
                let a2 = (a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            BiquadKind::HighShelf => {
                let b0 = a * ((a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha);
                let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos);
                let b2 = a * ((a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha);
                let a0 = (a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha;
                let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos);
                let a2 = (a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        // Normalizar por a0 (a0 siempre > 0 para estos tipos).
        let inv_a0 = 1.0 / a0;
        b0 *= inv_a0;
        b1 *= inv_a0;
        b2 *= inv_a0;
        a1 *= inv_a0;
        a2 *= inv_a0;

        Self { b0, b1, b2, a1, a2 }
    }

    /// Coeficientes de paso directo (y[n] = x[n]).
    pub fn passthrough() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

/// Filtro biquad en tiempo real (estado interno).
#[derive(Debug, Clone)]
pub struct BiquadFilter {
    coeffs: BiquadCoeffs,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadFilter {
    /// Crea un filtro con los coeficientes dados (estado limpio).
    pub fn new(coeffs: BiquadCoeffs) -> Self {
        Self {
            coeffs,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// Diseña y crea un filtro a partir de los parámetros dados.
    pub fn design(params: BiquadParams, sample_rate: u32) -> Self {
        Self::new(BiquadCoeffs::design(params, sample_rate))
    }

    /// Limpia el estado interno (sin cambiar los coeficientes).
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// Procesa una muestra (equación en diferencias, forma directa 1).
    #[inline]
    pub fn process(&mut self, sample: f32) -> f32 {
        let c = &self.coeffs;
        let y = c.b0 * sample + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = sample;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(f: &mut BiquadFilter, input: &[f32]) -> Vec<f32> {
        input.iter().map(|&x| f.process(x)).collect()
    }

    #[test]
    fn lowpass_passes_dc_unity_gain() {
        let mut f = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::LowPass,
                freq_hz: 1000.0,
                gain_db: 0.0,
                q: 0.707,
            },
            48_000,
        );
        let out = run(&mut f, &[1.0; 256]);
        let steady = out[out.len() - 1];
        assert!((steady - 1.0).abs() < 1e-2, "DC gain {steady} != ~1");
    }

    #[test]
    fn highpass_removes_dc() {
        let mut f = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::HighPass,
                freq_hz: 80.0,
                gain_db: 0.0,
                q: 0.707,
            },
            48_000,
        );
        let out = run(&mut f, &[1.0; 1024]);
        let steady = out[out.len() - 1];
        assert!(steady.abs() < 1e-2, "DC residual {steady}");
    }

    #[test]
    fn peaking_boosts_center_frequency() {
        // Un seno a 1 kHz debe salir amplificado ~+12 dB con Q alto.
        let mut f = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::Peaking,
                freq_hz: 1000.0,
                gain_db: 12.0,
                q: 4.0,
            },
            48_000,
        );
        let sr = 48_000.0;
        let n = 4096;
        let mut input = Vec::with_capacity(n);
        for i in 0..n {
            input.push((2.0 * PI * 1000.0 * i as f32 / sr).sin() * 0.1);
        }
        let out = run(&mut f, &input);
        // Comparar RMS de la segunda mitad (transitorio ya estable).
        let gain = rms(&out[n / 2..]) / rms(&input[n / 2..]);
        let expected = 10f32.powf(12.0 / 20.0);
        assert!(
            (gain - expected).abs() < 0.4,
            "peaking gain {gain:.2} != {expected:.2}"
        );
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn invalid_params_degrade_to_passthrough() {
        let f = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::LowPass,
                freq_hz: 0.0,
                gain_db: 0.0,
                q: 1.0,
            },
            0,
        );
        assert_eq!(f.coeffs, BiquadCoeffs::passthrough());
    }
}
