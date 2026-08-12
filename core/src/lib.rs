//! # voxlfa-core
//!
//! Motor DSP + IA de **VoxLFA**: procesador vocal en vivo que toma audio de un
//! micrófono, lo limpia y lo mejora en tiempo real.
//!
//! Este crate es el núcleo compartido del monorepo y se reutiliza en todas las
//! plataformas (desktop y, en el futuro, móvil). Por diseño:
//!
//! - **No depende de la UI ni de Tauri.** Puede usarse desde cualquier
//!   aplicación.
//! - **El DSP es puro** (`dsp/`): procesa bloques de audio sin tocar hardware.
//! - **La I/O de audio es opcional** (`audio/`, feature `audio`): captura y
//!   salida con `cpal`, medida de latencia y dispositivos.
//! - **El protocolo** (`protocol/`) define el contrato de datos con la UI y la
//!   app móvil mediante tipos serde serializados a JSON.
//!
//! ## Ejemplo mínimo (passthrough con latencia medida)
//!
//! ```no_run
//! # #[cfg(feature = "audio")]
//! # fn main() -> voxlfa_core::Result<()> {
//! use std::sync::mpsc;
//! use voxlfa_core::audio::{AudioEngine, AudioEngineConfig};
//! use voxlfa_core::protocol::EngineEvent;
//!
//! let (tx, rx) = mpsc::channel();
//! let (handle, dsp, _analysis) = AudioEngine::start(AudioEngineConfig::default(), tx)?;
//!
//! // Escuchar eventos del motor en este hilo...
//! for event in rx {
//!     if let EngineEvent::Level(sample) = event {
//!         println!("RMS {:.1} dBFS, latencia {:.1} ms", sample.input_rms_db, sample.latency_ms);
//!     }
//! }
//! # drop(handle);
//! # drop(dsp);
//! # Ok(())
//! # }
//! #
//! # #[cfg(not(feature = "audio"))]
//! # fn main() {}
//! ```
//!
//! ## Características (`features`)
//!
//! - `audio`: motor de captura/salida en tiempo real con `cpal` (solo escritorio).
//!
//! Los nombres de los módulos, tipos y funciones están en inglés; los
//! comentarios y la documentación, en español.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod analysis;
pub mod config;
pub mod dsp;
pub mod error;
pub mod protocol;

#[cfg(feature = "audio")]
pub mod audio;

pub use error::Error;

/// Resultado de operaciones que pueden fallar en el core.
pub type Result<T> = std::result::Result<T, Error>;
