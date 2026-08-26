//! Quick OBS WebSocket handshake test: `cargo run --example obs_ws_probe`
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::env;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};

fn auth(password: &str, salt: &str, challenge: &str) -> String {
    let mut h = Sha256::new();
    h.update(password.as_bytes());
    h.update(salt.as_bytes());
    let secret = B64.encode(h.finalize());
    let mut h = Sha256::new();
    h.update(secret.as_bytes());
    h.update(challenge.as_bytes());
    B64.encode(h.finalize())
}

fn read_password_from_obs_config() -> Option<String> {
    let path = std::env::var("APPDATA")
        .ok()
        .map(std::path::PathBuf::from)?;
    let path = path
        .join("obs-studio")
        .join("plugin_config")
        .join("obs-websocket")
        .join("config.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    v.get("server_password")?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[tokio::main]
async fn main() {
    let password = env::args().nth(1).or_else(read_password_from_obs_config);

    let Some(password) = password else {
        eprintln!("usage: obs_ws_probe [password]  (or set OBS config.json)");
        std::process::exit(1);
    };

    let mut req = "ws://127.0.0.1:4455"
        .into_client_request()
        .expect("request");
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static("obswebsocket.json"),
    );

    let (ws, _) = connect_async(req).await.expect("connect");
    let (mut write, mut read) = ws.split();

    while let Some(Ok(msg)) = read.next().await {
        if !msg.is_text() {
            continue;
        }
        let text = msg.to_text().unwrap();
        let v: Value = serde_json::from_str(text).unwrap();
        let op = v["op"].as_u64().unwrap_or(999);
        println!("<< op={op} {text}");

        if op == 0 {
            let challenge = v["d"]["authentication"]["challenge"]
                .as_str()
                .expect("challenge");
            let salt = v["d"]["authentication"]["salt"].as_str().expect("salt");
            let token = auth(&password, salt, challenge);
            let identify = json!({
                "op": 1,
                "d": {
                    "rpcVersion": v["d"]["rpcVersion"].as_i64().unwrap_or(1),
                    "authentication": token,
                    "eventSubscriptions": 1,
                    "ignoreInvalidMessages": true,
                }
            });
            let out = identify.to_string();
            println!(">> {out}");
            write.send(Message::Text(out.into())).await.expect("send");
        }

        if op == 2 {
            println!("IDENTIFIED OK");
            break;
        }
    }
}
