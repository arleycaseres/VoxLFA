//! Eventos emitidos por el motor hacia la UI y el móvil.
//!
//! Todos se serializan con `tag = "type"` para que el receptor pueda hacer
//! dispatch por el discriminante `type`.

use serde::{Deserialize, Serialize};

/// Estado general del motor de audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineState {
    /// Motor detenido; sin flujo de audio.
    Stopped,
    /// Motor inicializando dispositivos y streams.
    Starting,
    /// Capturando y reproduciendo audio en vivo.
    Running,
    /// Deteniéndose de forma controlada.
    Stopping,
    /// Estado de error; el pipeline no está operativo.
    Error,
}

/// Descripción de un dispositivo de audio disponible en el sistema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    /// Nombre único del dispositivo (identificador que usa el motor).
    pub name: String,
    /// `true` si el sistema lo tiene configurado como predeterminado.
    pub is_default: bool,
}

/// Evento de alto nivel del motor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EngineEvent {
    /// Cambio de estado del motor (ver [`EngineState`]).
    Status(EngineStatus),
    /// Muestra de nivel + latencia en tiempo real (frecuencia acotada).
    Level(LevelSample),
    /// Listado de dispositivos de entrada/salida disponibles.
    Devices {
        /// Dispositivos de captura (micrófonos, interfaces).
        inputs: Vec<AudioDeviceInfo>,
        /// Dispositivos de reproducción.
        outputs: Vec<AudioDeviceInfo>,
    },
    /// Estado de la cadena DSP (preset activo, módulos y bypass).
    Dsp(super::dsp::DspState),
    /// Aviso no fatal (underrun de salida, etc.). El motor sigue corriendo.
    Warning {
        /// Descripción legible del aviso (en inglés, para logs).
        message: String,
    },
}

/// Instantánea del estado y configuración del motor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    /// Estado actual del motor.
    pub state: EngineState,
    /// Frecuencia de muestreo (Hz) usada por el pipeline.
    pub sample_rate: u32,
    /// Tamaño de buffer en muestras por callback.
    pub buffer_size: usize,
    /// Latencia medida captura→salida en milisegundos.
    pub latency_ms: f32,
    /// Nombre del dispositivo de entrada en uso (si hay).
    pub input_device: Option<String>,
    /// Nombre del dispositivo de salida en uso (si hay).
    pub output_device: Option<String>,
}

/// Muestra de nivel del audio capturado y latencia actual.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelSample {
    /// Nivel RMS de la entrada en dBFS (silencioso ≈ `-120.0`).
    pub input_rms_db: f32,
    /// Nivel pico de la entrada en dBFS.
    pub input_peak_db: f32,
    /// Nivel RMS de la salida (tras la cadena DSP) en dBFS.
    pub output_rms_db: f32,
    /// Nivel pico de la salida en dBFS.
    pub output_peak_db: f32,
    /// Latencia actual captura→salida en milisegundos.
    pub latency_ms: f32,
    /// Tiempo monotónico (ms) de la captura; útil para gráficas.
    pub captured_at_ms: u64,
}
