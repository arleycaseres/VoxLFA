//! Implementación del motor de audio con cpal y el buffer de anillo lock-free.
//!
//! Arquitectura del flujo de audio (Fase 0):
//!
//! ```text
//! dispositivo de entrada ─► [callback captura] ─► cadena DSP ─► ring buffer
//!                                                                      │
//! dispositivo de salida  ◄─ [callback salida]   ◄───────────────────────┘
//! ```
//!
//! Reglas que cumplen los callbacks de audio (ver AGENTS.md):
//!   * No asignan memoria (los buffers se preasignan una sola vez).
//!   * No toman bloqueos largos ni hacen I/O: solo copian muestras y
//!     acumulan/emiten eventos por canal a un hilo dedicado.
//!
//! La **latencia** se mide como el número de muestras en vuelo en el ring
//! buffer convertido a milisegundos (la forma estándar en pipelines dúplex).

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cpal::traits::{DeviceTrait, HostTrait};
use ringbuf::traits::*;
use ringbuf::HeapRb;

use crate::analysis::{
    AnalysisHandle, AnalysisShared, BandSplitter, SessionTracker, SpectrumAnalyzer,
    SuggestionEngine, VoiceAnalyzer, VoiceFrame,
};
use crate::dsp::denoise_thread::{self, AtomicF32, DenoiseHandle, DenoiseShared};
use crate::dsp::{
    AudioProcessor, ChainProcessor, DspCommand, DspHandle, LevelMeter, ProcessingInfo,
};
use crate::error::Error;
use crate::protocol::{
    AnalysisSample, AudioDeviceInfo, AudioHostInfo, EngineEvent, EngineState, EngineStatus,
    LevelSample, PresetId, SpectrumSample, SPECTRUM_BIN_COUNT,
};
use crate::Result;

/// Capacidad del buffer intermedio captura→salida, en segundos de señal.
///
/// Es un tope de seguridad; en funcionamiento normal se llena solo con el
/// backlog de latencia (unas pocas decenas de ms).
const RING_CAPACITY_SECS: u32 = 2;

/// Intervalo mínimo entre muestras de nivel emitidas a la UI (evita saturar
/// el canal y el frontend con decenas de miles de eventos por segundo).
const LEVEL_EMIT_INTERVAL: Duration = Duration::from_millis(50);

/// Intervalo entre marcos de análisis extraídos en el callback de audio.
///
/// Cada marco agrupa `ANALYSIS_FRAME_INTERVAL` ms de señal acumulada por el
/// [`BandSplitter`] y se envía al hilo de análisis por un canal acotado.
const ANALYSIS_FRAME_INTERVAL: Duration = Duration::from_millis(200);

/// Intervalo mínimo entre eventos `EngineEvent::Spectrum` emitidos a la UI.
///
/// La FFT se calcula en cada avance de ventana (50 % de solapamiento); esta
/// constante solo acota la frecuencia de emisión para no saturar el canal ni
/// el frontend (20 Hz es suficiente para una vista fluida).
const SPECTRUM_EMIT_INTERVAL: Duration = Duration::from_millis(50);

/// Ventana deslizante de la voz (ms) usada para calcular las métricas.
const ANALYSIS_WINDOW_MS: u32 = 2000;

/// Intervalo mínimo entre eventos `EngineEvent::Analysis` emitidos a la UI
/// (las métricas cambian lento; 2 eventos/s es suficiente y barato).
const ANALYSIS_EMIT_INTERVAL: Duration = Duration::from_millis(500);

/// Capacidad del canal callback → hilo de análisis (marcos).
const ANALYSIS_CHANNEL_CAPACITY: usize = 64;

/// Capacidad de los ring buffers de denoise (muestras).
///
/// ~85 ms a 48 kHz: suficiente para absorber la jitter entre el callback
/// y el hilo de denoise.
const DENOISE_RING_CAPACITY: usize = 4096;

/// Latencia reportada antes de que haya señal circulando.
const LATENCY_UNKNOWN: f32 = 0.0;

/// Piso de dBFS para las bandas del espectro antes de la primera FFT.
const SILENCE_DB: f32 = -120.0;

/// Configuración con la que se arranca el motor de audio.
#[derive(Debug, Clone)]
pub struct AudioEngineConfig {
    /// Frecuencia de muestreo objetivo (Hz). Se usa si ambos dispositivos la
    /// soportan; si no, se elige una compatible.
    pub sample_rate: u32,
    /// Tamaño de buffer objetivo (muestras por callback). `None` → se elige
    /// automáticamente con una heurística según el tipo de dispositivo.
    pub buffer_size: Option<usize>,
    /// Host de audio a usar (p. ej. `"alsa"`, `"jack"`, `"pipewire"`).
    /// `None` → predeterminado del sistema.
    pub audio_host: Option<String>,
    /// Dispositivo de entrada (micrófono). `None` → predeterminado del host.
    pub input_device: Option<String>,
    /// Dispositivo de salida (altavoces/interfaz). `None` → predeterminado.
    pub output_device: Option<String>,
    /// Preset de la cadena DSP con el que arranca el pipeline.
    pub initial_preset: PresetId,
}

impl Default for AudioEngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            buffer_size: None,
            audio_host: None,
            input_device: None,
            output_device: None,
            initial_preset: PresetId::Dry,
        }
    }
}

/// Mango para controlar el ciclo de vida de un motor en ejecución.
pub struct EngineHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl EngineHandle {
    /// Solicita la detención controlada del motor.
    ///
    /// Los streams se cierran de forma ordenada y se emite un evento
    /// [`EngineEvent::Status`] con estado `Stopped`.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Bloquea hasta que el hilo del motor haya terminado por completo.
    pub fn join(mut self) {
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

/// Dirección de un dispositivo de audio (entrada o salida).
#[derive(Debug, Clone, Copy)]
enum Direction {
    /// Captura (micrófonos, interfaces).
    Input,
    /// Reproducción (altavoces, auriculares).
    Output,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Input => write!(f, "input"),
            Direction::Output => write!(f, "output"),
        }
    }
}

/// Motor de audio: captura → procesado → salida en tiempo real.
pub struct AudioEngine;

impl AudioEngine {
    /// Enumera los hosts de audio disponibles en el sistema.
    ///
    /// Cada host (ALSA, JACK, PipeWire, etc.) tiene su propio conjunto de
    /// dispositivos. Devuelve la lista de hosts y el identificador del
    /// predeterminado.
    pub fn list_hosts() -> Result<(Vec<AudioHostInfo>, String)> {
        let default_host_id = cpal::default_host().id();
        let hosts: Vec<AudioHostInfo> = cpal::available_hosts()
            .into_iter()
            .map(|id| {
                let name = id.name().to_string();
                let is_default = id == default_host_id;
                AudioHostInfo {
                    id: name.to_lowercase(),
                    name,
                    is_default,
                }
            })
            .collect();
        let default_id = default_host_id.name().to_lowercase();
        Ok((hosts, default_id))
    }

    /// Enumera los dispositivos de entrada y salida de un host específico.
    ///
    /// Si el `host_id` no es válido, devuelve un error. Esto permite al
    /// usuario elegir dispositivos de un backend concreto (p. ej. JACK para
    /// routing profesional en Linux).
    pub fn list_devices_for_host(
        host_id: &str,
    ) -> Result<(Vec<AudioDeviceInfo>, Vec<AudioDeviceInfo>)> {
        let host = resolve_host(host_id)?;

        let default_input = host.default_input_device().and_then(|d| d.name().ok());
        let default_output = host.default_output_device().and_then(|d| d.name().ok());

        let inputs = host
            .input_devices()
            .map_err(|e| Error::audio(format!("list input devices for {host_id}: {e}")))?;
        let outputs = host
            .output_devices()
            .map_err(|e| Error::audio(format!("list output devices for {host_id}: {e}")))?;

        Ok((
            collect_devices(inputs, default_input)?,
            collect_devices(outputs, default_output)?,
        ))
    }

    /// Enumera los dispositivos de entrada y salida disponibles en el sistema.
    ///
    /// Devuelve `(inputs, outputs)`; cada dispositivo marca si es el
    /// predeterminado del sistema.
    pub fn list_devices() -> Result<(Vec<AudioDeviceInfo>, Vec<AudioDeviceInfo>)> {
        let host = cpal::default_host();

        let default_input = host.default_input_device().and_then(|d| d.name().ok());
        let default_output = host.default_output_device().and_then(|d| d.name().ok());

        let inputs = host
            .input_devices()
            .map_err(|e| Error::audio(format!("list input devices: {e}")))?;
        let outputs = host
            .output_devices()
            .map_err(|e| Error::audio(format!("list output devices: {e}")))?;

        Ok((
            collect_devices(inputs, default_input)?,
            collect_devices(outputs, default_output)?,
        ))
    }

    /// Arranca el pipeline de audio con la configuración indicada.
    ///
    /// Los eventos del motor (niveles, estado, DSP, análisis, avisos) se envían
    /// por el canal `tx`. El pipeline corre en un hilo dedicado; usa
    /// [`EngineHandle`] para detenerlo, [`DspHandle`] para reconfigurar la
    /// cadena en vivo y [`AnalysisHandle`] para consultar el análisis vocal.
    ///
    /// Devuelve `(EngineHandle, DspHandle, AnalysisHandle)`: el primero
    /// controla el ciclo de vida; el segundo, la cadena DSP (presets y bypass);
    /// el tercero, el análisis vocal (últimas métricas, resumen de sesión y
    /// aplicación de sugerencias).
    pub fn start(
        config: AudioEngineConfig,
        tx: mpsc::Sender<EngineEvent>,
    ) -> Result<(EngineHandle, DspHandle, AnalysisHandle)> {
        // Resolver el host de audio: el elegido o el predeterminado del sistema.
        let host = match &config.audio_host {
            Some(host_id) => resolve_host(host_id)?,
            None => cpal::default_host(),
        };

        // Resolver dispositivos: por nombre o los predeterminados del host.
        let input = resolve_device(&host, config.input_device.as_deref(), Direction::Input)?;
        let output = resolve_device(&host, config.output_device.as_deref(), Direction::Output)?;

        let input_name = input.name().unwrap_or_else(|_| "input".to_string());
        let output_name = output.name().unwrap_or_else(|_| "output".to_string());

        // Tasa de muestreo común (ambos streams deben usar la misma para que el
        // ring buffer sea una copia 1:1 sin resampling).
        let sample_rate = pick_sample_rate(&input, &output, config.sample_rate);

        // Tamaño de buffer efectivo: el pedido o una heurística por dispositivo.
        let buffer_size = config
            .buffer_size
            .unwrap_or_else(|| heuristic_buffer_size(&input_name, &output_name));

        let input_config = build_stream_config(&input, Direction::Input, buffer_size)?;
        let output_config = build_stream_config(&output, Direction::Output, buffer_size)?;

        // Buffer de anillo lock-free: puente captura → salida.
        let ring_capacity = sample_rate as usize * RING_CAPACITY_SECS as usize;
        let (mut producer, mut consumer) = HeapRb::<f32>::new(ring_capacity).split();

        let stop = Arc::new(AtomicBool::new(false));
        // Latencia actual (ms), compartida entre los dos callbacks vía bits.
        let last_latency = Arc::new(AtomicU32::new(LATENCY_UNKNOWN.to_bits()));

        // --- Canal de control DSP (UI → hilo de audio) ----------------------
        // El hilo de audio lo consume con `try_recv()` en cada callback: una
        // consulta sin espera. El hilo de control (UI) construye las nuevas
        // cadenas y las envía ya listas, de modo que en el callback no se
        // asigna memoria: solo se cambia el puntero y se libera la anterior.
        let (dsp_tx, dsp_rx) = mpsc::channel::<DspCommand>();
        let max_frames = buffer_size.max(1);
        let initial_chain = ChainProcessor::new(config.initial_preset, sample_rate, max_frames);
        let initial_state = initial_chain.state();
        let dsp_handle = DspHandle::new(
            dsp_tx,
            tx.clone(),
            config.initial_preset,
            sample_rate,
            max_frames,
        );

        // --- Análisis vocal ---------------------------------------------------
        // El callback acumula bandas (sin asignación) y envía un marco por
        // `ANALYSIS_FRAME_INTERVAL` a este canal; el hilo de análisis calcula
        // métricas, sugerencias y el resumen de sesión y los expone a la UI.
        let analysis_shared = Arc::new(Mutex::new(AnalysisShared::default()));
        let (analysis_tx, analysis_rx) =
            mpsc::sync_channel::<VoiceFrame>(ANALYSIS_CHANNEL_CAPACITY);

        // --- Hilo de denoise offloaded ----------------------------------------
        // Si el preset tiene denoise ONNX, se lanza un hilo dedicado que
        // ejecuta la inferencia fuera del callback de audio. El callback envía
        // audio crudo por un ring buffer y recibe el resultado denoiseado por
        // otro. Si el hilo se retrasa, el callback usa la señal sin denoise.
        let denoise_shared = Arc::new(DenoiseShared {
            enabled: AtomicBool::new(false),
            mix: AtomicF32::new(0.0),
        });
        let stop_denoise = Arc::new(AtomicBool::new(false));
        let mut denoise_handle: Option<DenoiseHandle> = None;

        // Buffers reutilizables para denoise en el callback.
        let mut denoise_in_buf = Vec::with_capacity(max_frames);
        let mut denoise_out_buf = Vec::with_capacity(max_frames);

        // Ring buffers para comunicación callback ↔ hilo de denoise.
        // Siempre se crean (el callback los ignora si no hay hilo).
        let (mut denoise_in_prod, denoise_in_cons) =
            HeapRb::<f32>::new(DENOISE_RING_CAPACITY).split();
        let (denoise_out_prod, mut denoise_out_cons) =
            HeapRb::<f32>::new(DENOISE_RING_CAPACITY).split();

        // Lanzar el hilo de denoise si el preset tiene denoise y los modelos
        // ONNX están disponibles.
        if initial_chain.has_denoise() {
            match denoise_thread::spawn_denoise_thread(
                build_denoise_processor(sample_rate, max_frames),
                sample_rate,
                denoise_in_cons,
                denoise_out_prod,
                denoise_shared.clone(),
                stop_denoise.clone(),
            ) {
                Ok(handle) => {
                    denoise_handle = Some(handle);
                    log::info!("denoise thread spawned (offloaded mode)");
                }
                Err(e) => {
                    log::warn!("denoise offload failed, using inline: {e}");
                }
            }
        }

        // --- Stream de entrada (captura) -------------------------------------
        let mut level_meter = LevelMeter::new();
        let mut output_meter = LevelMeter::new();
        let mut chain = initial_chain;
        // Buffer reutilizable para la salida de la cadena DSP (sin asignar por
        // callback).
        let mut scratch = Vec::with_capacity(max_frames);
        let mut last_emit = Instant::now();
        let mut overrun_warned = false;
        let tx_capture = tx.clone();
        let tx_capture_errors = tx.clone();
        let stop_capture = stop.clone();
        let last_latency_in = last_latency.clone();

        // Análisis vocal: divisor de bandas del callback + canal hacia el hilo.
        let mut splitter = BandSplitter::new(sample_rate);
        let mut last_frame_emit = Instant::now();
        let analysis_frame_tx = analysis_tx.clone();

        // Espectro (FFT): analizador sin asignación en el callback; las bandas
        // más recientes se copian y se emiten acotadas por tiempo.
        let mut spectrum = SpectrumAnalyzer::new(sample_rate);
        let mut last_spectrum = [SILENCE_DB; SPECTRUM_BIN_COUNT];
        let mut last_spectrum_emit = Instant::now();
        let tx_spectrum = tx.clone();

        let input_stream = input
            .build_input_stream::<f32, _, _>(
                &input_config,
                move |samples: &[f32], _info: &cpal::InputCallbackInfo| {
                    // Este callback corre en el hilo de audio: O(n), sin
                    // bloqueos ni asignaciones.
                    if samples.is_empty() || stop_capture.load(Ordering::Relaxed) {
                        return;
                    }

                    // 0) Aplicar comandos DSP pendientes (preset/bypass). El
                    //    `ApplyPreset` llega con la cadena ya construida; aquí
                    //    solo se intercambia y se libera la anterior.
                    while let Ok(command) = dsp_rx.try_recv() {
                        match command {
                            DspCommand::ApplyPreset(new_chain) => {
                                chain = *new_chain;
                            }
                            DspCommand::SetGlobalBypass(bypass) => {
                                chain.set_global_bypass(bypass);
                            }
                            DspCommand::SetLinkBypass { name, bypass } => {
                                chain.set_link_bypass(&name, bypass);
                            }
                            DspCommand::SetLinkProcessor {
                                name,
                                processor,
                                eq_bands,
                            } => {
                                chain.set_link_processor(&name, processor, eq_bands);
                            }
                            DspCommand::SetLinkGate { processor, params } => {
                                chain.set_link_gate(processor, params);
                            }
                            DspCommand::SetDenoise { processor, params } => {
                                chain.set_link_denoise(processor, params);
                            }
                            DspCommand::SetFeedbackSuppressor { processor, params } => {
                                chain.set_link_feedback(processor, params);
                            }
                            DspCommand::SetPitchCorrection { processor, params } => {
                                chain.set_link_pitch_correction(processor, params);
                            }
                            DspCommand::SetLinkDelay { processor, params } => {
                                chain.set_link_delay(processor, params);
                            }
                            DspCommand::SetLinkReverb { processor, params } => {
                                chain.set_link_reverb(processor, params);
                            }
                        }
                    }

                    // 1) Cadena DSP: offloaded denoise o procesamiento inline.
                    scratch.resize(samples.len(), 0.0);
                    let info = ProcessingInfo {
                        sample_rate,
                        frames: samples.len(),
                    };

                    if chain.has_denoise() && denoise_handle.is_some() {
                        // --- Modo offloaded: denoise en hilo dedicado ---
                        // a) Procesar módulos pre-denoise (HighPass, etc.).
                        denoise_in_buf.resize(samples.len(), 0.0);
                        chain.process_pre_denoise(samples, &mut denoise_in_buf, &info);

                        // b) Enviar audio crudo al hilo de denoise.
                        let _ = denoise_in_prod.push_slice(&denoise_in_buf);

                        // c) Leer resultado denoiseado del ring de salida.
                        denoise_out_buf.resize(samples.len(), 0.0);
                        let n = denoise_out_cons.pop_slice(&mut denoise_out_buf);

                        // d) Mezclar seco/húmedo y procesar módulos post-denoise.
                        if n > 0 {
                            let mix = denoise_shared.mix.load(Ordering::Relaxed);
                            let dry = 1.0 - mix;
                            for i in 0..n {
                                denoise_out_buf[i] =
                                    denoise_in_buf[i] * dry + denoise_out_buf[i] * mix;
                            }
                            chain.process_post_denoise(&denoise_out_buf[..n], &mut scratch, &info);
                        } else {
                            // Sin datos denoiseados aún: usar la señal pre-denoise
                            // como fallback (degradación transparente).
                            chain.process_post_denoise(&denoise_in_buf, &mut scratch, &info);
                        }
                    } else {
                        // --- Modo inline: cadena completa (sin denoise ONNX). ---
                        chain.process(samples, &mut scratch, &info);
                    }

                    // 2) Encolar la señal procesada hacia la salida.
                    let pushed = producer.push_slice(&scratch);
                    if pushed < scratch.len() && !overrun_warned {
                        overrun_warned = true;
                        let _ = tx_capture.send(EngineEvent::Warning {
                            message: "input overrun: el buffer intermedio se llenó \
                                      (aumenta el tamaño de buffer)"
                                .into(),
                        });
                    }

                    // 3) Análisis vocal: acumular bandas del audio crudo y
                    //    extraer un marco cada intervalo (sin asignar memoria).
                    splitter.process(samples);
                    if last_frame_emit.elapsed() >= ANALYSIS_FRAME_INTERVAL {
                        last_frame_emit = Instant::now();
                        let frame = splitter.frame();
                        let _ = analysis_frame_tx.try_send(frame);
                    }

                    // 4) Espectro: acumular la FFT (sin asignar) y emitir la
                    //    última ventana de bandas acotada por tiempo.
                    if let Some(bins) = spectrum.process(samples) {
                        last_spectrum = bins;
                    }
                    if last_spectrum_emit.elapsed() >= SPECTRUM_EMIT_INTERVAL {
                        last_spectrum_emit = Instant::now();
                        let _ = tx_spectrum.send(EngineEvent::Spectrum(SpectrumSample {
                            bins_db: last_spectrum,
                            sample_rate,
                            captured_at_ms: now_ms(),
                        }));
                    }

                    // 5) Medir nivel de entrada y de salida (pre/post) y emitir
                    //    evento, acotado por tiempo.
                    let input_levels = level_meter.process(samples);
                    let output_levels = output_meter.process(&scratch);
                    if last_emit.elapsed() >= LEVEL_EMIT_INTERVAL {
                        last_emit = Instant::now();
                        let _ = tx_capture.send(EngineEvent::Level(LevelSample {
                            input_rms_db: input_levels.rms_db,
                            input_peak_db: input_levels.peak_db,
                            output_rms_db: output_levels.rms_db,
                            output_peak_db: output_levels.peak_db,
                            latency_ms: f32::from_bits(last_latency_in.load(Ordering::Relaxed)),
                            captured_at_ms: now_ms(),
                        }));
                    }
                },
                move |err| {
                    let _ = tx_capture_errors.send(EngineEvent::Warning {
                        message: format!("input stream: {err}"),
                    });
                },
                None,
            )
            .map_err(|e| Error::audio(format!("build input stream: {e}")))?;

        // --- Stream de salida (playback) -------------------------------------
        let mut underflow_warned = false;
        let tx_output = tx.clone();
        let tx_output_errors = tx.clone();
        let last_latency_out = last_latency.clone();
        let stop_output = stop.clone();

        let output_stream = output
            .build_output_stream::<f32, _, _>(
                &output_config,
                move |out: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    if stop_output.load(Ordering::Relaxed) {
                        return;
                    }

                    // 1) Consumir la señal procesada; lo que no alcance queda a
                    // cero (silencio) en ese callback.
                    let consumed = consumer.pop_slice(out);

                    // 2) Latencia = muestras en vuelo → ms.
                    let latency_ms = consumer.occupied_len() as f32 / sample_rate as f32 * 1000.0;
                    last_latency_out.store(latency_ms.to_bits(), Ordering::Relaxed);

                    // 3) Aviso único de underrun (buffer vacío = huecos de audio).
                    if consumed < out.len() && !underflow_warned {
                        underflow_warned = true;
                        let _ = tx_output.send(EngineEvent::Warning {
                            message: "output underrun: aumente el tamaño de buffer \
                                      para evitar huecos de audio"
                                .into(),
                        });
                    }
                },
                move |err| {
                    let _ = tx_output_errors.send(EngineEvent::Warning {
                        message: format!("output stream: {err}"),
                    });
                },
                None,
            )
            .map_err(|e| Error::audio(format!("build output stream: {e}")))?;

        // --- Hilo de análisis vocal --------------------------------------------
        // Consume los marcos del callback, desliza la ventana de métricas,
        // evalúa sugerencias y mantiene el resumen de sesión. Aquí (no en el
        // callback) es donde sí se construyen Strings y se envían eventos.
        // El hilo se autofinaliza con el flag `stop`; no se une explícitamente.
        let _analysis_thread = {
            let stop_analysis = stop.clone();
            let tx_analysis = tx.clone();
            let shared = analysis_shared.clone();
            thread::Builder::new()
                .name("voxlfa-analysis".to_string())
                .spawn(move || {
                    let frame_interval_ms = ANALYSIS_FRAME_INTERVAL.as_millis() as u32;
                    let mut analyzer = VoiceAnalyzer::new(ANALYSIS_WINDOW_MS, frame_interval_ms);
                    let mut session = SessionTracker::new(now_ms(), frame_interval_ms);
                    let suggestions = SuggestionEngine;
                    let mut last_emit = Instant::now();

                    loop {
                        match analysis_rx.recv_timeout(Duration::from_millis(100)) {
                            Ok(frame) => {
                                session.update(&frame);
                                analyzer.push(frame);
                                if let Some(metrics) = analyzer.metrics() {
                                    let suggestions = suggestions.evaluate(&metrics);
                                    session.add_suggestions(suggestions.len() as u32);
                                    let sample = AnalysisSample {
                                        metrics,
                                        suggestions,
                                        captured_at_ms: now_ms(),
                                    };
                                    if let Ok(mut guard) = shared.lock() {
                                        guard.last_sample = Some(sample.clone());
                                        guard.session = Some(session.summary());
                                    }
                                    if last_emit.elapsed() >= ANALYSIS_EMIT_INTERVAL {
                                        last_emit = Instant::now();
                                        let _ = tx_analysis.send(EngineEvent::Analysis(sample));
                                    }
                                }
                            }
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                        }
                        if stop_analysis.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                    // Resumen final de la sesión (visible tras detener el motor).
                    if let Ok(mut guard) = shared.lock() {
                        guard.session = Some(session.summary());
                    }
                })?
        };

        // --- Hilo del motor: mantiene vivos los streams y gestiona el ciclo ---
        let host_name = config.audio_host.clone();
        let tx_thread = tx.clone();
        let stop_thread = stop.clone();
        let thread = thread::Builder::new()
            .name("voxlfa-audio-engine".to_string())
            .spawn(move || {
                // La propiedad de los streams vive aquí: si se soltaran, cpal
                // cerraría los dispositivos y el audio se detendría.
                let _capture = input_stream;
                let _playback = output_stream;

                let _ = tx_thread.send(status_event(
                    EngineState::Running,
                    sample_rate,
                    buffer_size,
                    0.0,
                    host_name,
                    Some(input_name),
                    Some(output_name),
                ));

                // Estado inicial de la cadena DSP (una vez en Running).
                let _ = tx_thread.send(EngineEvent::Dsp(initial_state));

                while !stop_thread.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(200));
                }

                let _ = tx_thread.send(status_event(
                    EngineState::Stopped,
                    sample_rate,
                    buffer_size,
                    0.0,
                    None,
                    None,
                    None,
                ));
            })?;

        let analysis_handle = AnalysisHandle::new(analysis_shared, dsp_handle.clone());

        Ok((
            EngineHandle {
                stop,
                thread: Some(thread),
            },
            dsp_handle,
            analysis_handle,
        ))
    }
}

/// Elige un tamaño de buffer según el tipo de dispositivo detectado.
///
/// Heurística de "ajuste fino de latencia por dispositivo": los dispositivos
/// con latencia inherente alta (Bluetooth, HDMI) reciben un buffer grande para
/// evitar *underruns*; las interfaces USB profesionales, uno pequeño para
/// minimizar latencia; el resto usa un valor equilibrado. El valor es un
/// objetivo: `build_stream_config` lo valida contra el rango soportado.
fn heuristic_buffer_size(input_name: &str, output_name: &str) -> usize {
    let names = format!("{input_name} {output_name}").to_lowercase();

    // Dispositivos de alta latencia inherente → buffer grande (estable).
    const HIGH_LATENCY: &[&str] = &[
        "bluetooth",
        "wireless",
        "hdmi",
        "displayport",
        "airpods",
        "bluez",
    ];
    if HIGH_LATENCY.iter().any(|kw| names.contains(kw)) {
        return 1024;
    }

    // Interfaces USB de gama alta → buffer pequeño (baja latencia).
    const LOW_LATENCY: &[&str] = &["scarlett", "focusrite", "steinberg", "rme"];
    if LOW_LATENCY.iter().any(|kw| names.contains(kw)) {
        return 128;
    }

    // Interfaces USB de gama media/baja (Behringer, genéricas) → buffer
    // moderado. Estas interfaces tienen mayor latencia USB inherente y
    // necesitan más margen para evitar underruns con DSP activo.
    const BUDGET_USB: &[&str] = &[
        "behringer",
        "umc",
        "u-phoria",
        "yamaha",
        "presonus",
        "arturia",
        "maono",
        "mtrack",
        "xonar",
    ];
    if BUDGET_USB.iter().any(|kw| names.contains(kw)) {
        return 512;
    }

    // Cualquier otro dispositivo USB no clasificado.
    if names.contains("usb") || names.contains("interface") {
        return 256;
    }

    // Predeterminado equilibrado para el resto (micrófonos integrados, etc.).
    256
}

/// Convierte un iterador de dispositivos en la lista de información tipada.
fn collect_devices<I>(devices: I, default: Option<String>) -> Result<Vec<AudioDeviceInfo>>
where
    I: IntoIterator<Item = cpal::Device>,
{
    let mut list = Vec::new();
    for device in devices {
        let name = device
            .name()
            .map_err(|e| Error::audio(format!("device name: {e}")))?;
        let is_default = default.as_deref() == Some(name.as_str());
        list.push(AudioDeviceInfo { name, is_default });
    }
    Ok(list)
}

/// Construye un evento de estado a partir de sus partes.
fn status_event(
    state: EngineState,
    sample_rate: u32,
    buffer_size: usize,
    latency_ms: f32,
    audio_host: Option<String>,
    input_device: Option<String>,
    output_device: Option<String>,
) -> EngineEvent {
    EngineEvent::Status(EngineStatus {
        state,
        sample_rate,
        buffer_size,
        latency_ms,
        audio_host,
        input_device,
        output_device,
    })
}

/// Resuelve el dispositivo pedido (por nombre) o el predeterminado del sistema.
fn resolve_device(
    host: &cpal::Host,
    name: Option<&str>,
    direction: Direction,
) -> Result<cpal::Device> {
    match name {
        Some(requested) => find_device(host, requested, direction)
            .ok_or_else(|| Error::audio(format!("device not found: {requested}"))),
        None => match direction {
            Direction::Input => host
                .default_input_device()
                .ok_or_else(|| Error::audio("no default input device available")),
            Direction::Output => host
                .default_output_device()
                .ok_or_else(|| Error::audio("no default output device available")),
        },
    }
}

/// Busca un dispositivo por su nombre exacto en la dirección indicada.
fn find_device(host: &cpal::Host, name: &str, direction: Direction) -> Option<cpal::Device> {
    let devices = match direction {
        Direction::Input => host.input_devices().ok()?,
        Direction::Output => host.output_devices().ok()?,
    };
    devices
        .into_iter()
        .find_map(|device| device.name().ok().filter(|n| n == name).map(|_| device))
}

/// Resuelve un host de audio por su ID (nombre en minúsculas, p. ej. `"alsa"`,
/// `"jack"`, `"pipewire"`).
///
/// El `HostId` de cpal es un enum; este helper hace la conversión desde string
/// comparando con los hosts disponibles. Devuelve error si el ID no corresponde
/// a ningún host registrado.
fn resolve_host(host_id: &str) -> Result<cpal::Host> {
    let available = cpal::available_hosts();
    let target = available
        .into_iter()
        .find(|id| id.name().to_lowercase() == host_id);
    match target {
        Some(id) => {
            cpal::host_from_id(id).map_err(|e| Error::audio(format!("create host {host_id}: {e}")))
        }
        None => Err(Error::audio(format!("unknown audio host: {host_id}"))),
    }
}

/// Elige una frecuencia de muestreo compatible con ambos dispositivos.
///
/// Prefiere la tasa pedida; si algún dispositivo no la soporta, usa la tasa
/// por defecto de la entrada (fallo seguro).
fn pick_sample_rate(input: &cpal::Device, output: &cpal::Device, preferred: u32) -> u32 {
    if supports_sample_rate(input, Direction::Input, preferred)
        && supports_sample_rate(output, Direction::Output, preferred)
    {
        preferred
    } else {
        input
            .default_input_config()
            .map(|config| config.sample_rate().0)
            .unwrap_or(preferred)
    }
}

/// Indica si el dispositivo soporta la frecuencia de muestreo dada.
fn supports_sample_rate(device: &cpal::Device, direction: Direction, rate: u32) -> bool {
    let in_range = |range: cpal::SupportedStreamConfigRange| {
        rate >= range.min_sample_rate().0 && rate <= range.max_sample_rate().0
    };
    match direction {
        Direction::Input => device
            .supported_input_configs()
            .is_ok_and(|mut ranges| ranges.any(in_range)),
        Direction::Output => device
            .supported_output_configs()
            .is_ok_and(|mut ranges| ranges.any(in_range)),
    }
}

/// Construye la configuración de stream de un dispositivo.
///
/// Pide un tamaño de buffer fijo (baja latencia) solo si el dispositivo lo
/// soporta; en otro caso deja el predeterminado del sistema.
fn build_stream_config(
    device: &cpal::Device,
    direction: Direction,
    buffer_size: usize,
) -> Result<cpal::StreamConfig> {
    use cpal::{BufferSize, SupportedBufferSize};

    let default = match direction {
        Direction::Input => device.default_input_config(),
        Direction::Output => device.default_output_config(),
    }
    .map_err(|e| Error::audio(format!("default {direction} config: {e}")))?;

    let buffer_size = match default.buffer_size() {
        SupportedBufferSize::Range { min, max }
            if (buffer_size as u32) >= *min && (buffer_size as u32) <= *max =>
        {
            BufferSize::Fixed(buffer_size as u32)
        }
        _ => BufferSize::Default,
    };

    Ok(cpal::StreamConfig {
        channels: default.channels(),
        sample_rate: default.sample_rate(),
        buffer_size,
    })
}

/// Tiempo monotónico (ms) desde la época del sistema, para graficar muestras.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Construye el procesador de denoise para el hilo dedicado.
///
/// Prioriza ONNX DeepFilterNet3 si los modelos están disponibles;
/// en caso contrario, usa RNNoise (puro Rust).
fn build_denoise_processor(_sample_rate: u32, _max_frames: usize) -> Box<dyn AudioProcessor> {
    #[cfg(feature = "onnx")]
    {
        if let Some(dir) = crate::models::models_dir() {
            if crate::models::ModelStatus::check(&dir).available {
                match crate::dsp::denoise_onnx::OnnxDenoise::new(dir) {
                    Ok(processor) => return Box::new(processor),
                    Err(e) => {
                        log::warn!("ONNX denoise init failed for thread: {e}");
                    }
                }
            }
        }
    }
    #[cfg(feature = "rnnoise")]
    {
        #[allow(clippy::needless_return)]
        return Box::new(crate::dsp::RnnoiseDenoise::new());
    }
    #[cfg(not(any(feature = "onnx", feature = "rnnoise")))]
    {
        Box::new(crate::dsp::PassThroughProcessor::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristic_prefers_small_buffer_for_usb_interfaces() {
        // Interfaces de gama alta → 128 (latencia mínima).
        assert_eq!(heuristic_buffer_size("Scarlett 2i2 USB", "Monitor 01"), 128);
        assert_eq!(
            heuristic_buffer_size("Focusrite Scarlett Solo", "USB Audio"),
            128
        );
        assert_eq!(heuristic_buffer_size("RME Babyface Pro", "USB Audio"), 128);
    }

    #[test]
    fn heuristic_uses_moderate_buffer_for_budget_usb_interfaces() {
        // Interfaces de gama media/baja → 512 (estabilidad con DSP activo).
        assert_eq!(
            heuristic_buffer_size("UMC22 USB Audio", "UMC22 USB Audio"),
            512
        );
        assert_eq!(
            heuristic_buffer_size("U-Phoria UMC22", "USB Audio Codec"),
            512
        );
        assert_eq!(heuristic_buffer_size("BEHRINGER UMC 22", "USB Audio"), 512);
        // USB genérico no clasificado → 256.
        assert_eq!(
            heuristic_buffer_size("Micrófono (USB Audio)", "Altavoces (USB Audio)"),
            256
        );
    }

    #[test]
    fn heuristic_uses_large_buffer_for_bluetooth_and_hdmi() {
        assert_eq!(
            heuristic_buffer_size("Micrófono (Bluetooth)", "AirPods"),
            1024
        );
        assert_eq!(
            heuristic_buffer_size("BuiltIn Microphone", "HDMI Output"),
            1024
        );
    }

    #[test]
    fn heuristic_defaults_to_balanced() {
        assert_eq!(
            heuristic_buffer_size("BuiltIn Microphone", "BuiltIn Output"),
            256
        );
    }
}
