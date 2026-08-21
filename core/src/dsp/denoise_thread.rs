//! Hilo dedicado de supresión de ruido (DeepFilterNet3 / RNNoise).
//!
//! Extrae la inferencia ONNX del callback de audio en tiempo real y la ejecuta
//! en un hilo separado sin restricciones de latencia. Comunica con el callback
//! mediante dos ring buffers lock-free:
//!
//! ```text
//! callback ──► [denoise_in ring]  ──► hilo denoise ──► [denoise_out ring] ──► callback
//! ```
//!
//! El hilo consume audio crudo del ring de entrada, lo procesa y escribe el
//! resultado en el ring de salida. Si el hilo se retrasa, el callback usa
//! directamente la señal sin denoise (degradación transparente).

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ringbuf::traits::{Consumer, Producer};

use super::processor::{AudioProcessor, ProcessingInfo};

/// Intervalo de descanso del hilo cuando no hay datos disponibles (μs).
const IDLE_SLEEP_US: u64 = 500;

/// Estado compartido entre el callback de audio y el hilo de denoise.
pub struct DenoiseShared {
    /// `true` cuando hay un procesador ONNX/RNNoise activo y el hilo debe
    /// ejecutar inferencia. Si es `false`, el hilo duerme y el callback usa
    /// la señal sin procesar.
    pub enabled: AtomicBool,
    /// Mezcla seco/húmedo (0.0–1.0) leída por el callback; el hilo siempre
    /// produce señal 100 % húmeda (denoise completo) para que el blend se
    /// controle en el callback.
    pub mix: AtomicF32,
}

/// Atómica para `f32` usando representación de bits (C-like, sin `AtomicF32`).
pub struct AtomicF32 {
    storage: std::sync::atomic::AtomicU32,
}

impl AtomicF32 {
    /// Crea una nueva atómica con el valor dado.
    pub fn new(val: f32) -> Self {
        Self {
            storage: std::sync::atomic::AtomicU32::new(val.to_bits()),
        }
    }

    /// Lee el valor actual.
    pub fn load(&self, order: Ordering) -> f32 {
        f32::from_bits(self.storage.load(order))
    }

    /// Almacena un nuevo valor.
    #[allow(dead_code)]
    pub fn store(&self, val: f32, order: Ordering) {
        self.storage.store(val.to_bits(), order);
    }
}

/// Mango de control del hilo de denoise (lado del hilo de audio/engine).
pub struct DenoiseHandle {
    thread: Option<JoinHandle<()>>,
    stop: std::sync::Arc<AtomicBool>,
}

impl DenoiseHandle {
    /// Solicita la detención del hilo y espera a que termine.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Lanza el hilo de denoise con los ring buffers dados.
///
/// El hilo toma posesión del `processor` (ONNX/RNNoise/Passthrough) y de los
/// consumidores/productores de los ring buffers. Devuelve un [`DenoiseHandle`]
/// para detener el hilo al cerrar el motor.
pub fn spawn_denoise_thread<C, P>(
    processor: Box<dyn AudioProcessor>,
    sample_rate: u32,
    mut ring_in: C,
    mut ring_out: P,
    shared: std::sync::Arc<DenoiseShared>,
    stop: std::sync::Arc<AtomicBool>,
) -> Result<DenoiseHandle, crate::Error>
where
    C: Consumer<Item = f32> + Send + 'static,
    P: Producer<Item = f32> + Send + 'static,
{
    let stop_clone = stop.clone();
    let handle = thread::Builder::new()
        .name("voxlfa-denoise".to_string())
        .spawn(move || {
            denoise_loop(
                processor,
                sample_rate,
                &mut ring_in,
                &mut ring_out,
                &shared,
                &stop_clone,
            );
        })
        .map_err(|e| crate::Error::audio(format!("spawn denoise thread: {e}")))?;

    Ok(DenoiseHandle {
        thread: Some(handle),
        stop,
    })
}

/// Loop principal del hilo de denoise.
///
/// Lee bloques del ring de entrada, los procesa y escribe el resultado en el
/// ring de salida. Cuando no hay datos disponibles, duerme brevemente para no
/// consumir CPU.
fn denoise_loop<C, P>(
    mut processor: Box<dyn AudioProcessor>,
    sample_rate: u32,
    ring_in: &mut C,
    ring_out: &mut P,
    shared: &DenoiseShared,
    stop: &AtomicBool,
) where
    C: Consumer<Item = f32>,
    P: Producer<Item = f32>,
{
    // Buffers reutilizables (sin asignación en el loop).
    let mut input_buf = vec![0.0f32; super::MAX_DENOISE_CHUNK];
    let mut output_buf = vec![0.0f32; super::MAX_DENOISE_CHUNK];

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // Si el denoise está deshabilitado, solo consume y descarta.
        if !shared.enabled.load(Ordering::Relaxed) {
            let n = ring_in.pop_slice(&mut input_buf);
            if n == 0 {
                thread::sleep(Duration::from_micros(IDLE_SLEEP_US));
            }
            continue;
        }

        // Leer el mayor bloque disponible (hasta el tamaño del buffer).
        let n = ring_in.pop_slice(&mut input_buf);
        if n == 0 {
            thread::sleep(Duration::from_micros(IDLE_SLEEP_US));
            continue;
        }

        // Procesar (ONNX inference, RNNoise, etc.).
        let info = ProcessingInfo {
            sample_rate,
            frames: n,
        };
        processor.process(&input_buf[..n], &mut output_buf[..n], &info);

        // Escribir resultado al ring de salida.
        ring_out.push_slice(&output_buf[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::traits::Split as _;
    use ringbuf::HeapRb;

    #[test]
    fn denoise_thread_passes_audio_through() {
        let (mut in_prod, in_cons) = HeapRb::<f32>::new(1024).split();
        let (out_prod, mut out_cons) = HeapRb::<f32>::new(1024).split();

        let shared = std::sync::Arc::new(DenoiseShared {
            enabled: AtomicBool::new(true),
            mix: AtomicF32::new(1.0),
        });
        let stop = std::sync::Arc::new(AtomicBool::new(false));

        let processor: Box<dyn AudioProcessor> =
            Box::new(super::super::passthrough::PassThroughProcessor::default());

        let handle = spawn_denoise_thread(
            processor,
            48_000,
            in_cons,
            out_prod,
            shared.clone(),
            stop.clone(),
        )
        .unwrap();

        let test_data: Vec<f32> = (0..256).map(|i| i as f32 * 0.01).collect();
        in_prod.push_slice(&test_data);

        thread::sleep(Duration::from_millis(50));

        let mut output = vec![0.0f32; 256];
        let n = out_cons.pop_slice(&mut output);

        assert_eq!(n, 256, "debería recibir 256 muestras denoised");
        for i in 0..256 {
            assert!(
                (output[i] - test_data[i]).abs() < 1e-6,
                "passthrough falló en sample {i}: {} != {}",
                output[i],
                test_data[i]
            );
        }

        stop.store(true, Ordering::Relaxed);
        handle.stop();
    }

    #[test]
    fn denoise_thread_skips_when_disabled() {
        let (mut in_prod, in_cons) = HeapRb::<f32>::new(1024).split();
        let (out_prod, mut out_cons) = HeapRb::<f32>::new(1024).split();

        let shared = std::sync::Arc::new(DenoiseShared {
            enabled: AtomicBool::new(false),
            mix: AtomicF32::new(0.0),
        });
        let stop = std::sync::Arc::new(AtomicBool::new(false));

        let processor: Box<dyn AudioProcessor> =
            Box::new(super::super::passthrough::PassThroughProcessor::default());

        let handle = spawn_denoise_thread(
            processor,
            48_000,
            in_cons,
            out_prod,
            shared.clone(),
            stop.clone(),
        )
        .unwrap();

        let test_data: Vec<f32> = vec![0.5; 128];
        in_prod.push_slice(&test_data);

        thread::sleep(Duration::from_millis(50));

        let mut output = vec![0.0f32; 128];
        let n = out_cons.pop_slice(&mut output);
        assert_eq!(
            n, 0,
            "no debería haber datos cuando denoise está deshabilitado"
        );

        stop.store(true, Ordering::Relaxed);
        handle.stop();
    }
}
