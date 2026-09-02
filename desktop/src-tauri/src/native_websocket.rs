use std::{collections::HashMap, sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use nostr::JsonUtil;
use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, plugin::TauriPlugin, Manager, Runtime};
use tokio::sync::{mpsc, oneshot, Mutex, OwnedSemaphorePermit, Semaphore};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::protocol::{frame::coding::CloseCode, CloseFrame, Message, WebSocketConfig},
};
use tokio_util::sync::CancellationToken;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_LOCAL_OWNER_CONNECTIONS: usize = 4;
const MAX_LOCAL_OWNER_TEXT_FRAME_BYTES: usize = 1024 * 1024;
const MAX_WEBSOCKET_CONTROL_PAYLOAD_BYTES: usize = 125;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(250);
const SEND_QUEUE_CAPACITY: usize = 64;

pub(crate) fn install_crypto_provider() {
    // Dependencies enable both rustls providers; choose one before TLS setup.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

type Id = u32;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "data")]
pub(crate) enum WebSocketMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<CloseFramePayload>),
}

#[derive(Debug, Deserialize)]
pub(crate) struct CloseFramePayload {
    code: u16,
    reason: String,
}

impl From<WebSocketMessage> for Message {
    fn from(message: WebSocketMessage) -> Self {
        match message {
            WebSocketMessage::Text(value) => Message::Text(value.into()),
            WebSocketMessage::Binary(value) => Message::Binary(value.into()),
            WebSocketMessage::Ping(value) => Message::Ping(value.into()),
            WebSocketMessage::Pong(value) => Message::Pong(value.into()),
            WebSocketMessage::Close(frame) => Message::Close(frame.map(|frame| CloseFrame {
                code: CloseCode::from(frame.code),
                reason: frame.reason.into(),
            })),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
enum OutboundMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<CloseFramePayloadOut>),
    Error(String),
}

#[derive(Serialize)]
struct CloseFramePayloadOut {
    code: u16,
    reason: String,
}

struct SendRequest {
    message: Message,
    result: oneshot::Sender<Result<(), String>>,
}

struct ConnectionHandle {
    sender: mpsc::Sender<SendRequest>,
    cancel: CancellationToken,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    _local_owner_slot: Option<OwnedSemaphorePermit>,
}

#[derive(Clone)]
pub(crate) struct WebSocketManager {
    connections: Arc<Mutex<HashMap<Id, Arc<ConnectionHandle>>>>,
    connect_cancel: Arc<Mutex<CancellationToken>>,
    local_owner_slots: Arc<Semaphore>,
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self {
            connections: Arc::default(),
            connect_cancel: Arc::new(Mutex::new(CancellationToken::new())),
            local_owner_slots: Arc::new(Semaphore::new(MAX_LOCAL_OWNER_CONNECTIONS)),
        }
    }
}

impl WebSocketManager {
    async fn remove(&self, id: Id) -> Option<Arc<ConnectionHandle>> {
        self.connections.lock().await.remove(&id)
    }

    async fn disconnect_handle(handle: Arc<ConnectionHandle>) {
        handle.cancel.cancel();
        if let Some(mut task) = handle.task.lock().await.take() {
            if tokio::time::timeout(SHUTDOWN_TIMEOUT, &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
    }

    async fn disconnect(&self, id: Id) {
        if let Some(handle) = self.remove(id).await {
            Self::disconnect_handle(handle).await;
        }
    }
}

async fn open_connection(
    manager: &WebSocketManager,
    url: &str,
    on_message: Channel<serde_json::Value>,
    local_owner_slot: Option<OwnedSemaphorePermit>,
) -> Result<Id, String> {
    crate::local_owner_profile::require_relay(url)?;
    let connect_cancel = manager.connect_cancel.lock().await.clone();
    let websocket_config = local_owner_slot.as_ref().map(|_| {
        WebSocketConfig::default()
            .max_message_size(Some(MAX_LOCAL_OWNER_TEXT_FRAME_BYTES))
            .max_frame_size(Some(MAX_LOCAL_OWNER_TEXT_FRAME_BYTES))
    });
    let (socket, _) = tokio::select! {
        _ = connect_cancel.cancelled() => return Err("WebSocket connection cancelled".to_string()),
        result = tokio::time::timeout(
            CONNECT_TIMEOUT,
            connect_async_with_config(url, websocket_config, false),
        ) => result
            .map_err(|_| "WebSocket connection timed out".to_string())?
            .map_err(|error| error.to_string())?,
    };

    // Serialize registration with disconnect_all so a reload cannot miss a
    // connection that finished its handshake concurrently with teardown.
    let current_connect_cancel = manager.connect_cancel.lock().await;
    if connect_cancel.is_cancelled() {
        return Err("WebSocket connection cancelled".to_string());
    }

    let id = loop {
        let candidate = uuid::Uuid::new_v4().as_u128() as u32;
        if !manager.connections.lock().await.contains_key(&candidate) {
            break candidate;
        }
    };
    let (sender, receiver) = mpsc::channel(SEND_QUEUE_CAPACITY);
    let cancel = CancellationToken::new();
    let handle = Arc::new(ConnectionHandle {
        sender,
        cancel: cancel.clone(),
        task: Mutex::new(None),
        _local_owner_slot: local_owner_slot,
    });
    let mut task_slot = handle.task.lock().await;
    manager.connections.lock().await.insert(id, handle.clone());

    let task_manager = manager.clone();
    let task = tauri::async_runtime::spawn(run_connection(
        id,
        socket,
        receiver,
        cancel,
        on_message,
        task_manager,
    ));
    *task_slot = Some(task);
    drop(task_slot);
    drop(current_connect_cancel);
    Ok(id)
}

#[tauri::command]
async fn connect(
    manager: tauri::State<'_, WebSocketManager>,
    state: tauri::State<'_, crate::app_state::AppState>,
    url: String,
    on_message: Channel<serde_json::Value>,
    _config: Option<serde_json::Value>,
) -> Result<Id, String> {
    let local_owner_slot = if crate::local_owner_profile::profile_active() {
        if crate::local_owner_profile::recovery_active(&state) {
            return Err("identity recovery is required before relay connection".to_string());
        }
        state.signing_keys()?;
        Some(
            manager
                .local_owner_slots
                .clone()
                .try_acquire_owned()
                .map_err(|_| "local-owner websocket connection limit reached".to_string())?,
        )
    } else {
        None
    };
    open_connection(manager.inner(), &url, on_message, local_owner_slot).await
}

pub(crate) async fn send_message(
    manager: &WebSocketManager,
    id: Id,
    message: WebSocketMessage,
) -> Result<(), String> {
    // Egress guard: the NIP-49 local key backup must never reach a relay.
    // This is the single choke point for all webview-originated websocket
    // frames (see `crate::egress_guard`).
    match &message {
        WebSocketMessage::Text(text) => {
            crate::egress_guard::assert_no_key_backup(text, "websocket text frame")?
        }
        WebSocketMessage::Binary(bytes) => {
            crate::egress_guard::assert_no_key_backup_bytes(bytes, "websocket binary frame")?
        }
        _ => {}
    }
    let handle = manager
        .connections
        .lock()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("WebSocket connection {id} not found"))?;
    let (result_tx, result_rx) = oneshot::channel();
    tokio::time::timeout(
        WRITE_TIMEOUT,
        handle.sender.send(SendRequest {
            message: message.into(),
            result: result_tx,
        }),
    )
    .await
    .map_err(|_| "WebSocket send queue timed out".to_string())?
    .map_err(|_| "WebSocket connection closed".to_string())?;

    tokio::time::timeout(WRITE_TIMEOUT, result_rx)
        .await
        .map_err(|_| "WebSocket send timed out".to_string())?
        .map_err(|_| "WebSocket connection closed".to_string())?
}

#[tauri::command]
async fn send(
    manager: tauri::State<'_, WebSocketManager>,
    state: tauri::State<'_, crate::app_state::AppState>,
    id: Id,
    message: WebSocketMessage,
) -> Result<(), String> {
    if crate::local_owner_profile::profile_active() {
        if crate::local_owner_profile::recovery_active(&state) {
            return Err("identity recovery is required before relay send".to_string());
        }
        let owner = state.signing_keys()?.public_key().to_hex();
        validate_local_owner_frame(&message, &owner)?;
    }
    send_message(manager.inner(), id, message).await
}

fn validate_local_owner_frame(message: &WebSocketMessage, owner: &str) -> Result<(), String> {
    let WebSocketMessage::Text(text) = message else {
        return match message {
            WebSocketMessage::Ping(payload) | WebSocketMessage::Pong(payload)
                if payload.len() <= MAX_WEBSOCKET_CONTROL_PAYLOAD_BYTES =>
            {
                Ok(())
            }
            WebSocketMessage::Ping(_) | WebSocketMessage::Pong(_) => {
                Err("local-owner websocket control payload exceeds 125 bytes".to_string())
            }
            WebSocketMessage::Close(_) => Ok(()),
            WebSocketMessage::Binary(_) => {
                Err("local-owner build refuses binary relay frames".to_string())
            }
            WebSocketMessage::Text(_) => unreachable!(),
        };
    };
    if text.len() > MAX_LOCAL_OWNER_TEXT_FRAME_BYTES {
        return Err("local-owner relay frame exceeds the 1 MiB limit".to_string());
    }
    let frame: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "invalid relay frame JSON".to_string())?;
    let parts = frame
        .as_array()
        .ok_or_else(|| "relay frame must be a JSON array".to_string())?;
    let operation = parts
        .first()
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "relay frame is missing an operation".to_string())?;
    match operation {
        "REQ" | "COUNT" | "CLOSE" => Ok(()),
        "EVENT" | "AUTH" => {
            let raw_event = parts
                .get(1)
                .ok_or_else(|| format!("{operation} frame is missing its event"))?;
            let event = nostr::Event::from_json(raw_event.to_string())
                .map_err(|_| format!("{operation} frame contains an invalid event"))?;
            event
                .verify()
                .map_err(|_| format!("{operation} frame contains an invalid signature"))?;
            if !event.pubkey.to_hex().eq_ignore_ascii_case(owner) {
                return Err(format!(
                    "{operation} frame is not signed by the admitted owner"
                ));
            }
            if operation == "AUTH" {
                if event.kind.as_u16() != 22_242 {
                    return Err("AUTH frame must contain a kind 22242 event".to_string());
                }
                Ok(())
            } else {
                crate::local_owner_profile::require_webview_signed_event(&event)
            }
        }
        _ => Err(format!(
            "local-owner build refuses outbound relay frame operation {operation:?}"
        )),
    }
}

#[tauri::command]
async fn disconnect(manager: tauri::State<'_, WebSocketManager>, id: Id) -> Result<(), String> {
    manager.disconnect(id).await;
    Ok(())
}

#[tauri::command]
async fn disconnect_all(manager: tauri::State<'_, WebSocketManager>) -> Result<(), String> {
    let mut connect_cancel = manager.connect_cancel.lock().await;
    connect_cancel.cancel();
    *connect_cancel = CancellationToken::new();
    let handles = {
        let mut connections = manager.connections.lock().await;
        connections
            .drain()
            .map(|(_, handle)| handle)
            .collect::<Vec<_>>()
    };
    futures_util::future::join_all(handles.into_iter().map(WebSocketManager::disconnect_handle))
        .await;
    Ok(())
}

async fn run_connection<S>(
    id: Id,
    mut socket: tokio_tungstenite::WebSocketStream<S>,
    mut receiver: mpsc::Receiver<SendRequest>,
    cancel: CancellationToken,
    on_message: Channel<serde_json::Value>,
    manager: WebSocketManager,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = tokio::time::timeout(
                    SHUTDOWN_TIMEOUT,
                    socket.send(Message::Close(Some(CloseFrame {
                        code: CloseCode::Normal,
                        reason: "disconnect".into(),
                    }))),
                ).await;
                break;
            }
            request = receiver.recv() => {
                let Some(request) = request else { break };
                let result = tokio::time::timeout(WRITE_TIMEOUT, socket.send(request.message))
                    .await
                    .map_err(|_| "WebSocket send timed out".to_string())
                    .and_then(|result| result.map_err(|error| error.to_string()));
                let failed = result.is_err();
                let _ = request.result.send(result);
                if failed { break; }
            }
            incoming = socket.next() => {
                let message = match incoming {
                    Some(Ok(message)) => outbound_message(message),
                    Some(Err(error)) => OutboundMessage::Error(error.to_string()),
                    None => OutboundMessage::Close(None),
                };
                let terminal = matches!(message, OutboundMessage::Close(_) | OutboundMessage::Error(_));
                if let Ok(value) = serde_json::to_value(message) {
                    let _ = on_message.send(value);
                }
                if terminal { break; }
            }
        }
    }
    manager.remove(id).await;
}

fn outbound_message(message: Message) -> OutboundMessage {
    match message {
        Message::Text(value) => OutboundMessage::Text(value.to_string()),
        Message::Binary(value) => OutboundMessage::Binary(value.to_vec()),
        Message::Ping(value) => OutboundMessage::Ping(value.to_vec()),
        Message::Pong(value) => OutboundMessage::Pong(value.to_vec()),
        Message::Close(frame) => OutboundMessage::Close(frame.map(|frame| CloseFramePayloadOut {
            code: frame.code.into(),
            reason: frame.reason.to_string(),
        })),
        Message::Frame(_) => OutboundMessage::Error("unexpected raw WebSocket frame".to_string()),
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    install_crypto_provider();
    tauri::plugin::Builder::new("websocket")
        .invoke_handler(tauri::generate_handler![
            connect,
            send,
            disconnect,
            disconnect_all
        ])
        .setup(|app, _api| {
            app.manage(WebSocketManager::default());
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;
    use std::sync::atomic::{AtomicBool, Ordering};

    use tauri::ipc::InvokeResponseBody;
    use tokio::io::duplex;
    use tokio_tungstenite::{tungstenite::protocol::Role, WebSocketStream};

    fn silent_channel() -> Channel<serde_json::Value> {
        Channel::new(|_: InvokeResponseBody| Ok(()))
    }

    fn event_frame(event: &nostr::Event) -> WebSocketMessage {
        WebSocketMessage::Text(serde_json::json!(["EVENT", event]).to_string())
    }

    #[test]
    fn local_owner_frames_allow_reads_and_owner_interactions_only() {
        crate::local_owner_profile::with_test_profile_active(|| {
            let owner = nostr::Keys::generate();
            let message = nostr::EventBuilder::new(nostr::Kind::Custom(9), "hello")
                .sign_with_keys(&owner)
                .unwrap();
            assert!(validate_local_owner_frame(
                &event_frame(&message),
                &owner.public_key().to_hex()
            )
            .is_ok());
            assert!(validate_local_owner_frame(
                &WebSocketMessage::Text(
                    serde_json::json!(["REQ", "subscription", {"kinds": [9]}]).to_string(),
                ),
                &owner.public_key().to_hex(),
            )
            .is_ok());

            let workflow = nostr::EventBuilder::new(nostr::Kind::Custom(30_620), "workflow")
                .sign_with_keys(&owner)
                .unwrap();
            assert!(validate_local_owner_frame(
                &event_frame(&workflow),
                &owner.public_key().to_hex()
            )
            .is_err());
            assert!(validate_local_owner_frame(
                &event_frame(&message),
                &nostr::Keys::generate().public_key().to_hex()
            )
            .is_err());
            assert!(validate_local_owner_frame(
                &WebSocketMessage::Binary(vec![1, 2, 3]),
                &owner.public_key().to_hex(),
            )
            .is_err());
        });
    }

    #[test]
    fn local_owner_auth_frame_requires_a_valid_owner_kind_22242_event() {
        let owner = nostr::Keys::generate();
        let auth = nostr::EventBuilder::new(nostr::Kind::Custom(22_242), "challenge")
            .sign_with_keys(&owner)
            .unwrap();
        let frame = WebSocketMessage::Text(serde_json::json!(["AUTH", auth]).to_string());
        assert!(validate_local_owner_frame(&frame, &owner.public_key().to_hex()).is_ok());

        let wrong_kind = nostr::EventBuilder::new(nostr::Kind::Custom(9), "challenge")
            .sign_with_keys(&owner)
            .unwrap();
        let frame = WebSocketMessage::Text(serde_json::json!(["AUTH", wrong_kind]).to_string());
        assert!(validate_local_owner_frame(&frame, &owner.public_key().to_hex()).is_err());
    }

    #[tokio::test]
    async fn secure_websocket_reaches_tls_without_panicking() {
        install_crypto_provider();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_millis(100)).await;
        });
        let result = std::panic::AssertUnwindSafe(tokio_tungstenite::connect_async(format!(
            "wss://{address}"
        )))
        .catch_unwind()
        .await;

        assert!(result.is_ok(), "TLS setup must not panic");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn live_tcp_server_connect_send_and_disconnect() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (received_tx, received_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let message = socket.next().await.unwrap().unwrap();
            received_tx.send(message).unwrap();
            while let Some(message) = socket.next().await {
                if matches!(message, Ok(Message::Close(_))) {
                    break;
                }
            }
        });

        let manager = WebSocketManager::default();
        let id = open_connection(&manager, &format!("ws://{address}"), silent_channel(), None)
            .await
            .unwrap();
        send_message(&manager, id, WebSocketMessage::Text("live-probe".into()))
            .await
            .unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), received_rx)
                .await
                .unwrap()
                .unwrap(),
            Message::Text("live-probe".into())
        );

        manager.disconnect(id).await;
        assert!(!manager.connections.lock().await.contains_key(&id));
        tokio::time::timeout(Duration::from_secs(1), server)
            .await
            .expect("live server should observe native socket shutdown")
            .unwrap();
    }

    #[tokio::test]
    async fn eof_removes_connection() {
        let manager = WebSocketManager::default();
        let (client_io, server_io) = duplex(1024);
        let (client, server) = tokio::join!(
            WebSocketStream::from_raw_socket(client_io, Role::Client, None),
            WebSocketStream::from_raw_socket(server_io, Role::Server, None),
        );
        let (sender, receiver) = mpsc::channel(SEND_QUEUE_CAPACITY);
        let handle = Arc::new(ConnectionHandle {
            sender,
            cancel: CancellationToken::new(),
            task: Mutex::new(None),
            _local_owner_slot: None,
        });
        manager.connections.lock().await.insert(1, handle.clone());
        let task = tauri::async_runtime::spawn(run_connection(
            1,
            client,
            receiver,
            handle.cancel.clone(),
            silent_channel(),
            manager.clone(),
        ));
        *handle.task.lock().await = Some(task);

        drop(server);
        tokio::time::timeout(Duration::from_secs(1), async {
            while manager.connections.lock().await.contains_key(&1) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("EOF should clean up its native connection ID");
    }

    #[tokio::test]
    async fn disconnect_removes_and_drops_task_before_returning() {
        struct DropGuard(Arc<AtomicBool>);
        impl Drop for DropGuard {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let manager = WebSocketManager::default();
        let dropped = Arc::new(AtomicBool::new(false));
        let task_dropped = dropped.clone();
        let (ready_tx, ready_rx) = oneshot::channel();
        let (sender, _receiver) = mpsc::channel(SEND_QUEUE_CAPACITY);
        let handle = Arc::new(ConnectionHandle {
            sender,
            cancel: CancellationToken::new(),
            task: Mutex::new(Some(tauri::async_runtime::spawn(async move {
                let _guard = DropGuard(task_dropped);
                ready_tx.send(()).unwrap();
                std::future::pending::<()>().await;
            }))),
            _local_owner_slot: None,
        });
        manager.connections.lock().await.insert(7, handle);
        ready_rx.await.unwrap();

        tokio::time::timeout(Duration::from_secs(1), manager.disconnect(7))
            .await
            .expect("disconnect should abort an unresponsive task");
        assert!(!manager.connections.lock().await.contains_key(&7));
        assert!(dropped.load(Ordering::SeqCst));

        // Repeated teardown is intentionally a no-op.
        manager.disconnect(7).await;
    }

    #[tokio::test]
    async fn teardown_gate_stays_closed_until_tasks_stop() {
        let manager = WebSocketManager::default();
        let gate = manager.connect_cancel.lock().await;
        let (sender, _receiver) = mpsc::channel(SEND_QUEUE_CAPACITY);
        let handle = Arc::new(ConnectionHandle {
            sender,
            cancel: CancellationToken::new(),
            task: Mutex::new(Some(tauri::async_runtime::spawn(async {
                std::future::pending::<()>().await;
            }))),
            _local_owner_slot: None,
        });
        manager.connections.lock().await.insert(1, handle);
        gate.cancel();
        let handles = {
            let mut connections = manager.connections.lock().await;
            connections
                .drain()
                .map(|(_, handle)| handle)
                .collect::<Vec<_>>()
        };

        let shutdown = futures_util::future::join_all(
            handles.into_iter().map(WebSocketManager::disconnect_handle),
        );
        assert!(manager.connect_cancel.try_lock().is_err());
        shutdown.await;
        drop(gate);
        assert!(manager.connect_cancel.try_lock().is_ok());
    }

    #[tokio::test]
    async fn one_connection_does_not_block_another_send_queue() {
        let manager = WebSocketManager::default();
        let (blocked_sender, blocked_receiver) = mpsc::channel(1);
        blocked_sender
            .send(SendRequest {
                message: Message::Text("blocked".into()),
                result: oneshot::channel().0,
            })
            .await
            .unwrap();
        let blocked = Arc::new(ConnectionHandle {
            sender: blocked_sender,
            cancel: CancellationToken::new(),
            task: Mutex::new(None),
            _local_owner_slot: None,
        });
        manager.connections.lock().await.insert(1, blocked);

        let (healthy_sender, mut healthy_receiver) = mpsc::channel(1);
        let healthy = Arc::new(ConnectionHandle {
            sender: healthy_sender.clone(),
            cancel: CancellationToken::new(),
            task: Mutex::new(None),
            _local_owner_slot: None,
        });
        manager.connections.lock().await.insert(2, healthy);

        let (result, _) = oneshot::channel();
        tokio::time::timeout(
            Duration::from_millis(50),
            healthy_sender.send(SendRequest {
                message: Message::Text("healthy".into()),
                result,
            }),
        )
        .await
        .expect("a full queue on one connection must not block another")
        .unwrap();
        assert!(healthy_receiver.recv().await.is_some());
        drop(blocked_receiver);
    }
}
