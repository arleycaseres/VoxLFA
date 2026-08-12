//! Gestión del ciclo de vida del motor de audio desde la app.
//!
//! `EngineManager` arranca y detiene el motor de `voxlfa-core`, mantiene el
//! último estado conocido y difunde cada evento a dos audiencias:
//!   * la UI (vía callback de frontend, inyectado por la capa Tauri), y
//!   * el WebSocket de la app móvil (vía canal broadcast de JSON).
//!
//! Un hilo dedicado ("forwarder") consume el canal del motor para no hacer
//! trabajo en los callbacks de audio.

use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use tokio::sync::broadcast;
use voxlfa_core::audio::{AudioEngine, AudioEngineConfig, DspHandle, EngineHandle};
use voxlfa_core::protocol::{DspState, EngineEvent, EngineStatus, LevelSample};

/// Errores del gestor del motor.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// El motor ya está corriendo; no se puede arrancar dos veces.
    #[error("el motor de audio ya está en ejecución")]
    AlreadyRunning,
    /// Error delegado del core de audio.
    #[error(transparent)]
    Core(#[from] voxlfa_core::Error),
}

/// Callback de reenvío de eventos hacia la UI (inyectado por la capa Tauri).
pub type FrontendCallback = dyn Fn(&EngineEvent) + Send + Sync;

/// Estado y ciclo de vida del motor, compartido entre comandos y el WS.
pub struct EngineManager {
    handle: Option<EngineHandle>,
    dsp: Option<DspHandle>,
    /// Último estado conocido del motor (para consultas de la UI).
    status: Arc<Mutex<Option<EngineStatus>>>,
    /// Último nivel medido (renderizado inicial de la UI).
    level: Arc<Mutex<Option<LevelSample>>>,
    /// Último estado de la cadena DSP conocido.
    dsp_state: Arc<Mutex<Option<DspState>>>,
    /// Emisor de eventos serializados (JSON) hacia el WebSocket.
    events: broadcast::Sender<String>,
}

impl EngineManager {
    /// Crea un gestor vacío ligado al canal de eventos dado.
    pub fn new(events: broadcast::Sender<String>) -> Self {
        Self {
            handle: None,
            dsp: None,
            status: Arc::new(Mutex::new(None)),
            level: Arc::new(Mutex::new(None)),
            dsp_state: Arc::new(Mutex::new(None)),
            events,
        }
    }

    /// Indica si el motor está corriendo actualmente.
    pub fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    /// Último estado del motor observado, si lo hay.
    pub fn last_status(&self) -> Option<EngineStatus> {
        self.status.lock().ok().and_then(|status| status.clone())
    }

    /// Último nivel de audio medido, si lo hay.
    pub fn last_level(&self) -> Option<LevelSample> {
        self.level.lock().ok().and_then(|level| *level)
    }

    /// Último estado de la cadena DSP observado, si lo hay.
    pub fn last_dsp_state(&self) -> Option<DspState> {
        self.dsp_state.lock().ok().and_then(|state| state.clone())
    }

    /// Mango de control de la cadena DSP (si el motor está corriendo).
    pub fn dsp_handle(&self) -> Option<&DspHandle> {
        self.dsp.as_ref()
    }

    /// Arranca el motor con la configuración indicada.
    ///
    /// `on_frontend` es un callback opcional que se invoca con cada evento
    /// para reenviarlo a la UI (la capa Tauri lo conecta a `app.emit`).
    pub fn start<F>(
        &mut self,
        config: AudioEngineConfig,
        on_frontend: Option<F>,
    ) -> Result<(), EngineError>
    where
        F: Fn(&EngineEvent) + Send + Sync + 'static,
    {
        if self.handle.is_some() {
            return Err(EngineError::AlreadyRunning);
        }

        let (tx, rx) = mpsc::channel();
        let (handle, dsp) = AudioEngine::start(config, tx)?;

        // Hilo forwarder: canal del motor → UI + WebSocket.
        let status = self.status.clone();
        let level = self.level.clone();
        let dsp_state = self.dsp_state.clone();
        let events = self.events.clone();
        let on_frontend: Option<Arc<FrontendCallback>> =
            on_frontend.map(|f| Arc::new(f) as Arc<FrontendCallback>);

        thread::Builder::new()
            .name("voxlfa-event-forwarder".to_string())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    // 1) Actualizar el estado compartido.
                    if let EngineEvent::Status(status_event) = &event {
                        if let Ok(mut guard) = status.lock() {
                            *guard = Some(status_event.clone());
                        }
                    }
                    if let EngineEvent::Level(level_event) = &event {
                        if let Ok(mut guard) = level.lock() {
                            *guard = Some(*level_event);
                        }
                    }
                    if let EngineEvent::Dsp(state) = &event {
                        if let Ok(mut guard) = dsp_state.lock() {
                            *guard = Some(state.clone());
                        }
                    }

                    // 2) Reenviar a la UI (si hay callback).
                    if let Some(callback) = &on_frontend {
                        callback(&event);
                    }

                    // 3) Difundir al WebSocket de la app móvil.
                    if let Ok(json) = serde_json::to_string(&event) {
                        let _ = events.send(json);
                    }
                }
            })
            .map_err(voxlfa_core::Error::from)?;

        self.handle = Some(handle);
        self.dsp = Some(dsp);
        Ok(())
    }

    /// Detiene el motor de forma controlada.
    ///
    /// La unión del hilo del motor ocurre en background (el motor sondea su
    /// flag cada ~200 ms y cierra los streams); no bloquea la UI.
    pub fn stop(&mut self) {
        self.dsp = None;
        if let Some(handle) = self.handle.take() {
            handle.request_stop();
            let _ = thread::Builder::new()
                .name("voxlfa-engine-join".to_string())
                .spawn(move || handle.join());
        }
    }
}
