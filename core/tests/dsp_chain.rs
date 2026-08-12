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

#[test]
fn eq_link_exposes_bands_from_the_preset() {
    let chain = ChainProcessor::new(PresetId::VozLimpia, 48_000, 256);
    let state = chain.state();
    let eq = state
        .links
        .iter()
        .find(|link| link.name == "eq")
        .expect("preset vozLimpia incluye eq");
    let bands = eq.eq_bands.as_ref().expect("el eslabón eq lleva bandas");
    assert_eq!(bands.len(), 3);

    // El preset Dry no tiene EQ: sus eslabones llevan `eq_bands` a `None`.
    let dry = ChainProcessor::new(PresetId::Dry, 48_000, 256);
    assert!(dry.state().links.is_empty());
}

#[test]
fn set_eq_band_rebuilds_the_eq_and_emits_new_state() {
    let (cmd_tx, cmd_rx) = mpsc::channel::<DspCommand>();
    let (event_tx, event_rx) = mpsc::channel::<EngineEvent>();
    let handle = DspHandle::new(cmd_tx, event_tx, PresetId::Warm, 48_000, 256);

    handle.set_eq_band(0, 6.0).unwrap();

    // El hilo de audio recibe el EQ nuevo ya construido con la banda ajustada.
    match cmd_rx.recv().unwrap() {
        DspCommand::SetLinkProcessor {
            name,
            processor,
            eq_bands,
        } => {
            assert_eq!(name, "eq");
            let bands = eq_bands.expect("lleva bandas");
            assert_eq!(bands[0].gain_db, 6.0);
            assert_eq!(processor.name(), "eq");
        }
        _ => panic!("esperaba SetLinkProcessor"),
    }

    // Se emite un evento `dsp` con el estado actualizado.
    match event_rx.recv().unwrap() {
        EngineEvent::Dsp(state) => {
            let eq = state
                .links
                .iter()
                .find(|link| link.name == "eq")
                .expect("hay eslabón eq");
            assert_eq!(
                eq.eq_bands.as_ref().unwrap()[0].gain_db,
                6.0,
                "el espejo refleja la banda ajustada"
            );
        }
        other => panic!("esperaba Dsp, obtuve {other:?}"),
    }
}

#[test]
fn set_eq_band_rejects_missing_eq_or_bad_index() {
    let (cmd_tx, cmd_rx) = mpsc::channel::<DspCommand>();
    let (event_tx, _event_rx) = mpsc::channel::<EngineEvent>();

    // Preset Dry: no hay módulo EQ.
    let dry = DspHandle::new(cmd_tx, event_tx, PresetId::Dry, 48_000, 256);
    assert!(dry.set_eq_band(0, 2.0).is_err());

    // Índice fuera de rango en un preset con EQ.
    let (cmd_tx2, _cmd_rx2) = mpsc::channel::<DspCommand>();
    let (event_tx2, _event_rx2) = mpsc::channel::<EngineEvent>();
    let warm = DspHandle::new(cmd_tx2, event_tx2, PresetId::Warm, 48_000, 256);
    assert!(warm.set_eq_band(99, 2.0).is_err());

    drop(cmd_rx);
    let _ = dry;
    let _ = warm;
}

#[test]
fn set_eq_bands_replaces_all_bands_at_once() {
    let (cmd_tx, cmd_rx) = mpsc::channel::<DspCommand>();
    let (event_tx, event_rx) = mpsc::channel::<EngineEvent>();
    let handle = DspHandle::new(cmd_tx, event_tx, PresetId::VozLimpia, 48_000, 256);

    let mut bands = handle
        .get_state()
        .unwrap()
        .links
        .iter()
        .find(|link| link.name == "eq")
        .and_then(|link| link.eq_bands.clone())
        .expect("preset vozLimpia incluye eq");
    bands[1].gain_db = 7.5;
    handle.set_eq_bands(bands).unwrap();

    match cmd_rx.recv().unwrap() {
        DspCommand::SetLinkProcessor {
            name,
            processor,
            eq_bands,
        } => {
            assert_eq!(name, "eq");
            let received = eq_bands.expect("lleva bandas");
            assert_eq!(received.len(), 3);
            assert_eq!(received[1].gain_db, 7.5);
            assert_eq!(processor.name(), "eq");
        }
        _ => panic!("esperaba SetLinkProcessor"),
    }

    match event_rx.recv().unwrap() {
        EngineEvent::Dsp(state) => {
            let eq = state.links.iter().find(|link| link.name == "eq").unwrap();
            assert_eq!(eq.eq_bands.as_ref().unwrap()[1].gain_db, 7.5);
        }
        other => panic!("esperaba Dsp, obtuve {other:?}"),
    }
}

#[test]
fn set_eq_bands_rejects_preset_without_eq() {
    let (cmd_tx, _cmd_rx) = mpsc::channel::<DspCommand>();
    let (event_tx, _event_rx) = mpsc::channel::<EngineEvent>();
    let dry = DspHandle::new(cmd_tx, event_tx, PresetId::Dry, 48_000, 256);
    assert!(dry.set_eq_bands(vec![]).is_err());
}
