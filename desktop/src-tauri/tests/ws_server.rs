//! Test de integración del servidor WebSocket: autenticación por token y
//! difusión de eventos del motor hacia la app móvil.

use futures_util::StreamExt;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::protocol::Message;
use voxlfa_core::protocol::{EngineEvent, LevelSample};
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
    tokio::spawn(run_ws_server(events, TEST_CODE.to_string(), port));
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
