//! Tipos de error del crate.
//!
//! Los mensajes internos están en inglés (estándar de logs); la UI los
//! traduce si lo necesita.

use thiserror::Error;

/// Error unificado del crate `voxlfa-core`.
///
/// Se usa `?` en todo el código de producción (nada de `unwrap()`/`expect()`).
#[derive(Debug, Error)]
pub enum Error {
    /// Error del motor de audio o de los dispositivos de captura/salida.
    #[error("audio error: {0}")]
    Audio(String),

    /// Error al serializar/deserializar mensajes del protocolo.
    #[error("protocol serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Error de I/O del sistema.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Construye un [`Error::Audio`] con un mensaje formateado.
    pub fn audio<S: Into<String>>(message: S) -> Self {
        Error::Audio(message.into())
    }
}
