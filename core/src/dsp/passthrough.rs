//! Procesador de paso directo (`passthrough`): copia la entrada a la salida.
//!
//! Es el punto de partida de la Fase 0: valida el pipeline de audio
//! (captura → procesado → salida) sin alterar la señal, sirviendo además de
//! plantilla para los procesadores reales de fases posteriores.

use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};

/// Procesador que copia la señal de entrada a la salida sin modificarla.
///
/// Reporta una latencia configurable (por defecto 0) para que la cadena pueda
/// simular el coste de un procesador real en pruebas.
#[derive(Debug, Clone, Copy)]
pub struct PassThroughProcessor {
    /// Latencia (ms) que este procesador reporta aportar.
    latency_ms: f32,
}

impl PassThroughProcessor {
    /// Crea un passthrough con la latencia reportada indicada.
    pub fn new(latency_ms: f32) -> Self {
        Self { latency_ms }
    }
}

impl Default for PassThroughProcessor {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl AudioProcessor for PassThroughProcessor {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        // La salida debe tener la misma longitud que la entrada; si el
        // consumidor la pide más corta, copiamos lo que quepa (nunca panics).
        let frames = input.len().min(output.len());
        output[..frames].copy_from_slice(&input[..frames]);
        ProcessResult {
            latency_ms: self.latency_ms,
        }
    }

    fn name(&self) -> &'static str {
        "passthrough"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_copies_input_to_output() {
        let input = [0.1, 0.5, -0.3, 0.0];
        let mut output = [0.0; 4];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4,
        };
        let result = PassThroughProcessor::new(2.5).process(&input, &mut output, &info);
        assert_eq!(output, input);
        assert!((result.latency_ms - 2.5).abs() < 1e-6);
    }

    #[test]
    fn passthrough_handles_shorter_output_without_panic() {
        let input = [0.1, 0.5, -0.3, 0.0];
        let mut output = [0.0; 2];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4,
        };
        PassThroughProcessor::default().process(&input, &mut output, &info);
        assert_eq!(output, [0.1, 0.5]);
    }
}
