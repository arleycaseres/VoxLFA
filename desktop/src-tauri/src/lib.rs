//! # voxlfa-desktop
//!
//! Cáscara de escritorio de **VoxLFA**. Orquesta tres piezas:
//!
//! 1. **`engine`** — ciclo de vida del motor de audio de `voxlfa-core`
//!    (arrancar/detener, estado compartido, difusión de eventos).
//! 2. **`pairing`** — código de emparejamiento para la app móvil.
//! 3. **`ws`** — servidor WebSocket local que difunde los eventos del motor al
//!    móvil y recibe sus comandos de control (autenticado por el código de
//!    emparejamiento).
//!
//! Con el feature `webview` (por defecto) se añade la integración Tauri:
//! comandos para la UI y la ventana de escritorio.
//!
//! Este crate **no** contiene lógica de audio: toda vive en `voxlfa-core`.

pub mod engine;
pub mod pairing;
pub mod ws;

pub use engine::{EngineError, EngineManager};

#[cfg(feature = "webview")]
mod tauri_app;

/// Arranca la aplicación de escritorio completa (Tauri).
///
/// Solo disponible con el feature `webview`.
#[cfg(feature = "webview")]
pub fn run() {
    tauri_app::run();
}
