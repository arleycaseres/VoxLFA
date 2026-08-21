//! Reverberación multi-modo: Plate, Hall y Room.
//!
//! Cada modo usa una topología diferente de combs + allpass para simular
//! características acústicas distintas:
//! - **Plate**: reflejos rápidos y densos, cola larga (estándar para vocales).
//! - **Hall**: espacio grande, difusión amplia, cola muy larga.
//! - **Room**: espacio pequeño-mediano, intimate, cola corta.
//!
//! Todos los modos incluyen pre-delay y filtros de corte de graves/agudos
//! en el return (práctica estándar en vivo).

use super::biquad::{BiquadFilter, BiquadKind, BiquadParams};
use super::delay::DelayLine;
use super::processor::{AudioProcessor, ProcessResult, ProcessingInfo};
use crate::protocol::{ReverbMode, ReverbParams};

/// Configuración de topología para cada modo de reverb.
struct ReverbTopology {
    comb_ms: &'static [f32],
    comb_feedback: f32,
    allpass_ms: &'static [f32],
    allpass_feedback: f32,
}

const TOPOLOGY_PLATE: ReverbTopology = ReverbTopology {
    comb_ms: &[23.7, 30.1, 37.3, 41.7, 43.9, 49.1],
    comb_feedback: 0.82,
    allpass_ms: &[5.3, 1.7, 3.1],
    allpass_feedback: 0.5,
};

const TOPOLOGY_HALL: ReverbTopology = ReverbTopology {
    comb_ms: &[29.7, 37.1, 41.1, 43.7, 53.3, 61.7, 71.3],
    comb_feedback: 0.88,
    allpass_ms: &[5.0, 1.7, 3.3, 7.1],
    allpass_feedback: 0.5,
};

const TOPOLOGY_ROOM: ReverbTopology = ReverbTopology {
    comb_ms: &[16.3, 21.7, 25.3, 29.1],
    comb_feedback: 0.72,
    allpass_ms: &[3.7, 1.3],
    allpass_feedback: 0.4,
};

/// Reverberación multi-modo con pre-delay y filtros de return.
#[derive(Debug, Clone)]
pub struct Reverb {
    #[allow(dead_code)]
    mode: ReverbMode,
    comb_lines: Vec<DelayLine>,
    comb_delays: Vec<usize>,
    comb_feedback: f32,
    allpass_lines: Vec<DelayLine>,
    allpass_delays: Vec<usize>,
    allpass_feedback: f32,
    mix: f32,
    damping: f32,
    pre_delay_line: DelayLine,
    pre_delay_samples: usize,
    high_cut: BiquadFilter,
    low_cut: BiquadFilter,
}

impl Reverb {
    /// Crea una reverb a partir de parámetros del protocolo.
    pub fn from_params(params: ReverbParams, sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        let topology = match params.mode {
            ReverbMode::Plate => &TOPOLOGY_PLATE,
            ReverbMode::Hall => &TOPOLOGY_HALL,
            ReverbMode::Room => &TOPOLOGY_ROOM,
        };

        let comb_delays: Vec<usize> = topology
            .comb_ms
            .iter()
            .map(|&ms| ms_to_samples(ms, sr))
            .collect();
        let comb_lines: Vec<DelayLine> = comb_delays
            .iter()
            .map(|&d| DelayLine::new(d.max(1) + 2))
            .collect();

        let allpass_delays: Vec<usize> = topology
            .allpass_ms
            .iter()
            .map(|&ms| ms_to_samples(ms, sr))
            .collect();
        let allpass_lines: Vec<DelayLine> = allpass_delays
            .iter()
            .map(|&d| DelayLine::new(d.max(1) + 2))
            .collect();

        let room_factor = params.room_size.clamp(0.0, 1.0);
        let comb_feedback = topology.comb_feedback * (0.6 + 0.4 * room_factor);

        let pre_delay_samples = ms_to_samples(params.pre_delay_ms, sr);
        let pre_delay_line = DelayLine::new(pre_delay_samples.max(1) + 1);

        let high_cut = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::LowPass,
                freq_hz: params.high_cut_hz,
                gain_db: 0.0,
                q: 0.707,
            },
            sample_rate,
        );
        let low_cut = BiquadFilter::design(
            BiquadParams {
                kind: BiquadKind::HighPass,
                freq_hz: params.low_cut_hz,
                gain_db: 0.0,
                q: 0.707,
            },
            sample_rate,
        );

        Self {
            mode: params.mode,
            comb_lines,
            comb_delays,
            comb_feedback,
            allpass_lines,
            allpass_delays,
            allpass_feedback: topology.allpass_feedback,
            mix: params.wet.clamp(0.0, 1.0),
            damping: params.damping.clamp(0.0, 0.95),
            pre_delay_line,
            pre_delay_samples,
            high_cut,
            low_cut,
        }
    }

    /// Crea una reverb legacy (Schroeder) compatible con la version anterior.
    pub fn new(room_ms: f32, mix: f32, damping: f32, sample_rate: u32) -> Self {
        let room_size = ((room_ms - 50.0) / 350.0).clamp(0.0, 1.0);
        Self::from_params(
            ReverbParams {
                mode: ReverbMode::Plate,
                room_size,
                damping,
                wet: mix,
                pre_delay_ms: 0.0,
                high_cut_hz: 18000.0,
                low_cut_hz: 50.0,
            },
            sample_rate,
        )
    }
}

impl AudioProcessor for Reverb {
    fn process(
        &mut self,
        input: &[f32],
        output: &mut [f32],
        _info: &ProcessingInfo,
    ) -> ProcessResult {
        let frames = input.len().min(output.len());
        let dry = 1.0 - self.mix;

        for i in 0..frames {
            let delayed_input = self.pre_delay_line.push(input[i], self.pre_delay_samples);
            let combs = self.comb_step(delayed_input);
            let wet = self.allpass_step(combs);
            let filtered = self.low_cut.process(wet);
            let filtered = self.high_cut.process(filtered);
            output[i] = input[i] * dry + filtered * self.mix;
        }
        ProcessResult { latency_ms: 0.0 }
    }

    fn name(&self) -> &'static str {
        "reverb"
    }
}

impl Reverb {
    fn comb_step(&mut self, x: f32) -> f32 {
        let n = self.comb_lines.len();
        let mut acc = 0.0;
        for idx in 0..n {
            let delay = self.comb_delays[idx];
            let line = &mut self.comb_lines[idx];
            let len = line.buffer.len();
            let delayed = line.buffer[(line.write + len - delay) % len];
            line.buffer[line.write] = x + delayed * self.comb_feedback;
            line.write = (line.write + 1) % len;
            acc += delayed;
        }
        acc / n as f32
    }

    fn allpass_step(&mut self, x: f32) -> f32 {
        let mut cur = x;
        for idx in 0..self.allpass_delays.len() {
            let delay = self.allpass_delays[idx];
            let line = &mut self.allpass_lines[idx];
            let len = line.buffer.len();
            let delayed = line.buffer[(line.write + len - delay) % len];
            let g = self.allpass_feedback * (1.0 - self.damping);
            let out = -g * cur + delayed;
            line.buffer[line.write] = cur + g * delayed;
            line.write = (line.write + 1) % len;
            cur = out;
        }
        cur
    }
}

fn ms_to_samples(ms: f32, sample_rate: f32) -> usize {
    (ms.max(0.0) * sample_rate / 1000.0).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverb_decays_toward_silence() {
        let sr = 48_000;
        let mut reverb = Reverb::new(200.0, 1.0, 0.0, sr);
        let n = sr as usize;
        let mut input = vec![0.0; n];
        input[0] = 1.0;
        let mut out = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };
        reverb.process(&input, &mut out, &info);
        let early: f32 = out[..n / 2].iter().map(|v| v * v).sum();
        let late: f32 = out[n / 2..].iter().map(|v| v * v).sum();
        assert!(early > late, "cola no decae (early={early}, late={late})");
        assert!(late < 1e-3, "cola demasiado larga (late={late})");
    }

    #[test]
    fn zero_mix_is_dry() {
        let mut reverb = Reverb::new(200.0, 0.0, 0.0, 48_000);
        let input = [0.1, 0.2, -0.3];
        let mut out = [0.0; 3];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 3,
        };
        reverb.process(&input, &mut out, &info);
        assert_eq!(out, input);
    }

    #[test]
    fn from_params_plate_works() {
        let params = ReverbParams {
            mode: ReverbMode::Plate,
            room_size: 0.5,
            damping: 0.3,
            wet: 0.15,
            pre_delay_ms: 20.0,
            high_cut_hz: 8000.0,
            low_cut_hz: 200.0,
        };
        let mut reverb = Reverb::from_params(params, 48_000);
        let mut input = vec![0.0; 4800];
        input[0] = 1.0;
        let mut out = vec![0.0; 4800];
        let info = ProcessingInfo {
            sample_rate: 48_000,
            frames: 4800,
        };
        reverb.process(&input, &mut out, &info);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn hall_has_longer_tail_than_room() {
        let params_hall = ReverbParams {
            mode: ReverbMode::Hall,
            room_size: 0.8,
            damping: 0.2,
            wet: 1.0,
            pre_delay_ms: 0.0,
            high_cut_hz: 18000.0,
            low_cut_hz: 50.0,
        };
        let params_room = ReverbParams {
            mode: ReverbMode::Room,
            room_size: 0.8,
            damping: 0.2,
            wet: 1.0,
            pre_delay_ms: 0.0,
            high_cut_hz: 18000.0,
            low_cut_hz: 50.0,
        };
        let sr = 48_000;
        let n = sr as usize;

        let mut hall = Reverb::from_params(params_hall, sr);
        let mut room = Reverb::from_params(params_room, sr);

        let mut input = vec![0.0; n];
        input[0] = 1.0;

        let mut out_hall = vec![0.0; n];
        let mut out_room = vec![0.0; n];
        let info = ProcessingInfo {
            sample_rate: sr,
            frames: n,
        };

        hall.process(&input, &mut out_hall, &info);
        room.process(&input, &mut out_room, &info);

        let hall_late: f32 = out_hall[n * 3 / 4..].iter().map(|v| v * v).sum();
        let room_late: f32 = out_room[n * 3 / 4..].iter().map(|v| v * v).sum();
        assert!(
            hall_late > room_late,
            "hall debería tener cola más larga que room (hall={hall_late}, room={room_late})"
        );
    }
}
