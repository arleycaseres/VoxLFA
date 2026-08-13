//! Servidor WebSocket de monitoreo y control remoto (app móvil).
//!
//! Escucha en la red local (`0.0.0.0`), **difunde** los eventos del motor a
//! los clientes conectados y **recibe** comandos de control ([`ControlCommand`]).
//!
//! Los comandos se ejecutan contra el [`EngineManager`] compartido: cada cambio
//! genera a su vez un evento (`dsp`, `status`…) que se difunde al propio móvil,
//! de modo que la UI refleja el resultado sin respuestas dedicadas. Si un
//! comando falla (motor detenido, JSON inválido, …) se responde con un evento
//! `warning` dirigido solo al cliente que lo envió.
//!
//! # Autenticación
//! El cliente debe conectar con `ws://<host>:<puerto>/?token=<código>`.
//! Sin el token correcto el handshake se rechaza con HTTP 401 y la conexión
//! se cierra. Como el token también autoriza el control del motor, el código de
//! emparejamiento equivale a "mando remoto": no debe compartirse (ver
//! `docs/seguridad.md`).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio_tungstenite::tungstenite::http;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::WebSocketStream;
use voxlfa_core::protocol::{ControlCommand, EngineEvent};

use crate::engine::EngineManager;
use crate::pairing::{AuthResult, PairingState};

/// Espera entre reintentos al aceptar conexiones tras un error.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

/// Tamaño máximo de un comando entrante por el WebSocket (bytes).
///
/// Los comandos son mensajes pequeños; acotar la entrada evita abusos por red
/// (ver `docs/seguridad.md`).
const MAX_CONTROL_BYTES: usize = 1024;

/// Ganancia mínima por banda del EQ aceptada de la red (dB).
const EQ_GAIN_MIN: f32 = -18.0;
/// Ganancia máxima por banda del EQ aceptada de la red (dB).
const EQ_GAIN_MAX: f32 = 18.0;

/// Errores del servidor WebSocket.
#[derive(Debug, thiserror::Error)]
pub enum WsError {
    /// El handshake WebSocket falló (incluye rechazo por token inválido).
    #[error("websocket handshake fallido: {0}")]
    Handshake(#[from] tokio_tungstenite::tungstenite::Error),
    /// Error de red durante la conexión.
    #[error("websocket I/O: {0}")]
    Io(String),
}

/// Arranca el servidor WebSocket en el puerto indicado (tarea asíncrona).
///
/// `pairing` es el estado compartido del código de emparejamiento: cada
/// handshake lo consulta y, si se superan `MAX_FAILED_ATTEMPTS` intentos
/// fallidos consecutivos, rota el código y publica el nuevo en
/// `pairing_events` para que la cabina lo muestre.
///
/// Corre para siempre; si no puede abrir el puerto, lo avisa por consola y
/// termina (la app sigue funcionando, solo se pierde el control remoto).
pub async fn run_ws_server(
    events: broadcast::Sender<String>,
    pairing: Arc<Mutex<PairingState>>,
    pairing_events: broadcast::Sender<String>,
    engine: Arc<Mutex<EngineManager>>,
    port: u16,
) {
    let address = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("[voxlfa] no se pudo abrir el WebSocket en el puerto {port}: {err}");
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let events = events.clone();
                let pairing = pairing.clone();
                let pairing_events = pairing_events.clone();
                let engine = engine.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        handle_connection(stream, events, pairing, pairing_events, engine).await
                    {
                        eprintln!("[voxlfa] ws {peer}: {err}");
                    }
                });
            }
            Err(err) => {
                eprintln!("[voxlfa] error aceptando conexión WebSocket: {err}");
                tokio::time::sleep(ACCEPT_RETRY).await;
            }
        }
    }
}

/// Tipo de respuesta de rechazo del handshake (dictado por tungstenite).
type ErrorResponse = http::Response<Option<String>>;

/// Atiende una conexión: autentica, difunde eventos y enruta los comandos de
/// control hacia el gestor del motor.
///
/// `result_large_err` se permite porque el tipo de error lo impone la API
/// pública de tungstenite (no es una decisión de diseño de este crate).
#[allow(clippy::result_large_err)]
async fn handle_connection(
    stream: TcpStream,
    events: broadcast::Sender<String>,
    pairing: Arc<Mutex<PairingState>>,
    pairing_events: broadcast::Sender<String>,
    engine: Arc<Mutex<EngineManager>>,
) -> Result<(), WsError> {
    // 1) Autenticación por token en la URL (`?token=<código>`). Los fallos
    //    consecutivos rotan el código (ver `PairingState`).
    let mut rotated_code: Option<String> = None;
    let accept_result = tokio_tungstenite::accept_hdr_async(
        stream,
        |request: &http::Request<()>, response| -> Result<_, ErrorResponse> {
            let token = token_from_query(request.uri().query());
            let mut guard = lock_pairing(&pairing);
            match guard.authenticate(token.as_deref()) {
                AuthResult::Accepted => {
                    // Proceder con el handshake estándar (HTTP 101).
                    Ok(response)
                }
                AuthResult::Rejected => {
                    // Rechazar: tungstenite escribe el 401 y cierra la conexión.
                    Err(unauthorized_response())
                }
                AuthResult::RejectedAndRotated => {
                    rotated_code = Some(guard.code().to_string());
                    Err(unauthorized_response())
                }
            }
        },
    )
    .await;

    // Una rotación solo ocurre con un handshake rechazado: publicarla para que
    // la cabina actualice el código mostrado al usuario.
    let ws = match accept_result {
        Ok(ws) => ws,
        Err(err) => {
            if let Some(code) = rotated_code.take() {
                let _ = pairing_events.send(code);
            }
            return Err(WsError::Handshake(err));
        }
    };

    // 2) Bucle de difusión + recepción de comandos de control.
    let (mut sink, mut incoming) = ws.split();
    let mut rx = events.subscribe();

    loop {
        tokio::select! {
            incoming_message = incoming.next() => {
                match incoming_message {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Text(text))) => {
                        handle_command(&mut sink, &engine, &text).await?;
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        // Responder al ping para mantener la conexión viva.
                        if sink.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    // El resto de frames (binario, pong…) no se usa.
                    Some(Ok(_)) => {}
                    Some(Err(err)) => return Err(WsError::Io(err.to_string())),
                }
            }
            event = rx.recv() => match event {
                Ok(json) => {
                    if sink.send(Message::Text(json.into())).await.is_err() {
                        // El cliente se desconectó.
                        break;
                    }
                }
                Err(RecvError::Lagged(_)) => {
                    // Cliente más lento que el motor: se omiten eventos
                    // antiguos (mejor que acumular latencia).
                    continue;
                }
                Err(RecvError::Closed) => break,
            },
        }
    }

    Ok(())
}

/// Valida y ejecuta un comando de control llegado por el WebSocket.
///
/// Un comando bien formado se ejecuta contra el motor compartido; el resultado
/// se refleja en los eventos difundidos (`dsp`, `status`…). Si el comando no se
/// puede ejecutar se responde con un `warning` dirigido solo a este cliente.
#[allow(clippy::result_large_err)]
async fn handle_command(
    sink: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    engine: &Arc<Mutex<EngineManager>>,
    text: &str,
) -> Result<(), WsError> {
    if text.len() > MAX_CONTROL_BYTES {
        return send_warning(sink, "comando remoto demasiado largo").await;
    }

    let command: ControlCommand = match serde_json::from_str(text) {
        Ok(command) => command,
        Err(_) => return send_warning(sink, "comando remoto inválido").await,
    };

    let result = {
        let mut guard = match engine.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        execute_command(&mut guard, command)
    };

    match result {
        Ok(()) => Ok(()),
        Err(message) => send_warning(sink, &message).await,
    }
}

/// Ejecuta un comando contra el gestor del motor (sin mantener el mutex al
/// enviar por el socket).
fn execute_command(engine: &mut EngineManager, command: ControlCommand) -> Result<(), String> {
    match command {
        // Arrancar el motor exige el callback de eventos de la ventana (Tauri);
        // no se permite desde el móvil para no desincronizar la cabina.
        ControlCommand::Start { .. } => {
            Err("arrancar el motor solo se permite desde la cabina".into())
        }
        ControlCommand::Stop => {
            engine.stop();
            Ok(())
        }
        ControlCommand::SetPreset { preset } => {
            engine.apply_preset(preset).map_err(|err| err.to_string())
        }
        ControlCommand::SetGlobalBypass { bypass } => engine
            .set_global_bypass(bypass)
            .map_err(|err| err.to_string()),
        ControlCommand::SetLinkBypass { link, bypass } => engine
            .set_link_bypass(link, bypass)
            .map_err(|err| err.to_string()),
        ControlCommand::SetEqBand {
            band_index,
            gain_db,
        } => {
            let gain_db = clamp_gain_db(gain_db);
            engine
                .set_eq_band(band_index, gain_db)
                .map_err(|err| err.to_string())
        }
    }
}

/// Acota la ganancia del EQ a la ventana soportada por la cabina (dB).
///
/// Un valor no numérico (p. ej. `NaN`, imposible por JSON pero defensivo) se
/// trata como 0 dB.
fn clamp_gain_db(gain_db: f32) -> f32 {
    if gain_db.is_nan() {
        return 0.0;
    }
    gain_db.clamp(EQ_GAIN_MIN, EQ_GAIN_MAX)
}

/// Responde al cliente con un evento `warning` (fallo de un comando remoto).
async fn send_warning(
    sink: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    message: &str,
) -> Result<(), WsError> {
    let event = EngineEvent::Warning {
        message: message.to_string(),
    };
    let json = serde_json::to_string(&event).map_err(|err| WsError::Io(err.to_string()))?;
    sink.send(Message::Text(json.into()))
        .await
        .map_err(|err| WsError::Io(err.to_string()))
}

/// Bloquea el estado de emparejamiento recuperando el guard de un mutex
/// envenenado (un panic previo no bloquea la autenticación para siempre).
fn lock_pairing(pairing: &Arc<Mutex<PairingState>>) -> std::sync::MutexGuard<'_, PairingState> {
    match pairing.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Respuesta HTTP 401 para un handshake rechazado (dictado por tungstenite).
fn unauthorized_response() -> ErrorResponse {
    let mut response = http::Response::new(None);
    *response.status_mut() = http::StatusCode::UNAUTHORIZED;
    response
}

/// Extrae el token de emparejamiento del query de una URL WebSocket.
///
/// Acepta `?token=XXXX` o `?foo=bar&token=XXXX` (los valores se decodifican
/// como form-urlencoded). Devuelve `None` si no hay token.
fn token_from_query(query: Option<&str>) -> Option<String> {
    url::form_urlencoded::parse(query?.as_bytes())
        .find(|(key, _)| key == "token")
        .map(|(_, value)| value.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_token_from_plain_query() {
        assert_eq!(
            token_from_query(Some("token=ABC234")),
            Some("ABC234".to_string())
        );
    }

    #[test]
    fn extracts_token_among_other_params() {
        assert_eq!(
            token_from_query(Some("foo=bar&token=ABC234&x=1")),
            Some("ABC234".to_string())
        );
    }

    #[test]
    fn missing_token_is_none() {
        assert_eq!(token_from_query(Some("foo=bar")), None);
        assert_eq!(token_from_query(None), None);
    }

    #[test]
    fn gain_from_network_is_clamped_to_cabin_window() {
        assert_eq!(clamp_gain_db(0.0), 0.0);
        assert_eq!(clamp_gain_db(-6.5), -6.5);
        assert_eq!(clamp_gain_db(24.0), EQ_GAIN_MAX);
        assert_eq!(clamp_gain_db(-40.0), EQ_GAIN_MIN);
        assert_eq!(clamp_gain_db(f32::NAN), 0.0);
    }
}
