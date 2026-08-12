//! Contrato de los procesadores de audio y metadatos asociados.

/// Metadatos de un bloque de audio que se va a procesar.
#[derive(Debug, Clone, Copy)]
pub struct ProcessingInfo {
    /// Frecuencia de muestreo del pipeline en Hz.
    pub sample_rate: u32,
    /// Número de muestras por canal del bloque (frames).
    pub frames: usize,
}

/// Resultado del procesamiento de un bloque.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessResult {
    /// Latencia estimada (ms) que aporta este procesador en esta pasada.
    pub latency_ms: f32,
}

/// Contrato de un procesador de audio.
///
/// Transforma un bloque de entrada en un bloque de salida de la misma longitud.
/// La implementación no debe asignar memoria ni hacer I/O: corre en el hilo de
/// audio en tiempo real. Debe ser `Send` porque los callbacks de cpal corren en
/// hilos ajenos.
pub trait AudioProcessor: Send {
    /// Procesa `input` y escribe el resultado en `output` (misma longitud).
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        info: &ProcessingInfo,
    ) -> ProcessResult;

    /// Nombre corto del procesador (identificador para la UI/bypass).
    fn name(&self) -> &'static str {
        "processor"
    }

    /// Limpia el estado interno (buffers de retardo, envolventes, etc.).
    ///
    /// Se usa al reiniciar el pipeline o cambiar la frecuencia de muestreo.
    fn reset(&mut self) {}
}
