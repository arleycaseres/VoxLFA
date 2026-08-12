//! Integración de la aplicación con Tauri (solo con el feature `webview`).
//!
//! Define el estado global, los comandos que la UI invoca y el arranque de la
//! ventana. No hay lógica de audio aquí: solo orquestación.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::broadcast;
use voxlfa_core::audio::AudioEngineConfig;
use voxlfa_core::config::AppConfig;
use voxlfa_core::protocol::{
    AnalysisSample, AudioDeviceInfo, DspState, EngineEvent, EngineStatus, PresetId, PresetInfo,
    SessionSummary,
};

use crate::engine::EngineManager;
use crate::pairing::generate_pairing_code;
use crate::ws::run_ws_server;

/// Puerto del WebSocket de monitoreo remoto (app móvil).
pub const WS_PORT: u16 = 4356;

/// Capacidad del canal broadcast de eventos serializados.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Longitud del código de emparejamiento mostrado al usuario.
const PAIRING_CODE_LENGTH: usize = 6;

/// Estado global de la aplicación, gestionado por Tauri.
pub struct AppState {
    /// Gestor del motor de audio (protegido contra acceso concurrente).
    pub engine: Mutex<EngineManager>,
    /// Emisor de eventos serializados (JSON) hacia el WebSocket.
    pub events: broadcast::Sender<String>,
    /// Código de emparejamiento para la app móvil (nunca se loguea).
    pub pairing_code: Arc<String>,
}

impl AppState {
    fn new() -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            engine: Mutex::new(EngineManager::new(events.clone())),
            events,
            pairing_code: Arc::new(generate_pairing_code(PAIRING_CODE_LENGTH)),
        }
    }
}

/// Respuesta del comando `list_devices`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceListResponse {
    inputs: Vec<AudioDeviceInfo>,
    outputs: Vec<AudioDeviceInfo>,
}

/// Respuesta del comando `get_pairing_info`: datos para conectar el móvil.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingInfo {
    /// Código de emparejamiento que el móvil debe ingresar.
    code: String,
    /// Puerto del WebSocket de monitoreo.
    port: u16,
    /// IP local del escritorio en la red (si se puede detectar).
    lan_address: Option<String>,
}

/// Lista los dispositivos de entrada y salida disponibles.
#[tauri::command]
fn list_devices() -> Result<DeviceListResponse, String> {
    let (inputs, outputs) =
        voxlfa_core::audio::AudioEngine::list_devices().map_err(|err| err.to_string())?;
    Ok(DeviceListResponse { inputs, outputs })
}

/// Arranca el motor con los dispositivos indicados (`None` = predeterminado).
/// `buffer_size` (muestras/callback) es opcional: si es `None`, el core elige
/// uno automáticamente según el tipo de dispositivo (heurística de latencia).
#[tauri::command]
fn start_engine(
    app: AppHandle,
    state: State<AppState>,
    input_device: Option<String>,
    output_device: Option<String>,
    buffer_size: Option<usize>,
) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;

    // Reenviar cada evento del motor a la UI de la ventana.
    let app_handle = app.clone();
    let on_frontend = Some(move |event: &EngineEvent| {
        let _ = app_handle.emit("engine-event", event);
    });

    engine
        .start(
            AudioEngineConfig {
                input_device,
                output_device,
                buffer_size,
                ..Default::default()
            },
            on_frontend,
        )
        .map_err(|err| err.to_string())
}

/// Detiene el motor de forma controlada.
#[tauri::command]
fn stop_engine(state: State<AppState>) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine.stop();
    Ok(())
}

/// Devuelve el último estado conocido del motor (o `null` si nunca arrancó).
#[tauri::command]
fn get_engine_status(state: State<AppState>) -> Result<Option<EngineStatus>, String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    Ok(engine.last_status())
}

/// Devuelve el último nivel medido (renderizado inicial de la UI).
#[tauri::command]
fn get_last_level(
    state: State<AppState>,
) -> Result<Option<voxlfa_core::protocol::LevelSample>, String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    Ok(engine.last_level())
}

/// Lista los presets de la cabina con sus metadatos.
#[tauri::command]
fn get_presets() -> Result<Vec<PresetInfo>, String> {
    Ok(voxlfa_core::dsp::PresetFactory::all())
}

/// Último estado de la cadena DSP conocido (o `null` si el motor no corre).
#[tauri::command]
fn get_dsp_state(state: State<AppState>) -> Result<Option<DspState>, String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    Ok(engine.last_dsp_state())
}

/// Última muestra de análisis vocal (o `null` si el motor no arrancó).
#[tauri::command]
fn get_analysis(state: State<AppState>) -> Result<Option<AnalysisSample>, String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    let analysis = engine
        .analysis_handle()
        .ok_or_else(|| "el motor no está corriendo".to_string())?;
    analysis.get_analysis().map_err(|err| err.to_string())
}

/// Resumen acumulado de la sesión actual (o `null` si no hubo sesión).
#[tauri::command]
fn get_session_summary(state: State<AppState>) -> Result<Option<SessionSummary>, String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    let analysis = engine
        .analysis_handle()
        .ok_or_else(|| "el motor no está corriendo".to_string())?;
    analysis
        .get_session_summary()
        .map_err(|err| err.to_string())
}

/// Aplica la acción de una sugerencia (con confirmación del usuario).
#[tauri::command]
fn apply_suggestion(state: State<AppState>, suggestion_id: u8) -> Result<(), String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    let analysis = engine
        .analysis_handle()
        .ok_or_else(|| "el motor no está corriendo".to_string())?;
    analysis
        .apply_suggestion(suggestion_id)
        .map_err(|err| err.to_string())
}

/// Aplica un preset a la cadena DSP en vivo (y lo persiste en el perfil del
/// dispositivo actual).
#[tauri::command]
fn apply_preset(state: State<AppState>, preset: PresetId) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine.apply_preset(preset).map_err(|err| err.to_string())
}

/// Activa o desactiva el bypass global de la cadena DSP (y lo persiste).
#[tauri::command]
fn set_global_bypass(state: State<AppState>, bypass: bool) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine
        .set_global_bypass(bypass)
        .map_err(|err| err.to_string())
}

/// Activa o desactiva el bypass de un módulo por su nombre (y lo persiste).
#[tauri::command]
fn set_link_bypass(state: State<AppState>, link: String, bypass: bool) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine
        .set_link_bypass(link, bypass)
        .map_err(|err| err.to_string())
}

/// Ajusta la ganancia de una banda del EQ del preset activo en vivo.
#[tauri::command]
fn set_eq_band(state: State<AppState>, band_index: usize, gain_db: f32) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine
        .set_eq_band(band_index, gain_db)
        .map_err(|err| err.to_string())
}

/// Devuelve la configuración persistida (para precargar la cabina).
#[tauri::command]
fn get_config(state: State<AppState>) -> Result<AppConfig, String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    Ok(engine.get_config())
}

/// Datos de emparejamiento para conectar la app móvil por WebSocket.
#[tauri::command]
fn get_pairing_info(state: State<AppState>) -> Result<PairingInfo, String> {
    let lan_address = local_ip_address::local_ip().ok().map(|ip| ip.to_string());
    Ok(PairingInfo {
        code: state.pairing_code.to_string(),
        port: WS_PORT,
        lan_address,
    })
}

/// Arranca la aplicación Tauri completa (ventana + backend + WebSocket).
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            list_devices,
            start_engine,
            stop_engine,
            get_engine_status,
            get_last_level,
            get_presets,
            get_dsp_state,
            apply_preset,
            set_global_bypass,
            set_link_bypass,
            set_eq_band,
            get_config,
            get_analysis,
            get_session_summary,
            apply_suggestion,
            get_pairing_info,
        ])
        .setup(|app| {
            // Servidor WebSocket para el monitoreo remoto desde el móvil.
            let state = app.state::<AppState>();
            let events = state.events.clone();
            let code = state.pairing_code.to_string();
            tauri::async_runtime::spawn(run_ws_server(events, code, WS_PORT));
            Ok(())
        });

    match builder.build(tauri::generate_context!()) {
        Ok(app) => {
            app.run(|app_handle, event| {
                // Detener el motor limpiamente al salir de la app.
                if let tauri::RunEvent::Exit = event {
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        if let Ok(mut engine) = state.engine.lock() {
                            engine.stop();
                        }
                    }
                }
            });
        }
        Err(err) => {
            eprintln!("[voxlfa] error al inicializar la aplicación: {err}");
            std::process::exit(1);
        }
    }
}
