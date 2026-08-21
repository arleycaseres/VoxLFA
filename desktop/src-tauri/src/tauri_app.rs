//! Integración de la aplicación con Tauri (solo con el feature `webview`).
//!
//! Define el estado global, los comandos que la UI invoca y el arranque de la
//! ventana. No hay lógica de audio aquí: solo orquestación.

use std::sync::{Arc, Mutex};

use log::info;
use once_cell::sync::Lazy;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::broadcast;
use voxlfa_core::audio::AudioEngineConfig;
use voxlfa_core::config::AppConfig;
use voxlfa_core::protocol::{
    AnalysisSample, AudioDeviceInfo, AudioHostInfo, DspState, EngineEvent, EngineStatus, PresetId,
    PresetInfo, SessionSummary, Suggestion,
};
use voxlfa_core::telemetry;

use crate::engine::EngineManager;
use crate::mdns::MdnsAdvertiser;
use crate::pairing::{PairingState, DEFAULT_CODE_LENGTH};
use crate::ws::run_ws_server;

/// Puerto del WebSocket de monitoreo remoto (app móvil).
pub const WS_PORT: u16 = 4356;

/// Endpoint HTTP para telemetría anónima (opt-in).
///
/// Se envía un POST por evento con body JSON. Cuando se despliegue el
/// endpoint real, actualice esta constante. Si la URL está vacía, el
/// envío HTTP se desactiva (solo stderr).
const TELEMETRY_ENDPOINT: &str = "";

/// Clave de la API de Groq para el asesor de IA.
///
/// Se lee de la variable de entorno `GROQ_API_KEY` en tiempo de ejecución.
/// Si está vacía o no existe, el asesor de IA no estará disponible (la UI
/// muestra un mensaje).
static GROQ_API_KEY: Lazy<String> = Lazy::new(|| std::env::var("GROQ_API_KEY").unwrap_or_default());

/// Capacidad del canal broadcast de eventos serializados.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Capacidad del canal broadcast de códigos de emparejamiento rotados.
const PAIRING_EVENT_CHANNEL_CAPACITY: usize = 8;

/// Evento que la cabina escucha para refrescar el código de emparejamiento.
pub const PAIRING_EVENT_NAME: &str = "pairing-event";

/// Estado global de la aplicación, gestionado por Tauri.
pub struct AppState {
    /// Gestor del motor de audio (protegido contra acceso concurrente).
    ///
    /// Es un `Arc` para compartirlo con el servidor WebSocket, que ejecuta los
    /// comandos de control del móvil contra el mismo gestor que la UI.
    pub engine: Arc<Mutex<EngineManager>>,
    /// Emisor de eventos serializados (JSON) hacia el WebSocket.
    pub events: broadcast::Sender<String>,
    /// Estado del código de emparejamiento (se rota tras fallos; el WS lo
    /// comparte con la cabina).
    pub pairing: Arc<Mutex<PairingState>>,
    /// Emisor del código nuevo cuando el emparejamiento rota (solo cabina).
    pub pairing_events: broadcast::Sender<String>,
    /// Anuncio mDNS del escritorio (`_voxlfa._tcp.local.`) para que el móvil lo
    /// descubra; `None` si no se pudo publicar.
    pub mdns: Option<MdnsAdvertiser>,
    /// Receptor de eventos de telemetría (para enviar al backend de telemetría).
    telemetry_rx: std::sync::Mutex<Option<telemetry::TelemetryReceiver>>,
    /// Últimas sugerencias generadas por el asesor de IA (Groq).
    ai_suggestions: std::sync::Mutex<Vec<Suggestion>>,
}

impl AppState {
    fn new() -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let (pairing_events, _) = broadcast::channel(PAIRING_EVENT_CHANNEL_CAPACITY);
        let (telemetry_handle, telemetry_rx) = telemetry::channel();
        // Autodetección: anunciar el escritorio por mDNS con la IP LAN actual.
        // Si falla (red sin multicast, p. ej.) la app sigue funcionando; solo
        // se pierde el descubrimiento automático del móvil.
        let mdns = local_ip_address::local_ip()
            .ok()
            .and_then(|ip| MdnsAdvertiser::start(&[ip], WS_PORT).ok());
        Self {
            engine: Arc::new(Mutex::new(EngineManager::new(
                events.clone(),
                telemetry_handle,
            ))),
            events,
            pairing: Arc::new(Mutex::new(PairingState::new(DEFAULT_CODE_LENGTH))),
            pairing_events,
            mdns,
            telemetry_rx: std::sync::Mutex::new(Some(telemetry_rx)),
            ai_suggestions: std::sync::Mutex::new(Vec::new()),
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

/// Respuesta del comando `list_audio_hosts`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostListResponse {
    hosts: Vec<AudioHostInfo>,
    default_id: String,
}

/// Lista los hosts de audio disponibles (ALSA, JACK, PipeWire, etc.).
#[tauri::command]
fn list_audio_hosts() -> Result<HostListResponse, String> {
    let (hosts, default_id) =
        voxlfa_core::audio::AudioEngine::list_hosts().map_err(|err| err.to_string())?;
    Ok(HostListResponse { hosts, default_id })
}

/// Lista los dispositivos de un host específico.
#[tauri::command]
fn list_devices_for_host(host_id: String) -> Result<DeviceListResponse, String> {
    let (inputs, outputs) = voxlfa_core::audio::AudioEngine::list_devices_for_host(&host_id)
        .map_err(|err| err.to_string())?;
    Ok(DeviceListResponse { inputs, outputs })
}

/// Arranca el motor con los dispositivos indicados (`None` = predeterminado).
/// `buffer_size` (muestras/callback) es opcional: si es `None`, el core elige
/// uno automáticamente según el tipo de dispositivo (heurística de latencia).
/// `audio_host` permite elegir el backend de audio (p. ej. `"jack"`, `"alsa"`).
#[tauri::command]
fn start_engine(
    app: AppHandle,
    state: State<AppState>,
    input_device: Option<String>,
    output_device: Option<String>,
    buffer_size: Option<usize>,
    audio_host: Option<String>,
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
                audio_host,
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

/// Devuelve el último espectro emitido (renderizado inicial de la UI).
#[tauri::command]
fn get_last_spectrum(
    state: State<AppState>,
) -> Result<Option<voxlfa_core::protocol::SpectrumSample>, String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    Ok(engine.last_spectrum())
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

/// Ajusta los parámetros de la puerta de ruido del preset activo en vivo.
#[tauri::command]
fn set_noise_gate(
    state: State<AppState>,
    params: voxlfa_core::protocol::NoiseGateParams,
) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine.set_noise_gate(params).map_err(|err| err.to_string())
}

/// Ajusta la mezcla seco/húmedo del denoise del preset activo en vivo.
#[tauri::command]
fn set_denoise(
    state: State<AppState>,
    params: voxlfa_core::protocol::DenoiseParams,
) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine.set_denoise(params).map_err(|err| err.to_string())
}

/// Ajusta los parámetros del feedback suppressor del preset activo en vivo.
#[tauri::command]
fn set_feedback(
    state: State<AppState>,
    params: voxlfa_core::protocol::FeedbackSuppressorParams,
) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine.set_feedback(params).map_err(|err| err.to_string())
}

/// Ajusta los parámetros de corrección de tono del preset activo en vivo.
#[tauri::command]
fn set_pitch_correction(
    state: State<AppState>,
    params: voxlfa_core::protocol::PitchCorrectionParams,
) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine
        .set_pitch_correction(params)
        .map_err(|err| err.to_string())
}

/// Pide sugerencias al asesor de IA (Groq) con las métricas actuales.
///
/// Ejecuta la petición HTTP en un hilo bloqueante para no bloquear la UI.
/// Devuelve las sugerencias generadas (o un error si la clave no está
/// configurada o la petición falla).
#[tauri::command]
async fn request_ai_suggestions(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Vec<Suggestion>, String> {
    // Obtener el análisis actual y el estado DSP (requiere el motor corriendo).
    let (analysis, dsp_state) = {
        let engine = state.engine.lock().map_err(|err| err.to_string())?;
        let handle = engine
            .analysis_handle()
            .ok_or_else(|| "el motor no está corriendo".to_string())?;
        let analysis = handle
            .get_analysis()
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "aún no hay análisis disponible".to_string())?;
        let dsp = engine.last_dsp_state();
        (analysis, dsp)
    };

    let api_key = GROQ_API_KEY.to_string();

    // Ejecutar la petición LLM en un hilo bloqueante.
    let result = tokio::task::spawn_blocking(move || {
        crate::llm::request_suggestions(&api_key, &analysis, dsp_state.as_ref())
    })
    .await
    .map_err(|e| format!("LLM task failed: {e}"))?;

    if !result.error.is_empty() {
        return Err(result.error);
    }

    // Guardar las sugerencias en el estado compartido.
    if let Ok(mut guard) = state.ai_suggestions.lock() {
        *guard = result.suggestions.clone();
    }

    // Emitir evento para que la UI se actualice.
    let _ = app.emit("ai-suggestions-updated", &result.suggestions);

    Ok(result.suggestions)
}

/// Devuelve las últimas sugerencias del asesor de IA (o vector vacío).
#[tauri::command]
fn get_ai_suggestions(state: State<AppState>) -> Result<Vec<Suggestion>, String> {
    let guard = state.ai_suggestions.lock().map_err(|err| err.to_string())?;
    Ok(guard.clone())
}

/// Devuelve la configuración persistida (para precargar la cabina).
#[tauri::command]
fn get_config(state: State<AppState>) -> Result<AppConfig, String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    Ok(engine.get_config())
}

/// Devuelve el estado del consentimiento de telemetría.
///
/// - `None` = sin decidir (mostrar diálogo).
/// - `Some(true)` = activada.
/// - `Some(false)` = desactivada.
#[tauri::command]
fn get_telemetry_consent(state: State<AppState>) -> Result<Option<bool>, String> {
    let engine = state.engine.lock().map_err(|err| err.to_string())?;
    Ok(engine.get_telemetry_consent())
}

/// Establece el consentimiento de telemetría (opt-in / opt-out).
#[tauri::command]
fn set_telemetry_consent(state: State<AppState>, enabled: bool) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|err| err.to_string())?;
    engine.set_telemetry_consent(enabled);
    Ok(())
}

/// Estado de los modelos ONNX en el disco.
#[tauri::command]
fn get_model_status() -> Result<voxlfa_core::models::ModelStatus, String> {
    let dir = voxlfa_core::models::models_dir()
        .ok_or_else(|| "cannot determine models directory".to_string())?;
    Ok(voxlfa_core::models::ModelStatus::check(&dir))
}

/// Descarga los modelos ONNX desde los assets de GitHub.
///
/// Emite eventos `model-download-progress` con `{ step, total }` para que la
/// cabina muestre una barra de progreso. Devuelve el estado final de los
/// modelos.
#[tauri::command]
async fn download_models(app: AppHandle) -> Result<voxlfa_core::models::ModelStatus, String> {
    #[cfg(not(feature = "onnx"))]
    {
        return Err("Models feature (onnx) is disabled in this build".to_string());
    }

    #[cfg(feature = "onnx")]
    {
        let version = env!("CARGO_PKG_VERSION");
        let version_tag = format!("v{version}");

        let handle = app.clone();
        let status = tokio::task::spawn_blocking(move || {
            voxlfa_core::models::download_models(&version_tag, |step, total| {
                let _ = handle.emit(
                    "model-download-progress",
                    serde_json::json!({ "step": step, "total": total }),
                );
            })
        })
        .await
        .map_err(|e| format!("download task failed: {e}"))?
        .map_err(|e| format!("download failed: {e}"))?;

        return Ok(voxlfa_core::models::ModelStatus::check(&status));
    }
}

/// Datos de emparejamiento para conectar la app móvil por WebSocket.
#[tauri::command]
fn get_pairing_info(state: State<AppState>) -> Result<PairingInfo, String> {
    let lan_address = local_ip_address::local_ip().ok().map(|ip| ip.to_string());
    let code = state
        .pairing
        .lock()
        .map_err(|err| err.to_string())?
        .code()
        .to_string();
    Ok(PairingInfo {
        code,
        port: WS_PORT,
        lan_address,
    })
}

/// Arranca la aplicación Tauri completa (ventana + backend + WebSocket).
pub fn run() {
    // Cargar .env del directorio de trabajo antes de leer variables de entorno.
    dotenvy::dotenv().ok();

    let builder = tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            list_devices,
            list_audio_hosts,
            list_devices_for_host,
            start_engine,
            stop_engine,
            get_engine_status,
            get_last_level,
            get_last_spectrum,
            get_presets,
            get_dsp_state,
            apply_preset,
            set_global_bypass,
            set_link_bypass,
            set_eq_band,
            set_noise_gate,
            set_denoise,
            set_feedback,
            set_pitch_correction,
            request_ai_suggestions,
            get_ai_suggestions,
            get_config,
            get_analysis,
            get_session_summary,
            apply_suggestion,
            get_pairing_info,
            get_telemetry_consent,
            set_telemetry_consent,
            get_model_status,
            download_models,
        ])
        .setup(|app| {
            // Servidor WebSocket para el monitoreo y control remoto desde el móvil.
            let state = app.state::<AppState>();
            let events = state.events.clone();
            let pairing = state.pairing.clone();
            let engine = state.engine.clone();
            let pairing_events = state.pairing_events.clone();
            tauri::async_runtime::spawn(run_ws_server(
                events,
                pairing,
                state.pairing_events.clone(),
                engine,
                WS_PORT,
            ));

            // Reenviar las rotaciones del código a la cabina para que actualice
            // el badge de emparejamiento en vivo.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut rx = pairing_events.subscribe();
                while let Ok(code) = rx.recv().await {
                    let _ = app_handle.emit(PAIRING_EVENT_NAME, code);
                }
            });

            // Telemetría: consumir eventos y enviarlos por HTTP (opt-in).
            // El envío es best-effort: si falla, se registra en stderr y se
            // continúa. Si `TELEMETRY_ENDPOINT` está vacío, solo se loguea.
            if let Ok(mut guard) = state.telemetry_rx.lock() {
                if let Some(receiver) = guard.take() {
                    std::thread::Builder::new()
                        .name("voxlfa-telemetry".to_string())
                        .spawn(move || {
                            let use_http = !TELEMETRY_ENDPOINT.is_empty();
                            let client = if use_http {
                                match reqwest::blocking::Client::builder()
                                    .timeout(std::time::Duration::from_secs(5))
                                    .build()
                                {
                                    Ok(c) => Some(c),
                                    Err(e) => {
                                        eprintln!("[telemetry] HTTP client init failed: {e}");
                                        None
                                    }
                                }
                            } else {
                                None
                            };
                            while let Some(event) = receiver.recv() {
                                if let Some(ref client) = client {
                                    let payload = match serde_json::to_string(&event) {
                                        Ok(p) => p,
                                        Err(e) => {
                                            eprintln!("[telemetry] serialize failed: {e}");
                                            continue;
                                        }
                                    };
                                    match client
                                        .post(TELEMETRY_ENDPOINT)
                                        .header("Content-Type", "application/json")
                                        .body(payload)
                                        .send()
                                    {
                                        Ok(resp) if resp.status().is_success() => {}
                                        Ok(resp) => {
                                            eprintln!(
                                                "[telemetry] HTTP {}: {}",
                                                resp.status(),
                                                resp.text().unwrap_or_default()
                                            );
                                        }
                                        Err(e) => {
                                            eprintln!("[telemetry] HTTP send failed: {e}");
                                        }
                                    }
                                } else {
                                    log::debug!("[telemetry] {event:?}");
                                }
                            }
                        })
                        .ok();
                }
            }

            // Emitir `AppStarted` si la telemetría está habilitada.
            {
                if let Ok(engine) = state.engine.lock() {
                    engine.emit_app_started();
                }
            }

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
