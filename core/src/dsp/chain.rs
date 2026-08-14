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
    AudioProcessor, BoomSuppressor, Compressor, DeEsser, Delay, Gain, HighPass, Limiter, NoiseGate,
    Notch, ParametricEq, ProcessResult, ProcessingInfo, Reverb, Saturator,
};
use crate::error::Error;
use crate::protocol::{
    DspLinkState, DspModuleKind, DspModuleSpec, DspState, EngineEvent, EqBand, NoiseGateParams,
    PresetId,
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
    /// Bandas actuales del EQ si este eslabón es el ecualizador; `None` si no.
    /// Se reemplaza junto con el procesador en los ajustes finos.
    eq_bands: Option<Vec<EqBand>>,
    /// Parámetros actuales de la puerta de ruido si este eslabón es el gate;
    /// `None` si no. Se reemplaza junto con el procesador en los ajustes en vivo.
    gate_params: Option<NoiseGateParams>,
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
            .map(|spec| {
                let eq_bands = eq_bands_of(&spec.kind);
                let gate_params = gate_params_of(&spec.kind);
                ChainLink {
                    name: module_name(&spec.kind),
                    enabled: spec.enabled,
                    bypass: false,
                    processor: build_processor(spec, self.sample_rate, self.max_frames),
                    eq_bands,
                    gate_params,
                }
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

    /// Reemplaza el procesador de un módulo por su nombre.
    ///
    /// El procesador nuevo llega **ya construido** desde el hilo de control
    /// (aquí solo se intercambia el puntero, sin asignar memoria). Se usa para
    /// los ajustes finos, p. ej. reconstruir el EQ con una banda modificada.
    ///
    /// Devuelve `true` si el módulo existe y se actualizó.
    pub fn set_link_processor(
        &mut self,
        name: &str,
        processor: Box<dyn AudioProcessor>,
        eq_bands: Option<Vec<EqBand>>,
    ) -> bool {
        match self.links.iter_mut().find(|link| link.name == name) {
            Some(link) => {
                link.processor = processor;
                link.eq_bands = eq_bands;
                true
            }
            None => false,
        }
    }

    /// Reemplaza el procesador de la puerta de ruido por uno ya construido.
    ///
    /// Igual que [`ChainProcessor::set_link_processor`], pero además actualiza
    /// los parámetros del gate en el estado. Devuelve `true` si el módulo
    /// existe y se actualizó.
    pub fn set_link_gate(
        &mut self,
        processor: Box<dyn AudioProcessor>,
        params: NoiseGateParams,
    ) -> bool {
        match self.links.iter_mut().find(|link| link.name == "noisegate") {
            Some(link) => {
                link.processor = processor;
                link.gate_params = Some(params);
                true
            }
            None => false,
        }
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
                    eq_bands: link.eq_bands.clone(),
                    gate_params: link.gate_params,
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
    /// Reemplazar el procesador de un módulo por uno ya construido (ajuste fino,
    /// p. ej. el EQ con una banda modificada). El hilo de audio solo intercambia
    /// el puntero.
    SetLinkProcessor {
        /// Nombre del módulo (identificador de la cadena).
        name: String,
        /// Procesador nuevo, construido en el hilo de control.
        processor: Box<dyn AudioProcessor>,
        /// Bandas del EQ si el módulo reemplazado es el ecualizador; `None` si no.
        eq_bands: Option<Vec<EqBand>>,
    },
    /// Reemplazar el procesador de la puerta de ruido (ajuste en vivo de sus
    /// parámetros). El hilo de audio solo intercambia el puntero.
    SetLinkGate {
        /// Procesador nuevo, construido en el hilo de control.
        processor: Box<dyn AudioProcessor>,
        /// Parámetros actuales del gate para el estado de la cadena.
        params: NoiseGateParams,
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
                    eq_bands: eq_bands_of(&spec.kind),
                    gate_params: gate_params_of(&spec.kind),
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

    /// Reemplaza las bandas del ecualizador del preset activo por las indicadas
    /// (un solo intercambio de procesador, sin reconstruir el resto de la cadena).
    ///
    /// Se usa para aplicar los ajustes finos persistidos de un perfil al
    /// arrancar el motor y como base de [`DspHandle::set_eq_band`].
    ///
    /// Devuelve error si el preset actual no tiene módulo EQ.
    pub fn set_eq_bands(&self, bands: Vec<EqBand>) -> Result<()> {
        let mut state = self.get_state()?;
        let eq = state
            .links
            .iter_mut()
            .find(|link| link.name == "eq")
            .ok_or_else(|| Error::audio("el preset actual no tiene módulo ecualizador"))?;
        eq.eq_bands = Some(bands.clone());

        let processor = ParametricEq::new(bands.clone(), self.sample_rate, self.max_frames);
        self.send(DspCommand::SetLinkProcessor {
            name: "eq".to_string(),
            processor: Box::new(processor),
            eq_bands: Some(bands),
        })?;
        self.publish(state);
        Ok(())
    }

    /// Ajusta la ganancia de una banda del ecualizador del preset activo en vivo.
    ///
    /// El nuevo EQ se construye aquí (hilo de control) con la banda modificada
    /// y se envía ya listo al hilo de audio, que solo intercambia el puntero.
    ///
    /// Devuelve error si el preset actual no tiene módulo EQ o si el índice de
    /// banda no existe.
    pub fn set_eq_band(&self, index: usize, gain_db: f32) -> Result<()> {
        let mut bands = self
            .get_state()?
            .links
            .iter()
            .find(|link| link.name == "eq")
            .and_then(|link| link.eq_bands.clone())
            .ok_or_else(|| Error::audio("el preset actual no tiene módulo ecualizador"))?;
        let band = bands
            .get_mut(index)
            .ok_or_else(|| Error::audio(format!("band index fuera de rango: {index}")))?;
        band.gain_db = gain_db;
        self.set_eq_bands(bands)
    }

    /// Ajusta los parámetros de la puerta de ruido del preset activo en vivo.
    ///
    /// El nuevo `NoiseGate` se construye aquí (hilo de control) con los
    /// parámetros indicados y se envía ya listo al hilo de audio, que solo
    /// intercambia el puntero. Devuelve error si el preset actual no tiene
    /// puerta de ruido.
    pub fn set_noise_gate(&self, params: NoiseGateParams) -> Result<()> {
        let mut state = self.get_state()?;
        let link = state
            .links
            .iter_mut()
            .find(|link| link.name == "noisegate")
            .ok_or_else(|| Error::audio("el preset actual no tiene puerta de ruido"))?;
        link.gate_params = Some(params);

        let processor = NoiseGate::new(
            params.threshold_db,
            params.attack_ms,
            params.release_ms,
            params.hold_ms,
            params.range_db,
            self.sample_rate,
        );
        self.send(DspCommand::SetLinkGate {
            processor: Box::new(processor),
            params,
        })?;
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
        DspModuleKind::Notch { .. } => "notch",
        DspModuleKind::BoomSuppressor { .. } => "boomsuppressor",
        DspModuleKind::Eq { .. } => "eq",
        DspModuleKind::NoiseGate { .. } => "noisegate",
        DspModuleKind::Compressor { .. } => "compressor",
        DspModuleKind::DeEsser { .. } => "deesser",
        DspModuleKind::Saturator { .. } => "saturator",
        DspModuleKind::Delay { .. } => "delay",
        DspModuleKind::Reverb { .. } => "reverb",
        DspModuleKind::Limiter { .. } => "limiter",
    }
}

/// Bandas del EQ de una especificación de módulo, o `None` si no es un EQ.
fn eq_bands_of(kind: &DspModuleKind) -> Option<Vec<EqBand>> {
    match kind {
        DspModuleKind::Eq { bands } => Some(bands.clone()),
        _ => None,
    }
}

/// Parámetros de la puerta de ruido de una especificación de módulo, o `None`
/// si no es un gate.
fn gate_params_of(kind: &DspModuleKind) -> Option<NoiseGateParams> {
    match kind {
        DspModuleKind::NoiseGate {
            threshold_db,
            attack_ms,
            release_ms,
            hold_ms,
            range_db,
        } => Some(NoiseGateParams {
            threshold_db: *threshold_db,
            attack_ms: *attack_ms,
            release_ms: *release_ms,
            hold_ms: *hold_ms,
            range_db: *range_db,
        }),
        _ => None,
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
        DspModuleKind::Notch { freq_hz, q } => Box::new(Notch::new(freq_hz, q, sample_rate)),
        DspModuleKind::BoomSuppressor {
            threshold_db,
            freq_hz,
            amount,
        } => Box::new(BoomSuppressor::new(
            threshold_db,
            freq_hz,
            amount,
            sample_rate,
        )),
        DspModuleKind::Eq { bands } => Box::new(ParametricEq::new(bands, sample_rate, max_frames)),
        DspModuleKind::NoiseGate {
            threshold_db,
            attack_ms,
            release_ms,
            hold_ms,
            range_db,
        } => Box::new(NoiseGate::new(
            threshold_db,
            attack_ms,
            release_ms,
            hold_ms,
            range_db,
            sample_rate,
        )),
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
