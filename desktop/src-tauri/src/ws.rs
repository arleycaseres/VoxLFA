//! Servidor WebSocket de monitoreo remoto (app móvil).
//!
//! Escucha en la red local (`0.0.0.0`) y **difunde** los eventos del motor a
//! los clientes conectados. No recibe audio ni control: en la Fase 0 el móvil
//! solo visualiza.
//!
//! # Autenticación
//! El cliente debe conectar con `ws://<host>:<puerto>/?token=<código>`.
//! Sin el token correcto el handshake se rechaza con HTTP 401 y la conexión
//! se cierra. Esto impide que cualquier dispositivo en la WiFi controle o
//! espíe la sesión (ver `docs/seguridad.md`).

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio_tungstenite::tungstenite::http;
use tokio_tungstenite::tungstenite::protocol::Message;

/// Espera entre reintentos al aceptar conexiones tras un error.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

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
/// Corre para siempre; si no puede abrir el puerto, lo avisa por consola y
/// termina (la app sigue funcionando, solo se pierde el monitoreo remoto).
pub async fn run_ws_server(events: broadcast::Sender<String>, pairing_code: String, port: u16) {
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
                let code = pairing_code.clone();
                tokio::spawn(async move {
                    if let Err(err) = handle_connection(stream, events, code).await {
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

/// Atiende una conexión: autentica, valida el token y difunde eventos.
///
/// `result_large_err` se permite porque el tipo de error lo impone la API
/// pública de tungstenite (no es una decisión de diseño de este crate).
#[allow(clippy::result_large_err)]
async fn handle_connection(
    stream: TcpStream,
    events: broadcast::Sender<String>,
    pairing_code: String,
) -> Result<(), WsError> {
    // 1) Autenticación por token en la URL (`?token=<código>`).
    let mut authorized = false;
    let ws = tokio_tungstenite::accept_hdr_async(
        stream,
        |request: &http::Request<()>, response| -> Result<_, ErrorResponse> {
            authorized =
                token_from_query(request.uri().query()).as_deref() == Some(pairing_code.as_str());

            if authorized {
                // Proceder con el handshake estándar (HTTP 101).
                Ok(response)
            } else {
                // Rechazar: tungstenite escribe el 401 y cierra la conexión.
                let mut error_response = http::Response::new(None);
                *error_response.status_mut() = http::StatusCode::UNAUTHORIZED;
                Err(error_response)
            }
        },
    )
    .await?;

    // (Cuando `authorized` es falso, accept_hdr_async ya devuelve Err y
    // terminamos arriba; esta guardia es solo defensiva.)
    if !authorized {
        return Ok(());
    }

    // 2) Bucle de difusión: reenvía cada evento del motor al cliente.
    let (mut sink, mut incoming) = ws.split();
    let mut rx = events.subscribe();

    loop {
        tokio::select! {
            incoming_message = incoming.next() => {
                match incoming_message {
                    Some(Ok(Message::Close(_))) | None => break,
                    // Fase 0: el móvil es de solo lectura; se ignoran el resto.
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
}
