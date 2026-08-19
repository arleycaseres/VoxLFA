//! Telemetría opcional y anónima (Fase 3+).
//!
//! Recopila métricas de uso **sin información personal identificable** (PII):
//! versión de la app, plataforma, duración de sesiones, presets usados y
//! configuración de audio. El usuario debe dar su consentimiento explícito
//! antes de que se envíe cualquier dato.
//!
//! El módulo define los tipos de evento y un [`TelemetryHandle`] que emite
//! eventos por un canal `mpsc`. El consumidor (el desktop) decide cómo
//! transportarlos (HTTP, archivo local, etc.).
//!
//! **Diseño:** el core no depende de bibliotecas HTTP; la capa de red vive en
//! el desktop (`reqwest` o similar). Esto mantiene el core ligero y testeable.

use std::sync::mpsc;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Evento anónimo de telemetría emitido por el motor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TelemetryEvent {
    /// La aplicación se inició (una vez por arranque).
    AppStarted {
        /// Versión de voxlfa (ej. `"0.1.0"`).
        version: String,
        /// Sistema operativo: `"linux"`, `"windows"` o `"macos"`.
        os: String,
        /// Arquitectura de CPU: `"x86_64"`, `"aarch64"`, etc.
        arch: String,
    },
    /// Una sesión de audio comenzó.
    SessionStarted {
        /// Preset aplicado al arrancar.
        preset: String,
        /// Frecuencia de muestreo (Hz).
        sample_rate: u32,
        /// Tamaño de buffer (muestras/callback).
        buffer_size: usize,
    },
    /// Una sesión de audio terminó.
    SessionEnded {
        /// Duración de la sesión en segundos.
        duration_secs: f64,
        /// Preset activo al detener.
        preset: String,
        /// Número de cambios de preset durante la sesión.
        preset_changes: u32,
        /// Latencia media observada (ms).
        avg_latency_ms: f32,
    },
    /// Un módulo DSP fue activado/desactivado (bypass toggle).
    ModuleToggled {
        /// Nombre del módulo (ej. `"eq"`, `"compressor"`).
        module: String,
        /// `true` si se activó, `false` si se puso en bypass.
        enabled: bool,
    },
    /// La telemetría fue habilitada o deshabilitada por el usuario.
    ConsentChanged {
        /// `true` si el usuario activó la telemetría.
        enabled: bool,
    },
}

/// Handle para emitir eventos de telemetría desde cualquier hilo.
///
/// Es clonable y ligero; el canal interno es `mpsc` (un emisor por hilo).
#[derive(Clone)]
pub struct TelemetryHandle {
    tx: mpsc::Sender<TelemetryEvent>,
}

impl TelemetryHandle {
    /// Crea un handle conectado al receptor dado.
    pub fn new(tx: mpsc::Sender<TelemetryEvent>) -> Self {
        Self { tx }
    }

    /// Emite un evento de telemetría. Es best-effort: si el canal está
    /// cerrado o lleno, se descarta el evento silenciosamente.
    pub fn emit(&self, event: TelemetryEvent) {
        let _ = self.tx.send(event);
    }

    /// Emite `AppStarted` con la información de la plataforma.
    pub fn app_started(&self, version: &str) {
        self.emit(TelemetryEvent::AppStarted {
            version: version.to_string(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        });
    }
}

/// Receptor de eventos de telemetría.
///
/// El desktop consume eventos de este receptor para enviarlos (o almacenarlos)
/// de forma asíncrona.
pub struct TelemetryReceiver {
    rx: mpsc::Receiver<TelemetryEvent>,
}

impl TelemetryReceiver {
    /// Crea un receptor a partir de un canal `mpsc`.
    pub fn new(rx: mpsc::Receiver<TelemetryEvent>) -> Self {
        Self { rx }
    }

    /// Intenta recibir un evento sin bloquear.
    pub fn try_recv(&self) -> Option<TelemetryEvent> {
        self.rx.try_recv().ok()
    }

    /// Recibe un evento bloqueando hasta que haya uno disponible.
    pub fn recv(&self) -> Option<TelemetryEvent> {
        self.rx.recv().ok()
    }
}

/// Crea un par handle/receptor para telemetría.
pub fn channel() -> (TelemetryHandle, TelemetryReceiver) {
    let (tx, rx) = mpsc::channel();
    (TelemetryHandle::new(tx), TelemetryReceiver::new(rx))
}

/// Cronómetro de sesión que mide la duración entre start y stop.
pub struct SessionTimer {
    start: Instant,
    preset_changes: u32,
    total_latency_ms: f64,
    latency_samples: u32,
}

impl SessionTimer {
    /// Inicia el cronómetro.
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
            preset_changes: 0,
            total_latency_ms: 0.0,
            latency_samples: 0,
        }
    }

    /// Registra un cambio de preset.
    pub fn record_preset_change(&mut self) {
        self.preset_changes += 1;
    }

    /// Registra una muestra de latencia.
    pub fn record_latency(&mut self, latency_ms: f32) {
        self.total_latency_ms += latency_ms as f64;
        self.latency_samples += 1;
    }

    /// Devuelve la duración en segundos.
    pub fn duration_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// Devuelve el número de cambios de preset.
    pub fn preset_changes(&self) -> u32 {
        self.preset_changes
    }

    /// Devuelve la latencia media en ms.
    pub fn avg_latency_ms(&self) -> f32 {
        if self.latency_samples == 0 {
            return 0.0;
        }
        (self.total_latency_ms / self.latency_samples as f64) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_sends_events_through_channel() {
        let (handle, receiver) = channel();
        handle.app_started("0.1.0");
        let event = receiver.try_recv().expect("hay un evento");
        match event {
            TelemetryEvent::AppStarted { version, os, arch } => {
                assert_eq!(version, "0.1.0");
                assert!(!os.is_empty());
                assert!(!arch.is_empty());
            }
            _ => panic!("evento inesperado"),
        }
    }

    #[test]
    fn session_timer_tracks_duration() {
        let mut timer = SessionTimer::start();
        timer.record_preset_change();
        timer.record_latency(5.0);
        timer.record_latency(7.0);
        assert_eq!(timer.preset_changes(), 1);
        assert!((timer.avg_latency_ms() - 6.0).abs() < 0.1);
        assert!(timer.duration_secs() >= 0.0);
    }

    #[test]
    fn telemetry_event_serializes_to_json() {
        let event = TelemetryEvent::SessionEnded {
            duration_secs: 120.5,
            preset: "vozLimpia".to_string(),
            preset_changes: 3,
            avg_latency_ms: 4.2,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"sessionEnded\""));
        assert!(json.contains("\"durationSecs\":120.5"));
        assert!(json.contains("\"preset\":\"vozLimpia\""));
    }

    #[test]
    fn consent_changed_event_serializes() {
        let event = TelemetryEvent::ConsentChanged { enabled: true };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"consentChanged\""));
        assert!(json.contains("\"enabled\":true"));
    }
}
