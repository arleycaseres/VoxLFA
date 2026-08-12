//! Acumulación de métricas sobre una ventana deslizante y seguimiento de sesión.
//!
//! El [`VoiceAnalyzer`] guarda los últimos [`VoiceFrame`] en un anillo
//! preasignado y deriva métricas de voz (timbre, dinámica, fatiga, resonancia)
//! a partir de sus energías de banda. El [`SessionTracker`] acumula las mismas
//! métricas a lo largo de toda la sesión para el resumen exportable.
//!
//! Este módulo corre en un hilo dedicado (nunca en el callback de audio): aquí
//! sí se pueden construir `String` y asignar memoria.

use std::time::Instant;

use super::bands::VoiceFrame;
use crate::protocol::{SessionSummary, VoiceMetrics};

/// Número mínimo de marcos necesarios para emitir métricas.
const MIN_FRAMES: usize = 3;

/// Analizador de ventana deslizante de marcos de bandas.
#[derive(Debug, Clone)]
pub struct VoiceAnalyzer {
    /// Marcos guardados en el anillo (los más recientes de la ventana).
    frames: Vec<VoiceFrame>,
    /// Capacidad máxima del anillo (marcos).
    capacity: usize,
    /// Tamaño de la ventana en milisegundos (para reportar).
    window_ms: u32,
}

impl VoiceAnalyzer {
    /// Crea un analizador para una ventana de `window_ms`.
    ///
    /// `frame_interval_ms` es el intervalo entre marcos; la capacidad del anillo
    /// se deriva como `window_ms / frame_interval_ms` (mínimo 3).
    pub fn new(window_ms: u32, frame_interval_ms: u32) -> Self {
        let capacity = (window_ms / frame_interval_ms.max(1)).max(3) as usize;
        Self {
            frames: Vec::with_capacity(capacity),
            capacity,
            window_ms,
        }
    }

    /// Añade un marco al final del anillo (desplaza el más antiguo si llena).
    pub fn push(&mut self, frame: VoiceFrame) {
        if self.frames.len() == self.capacity {
            self.frames.remove(0);
        }
        self.frames.push(frame);
    }

    /// Número de marcos acumulados.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// `true` si la ventana aún no tiene marcos.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Metricas de la ventana actual, o `None` si aún no hay datos suficientes.
    pub fn metrics(&self) -> Option<VoiceMetrics> {
        if self.frames.len() < MIN_FRAMES {
            return None;
        }

        let count = self.frames.len() as f32;
        let avg_rms = self.frames.iter().map(|f| f.rms_db).sum::<f32>() / count;
        let peak = self
            .frames
            .iter()
            .map(|f| f.peak_db)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_rms = self
            .frames
            .iter()
            .map(|f| f.rms_db)
            .fold(f32::INFINITY, f32::min);
        let max_rms = self
            .frames
            .iter()
            .map(|f| f.rms_db)
            .fold(f32::NEG_INFINITY, f32::max);
        let brightness = self.frames.iter().map(brightness_of).sum::<f32>() / count;
        let resonance = self.frames.iter().map(|f| f.lowmid_ratio).sum::<f32>() / count;

        let dynamic_range_db = (max_rms - min_rms).max(0.0);
        let crest_db = peak - avg_rms;

        // Fatiga: esfuerzo sostenido = nivel alto + brillo (tensión).
        let loudness = clamp((avg_rms - (-40.0)) / 20.0);
        let strain = brightness * loudness;
        let fatigue_score = 0.5 * loudness + 0.5 * strain;

        Some(VoiceMetrics {
            rms_db: avg_rms,
            peak_db: peak,
            dynamic_range_db,
            crest_db,
            brightness,
            resonance_score: resonance,
            fatigue_score,
            window_ms: self.window_ms,
        })
    }
}

/// Brillo de un marco: energía ponderada hacia las bandas agudas (0–1 aprox.).
fn brightness_of(frame: &VoiceFrame) -> f32 {
    (0.25 * frame.lowmid_ratio + 0.6 * frame.mid_ratio + 1.0 * frame.high_ratio).clamp(0.0, 1.0)
}

fn clamp(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// Acumulador de métricas de toda la sesión (para el resumen exportable).
#[derive(Debug, Clone)]
pub struct SessionTracker {
    started_at_ms: u64,
    started: Instant,
    frame_interval_ms: u32,
    frame_count: u64,
    sum_rms: f32,
    min_rms: f32,
    max_rms: f32,
    peak: f32,
    sum_brightness: f32,
    fatigue_acc: f32,
    loud_frames: u64,
    suggestions_count: u32,
}

impl SessionTracker {
    /// Crea un acumulador de sesión que empieza a contar desde `started_at_ms`.
    pub fn new(started_at_ms: u64, frame_interval_ms: u32) -> Self {
        Self {
            started_at_ms,
            started: Instant::now(),
            frame_interval_ms,
            frame_count: 0,
            sum_rms: 0.0,
            min_rms: f32::INFINITY,
            max_rms: f32::NEG_INFINITY,
            peak: f32::NEG_INFINITY,
            sum_brightness: 0.0,
            fatigue_acc: 0.0,
            loud_frames: 0,
            suggestions_count: 0,
        }
    }

    /// Acumula un marco en las estadísticas de la sesión.
    pub fn update(&mut self, frame: &VoiceFrame) {
        self.frame_count += 1;
        self.sum_rms += frame.rms_db;
        self.min_rms = self.min_rms.min(frame.rms_db);
        self.max_rms = self.max_rms.max(frame.rms_db);
        self.peak = self.peak.max(frame.peak_db);
        let brightness = brightness_of(frame);
        self.sum_brightness += brightness;
        let loudness = clamp((frame.rms_db - (-40.0)) / 20.0);
        self.fatigue_acc += 0.5 * loudness + 0.5 * (brightness * loudness);
        if frame.rms_db > -20.0 {
            self.loud_frames += 1;
        }
    }

    /// Registra cuántas sugerencias activas hubo en una ventana.
    pub fn add_suggestions(&mut self, count: u32) {
        self.suggestions_count += count;
    }

    /// Resumen acumulado hasta ahora (frecuente y barato de calcular).
    pub fn summary(&self) -> SessionSummary {
        let count = self.frame_count.max(1) as f32;
        SessionSummary {
            started_at_ms: self.started_at_ms,
            duration_ms: self.frame_count * self.frame_interval_ms as u64,
            avg_rms_db: self.sum_rms / count,
            peak_db: if self.peak.is_finite() {
                self.peak
            } else {
                -120.0
            },
            dynamic_range_db: (self.max_rms - self.min_rms).max(0.0),
            avg_brightness: self.sum_brightness / count,
            fatigue_score: (self.fatigue_acc / count).clamp(0.0, 1.0),
            loud_time_ms: self.loud_frames * self.frame_interval_ms as u64,
            suggestions_count: self.suggestions_count,
        }
    }

    /// El tiempo de sesión transcurrido no depende de que lleguen marcos.
    pub fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(rms: f32, lowmid: f32, mid: f32, high: f32) -> VoiceFrame {
        VoiceFrame {
            rms_db: rms,
            peak_db: rms + 10.0,
            low_ratio: 1.0 - lowmid - mid - high,
            lowmid_ratio: lowmid,
            mid_ratio: mid,
            high_ratio: high,
            zcr: 0.1,
        }
    }

    #[test]
    fn metrics_require_minimum_frames() {
        let mut analyzer = VoiceAnalyzer::new(2000, 200);
        assert!(analyzer.metrics().is_none());
        for i in 0..3 {
            analyzer.push(frame(-20.0, 0.2, 0.5, 0.1));
            assert_eq!(analyzer.metrics().is_some(), i >= 2);
        }
    }

    #[test]
    fn window_slides_keeping_capacity() {
        let mut analyzer = VoiceAnalyzer::new(1000, 200);
        for _ in 0..100 {
            analyzer.push(frame(-20.0, 0.2, 0.5, 0.1));
        }
        assert!(analyzer.len() <= analyzer.capacity);
        assert!(analyzer.metrics().is_some());
    }

    #[test]
    fn bright_signal_yields_higher_fatigue() {
        let mut quiet = VoiceAnalyzer::new(1000, 200);
        for _ in 0..6 {
            quiet.push(frame(-60.0, 0.1, 0.1, 0.05));
        }
        let mut loud_bright = VoiceAnalyzer::new(1000, 200);
        for _ in 0..6 {
            loud_bright.push(frame(-12.0, 0.1, 0.3, 0.5));
        }
        let q = quiet.metrics().unwrap();
        let l = loud_bright.metrics().unwrap();
        assert!(l.fatigue_score > q.fatigue_score);
    }

    #[test]
    fn session_tracker_accumulates() {
        let mut session = SessionTracker::new(1_000_000, 200);
        for _ in 0..10 {
            session.update(&frame(-15.0, 0.2, 0.5, 0.1));
        }
        let summary = session.summary();
        assert_eq!(summary.duration_ms, 2000);
        assert_eq!(summary.loud_time_ms, 2000);
        assert!((summary.avg_rms_db - (-15.0)).abs() < 0.001);
    }
}
