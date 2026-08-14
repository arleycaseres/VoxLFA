//! Gestión del ciclo de vida del motor de audio desde la app.
//!
//! `EngineManager` arranca y detiene el motor de `voxlfa-core`, mantiene el
//! último estado conocido y difunde cada evento a dos audiencias:
//!   * la UI (vía callback de frontend, inyectado por la capa Tauri), y
//!   * el WebSocket de la app móvil (vía canal broadcast de JSON).
//!
//! Un hilo dedicado ("forwarder") consume el canal del motor para no hacer
//! trabajo en los callbacks de audio.
//!
//! También orquesta la **persistencia** (Fase 3): guarda la configuración en
//! `config.json` con perfiles por dispositivo de entrada y los reaplica al
//! arrancar (preset + ajuste fino del EQ + bypasses).

use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use tokio::sync::broadcast;
use voxlfa_core::analysis::AnalysisHandle;
use voxlfa_core::audio::{AudioEngine, AudioEngineConfig, DspHandle, EngineHandle};
use voxlfa_core::config::{ConfigStore, DEFAULT_DEVICE_KEY};
use voxlfa_core::dsp::PresetFactory;
use voxlfa_core::protocol::{
    DspState, EngineEvent, EngineStatus, LevelSample, NoiseGateParams, PresetId, SpectrumSample,
};

/// Errores del gestor del motor.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// El motor ya está corriendo; no se puede arrancar dos veces.
    #[error("el motor de audio ya está en ejecución")]
    AlreadyRunning,
    /// El motor no está corriendo (se exige para comandos DSP).
    #[error("el motor no está corriendo")]
    NotRunning,
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
    /// Mango de consulta del análisis vocal (métricas, resumen, sugerencias).
    analysis: Option<AnalysisHandle>,
    /// Último estado conocido del motor (para consultas de la UI).
    status: Arc<Mutex<Option<EngineStatus>>>,
    /// Último nivel medido (renderizado inicial de la UI).
    level: Arc<Mutex<Option<LevelSample>>>,
    /// Último espectro emitido (renderizado inicial de la UI).
    spectrum: Arc<Mutex<Option<SpectrumSample>>>,
    /// Último estado de la cadena DSP conocido.
    dsp_state: Arc<Mutex<Option<DspState>>>,
    /// Emisor de eventos serializados (JSON) hacia el WebSocket.
    events: broadcast::Sender<String>,
    /// Configuración persistente (config.json del usuario).
    config: ConfigStore,
    /// Clave de perfil del dispositivo de entrada en uso mientras el motor
    /// corre (`DEFAULT_DEVICE_KEY` si se arrancó con el predeterminado).
    current_device: Option<String>,
}

impl EngineManager {
    /// Crea un gestor vacío ligado al canal de eventos dado.
    ///
    /// La configuración se carga desde la ruta estándar del usuario
    /// (`$XDG_CONFIG_HOME/voxlfa/config.json`).
    pub fn new(events: broadcast::Sender<String>) -> Self {
        let config = match voxlfa_core::config::default_config_path() {
            Some(path) => ConfigStore::load(&path),
            None => ConfigStore::memory(),
        };
        Self {
            handle: None,
            dsp: None,
            analysis: None,
            status: Arc::new(Mutex::new(None)),
            level: Arc::new(Mutex::new(None)),
            spectrum: Arc::new(Mutex::new(None)),
            dsp_state: Arc::new(Mutex::new(None)),
            events,
            config,
            current_device: None,
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

    /// Último espectro emitido por el motor, si lo hay.
    pub fn last_spectrum(&self) -> Option<SpectrumSample> {
        self.spectrum.lock().ok().and_then(|spectrum| *spectrum)
    }

    /// Último estado de la cadena DSP observado, si lo hay.
    pub fn last_dsp_state(&self) -> Option<DspState> {
        self.dsp_state.lock().ok().and_then(|state| state.clone())
    }

    /// Mango de control de la cadena DSP (si el motor está corriendo).
    pub fn dsp_handle(&self) -> Option<&DspHandle> {
        self.dsp.as_ref()
    }

    /// Mango de consulta del análisis vocal (si el motor está corriendo).
    pub fn analysis_handle(&self) -> Option<&AnalysisHandle> {
        self.analysis.as_ref()
    }

    /// Configuración persistida actual (para precargar la cabina).
    pub fn get_config(&self) -> voxlfa_core::config::AppConfig {
        self.config.config().clone()
    }

    /// Arranca el motor con la configuración indicada.
    ///
    /// Si existe un perfil guardado para el dispositivo de entrada elegido, se
    /// arranca con su preset y se reaplican el ajuste fino del EQ y los bypasses
    /// nada más levantar el pipeline.
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

        let profile_key = config
            .input_device
            .clone()
            .unwrap_or_else(|| DEFAULT_DEVICE_KEY.to_string());
        let profile = self.config.config().profile(&profile_key).cloned();

        // Últimos dispositivos elegidos, para precargar la cabina al arrancar.
        let profile_input = config.input_device.clone();
        let profile_output = config.output_device.clone();
        let profile_buffer = config.buffer_size;

        // El preset del perfil se aplica al construir la cadena inicial.
        let mut engine_config = config;
        if let Some(profile) = &profile {
            engine_config.initial_preset = profile.preset;
        }

        let (tx, rx) = mpsc::channel();
        let (handle, dsp, analysis) = AudioEngine::start(engine_config, tx)?;

        // Reaplicar el ajuste fino del EQ, la puerta de ruido y los bypasses
        // persistidos.
        if let Some(profile) = profile {
            if !profile.eq_bands.is_empty() {
                let _ = dsp.set_eq_bands(profile.eq_bands);
            }
            if let Some(gate) = profile.gate_params {
                let _ = dsp.set_noise_gate(gate);
            }
            if profile.global_bypass {
                let _ = dsp.set_global_bypass(true);
            }
            for (link, bypass) in profile.link_bypass {
                let _ = dsp.set_link_bypass(&link, bypass);
            }
        }

        // Recordar los últimos dispositivos elegidos para precargar la UI.
        let cfg = self.config.config_mut();
        cfg.default_input = profile_input.clone();
        cfg.default_output = profile_output.clone();
        cfg.buffer_size = profile_buffer;

        self.current_device = Some(profile_key);

        // Hilo forwarder: canal del motor → UI + WebSocket.
        let status = self.status.clone();
        let level = self.level.clone();
        let spectrum = self.spectrum.clone();
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
                    if let EngineEvent::Spectrum(spectrum_event) = &event {
                        if let Ok(mut guard) = spectrum.lock() {
                            *guard = Some(*spectrum_event);
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
        self.analysis = Some(analysis);
        Ok(())
    }

    /// Detiene el motor de forma controlada.
    ///
    /// Antes de detenerlo se vuelca el perfil del dispositivo actual (preset,
    /// EQ y bypasses) a la configuración persistida. La unión del hilo del
    /// motor ocurre en background (el motor sondea su flag cada ~200 ms y cierra
    /// los streams); no bloquea la UI.
    pub fn stop(&mut self) {
        if let Some(profile_key) = self.current_device.take() {
            if let Some(dsp) = &self.dsp {
                if let Ok(state) = dsp.get_state() {
                    let profile = self.config.config_mut().profile_mut(&profile_key);
                    profile.preset = state.preset;
                    profile.global_bypass = state.global_bypass;
                    profile.eq_bands = state
                        .links
                        .iter()
                        .find(|link| link.name == "eq")
                        .and_then(|link| link.eq_bands.clone())
                        .unwrap_or_default();
                    profile.gate_params = state
                        .links
                        .iter()
                        .find(|link| link.name == "noisegate")
                        .and_then(|link| link.gate_params);
                    profile.link_bypass = state
                        .links
                        .iter()
                        .filter(|link| link.bypass)
                        .map(|link| (link.name.clone(), true))
                        .collect();
                }
            }
            // El guardado es best-effort: si falla, la sesión sigue.
            let _ = self.config.save();
        }
        self.dsp = None;
        // El mango de análisis se conserva: el resumen de sesión se consulta
        // tras detener el motor.
        if let Some(handle) = self.handle.take() {
            handle.request_stop();
            let _ = thread::Builder::new()
                .name("voxlfa-engine-join".to_string())
                .spawn(move || handle.join());
        }
    }

    /// Aplica un preset a la cadena en vivo y lo persiste en el perfil actual.
    pub fn apply_preset(&mut self, preset: PresetId) -> Result<(), EngineError> {
        let dsp = self.dsp.as_ref().ok_or(EngineError::NotRunning)?;
        dsp.apply_preset(preset)?;
        self.update_current_profile(|profile| {
            profile.preset = preset;
            profile.eq_bands = PresetFactory::eq_bands(preset);
            profile.gate_params = PresetFactory::gate_params(preset);
            profile.global_bypass = false;
            profile.link_bypass.clear();
        });
        self.save_config();
        Ok(())
    }

    /// Cambia el bypass global de la cadena y lo persiste.
    pub fn set_global_bypass(&mut self, bypass: bool) -> Result<(), EngineError> {
        let dsp = self.dsp.as_ref().ok_or(EngineError::NotRunning)?;
        dsp.set_global_bypass(bypass)?;
        self.update_current_profile(|profile| profile.global_bypass = bypass);
        self.save_config();
        Ok(())
    }

    /// Cambia el bypass de un módulo por su nombre y lo persiste.
    pub fn set_link_bypass(&mut self, link: String, bypass: bool) -> Result<(), EngineError> {
        let dsp = self.dsp.as_ref().ok_or(EngineError::NotRunning)?;
        dsp.set_link_bypass(&link, bypass)?;
        self.update_current_profile(|profile| {
            if bypass {
                profile.link_bypass.insert(link.clone(), true);
            } else {
                profile.link_bypass.remove(&link);
            }
        });
        self.save_config();
        Ok(())
    }

    /// Ajusta la ganancia de una banda del EQ en vivo.
    ///
    /// El perfil se actualiza en memoria (se vuelca al detener el motor, para
    /// no escribir el archivo en cada paso del slider).
    pub fn set_eq_band(&mut self, index: usize, gain_db: f32) -> Result<(), EngineError> {
        let dsp = self.dsp.as_ref().ok_or(EngineError::NotRunning)?;
        dsp.set_eq_band(index, gain_db)?;
        self.update_current_profile(|profile| {
            if let Some(band) = profile.eq_bands.get_mut(index) {
                band.gain_db = gain_db;
            }
        });
        Ok(())
    }

    /// Ajusta los parámetros de la puerta de ruido del preset activo en vivo.
    ///
    /// El perfil se actualiza en memoria (se vuelca al detener el motor, para
    /// no escribir el archivo en cada paso del slider).
    pub fn set_noise_gate(&mut self, params: NoiseGateParams) -> Result<(), EngineError> {
        let dsp = self.dsp.as_ref().ok_or(EngineError::NotRunning)?;
        dsp.set_noise_gate(params)?;
        self.update_current_profile(|profile| {
            profile.gate_params = Some(params);
        });
        Ok(())
    }

    /// Aplica un cambio al perfil del dispositivo en uso, si el motor corre.
    fn update_current_profile(
        &mut self,
        update: impl FnOnce(&mut voxlfa_core::config::DeviceProfile),
    ) {
        if let Some(profile_key) = self.current_device.clone() {
            let profile = self.config.config_mut().profile_mut(&profile_key);
            update(profile);
        }
    }

    /// Guarda la configuración en disco (best-effort).
    fn save_config(&mut self) {
        let _ = self.config.save();
    }
}
