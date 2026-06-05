use crate::storage::{ObsWebSocketSettings, SettingsStore};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::HeaderValue,
        Message,
    },
};
use tracing::{debug, info, warn};

const EVENT_REPLAY_SAVED: &str = "ReplayBufferSaved";
/// `General` category — includes `ReplayBufferSaved`.
const EVENT_SUBSCRIPTIONS_GENERAL: i64 = 1;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsConnectionStatus {
    pub connected: bool,
    pub error: Option<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaySavedPayload {
    pub message: String,
    pub saved_path: Option<String>,
}

pub struct ObsWebSocketManager {
    app: AppHandle,
    settings: SettingsStore,
    connected: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
    restart_tx: Mutex<Option<mpsc::Sender<()>>>,
}

impl ObsWebSocketManager {
    pub fn new(app: AppHandle, settings: SettingsStore) -> Self {
        let manager = Self {
            app: app.clone(),
            settings,
            connected: Arc::new(AtomicBool::new(false)),
            last_error: Arc::new(Mutex::new(None)),
            restart_tx: Mutex::new(None),
        };
        manager.spawn_worker();
        manager
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn status(&self) -> ObsConnectionStatus {
        let error = self.last_error.blocking_lock().clone();
        ObsConnectionStatus {
            connected: self.is_connected(),
            error,
        }
    }

    pub fn restart(&self) {
        let guard = self.restart_tx.blocking_lock();
        if let Some(tx) = guard.as_ref() {
            let _ = tx.try_send(());
        }
    }

    fn spawn_worker(&self) {
        let app = self.app.clone();
        let settings = self.settings.clone();
        let connected = self.connected.clone();
        let last_error = self.last_error.clone();
        let (restart_tx, mut restart_rx) = mpsc::channel::<()>(4);
        tauri::async_runtime::block_on(async {
            *self.restart_tx.lock().await = Some(restart_tx);
        });

        tauri::async_runtime::spawn(async move {
            loop {
                let mut config = match settings.effective_obs_websocket() {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(error = %e, "failed to load OBS WebSocket settings");
                        ObsWebSocketSettings::default()
                    }
                };

                if config.password.is_none() {
                    if settings.import_obs_websocket_from_plugin().unwrap_or(false) {
                        info!("loaded OBS WebSocket password from OBS config.json");
                        if let Ok(c) = settings.effective_obs_websocket() {
                            config = c;
                        }
                    }
                }

                if config.password.is_none() {
                    set_connected(
                        &app,
                        &connected,
                        &last_error,
                        false,
                        Some(
                            "No WebSocket password saved. Click “Sync from OBS” or paste the password from OBS → Tools → WebSocket Server Settings.".into(),
                        ),
                    );
                    tokio::select! {
                        _ = restart_rx.recv() => continue,
                        _ = tokio::time::sleep(Duration::from_secs(10)) => continue,
                    }
                }

                if !config.enabled {
                    set_connected(&app, &connected, &last_error, false, None);
                    tokio::select! {
                        _ = restart_rx.recv() => continue,
                        _ = tokio::time::sleep(Duration::from_secs(30)) => continue,
                    }
                }

                let (session_shutdown_tx, mut session_shutdown_rx) = mpsc::channel::<()>(1);

                let app_inner = app.clone();
                let connected_inner = connected.clone();
                let last_error_inner = last_error.clone();
                let config_clone = config.clone();

                let mut session = tauri::async_runtime::spawn(async move {
                    if let Err(e) = run_session(
                        &app_inner,
                        &connected_inner,
                        &last_error_inner,
                        &config_clone,
                        &mut session_shutdown_rx,
                    )
                    .await
                    {
                        set_connected(&app_inner, &connected_inner, &last_error_inner, false, Some(e));
                    }
                });

                tokio::select! {
                    _ = restart_rx.recv() => {
                        let _ = session_shutdown_tx.send(()).await;
                        let _ = session.await;
                        continue;
                    }
                    _ = &mut session => {}
                }

                if !connected.load(Ordering::Relaxed) {
                    let err = last_error.blocking_lock().clone();
                    let _ = app.emit(
                        "obs://connection",
                        ObsConnectionStatus {
                            connected: false,
                            error: err,
                        },
                    );
                }

                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }
}

fn set_connected(
    app: &AppHandle,
    flag: &AtomicBool,
    last_error: &Mutex<Option<String>>,
    connected: bool,
    error: Option<String>,
) {
    flag.store(connected, Ordering::Relaxed);
    if let Ok(mut guard) = last_error.try_lock() {
        if connected {
            *guard = None;
        } else if error.is_some() {
            *guard = error.clone();
        }
    }
    let _ = app.emit(
        "obs://connection",
        ObsConnectionStatus {
            connected,
            error,
        },
    );
}

async fn connect_obs(
    url: &str,
) -> Result<
    (
        futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
            Message,
        >,
        futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        >,
    ),
    String,
> {
    let mut request = url.into_client_request().map_err(|e| e.to_string())?;
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static("obswebsocket.json"),
    );

    let (ws, _) = connect_async(request).await.map_err(|e| {
        format!(
            "Could not reach OBS at {url}. Is OBS running with WebSocket enabled? ({e})"
        )
    })?;

    Ok(ws.split())
}

async fn run_session(
    app: &AppHandle,
    connected: &AtomicBool,
    last_error: &Mutex<Option<String>>,
    config: &ObsWebSocketSettings,
    shutdown_rx: &mut mpsc::Receiver<()>,
) -> Result<(), String> {
    let url = format!("ws://{}:{}", config.host, config.port);
    info!(%url, has_password = config.password.is_some(), "connecting to OBS WebSocket");

    let (mut write, mut read) = connect_obs(&url).await?;
    let mut identified = false;

    while let Some(msg) = read.next().await {
        if shutdown_rx.try_recv().is_ok() {
            return Ok(());
        }

        let msg = match msg {
            Ok(m) => m,
            Err(e) => return Err(format!("OBS WebSocket read error: {e}")),
        };

        if msg.is_close() {
            let reason = msg.to_text().unwrap_or("closed");
            warn!(%reason, "OBS WebSocket closed");
            return Err(format!(
                "OBS closed the connection ({reason}). Check WebSocket password in OBS matches 533clip."
            ));
        }

        if !msg.is_text() {
            continue;
        }

        let text = msg.to_text().map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let op = value.get("op").and_then(|v| v.as_u64()).unwrap_or(999);

        match op {
            0 => {
                if identified {
                    continue;
                }
                info!("OBS WebSocket Hello received, sending Identify");
                let identify = build_identify(&value, config.password.as_deref())?;
                write
                    .send(Message::Text(identify.into()))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            2 => {
                if is_identified_success(&value) {
                    identified = true;
                    set_connected(app, connected, last_error, true, None);
                    info!("OBS WebSocket identified");
                } else if let Some(err) = identify_failure_message(&value) {
                    return Err(err);
                }
            }
            5 => {
                if let Some(event_type) = value.pointer("/d/eventType").and_then(|v| v.as_str()) {
                    handle_obs_event(app, event_type, value.get("d"));
                }
            }
            7 | 8 | 9 => {
                debug!(op, "obs websocket request/batch message (ignored)");
            }
            op => {
                if !identified {
                    warn!(op, payload = %text, "unexpected OBS message before Identified");
                } else {
                    debug!(op, "obs websocket message");
                }
            }
        }
    }

    if identified {
        warn!("OBS WebSocket disconnected");
        Err("OBS WebSocket disconnected".into())
    } else {
        Err(
            "OBS closed before accepting the connection — wrong password or WebSocket disabled."
                .into(),
        )
    }
}

fn is_identified_success(value: &Value) -> bool {
    value
        .pointer("/d/negotiatedRpcVersion")
        .and_then(|v| v.as_i64())
        .is_some()
        || value.pointer("/d/serverInfo").is_some()
}

fn identify_failure_message(value: &Value) -> Option<String> {
    let code = value
        .pointer("/d/requestStatus/code")
        .and_then(|v| v.as_i64())?;
    if code == 100 {
        return None;
    }
    let comment = value
        .pointer("/d/requestStatus/comment")
        .and_then(|v| v.as_str())
        .unwrap_or("OBS rejected the connection");
    Some(format!("{comment} (code {code})"))
}

fn build_identify(hello: &Value, password: Option<&str>) -> Result<String, String> {
    let rpc_version = hello
        .pointer("/d/rpcVersion")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);

    let mut d = json!({
        "rpcVersion": rpc_version,
        "eventSubscriptions": EVENT_SUBSCRIPTIONS_GENERAL,
        "ignoreInvalidMessages": true,
    });

    // If Hello includes `authentication`, OBS requires a password (see obs-websocket protocol).
    if hello.pointer("/d/authentication").is_some() {
        let password = password.filter(|p| !p.is_empty()).ok_or_else(|| {
            "OBS requires a WebSocket password. Paste it from OBS → Tools → WebSocket Server Settings into the sidebar, then Save.".to_string()
        })?;
        let challenge = hello
            .pointer("/d/authentication/challenge")
            .and_then(|v| v.as_str())
            .ok_or("OBS auth challenge missing")?;
        let salt = hello
            .pointer("/d/authentication/salt")
            .and_then(|v| v.as_str())
            .ok_or("OBS auth salt missing")?;
        let token = compute_auth(password, salt, challenge);
        d["authentication"] = json!(token);
    }

    Ok(json!({ "op": 1, "d": d }).to_string())
}

fn compute_auth(password: &str, salt: &str, challenge: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt.as_bytes());
    let secret = B64.encode(hasher.finalize());

    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(challenge.as_bytes());
    B64.encode(hasher.finalize())
}

fn handle_obs_event(app: &AppHandle, event_type: &str, data: Option<&Value>) {
    if event_type != EVENT_REPLAY_SAVED {
        return;
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ReplayEventData {
        saved_replay_path: Option<String>,
    }

    let saved_path = data
        .and_then(|d| serde_json::from_value::<ReplayEventData>(d.clone()).ok())
        .and_then(|e| e.saved_replay_path);

    let message = if let Some(ref p) = saved_path {
        format!("Replay saved — {}", p)
    } else {
        "Replay saved".to_string()
    };

    info!(?saved_path, "OBS replay buffer saved");
    let _ = app.emit(
        "obs://replay-saved",
        ReplaySavedPayload {
            message: message.clone(),
            saved_path,
        },
    );
}
