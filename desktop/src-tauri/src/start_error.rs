//! Error estructurado que se serializa al frontend desde `start_engine`.
//!
//! A diferencia de [`crate::engine::EngineError`] (que es el error interno del
//! `EngineManager`), este tipo está pensado para llegar a la UI vía Tauri con
//! campos distinguidos (`tag = "kind"`) que permiten decidir en React si se
//! muestra un diálogo de confirmación, un error genérico, etc.

use serde::Serialize;

/// Error estructurado del comando `start_engine`.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StartEngineError {
    /// El motor ya está corriendo; no se puede arrancar dos veces.
    AlreadyRunning,
    /// El dispositivo no respondió dentro del tiempo de espera.
    Timeout {
        /// Nombre del dispositivo que se intentó abrir (si se conoce).
        device: Option<String>,
    },
    /// El dispositivo tiene un intento de apertura huérfano: el timeout ya se
    /// cumplió pero el hilo de cpal puede seguir bloqueado a nivel de kernel.
    /// El frontend debe pedir confirmación antes de reintentar.
    DeviceLikelyStuck {
        /// Nombre del dispositivo.
        device: String,
        /// Timestamp (epoch seconds) de cuándo quedó huérfano.
        orphaned_at: u64,
        /// Cuántas veces este dispositivo ha quedado huérfano en esta sesión.
        stuck_count: u32,
    },
    /// Error genérico del core de audio o del dispositivo.
    Core {
        /// Mensaje descriptivo.
        message: String,
    },
}

impl From<crate::engine::EngineError> for StartEngineError {
    fn from(err: crate::engine::EngineError) -> Self {
        match err {
            crate::engine::EngineError::AlreadyRunning => Self::AlreadyRunning,
            crate::engine::EngineError::Core(e) => Self::Core {
                message: e.to_string(),
            },
            // EngineError::NotRunning no debería ocurrir en start_engine.
            other => Self::Core {
                message: other.to_string(),
            },
        }
    }
}
