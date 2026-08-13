//! Test de integración del servidor WebSocket: autenticación por token,
//! rotación del código, difusión de eventos del motor y enrutado de comandos
//! de control del móvil.

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::protocol::Message;
use voxlfa_core::protocol::{EngineEvent, LevelSample};
use voxlfa_desktop_lib::engine::EngineManager;
use voxlfa_desktop_lib::pairing::{PairingState, MAX_FAILED_ATTEMPTS};
use voxlfa_desktop_lib::ws::run_ws_server;

/// Código de emparejamiento usado en las pruebas.
const TEST_CODE: &str = "ABC234";

/// Arranca el servidor en un puerto libre y devuelve `(puerto, emisor)`.
async fn start_server() -> (u16, broadcast::Sender<String>) {
    // Reservar un puerto libre para evitar colisiones entre ejecuciones.
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let (events, _) = broadcast::channel(64);
    let sender = events.clone();
    let (pairing_events, _) = broadcast::channel(16);
    let pairing = Arc::new(Mutex::new(PairingState::with_code(TEST_CODE.to_string())));
    let engine = Arc::new(Mutex::new(EngineManager::new(events.clone())));
    tokio::spawn(run_ws_server(events, pairing, pairing_events, engine, port));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (port, sender)
}

#[tokio::test]
async fn authorized_client_receives_broadcast_events() {
    let (port, sender) = start_server().await;

    let url = format!("ws://127.0.0.1:{port}/?token={TEST_CODE}");
    let (mut ws, _response) = tokio_tungstenite::connect_async(&url).await.unwrap();

    // El escritorio difunde un evento del motor como JSON.
    let event = EngineEvent::Level(LevelSample {
        input_rms_db: -30.0,
        input_peak_db: -20.0,
        output_rms_db: -31.0,
        output_peak_db: -21.0,
        latency_ms: 5.0,
        captured_at_ms: 1,
    });
    let json = serde_json::to_string(&event).unwrap();
    sender.send(json.clone()).unwrap();

    // El móvil recibe exactamente ese JSON.
    let received = ws
        .next()
        .await
        .expect("la conexión se cerró antes de recibir");
    match received {
        Ok(Message::Text(text)) => assert_eq!(*text, json),
        other => panic!("esperaba un mensaje de texto, recibí: {other:?}"),
    }

    let _ = ws.close(None).await;
}

#[tokio::test]
async fn unauthorized_client_is_rejected() {
    let (port, _sender) = start_server().await;

    let url = format!("ws://127.0.0.1:{port}/?token=WRONG");
    let result = tokio_tungstenite::connect_async(&url).await;
    assert!(
        result.is_err(),
        "un cliente con token incorrecto no debe poder conectar"
    );
}

#[tokio::test]
async fn missing_token_is_rejected() {
    let (port, _sender) = start_server().await;

    let url = format!("ws://127.0.0.1:{port}/");
    let result = tokio_tungstenite::connect_async(&url).await;
    assert!(
        result.is_err(),
        "un cliente sin token no debe poder conectar"
    );
}

#[tokio::test]
async fn invalid_command_gets_warning_back() {
    let (port, _sender) = start_server().await;

    let url = format!("ws://127.0.0.1:{port}/?token={TEST_CODE}");
    let (mut ws, _response) = tokio_tungstenite::connect_async(&url).await.unwrap();

    // JSON malformado: el servidor responde con un evento `warning`.
    ws.send(Message::Text(r#"{"type":"fly"}"#.into()))
        .await
        .unwrap();
    let received = ws.next().await.expect("la conexión se cerró").unwrap();
    match received {
        Message::Text(text) => {
            let event: EngineEvent = serde_json::from_str(&text).unwrap();
            assert!(
                matches!(event, EngineEvent::Warning { .. }),
                "esperaba un warning, recibí: {event:?}"
            );
        }
        other => panic!("esperaba un mensaje de texto, recibí: {other:?}"),
    }

    let _ = ws.close(None).await;
}

#[tokio::test]
async fn dsp_command_rejected_when_engine_stopped() {
    let (port, _sender) = start_server().await;

    let url = format!("ws://127.0.0.1:{port}/?token={TEST_CODE}");
    let (mut ws, _response) = tokio_tungstenite::connect_async(&url).await.unwrap();

    // El motor no corre: el comando falla y llega un `warning` al móvil.
    ws.send(Message::Text(
        r#"{"type":"setGlobalBypass","bypass":true}"#.into(),
    ))
    .await
    .unwrap();
    let received = ws.next().await.expect("la conexión se cerró").unwrap();
    match received {
        Message::Text(text) => {
            let event: EngineEvent = serde_json::from_str(&text).unwrap();
            assert!(
                matches!(event, EngineEvent::Warning { .. }),
                "esperaba un warning, recibí: {event:?}"
            );
        }
        other => panic!("esperaba un mensaje de texto, recibí: {other:?}"),
    }

    let _ = ws.close(None).await;
}

#[tokio::test]
async fn oversized_command_is_rejected() {
    let (port, _sender) = start_server().await;

    let url = format!("ws://127.0.0.1:{port}/?token={TEST_CODE}");
    let (mut ws, _response) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let huge = format!(r#"{{"type":"stop","padding":"{}"}}"#, "x".repeat(2048));
    ws.send(Message::Text(huge.into())).await.unwrap();
    let received = ws.next().await.expect("la conexión se cerró").unwrap();
    match received {
        Message::Text(text) => {
            let event: EngineEvent = serde_json::from_str(&text).unwrap();
            assert!(matches!(event, EngineEvent::Warning { .. }));
        }
        other => panic!("esperaba un mensaje de texto, recibí: {other:?}"),
    }

    let _ = ws.close(None).await;
}

#[tokio::test]
async fn stop_command_keeps_connection_alive() {
    let (port, sender) = start_server().await;

    let url = format!("ws://127.0.0.1:{port}/?token={TEST_CODE}");
    let (mut ws, _response) = tokio_tungstenite::connect_async(&url).await.unwrap();

    // `stop` con el motor detenido no es un error: no debe responder nada.
    ws.send(Message::Text(r#"{"type":"stop"}"#.into()))
        .await
        .unwrap();

    // La conexión sigue viva: el cliente recibe el siguiente evento difundido.
    let event = EngineEvent::Level(LevelSample {
        input_rms_db: -30.0,
        input_peak_db: -20.0,
        output_rms_db: -31.0,
        output_peak_db: -21.0,
        latency_ms: 5.0,
        captured_at_ms: 2,
    });
    let json = serde_json::to_string(&event).unwrap();
    sender.send(json.clone()).unwrap();

    let received = ws.next().await.expect("la conexión se cerró").unwrap();
    match received {
        Message::Text(text) => assert_eq!(*text, json),
        other => panic!("esperaba un mensaje de texto, recibí: {other:?}"),
    }

    let _ = ws.close(None).await;
}

#[tokio::test]
async fn repeated_failures_rotate_the_pairing_code() {
    // El `MAX_FAILED_ATTEMPTS` en vivo se comparte con el servidor: usar un
    // umbral propio aquí cambiaría el del crate, así que usamos el real y
    // lanzamos exactamente `MAX_FAILED_ATTEMPTS` handshakes fallidos.
    let (port, _sender) = start_server().await;

    for _ in 0..MAX_FAILED_ATTEMPTS {
        let url = format!("ws://127.0.0.1:{port}/?token=WRONG");
        let result = tokio_tungstenite::connect_async(&url).await;
        assert!(result.is_err(), "cada token incorrecto debe rechazarse");
    }

    // Con el código original ya rechazado, el nuevo handshake con `TEST_CODE`
    // también debe fallar: el código vigente rotó durante el último intento.
    let url = format!("ws://127.0.0.1:{port}/?token={TEST_CODE}");
    let result = tokio_tungstenite::connect_async(&url).await;
    assert!(
        result.is_err(),
        "tras superar el máximo de fallos, el código original debe haber rotado"
    );
}
