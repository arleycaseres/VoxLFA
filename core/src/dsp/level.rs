//! Medición de nivel de audio (RMS y pico) en dBFS.
//!
//! Referencia: 0 dBFS = muestra de amplitud 1.0 (escala completa). El piso de
//! -120 dBFS representa el silencio para que la UI no tenga que lidiar con
//! `-inf` (y para que la serialización a JSON no falle con NaN).

/// Valor de dBFS que representa el silencio en los reportes.
pub const DB_FLOOR: f32 = -120.0;

/// Niveles calculados para un bloque de audio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levels {
    /// Nivel RMS en dBFS.
    pub rms_db: f32,
    /// Nivel pico en dBFS.
    pub peak_db: f32,
}

/// Convierte una amplitud a dBFS con piso de silencio y sin NaN/±inf.
pub fn amplitude_to_db(amplitude: f32) -> f32 {
    if amplitude <= 0.0 || !amplitude.is_finite() {
        DB_FLOOR
    } else {
        (20.0 * amplitude.log10()).clamp(DB_FLOOR, 0.0)
    }
}

/// Mide RMS y pico de bloques de audio (sin estado interno por ahora).
#[derive(Debug, Default)]
pub struct LevelMeter;

impl LevelMeter {
    /// Crea un medidor de nivel.
    pub fn new() -> Self {
        Self
    }

    /// Mide el RMS y el pico del bloque dado, en dBFS.
    pub fn process(&mut self, samples: &[f32]) -> Levels {
        if samples.is_empty() {
            return Levels {
                rms_db: DB_FLOOR,
                peak_db: DB_FLOOR,
            };
        }

        let mut sum_sq = 0.0f64;
        let mut peak: f32 = 0.0;
        for &sample in samples {
            sum_sq += (sample as f64) * (sample as f64);
            peak = peak.max(sample.abs());
        }
        // RMS = raíz cuadrada de la media de los cuadrados.
        let rms = ((sum_sq / samples.len() as f64) as f32).sqrt();

        Levels {
            rms_db: amplitude_to_db(rms),
            peak_db: amplitude_to_db(peak),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_reports_floor() {
        let mut meter = LevelMeter::new();
        let levels = meter.process(&[0.0, 0.0, 0.0]);
        assert_eq!(levels.rms_db, DB_FLOOR);
        assert_eq!(levels.peak_db, DB_FLOOR);
    }

    #[test]
    fn full_scale_sine_reports_peak_at_0_db() {
        // Onda cuadrada de amplitud 1.0: pico 0 dBFS.
        let mut meter = LevelMeter::new();
        let levels = meter.process(&[1.0, -1.0, 1.0, -1.0]);
        assert!((levels.peak_db - 0.0).abs() < 1e-3);
        // RMS de la onda cuadrada = 1.0 → 0 dB.
        assert!((levels.rms_db - 0.0).abs() < 1e-3);
    }

    #[test]
    fn constant_half_amplitude_gives_about_minus_6_db() {
        let mut meter = LevelMeter::new();
        let levels = meter.process(&[0.5; 64]);
        let expected = 20.0 * 0.5f32.log10(); // ≈ -6.02 dB
        assert!((levels.rms_db - expected).abs() < 1e-3);
        assert!((levels.peak_db - expected).abs() < 1e-3);
    }

    #[test]
    fn empty_block_never_panics() {
        let mut meter = LevelMeter::new();
        let levels = meter.process(&[]);
        assert_eq!(
            levels,
            Levels {
                rms_db: DB_FLOOR,
                peak_db: DB_FLOOR
            }
        );
    }
}
