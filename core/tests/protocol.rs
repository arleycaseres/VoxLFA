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
fn engine_event_dsp_serializes_with_type_tag() {
    use voxlfa_core::protocol::{DspLinkState, DspState, PresetId};

    let event = EngineEvent::Dsp(DspState {
        preset: PresetId::VozLimpia,
        global_bypass: false,
        links: vec![DspLinkState {
            name: "eq".into(),
            enabled: true,
            bypass: false,
        }],
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"dsp\""));
    assert!(json.contains("\"preset\":\"vozLimpia\""));
    assert!(json.contains("\"globalBypass\":false"));
    assert!(json.contains("\"links\":"));

    let decoded: EngineEvent = serde_json::from_str(&json).unwrap();
    match decoded {
        EngineEvent::Dsp(state) => {
            assert_eq!(state.preset, PresetId::VozLimpia);
            assert_eq!(state.links[0].name, "eq");
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
