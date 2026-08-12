//! Tests de integración de la cadena DSP: presets, bypass y el mango de
//! control (`DspHandle`) tal como lo usa el motor en tiempo real.

use std::sync::mpsc;

use voxlfa_core::dsp::{AudioProcessor, ChainProcessor, DspCommand, DspHandle, ProcessingInfo};
use voxlfa_core::protocol::{DspState, EngineEvent, PresetId};

fn info(frames: usize) -> ProcessingInfo {
    ProcessingInfo {
        sample_rate: 48_000,
        frames,
    }
}

/// Procesa un DC de amplitud `level` y devuelve la salida de la última muestra.
fn dc_gain(chain: &mut ChainProcessor, level: f32, frames: usize) -> f32 {
    let input = vec![level; frames];
    let mut out = vec![0.0; frames];
    chain.process(&input, &mut out, &info(frames));
    out[frames - 1]
}

#[test]
fn dry_vs_voce_limpia_differ() {
    let mut dry = ChainProcessor::new(PresetId::Dry, 48_000, 256);
    let mut voz = ChainProcessor::new(PresetId::VozLimpia, 48_000, 256);
    let dry_out = dc_gain(&mut dry, 0.2, 8192);
    let voz_out = dc_gain(&mut voz, 0.2, 8192);
    // El preset aplicado cambia la señal (compresión + makeup + limiter).
    assert!(
        (voz_out - dry_out).abs() > 1e-3,
        "dry={dry_out} voz={voz_out}"
    );
}

#[test]
fn switching_preset_switches_chain() {
    let mut chain = ChainProcessor::new(PresetId::Dry, 48_000, 256);
    assert!(chain.state().links.is_empty());
    chain.apply_preset(PresetId::Warm);
    assert!(!chain.state().links.is_empty());
    assert_eq!(chain.state().preset, PresetId::Warm);
}

#[test]
fn global_bypass_flattens_any_preset() {
    let mut chain = ChainProcessor::new(PresetId::Radio, 48_000, 256);
    chain.set_global_bypass(true);
    assert_eq!(dc_gain(&mut chain, 0.25, 8192), 0.25);
}

#[test]
fn dsp_handle_round_trips_state_and_events() {
    let (cmd_tx, cmd_rx) = mpsc::channel::<DspCommand>();
    let (event_tx, event_rx) = mpsc::channel::<EngineEvent>();
    let handle = DspHandle::new(cmd_tx, event_tx, PresetId::Dry, 48_000, 256);

    // Estado inicial = preset Dry, sin módulos.
    let state = handle.get_state().unwrap();
    assert_eq!(state.preset, PresetId::Dry);
    assert!(state.links.is_empty());

    // Aplicar preset: el comando llega al "hilo de audio" y se emite un evento.
    handle.apply_preset(PresetId::VozLimpia).unwrap();
    let command = cmd_rx.recv().unwrap();
    match command {
        DspCommand::ApplyPreset(chain) => {
            assert_eq!(chain.state().preset, PresetId::VozLimpia);
        }
        _ => panic!("esperaba ApplyPreset, obtuve un comando distinto"),
    }
    match event_rx.recv().unwrap() {
        EngineEvent::Dsp(state) => assert_eq!(state.preset, PresetId::VozLimpia),
        other => panic!("esperaba Dsp, obtuve {other:?}"),
    }

    // El bypass de un módulo desconocido se rechaza con error.
    assert!(handle.set_link_bypass("nope", true).is_err());

    // El espejo del handle se actualizó (get_state ya ve el preset nuevo).
    let state = handle.get_state().unwrap();
    assert_eq!(state.preset, PresetId::VozLimpia);
}

#[test]
fn dsp_state_serializes_as_expected_by_ui_and_mobile() {
    let state: DspState = DspHandle::new(
        mpsc::channel().0,
        mpsc::channel().0,
        PresetId::Warm,
        48_000,
        256,
    )
    .get_state()
    .unwrap();
    let json = serde_json::to_string(&state).unwrap();
    assert!(json.contains("\"preset\":\"warm\""));
    assert!(json.contains("\"globalBypass\":false"));
    assert!(json.contains("\"links\":"));
    assert!(json.contains("\"name\":\"eq\""));
    assert!(json.contains("\"bypass\":false"));
}
