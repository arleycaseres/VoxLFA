//! Comandos de control que la UI envía al motor.

use serde::{Deserialize, Serialize};

/// Comando de control del motor, enviado por la UI (o el móvil) hacia el core.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ControlCommand {
    /// Arranca el pipeline con los dispositivos indicados.
    ///
    /// Si un dispositivo es `None`, se usa el predeterminado del sistema.
    Start {
        /// Nombre del dispositivo de entrada (micrófono).
        input_device: Option<String>,
        /// Nombre del dispositivo de salida (altavoces/interfaz).
        output_device: Option<String>,
    },
    /// Detiene el pipeline de forma controlada.
    Stop,
}
