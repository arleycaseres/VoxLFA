//! Tests de integración del protocolo: el contrato JSON que se usa entre el
//! core, la UI de escritorio y la app móvil.

use voxlfa_core::protocol::{
    AudioDeviceInfo, ControlCommand, EngineEvent, EngineState, EngineStatus, LevelSample,
};
use voxlfa_core::Result;

#[test]
fn engine_event_level_serializes_with_camel_case_and_tag() {
    let event = EngineEvent::Level(LevelSample {
        input_rms_db: -24.5,
        input_peak_db: -12.0,
        output_rms_db: -26.0,
        output_peak_db: -13.0,
        latency_ms: 12.3,
        captured_at_ms: 123456,
    });

    let json = serde_json::to_string(&event).unwrap();
    // El discriminante del enum debe ser `type` y los campos en camelCase.
    assert!(json.contains("\"type\":\"level\""));
    assert!(json.contains("\"inputRmsDb\":"));
    assert!(json.contains("\"inputPeakDb\":"));
    assert!(json.contains("\"outputRmsDb\":"));
    assert!(json.contains("\"outputPeakDb\":"));
    assert!(json.contains("\"latencyMs\":"));
    assert!(json.contains("\"capturedAtMs\":"));
}

#[test]
fn engine_event_status_round_trips() {
    let event = EngineEvent::Status(EngineStatus {
        state: EngineState::Running,
        sample_rate: 48_000,
        buffer_size: 256,
        latency_ms: 10.0,
        audio_host: Some("alsa".into()),
        input_device: Some("Mic".into()),
        output_device: None,
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"status\""));
    assert!(json.contains("\"sampleRate\":48000"));

    let decoded: EngineEvent = serde_json::from_str(&json).unwrap();
    match decoded {
        EngineEvent::Status(status) => {
            assert_eq!(status.state, EngineState::Running);
            assert_eq!(status.sample_rate, 48_000);
        }
        other => panic!("esperaba Status, obtuve {other:?}"),
    }
}

#[test]
fn engine_event_devices_round_trips() {
    let event = EngineEvent::Devices {
        inputs: vec![AudioDeviceInfo {
            name: "Built-in Microphone".into(),
            is_default: true,
        }],
        outputs: vec![AudioDeviceInfo {
            name: "Speakers".into(),
            is_default: true,
        }],
    };

    let json = serde_json::to_string(&event).unwrap();
    let decoded: EngineEvent = serde_json::from_str(&json).unwrap();
    match decoded {
        EngineEvent::Devices { inputs, outputs } => {
            assert_eq!(inputs.len(), 1);
            assert_eq!(outputs[0].name, "Speakers");
        }
        other => panic!("esperaba Devices, obtuve {other:?}"),
    }
}

#[test]
fn control_command_start_uses_camel_case() {
    let command = ControlCommand::Start {
        input_device: Some("Mic A".into()),
        output_device: None,
    };

    let json = serde_json::to_string(&command).unwrap();
    assert!(json.contains("\"type\":\"start\""));
    assert!(json.contains("\"inputDevice\":\"Mic A\""));
    assert!(json.contains("\"outputDevice\":null"));

    let decoded: ControlCommand = serde_json::from_str(&json).unwrap();
    match decoded {
        ControlCommand::Start {
            input_device,
            output_device,
        } => {
            assert_eq!(input_device.as_deref(), Some("Mic A"));
            assert!(output_device.is_none());
        }
        other => panic!("esperaba Start, obtuve {other:?}"),
    }
}

#[test]
fn control_command_remote_variants_use_camel_case_and_round_trip() {
    use voxlfa_core::protocol::PresetId;

    let commands = vec![
        ControlCommand::Stop,
        ControlCommand::SetPreset {
            preset: PresetId::Warm,
        },
        ControlCommand::SetGlobalBypass { bypass: true },
        ControlCommand::SetLinkBypass {
            link: "eq".into(),
            bypass: true,
        },
        ControlCommand::SetEqBand {
            band_index: 2,
            gain_db: -4.5,
        },
    ];

    for command in commands {
        let json = serde_json::to_string(&command).unwrap();
        let decoded: ControlCommand = serde_json::from_str(&json).unwrap();
        match (command, decoded) {
            (ControlCommand::Stop, ControlCommand::Stop) => {
                assert_eq!(json, r#"{"type":"stop"}"#);
            }
            (ControlCommand::SetPreset { preset }, ControlCommand::SetPreset { preset: p }) => {
                assert_eq!(preset, p);
                assert!(json.contains("\"type\":\"setPreset\""));
                assert!(json.contains("\"preset\":\"warm\""));
            }
            (
                ControlCommand::SetGlobalBypass { bypass },
                ControlCommand::SetGlobalBypass { bypass: b },
            ) => {
                assert_eq!(bypass, b);
                assert!(json.contains("\"type\":\"setGlobalBypass\""));
                assert!(json.contains("\"bypass\":true"));
            }
            (
                ControlCommand::SetLinkBypass { link, bypass },
                ControlCommand::SetLinkBypass { link: l, bypass: b },
            ) => {
                assert_eq!(link, l);
                assert_eq!(bypass, b);
                assert!(json.contains("\"type\":\"setLinkBypass\""));
                assert!(json.contains("\"link\":\"eq\""));
            }
            (
                ControlCommand::SetEqBand {
                    band_index,
                    gain_db,
                },
                ControlCommand::SetEqBand {
                    band_index: i,
                    gain_db: g,
                },
            ) => {
                assert_eq!(band_index, i);
                assert_eq!(gain_db, g);
                assert!(json.contains("\"type\":\"setEqBand\""));
                assert!(json.contains("\"bandIndex\":2"));
                assert!(json.contains("\"gainDb\":-4.5"));
            }
            other => panic!("el comando no preservó su variante: {other:?}"),
        }
    }
}

#[test]
fn unknown_control_command_type_is_rejected() {
    let invalid = r#"{"type":"fly"}"#;
    let result: Result<ControlCommand> = serde_json::from_str(invalid).map_err(Into::into);
    assert!(result.is_err());
}

#[test]
fn control_command_new_dsp_variants_round_trip() {
    use voxlfa_core::protocol::{
        DenoiseParams, FeedbackSuppressorParams, MusicalNote, MusicalScale, NoiseGateParams,
        PitchCorrectionParams,
    };

    let commands = vec![
        ControlCommand::SetNoiseGate {
            params: NoiseGateParams {
                threshold_db: -40.0,
                attack_ms: 1.0,
                release_ms: 100.0,
                hold_ms: 200.0,
                range_db: 30.0,
            },
        },
        ControlCommand::SetDenoise {
            params: DenoiseParams { mix: 0.75 },
        },
        ControlCommand::SetFeedback {
            params: FeedbackSuppressorParams {
                threshold_db: -30.0,
                q: 10.0,
            },
        },
        ControlCommand::SetPitchCorrection {
            params: PitchCorrectionParams {
                scale: MusicalScale::Major,
                root: MusicalNote::A,
                strength: 0.5,
                mix: 0.8,
            },
        },
    ];

    for command in commands {
        let json = serde_json::to_string(&command).unwrap();
        let decoded: ControlCommand = serde_json::from_str(&json).unwrap();
        match (&command, &decoded) {
            (ControlCommand::SetNoiseGate { .. }, ControlCommand::SetNoiseGate { .. }) => {
                assert!(json.contains("\"type\":\"setNoiseGate\""));
            }
            (ControlCommand::SetDenoise { .. }, ControlCommand::SetDenoise { .. }) => {
                assert!(json.contains("\"type\":\"setDenoise\""));
            }
            (ControlCommand::SetFeedback { .. }, ControlCommand::SetFeedback { .. }) => {
                assert!(json.contains("\"type\":\"setFeedback\""));
            }
            (
                ControlCommand::SetPitchCorrection { .. },
                ControlCommand::SetPitchCorrection { .. },
            ) => {
                assert!(json.contains("\"type\":\"setPitchCorrection\""));
            }
            other => panic!("el comando no preservó su variante: {other:?}"),
        }
    }
}

#[test]
fn engine_event_dsp_serializes_with_type_tag() {
    use voxlfa_core::protocol::{DspLinkState, DspState, EqBand, EqBandKind, PresetId};

    let event = EngineEvent::Dsp(DspState {
        preset: PresetId::VozLimpia,
        global_bypass: false,
        links: vec![DspLinkState {
            name: "eq".into(),
            enabled: true,
            bypass: false,
            eq_bands: Some(vec![EqBand {
                kind: EqBandKind::Peaking,
                freq_hz: 3000.0,
                gain_db: 2.0,
                q: 1.5,
            }]),
            gate_params: None,
            denoise_params: None,
            feedback_params: None,
            pitch_correction_params: None,
            delay_params: None,
            reverb_params: None,
        }],
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"dsp\""));
    assert!(json.contains("\"preset\":\"vozLimpia\""));
    assert!(json.contains("\"globalBypass\":false"));
    assert!(json.contains("\"links\":"));
    // Las bandas del EQ viajan en camelCase dentro del estado del eslabón.
    assert!(json.contains("\"eqBands\":"));
    assert!(json.contains("\"freqHz\":3000.0"));
    assert!(json.contains("\"gainDb\":2.0"));

    let decoded: EngineEvent = serde_json::from_str(&json).unwrap();
    match decoded {
        EngineEvent::Dsp(state) => {
            assert_eq!(state.preset, PresetId::VozLimpia);
            assert_eq!(state.links[0].name, "eq");
            assert_eq!(state.links[0].eq_bands.as_ref().unwrap().len(), 1);
        }
        other => panic!("esperaba Dsp, obtuve {other:?}"),
    }
}

#[test]
fn unknown_event_type_is_rejected() {
    let invalid = r#"{"type":"nope","rmsDb":-1.0}"#;
    let result: Result<EngineEvent> = serde_json::from_str(invalid).map_err(Into::into);
    assert!(result.is_err());
}
