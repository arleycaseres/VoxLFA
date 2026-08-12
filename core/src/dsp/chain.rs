//! Cadena de procesamiento DSP encadenable y mango de control en tiempo real.
//!
//! La cadena es propiedad **exclusiva del hilo de audio**: se construye con un
//! preset y se reconfigura desde el hilo de control mediante un canal mpsc
//! (`DspCommand`), consumido en el callback de captura con `try_recv()` (una
//! consulta sin espera, sin bloqueos largos ni asignaciones).
//!
//! El bypass por módulo y el bypass global son valores simples dentro de la
//! cadena; como esta solo la muta el hilo de audio, no hacen falta atomics.

use std::sync::{mpsc, Arc, Mutex};

use crate::dsp::{
    AudioProcessor, Compressor, DeEsser, Delay, Gain, HighPass, Limiter, ParametricEq,
    ProcessResult, ProcessingInfo, Reverb, Saturator,
};
use crate::error::Error;
use crate::protocol::{
    DspLinkState, DspModuleKind, DspModuleSpec, DspState, EngineEvent, PresetId,
};
use crate::Result;

use super::presets::PresetFactory;

/// Un eslabón de la cadena: procesador + su estado frente al bypass.
struct ChainLink {
    /// Nombre corto del módulo (identificador para el bypass y la UI).
    name: &'static str,
    /// `true` si el módulo está habilitado por el preset actual.
    enabled: bool,
    /// `true` si está en bypass (se omite en tiempo real).
    bypass: bool,
    /// Procesador real.
    processor: Box<dyn AudioProcessor>,
}

/// Cadena de procesamiento en serie, construida a partir de un preset.
///
/// Solo se muta desde el hilo de audio; no es `Send` entre hilos mientras
/// esté viva. Los buffers de trabajo se preasignan en la construcción.
pub struct ChainProcessor {
    preset: PresetId,
    global_bypass: bool,
    links: Vec<ChainLink>,
    sample_rate: u32,
    max_frames: usize,
    scratch_a: Vec<f32>,
    scratch_b: Vec<f32>,
}

impl ChainProcessor {
    /// Crea una cadena con el preset indicado.
    ///
    /// `max_frames` es la mayor cantidad de muestras por bloque esperada
    /// (tamaño de buffer del stream); los buffers se preasignan a esa medida.
    pub fn new(preset: PresetId, sample_rate: u32, max_frames: usize) -> Self {
        let max_frames = max_frames.max(1);
        let mut chain = Self {
            preset,
            global_bypass: false,
            links: Vec::new(),
            sample_rate,
            max_frames,
            scratch_a: vec![0.0; max_frames],
            scratch_b: vec![0.0; max_frames],
        };
        chain.apply_preset(preset);
        chain
    }

    /// Reconstruye la cadena para un preset (desde el hilo de control/audio).
    pub fn apply_preset(&mut self, preset: PresetId) {
        self.preset = preset;
        self.global_bypass = false;
        self.links = PresetFactory::specs(preset)
            .into_iter()
            .map(|spec| ChainLink {
                name: module_name(&spec.kind),
                enabled: spec.enabled,
                bypass: false,
                processor: build_processor(spec, self.sample_rate, self.max_frames),
            })
            .collect();
    }

    /// Activa o desactiva el bypass de un módulo por su nombre.
    ///
    /// Devuelve `true` si el módulo existe y se actualizó.
    pub fn set_link_bypass(&mut self, name: &str, bypass: bool) -> bool {
        match self.links.iter_mut().find(|link| link.name == name) {
            Some(link) => {
                link.bypass = bypass;
                true
            }
            None => false,
        }
    }

    /// Activa o desactiva el bypass global (paso directo de toda la cadena).
    pub fn set_global_bypass(&mut self, bypass: bool) {
        self.global_bypass = bypass;
    }

    /// Estado declarativo de la cadena para la UI (protocolo).
    pub fn state(&self) -> DspState {
        DspState {
            preset: self.preset,
            global_bypass: self.global_bypass,
            links: self
                .links
                .iter()
                .map(|link| DspLinkState {
                    name: link.name.to_string(),
                    enabled: link.enabled,
                    bypass: link.bypass,
                })
                .collect(),
        }
    }
}

impl AudioProcessor for ChainProcessor {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());

        // Caso anómalo: bloque mayor que la preasignación (no ocurre en
        // operación normal con un buffer de tamaño fijo).
        if frames > self.max_frames {
            self.max_frames = frames;
            self.scratch_a.resize(frames, 0.0);
            self.scratch_b.resize(frames, 0.0);
        }

        self.scratch_a[..frames].copy_from_slice(&input[..frames]);

        let mut total_latency = 0.0;
        if !self.global_bypass {
            for link in self.links.iter_mut() {
                if !link.enabled || link.bypass {
                    continue;
                }
                let result = link.processor.process(
                    &self.scratch_a[..frames],
                    &mut self.scratch_b[..frames],
                    info,
                );
                total_latency += result.latency_ms;
                std::mem::swap(&mut self.scratch_a, &mut self.scratch_b);
            }
        }

        output[..frames].copy_from_slice(&self.scratch_a[..frames]);
        ProcessResult {
            latency_ms: total_latency,
        }
    }

    fn name(&self) -> &'static str {
        "chain"
    }

    fn reset(&mut self) {
        for link in &mut self.links {
            link.processor.reset();
        }
    }
}

/// Comandos de reconfiguración enviados al hilo de audio.
///
/// Las cadenas nuevas se construyen en el hilo de control (que puede asignar
/// memoria); el hilo de audio solo intercambia el puntero.
pub enum DspCommand {
    /// Aplicar un preset: la cadena llega ya construida.
    ApplyPreset(Box<ChainProcessor>),
    /// Cambiar el bypass global.
    SetGlobalBypass(bool),
    /// Cambiar el bypass de un módulo por su nombre.
    SetLinkBypass {
        /// Nombre del módulo (identificador de la cadena).
        name: String,
        /// `true` para omitir el módulo en tiempo real.
        bypass: bool,
    },
}

/// Mango de control de la cadena DSP (hilo de UI/control).
///
/// Construye las nuevas cadenas y las envía al hilo de audio, mantiene el
/// último estado conocido y lo difunde como [`EngineEvent::Dsp`] para que la
/// UI y el móvil lo reciban por los mismos canales que el resto de eventos.
#[derive(Clone)]
pub struct DspHandle {
    tx: mpsc::Sender<DspCommand>,
    events: mpsc::Sender<EngineEvent>,
    state: Arc<Mutex<DspState>>,
    sample_rate: u32,
    max_frames: usize,
}

impl DspHandle {
    /// Crea un mango ligado al canal de comandos y al de eventos del motor.
    ///
    /// El estado inicial refleja `initial_preset` (mismo preset con el que se
    /// construyó la cadena del motor).
    pub fn new(
        tx: mpsc::Sender<DspCommand>,
        events: mpsc::Sender<EngineEvent>,
        initial_preset: PresetId,
        sample_rate: u32,
        max_frames: usize,
    ) -> Self {
        let state = Arc::new(Mutex::new(DspState {
            preset: initial_preset,
            global_bypass: false,
            links: PresetFactory::specs(initial_preset)
                .iter()
                .map(|spec| DspLinkState {
                    name: module_name(&spec.kind).to_string(),
                    enabled: spec.enabled,
                    bypass: false,
                })
                .collect(),
        }));
        Self {
            tx,
            events,
            state,
            sample_rate,
            max_frames: max_frames.max(1),
        }
    }

    /// Aplica un preset a la cadena en vivo.
    pub fn apply_preset(&self, preset: PresetId) -> Result<()> {
        let chain = ChainProcessor::new(preset, self.sample_rate, self.max_frames);
        let state = chain.state();
        self.tx
            .send(DspCommand::ApplyPreset(Box::new(chain)))
            .map_err(|_| Error::audio("dsp control channel closed (motor detenido)"))?;
        self.publish(state);
        Ok(())
    }

    /// Cambia el bypass global de la cadena.
    pub fn set_global_bypass(&self, bypass: bool) -> Result<()> {
        self.send(DspCommand::SetGlobalBypass(bypass))?;
        let mut state = self.get_state()?;
        state.global_bypass = bypass;
        self.publish(state);
        Ok(())
    }

    /// Cambia el bypass de un módulo por su nombre.
    ///
    /// Devuelve error si el módulo no existe en el preset actual.
    pub fn set_link_bypass(&self, name: &str, bypass: bool) -> Result<()> {
        self.send(DspCommand::SetLinkBypass {
            name: name.to_string(),
            bypass,
        })?;
        let mut state = self.get_state()?;
        let link = state
            .links
            .iter_mut()
            .find(|link| link.name == name)
            .ok_or_else(|| Error::audio(format!("dsp link not found: {name}")))?;
        link.bypass = bypass;
        self.publish(state);
        Ok(())
    }

    /// Último estado de la cadena (espejo del hilo de control).
    pub fn get_state(&self) -> Result<DspState> {
        self.state
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| Error::audio("dsp state lock poisoned"))
    }

    /// Envía un comando al hilo de audio (sin construir cadenas nuevas).
    fn send(&self, command: DspCommand) -> Result<()> {
        self.tx
            .send(command)
            .map_err(|_| Error::audio("dsp control channel closed (motor detenido)"))
    }

    /// Guarda el estado en el espejo y lo difunde como evento del motor.
    fn publish(&self, state: DspState) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = state.clone();
        }
        let _ = self.events.send(EngineEvent::Dsp(state));
    }
}

/// Nombre corto de un módulo a partir de su especificación.
fn module_name(kind: &DspModuleKind) -> &'static str {
    match kind {
        DspModuleKind::Gain { .. } => "gain",
        DspModuleKind::HighPass { .. } => "highpass",
        DspModuleKind::Eq { .. } => "eq",
        DspModuleKind::Compressor { .. } => "compressor",
        DspModuleKind::DeEsser { .. } => "deesser",
        DspModuleKind::Saturator { .. } => "saturator",
        DspModuleKind::Delay { .. } => "delay",
        DspModuleKind::Reverb { .. } => "reverb",
        DspModuleKind::Limiter { .. } => "limiter",
    }
}

/// Construye el procesador real para una especificación de módulo.
fn build_processor(
    spec: DspModuleSpec,
    sample_rate: u32,
    max_frames: usize,
) -> Box<dyn AudioProcessor> {
    match spec.kind {
        DspModuleKind::Gain { gain_db } => Box::new(Gain::new(gain_db)),
        DspModuleKind::HighPass { cutoff_hz } => Box::new(HighPass::new(cutoff_hz, sample_rate)),
        DspModuleKind::Eq { bands } => Box::new(ParametricEq::new(bands, sample_rate, max_frames)),
        DspModuleKind::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_db,
        } => Box::new(Compressor::new(
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            makeup_db,
            sample_rate,
        )),
        DspModuleKind::DeEsser {
            threshold_db,
            freq_hz,
            amount,
        } => Box::new(DeEsser::new(threshold_db, freq_hz, amount, sample_rate)),
        DspModuleKind::Saturator { drive, mix } => Box::new(Saturator::new(drive, mix)),
        DspModuleKind::Delay {
            time_ms,
            feedback,
            mix,
        } => Box::new(Delay::new(time_ms, feedback, mix, sample_rate)),
        DspModuleKind::Reverb {
            room_size,
            damping,
            wet,
        } => {
            // `room_size` (0–1) → duración de la cola en ms (50–400).
            let room_ms = 50.0 + room_size.clamp(0.0, 1.0) * 350.0;
            Box::new(Reverb::new(room_ms, wet, damping, sample_rate))
        }
        DspModuleKind::Limiter {
            threshold_db,
            lookahead_ms,
            release_ms,
        } => Box::new(Limiter::new(
            threshold_db,
            lookahead_ms,
            release_ms,
            sample_rate,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(frames: usize) -> ProcessingInfo {
        ProcessingInfo {
            sample_rate: 48_000,
            frames,
        }
    }

    #[test]
    fn dry_preset_is_passthrough() {
        let mut chain = ChainProcessor::new(PresetId::Dry, 48_000, 256);
        let input = [0.2, -0.1, 0.5, -0.3];
        let mut out = [0.0; 4];
        chain.process(&input, &mut out, &info(4));
        assert_eq!(out, input);
        assert_eq!(chain.state().links.len(), 0);
    }

    #[test]
    fn bypass_global_gives_passthrough() {
        let mut chain = ChainProcessor::new(PresetId::VozLimpia, 48_000, 256);
        let input = [0.2, -0.1, 0.5, -0.3];
        let mut out = [0.0; 4];
        chain.set_global_bypass(true);
        chain.process(&input, &mut out, &info(4));
        assert_eq!(out, input);
    }

    #[test]
    fn link_bypass_only_skips_that_module() {
        let mut chain = ChainProcessor::new(PresetId::VozLimpia, 48_000, 256);
        let n = 4096;
        let input = vec![0.3; n];
        let mut out = vec![0.0; n];

        // Con todo activo la señal debe diferir del passthrough.
        chain.process(&input, &mut out, &info(n));
        assert!(out.iter().any(|&v| (v - 0.3).abs() > 1e-4));

        // Byppass de todos los módulos → passthrough exacto.
        let names: Vec<String> = chain
            .state()
            .links
            .iter()
            .map(|link| link.name.clone())
            .collect();
        for name in &names {
            chain.set_link_bypass(name, true);
        }
        chain.process(&input, &mut out, &info(n));
        for &v in &out {
            assert!((v - 0.3).abs() < 1e-4);
        }
    }

    #[test]
    fn state_reflects_bypass_changes() {
        let mut chain = ChainProcessor::new(PresetId::Radio, 48_000, 256);
        chain.set_global_bypass(true);
        chain.set_link_bypass("eq", true);
        let state = chain.state();
        assert!(state.global_bypass);
        let eq = state
            .links
            .iter()
            .find(|link| link.name == "eq")
            .expect("preset radio incluye eq");
        assert!(eq.bypass);
        assert!(eq.enabled);
    }

    #[test]
    fn latency_is_accumulated() {
        let mut chain = ChainProcessor::new(PresetId::VozLimpia, 48_000, 256);
        let mut out = [0.0; 64];
        let result = chain.process(&[0.1; 64], &mut out, &info(64));
        // VozLimpia termina en limitador (lookahead) → latencia > 0.
        assert!(result.latency_ms > 0.0, "latency {:?}", result.latency_ms);
    }
}
