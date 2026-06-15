use futures_util::{SinkExt, StreamExt};
use native_tls::TlsConnector;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::protocol::Message,
    Connector,
};
use prost::Message as ProstMessage;
use zenth_dto::{WsNotification, NotificationType, WsFileChunkRequest, WsFileChunkResponse};

/// Global WebSocket connection state
static WS_CONNECTION: once_cell::sync::Lazy<Arc<Mutex<Option<WsState>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(None)));

type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    Message,
>;

struct WsState {
    sink: WsSink,
}

/// WebSocket message event payload
#[derive(Clone, serde::Serialize)]
struct WsMessageEvent {
    #[serde(rename = "type")]
    msg_type: String,
    data: Option<Vec<u8>>,
}

/// Connect to WebSocket with optional TLS bypass
#[tauri::command]
pub async fn ws_connect(app: AppHandle, url: String) -> Result<String, String> {

    {
        let conn_guard = WS_CONNECTION.lock().await;
        if conn_guard.is_some() {
            return Ok("already_connected".to_string());
        }
    }

    let accept_invalid_certs = crate::config::accept_invalid_certs();


    // Create TLS connector with optional bypass
    let tls_connector = if accept_invalid_certs {
        TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|e| format!("Failed to create TLS connector: {}", e))?
    } else {
        TlsConnector::new()
            .map_err(|e| format!("Failed to create TLS connector: {}", e))?
    };

    let connector = Connector::NativeTls(tls_connector);

    // Parse URL
    let url_parsed = url::Url::parse(&url)
        .map_err(|e| format!("Invalid URL: {}", e))?;

    // Connect
    let (ws_stream, _response) = connect_async_tls_with_config(
        url_parsed,
        None,
        false,
        Some(connector),
    )
    .await
    .map_err(|e| format!("WebSocket connection failed: {}", e))?;

    let (sink, mut stream) = ws_stream.split();

    // Store sink for sending
    {
        let mut conn_guard = WS_CONNECTION.lock().await;
        *conn_guard = Some(WsState { sink });
    }

    // Spawn task to handle incoming messages
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(Message::Binary(data)) => {
                    if !try_dispatch_p2p(&data, &app_clone).await {
                        let _ = app_clone.emit("ws-message", WsMessageEvent {
                            msg_type: "Binary".to_string(),
                            data: Some(data),
                        });
                    }
                }
                Ok(Message::Text(text)) => {
                    let _ = app_clone.emit("ws-message", WsMessageEvent {
                        msg_type: "Text".to_string(),
                        data: Some(text.into_bytes()),
                    });
                }
                Ok(Message::Close(_)) => {
                    let _ = app_clone.emit("ws-message", WsMessageEvent {
                        msg_type: "Close".to_string(),
                        data: None,
                    });
                    break;
                }
                Ok(Message::Ping(_)) => {}
                Ok(Message::Pong(_)) => {}
                Ok(Message::Frame(_)) => {}
                Err(e) => {
                    let _ = app_clone.emit("ws-message", WsMessageEvent {
                        msg_type: "Error".to_string(),
                        data: Some(e.to_string().into_bytes()),
                    });
                    break;
                }
            }
        }

        let mut conn_guard = WS_CONNECTION.lock().await;
        *conn_guard = None;
    });

    Ok("connected".to_string())
}

/// Send binary data over WebSocket
#[tauri::command]
pub async fn ws_send(data: Vec<u8>) -> Result<(), String> {
    let mut conn_guard = WS_CONNECTION.lock().await;

    let conn = conn_guard.as_mut()
        .ok_or("WebSocket not connected")?;

    let len = data.len();
    conn.sink
        .send(Message::Binary(data))
        .await
        .map_err(|e| format!("Failed to send: {}", e))?;

    Ok(())
}

/// Disconnect WebSocket
#[tauri::command]
pub async fn ws_disconnect() -> Result<(), String> {
    let mut conn_guard = WS_CONNECTION.lock().await;

    if let Some(mut conn) = conn_guard.take() {
        let _ = conn.sink.close().await;
    }

    Ok(())
}

/// Check if WebSocket is connected
#[tauri::command]
pub async fn ws_is_connected() -> bool {
    let conn_guard = WS_CONNECTION.lock().await;
    conn_guard.is_some()
}

/// Intercepte les notifications P2P (FILE_CHUNK_REQUEST / FILE_CHUNK) et les traite en Rust.
/// Retourne `true` si le message a été consommé (ne doit pas être émis vers JS).
async fn try_dispatch_p2p(data: &[u8], app: &AppHandle) -> bool {
    let notif = match WsNotification::decode(data) {
        Ok(n) => n,
        Err(_) => return false,
    };

    let notif_type = NotificationType::try_from(notif.notification_type)
        .unwrap_or(NotificationType::NotificationUnknown);

    match notif_type {
        NotificationType::FileChunkRequest => {
            if let Ok(req) = WsFileChunkRequest::decode(notif.payload.as_slice()) {
                crate::pages::chat::file_transfer::handle_chunk_request(req, app).await;
            }
            true
        }
        NotificationType::FileChunk => {
            if let Ok(resp) = WsFileChunkResponse::decode(notif.payload.as_slice()) {
                crate::pages::chat::file_transfer::handle_incoming_chunk(resp, app).await;
            }
            true
        }
        _ => false,
    }
}
