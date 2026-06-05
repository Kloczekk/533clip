use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsWebSocketSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_obs_host")]
    pub host: String,
    #[serde(default = "default_obs_port")]
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_obs_host() -> String {
    "127.0.0.1".to_string()
}

fn default_obs_port() -> u16 {
    4455
}

impl Default for ObsWebSocketSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            host: default_obs_host(),
            port: default_obs_port(),
            password: None,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    watch_path: Option<String>,
    #[serde(default)]
    obs_websocket: ObsWebSocketSettings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsWebSocketSettingsResponse {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub password_set: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsWebSocketSettingsUpdate {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    /// When `null`, keep the existing password. When `""`, clear it.
    pub password: Option<String>,
}

#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn open(data_dir: &Path) -> Result<Self, SettingsError> {
        std::fs::create_dir_all(data_dir)?;
        Ok(Self {
            path: data_dir.join("settings.json"),
        })
    }

    fn load(&self) -> Result<SettingsFile, SettingsError> {
        if !self.path.exists() {
            return Ok(SettingsFile::default());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        if raw.trim().is_empty() {
            return Ok(SettingsFile::default());
        }
        Ok(serde_json::from_str(&raw)?)
    }

    fn save(&self, settings: &SettingsFile) -> Result<(), SettingsError> {
        let json = serde_json::to_string_pretty(settings)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }

    pub fn watch_path(&self) -> Result<Option<String>, SettingsError> {
        Ok(self.load()?.watch_path)
    }

    pub fn set_watch_path(&self, path: Option<&str>) -> Result<(), SettingsError> {
        let mut settings = self.load()?;
        settings.watch_path = path.map(|p| p.to_string());
        self.save(&settings)
    }

    pub fn obs_websocket(&self) -> Result<ObsWebSocketSettings, SettingsError> {
        Ok(self.load()?.obs_websocket)
    }

    /// Stored settings, with password/port filled from local OBS config when missing.
    pub fn effective_obs_websocket(&self) -> Result<ObsWebSocketSettings, SettingsError> {
        let mut ws = self.obs_websocket()?;
        if ws.password.as_ref().is_none_or(|p| p.is_empty()) {
            if let Some(from_obs) = crate::obs::read_local_websocket_config() {
                if ws.password.is_none() {
                    ws.password = from_obs.password;
                }
                if from_obs.port > 0 {
                    ws.port = from_obs.port;
                }
            }
        }
        if let Some(p) = ws.password.as_mut() {
            *p = p.trim().to_string();
            if p.is_empty() {
                ws.password = None;
            }
        }
        Ok(ws)
    }

    /// Copy WebSocket password/port from OBS plugin config into `settings.json`.
    pub fn import_obs_websocket_from_plugin(&self) -> Result<bool, SettingsError> {
        let Some(from_obs) = crate::obs::read_local_websocket_config() else {
            return Ok(false);
        };
        let Some(password) = from_obs
            .password
            .filter(|p| !p.trim().is_empty())
        else {
            return Ok(false);
        };

        let mut settings = self.load()?;
        settings.obs_websocket.password = Some(password.trim().to_string());
        if from_obs.port > 0 {
            settings.obs_websocket.port = from_obs.port;
        }
        settings.obs_websocket.enabled = true;
        self.save(&settings)?;
        Ok(true)
    }

    pub fn obs_websocket_response(&self) -> Result<ObsWebSocketSettingsResponse, SettingsError> {
        let ws = self.obs_websocket()?;
        Ok(ObsWebSocketSettingsResponse {
            enabled: ws.enabled,
            host: ws.host,
            port: ws.port,
            password_set: ws
                .password
                .as_ref()
                .is_some_and(|p| !p.is_empty()),
        })
    }

    pub fn set_obs_websocket(&self, update: ObsWebSocketSettingsUpdate) -> Result<(), SettingsError> {
        let mut settings = self.load()?;
        let mut ws = settings.obs_websocket.clone();
        ws.enabled = update.enabled;
        ws.host = update.host;
        ws.port = update.port;
        match update.password {
            None => {}
            Some(p) if p.trim().is_empty() => ws.password = None,
            Some(p) => ws.password = Some(p.trim().to_string()),
        }
        settings.obs_websocket = ws;
        self.save(&settings)
    }
}
