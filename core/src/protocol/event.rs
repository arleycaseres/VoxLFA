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

/// Descripción de un host de audio disponible (ALSA, JACK, PipeWire, etc.).
///
/// Los hosts representan los backends de audio del sistema operativo. Cada host
/// tiene su propio conjunto de dispositivos; cambiar de host permite acceder a
/// interfaces de audio profesionales (JACK) o al stack moderno de Linux
/// (PipeWire) sin competir con el backend predeterminado (ALSA/WASAPI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioHostInfo {
    /// Identificador del host (p. ej. `"alsa"`, `"jack"`, `"pipewire"`).
    pub id: String,
    /// Nombre legible para la UI (p. ej. `"ALSA"`, `"JACK"`, `"PipeWire"`).
    pub name: String,
    /// `true` si el sistema lo tiene como predeterminado.
    pub is_default: bool,
}

/// Número fijo de bandas logarítmicas del espectro emitido por el motor.
///
/// El motor calcula una FFT sobre la entrada y reduce el resultado a estas
/// bandas (20 Hz → Nyquist). Mantén el mismo valor en los consumidores
/// (desktop y móvil); es parte del contrato de datos.
pub const SPECTRUM_BIN_COUNT: usize = 32;

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
    /// Espectro de la entrada en vivo (FFT reducida a bandas logarítmicas).
    Spectrum(SpectrumSample),
    /// Listado de dispositivos de entrada/salida disponibles.
    Devices {
        /// Dispositivos de captura (micrófonos, interfaces).
        inputs: Vec<AudioDeviceInfo>,
        /// Dispositivos de reproducción.
        outputs: Vec<AudioDeviceInfo>,
    },
    /// Estado de la cadena DSP (preset activo, módulos y bypass).
    Dsp(super::dsp::DspState),
    /// Análisis vocal en tiempo real (métricas + sugerencias de IA).
    Analysis(super::analysis::AnalysisSample),
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
    /// Host de audio activo (p. ej. `"alsa"`, `"jack"`).
    pub audio_host: Option<String>,
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

/// Muestra del espectro de la entrada (FFT) emitida en vivo.
///
/// La FFT (ventana Hann, 50 % de solapamiento) se reduce a
/// [`SPECTRUM_BIN_COUNT`] bandas logarítmicas entre ~20 Hz y el Nyquist de
/// `sample_rate`; cada valor es el nivel pico de la banda en dBFS, suavizado
/// con un envolvente ataque/release para estabilizar la visualización.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumSample {
    /// Nivel de cada banda logarítmica en dBFS (longitud fija).
    pub bins_db: [f32; SPECTRUM_BIN_COUNT],
    /// Frecuencia de muestreo (Hz) de la captura; define los bordes de banda.
    pub sample_rate: u32,
    /// Tiempo monotónico (ms) de la captura.
    pub captured_at_ms: u64,
}
