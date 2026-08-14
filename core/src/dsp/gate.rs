//! Puerta de ruido: atenúa la señal por debajo de un umbral.
//!
//! Detecta la envolvente del nivel de entrada, abre la puerta cuando supera el
//! umbral y la cierra (con atenuación limitada por `range_db`) cuando cae por
//! debajo, tras mantenerla abierta durante `hold_ms`. El paso abierto → cerrado
//! y el cierre → apertura se suavizan con ataque/liberación en el dominio dB
//! para evitar "zipper noise" y cortes bruscos de la voz.

use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Puerta de ruido con umbral, ataque, liberación, *hold* y atenuación
/// (rango) configurables.
#[derive(Debug, Clone)]
pub struct NoiseGate {
    threshold_db: f32,
    attack_ms: f32,
    release_ms: f32,
    hold_ms: f32,
    range_db: f32,

    /// Coeficiente de ataque por muestra (apertura).
    attack_coef: f32,
    /// Coeficiente de liberación por muestra (cierre).
    release_coef: f32,
    /// Muestras que permanece abierta tras caer bajo el umbral (hold).
    hold_samples: u32,
    /// Envolvente detectada (amplitud lineal).
    envelope: f32,
    /// Atenuación aplicada actualmente (dB, 0 = abierta, −range = cerrada).
    gate_db: f32,
    /// `true` mientras la puerta está abierta (o en hold).
    open: bool,
    /// Muestras restantes de hold.
    hold_left: u32,
}

impl NoiseGate {
    /// Crea una puerta de ruido.
    ///
    /// Los tiempos se convierten a coeficientes por muestra según la frecuencia
    /// de muestreo; los valores inválidos se sanan (`range_db ≥ 0`, el resto
    /// ≥ 0). Arranca cerrada para no dejar pasar silencio inicial.
    pub fn new(
        threshold_db: f32,
        attack_ms: f32,
        release_ms: f32,
        hold_ms: f32,
        range_db: f32,
        sample_rate: u32,
    ) -> Self {
        let sr = sample_rate.max(1) as f32;
        Self {
            threshold_db,
            attack_ms,
            release_ms,
            hold_ms,
            range_db: range_db.max(0.0),
            attack_coef: time_to_coef(attack_ms, sr),
            release_coef: time_to_coef(release_ms, sr),
            hold_samples: ((hold_ms.max(0.0) / 1000.0) * sr) as u32,
            envelope: 0.0,
            gate_db: -range_db.max(0.0),
            open: false,
            hold_left: 0,
        }
    }

    /// Umbral en dBFS.
    pub fn threshold_db(&self) -> f32 {
        self.threshold_db
    }

    /// Tiempo de ataque (ms).
    pub fn attack_ms(&self) -> f32 {
        self.attack_ms
    }

    /// Tiempo de liberación (ms).
    pub fn release_ms(&self) -> f32 {
        self.release_ms
    }

    /// Tiempo de *hold* (ms).
    pub fn hold_ms(&self) -> f32 {
        self.hold_ms
    }

    /// Atenuación máxima aplicada al cerrar (dB).
    pub fn range_db(&self) -> f32 {
        self.range_db
    }
}

impl AudioProcessor for NoiseGate {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        for i in 0..frames {
            let sample = input[i];

            // 1) Envolvente (pico con suavizado ataque/liberación).
            let detected = sample.abs();
            let coef = if detected > self.envelope {
                self.attack_coef
            } else {
                self.release_coef
            };
            self.envelope += coef * (detected - self.envelope);

            // 2) Decisión abierta/cerrada con hold.
            let level_db = 20.0 * self.envelope.log10().max(-120.0);
            if level_db > self.threshold_db {
                self.open = true;
                self.hold_left = self.hold_samples;
            } else if self.open {
                if self.hold_left > 0 {
                    self.hold_left -= 1;
                } else {
                    self.open = false;
                }
            }

            // 3) Atenuación objetivo y suavizado en dB.
            let target_db = if self.open { 0.0 } else { -self.range_db };
            let coef = if target_db > self.gate_db {
                self.attack_coef
            } else {
                self.release_coef
            };
            self.gate_db += coef * (target_db - self.gate_db);

            output[i] = sample * 10f32.powf(self.gate_db / 20.0);
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "noisegate"
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.gate_db = -self.range_db;
        self.open = false;
        self.hold_left = 0;
    }
}

/// Convierte un tiempo (ms) en el coeficiente de suavizado por muestra:
/// `coef = 1 - exp(-1 / (τ·fs))`, con `τ = ms/1000`.
fn time_to_coef(ms: f32, sample_rate: f32) -> f32 {
    let tau = ms.max(0.001) / 1000.0;
    (1.0 - (-1.0 / (tau * sample_rate)).exp()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn info(frames: usize) -> ProcessingInfo {
        ProcessingInfo {
            sample_rate: SR,
            frames,
        }
    }

    #[test]
    fn silence_is_gated_to_the_floor() {
        let mut gate = NoiseGate::new(-40.0, 2.0, 80.0, 20.0, 40.0, SR);
        let n = 48_000; // 1 s
        let input = vec![0.0001; n]; // −80 dBFS: muy por debajo del umbral.
        let mut out = vec![0.0; n];
        gate.process(&input, &mut out, &info(n));
        // Cerrada: atenuación ≈ −range_db → salida ≈ entrada − 40 dB.
        let expected = 0.0001 * 10f32.powf(-40.0 / 20.0);
        assert!(
            (out[n - 1] - expected).abs() < 1e-9,
            "floor {:.3e} != {expected:.3e}",
            out[n - 1]
        );
    }

    #[test]
    fn signal_above_threshold_passes_almost_unity() {
        let mut gate = NoiseGate::new(-40.0, 2.0, 80.0, 20.0, 40.0, SR);
        let n = 4800; // 100 ms de señal fuerte.
        let input = vec![0.1; n]; // −20 dBFS.
        let mut out = vec![0.0; n];
        gate.process(&input, &mut out, &info(n));
        assert!(
            (out[n - 1] - 0.1).abs() < 1e-3,
            "gate abierta debe pasar ≈ unitario, got {:.4}",
            out[n - 1]
        );
    }

    #[test]
    fn range_limits_the_maximum_attenuation() {
        // Rango de 12 dB: el silencio no se corta del todo, solo se atenúa.
        let mut gate = NoiseGate::new(-40.0, 2.0, 80.0, 10.0, 12.0, SR);
        let n = 48_000;
        let input = vec![0.0001; n];
        let mut out = vec![0.0; n];
        gate.process(&input, &mut out, &info(n));
        let expected = 0.0001 * 10f32.powf(-12.0 / 20.0);
        assert!(
            (out[n - 1] - expected).abs() < 1e-9,
            "rango {:.3e} != {expected:.3e}",
            out[n - 1]
        );
    }

    #[test]
    fn hold_keeps_gate_open_after_burst() {
        // Ataque rápido para abrir del todo; hold de 500 ms retiene la apertura.
        let mut gate = NoiseGate::new(-40.0, 2.0, 5.0, 500.0, 40.0, SR);
        let burst = 2400; // 50 ms de señal fuerte (≥ umbral).
        let tail = 2400; // 50 ms de señal bajo el umbral (dentro del hold).
        let input: Vec<f32> = [vec![0.2; burst], vec![0.005; tail]].concat();
        let mut out = vec![0.0; input.len()];
        gate.process(&input, &mut out, &info(input.len()));
        // La sonda baja pasa casi sin atenuar: la puerta sigue abierta.
        assert!(
            out[input.len() - 1] > 0.004,
            "hold no respetado: {}",
            out[input.len() - 1]
        );
    }

    #[test]
    fn gate_closes_after_hold_and_release() {
        let mut gate = NoiseGate::new(-40.0, 1.0, 5.0, 50.0, 40.0, SR);
        let burst = 2400; // 50 ms fuertes.
        let silence = 24_000; // 500 ms de silencio (≫ hold 50 ms + release).
        let tail = 2400; // Sonda baja tras el cierre.
        let input: Vec<f32> = [vec![0.2; burst], vec![0.0; silence], vec![0.005; tail]].concat();
        let mut out = vec![0.0; input.len()];
        gate.process(&input, &mut out, &info(input.len()));
        // La puerta cerró: la sonda se atenúa por el rango completo (40 dB).
        let expected = 0.005 * 10f32.powf(-40.0 / 20.0);
        assert!(
            (out[input.len() - 1] - expected).abs() < 1e-5,
            "la puerta no cerró: {} != {expected}",
            out[input.len() - 1]
        );
    }

    #[test]
    fn initial_state_is_closed() {
        let mut gate = NoiseGate::new(-40.0, 2.0, 80.0, 20.0, 40.0, SR);
        let input = [0.0; 64];
        let mut out = [0.0; 64];
        gate.process(&input, &mut out, &info(64));
        assert_eq!(out, input);
    }
}
