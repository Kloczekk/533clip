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

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    watch_path: Option<String>,
    #[serde(default)]
    r2: R2Settings,
    #[serde(default)]
    obs: ObsSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct R2Settings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_share_provider")]
    pub provider: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub endpoint_url: String,
    #[serde(default = "default_share_region")]
    pub region: String,
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default)]
    pub secret_access_key: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub public_base_url: String,
    #[serde(default = "default_delete_days")]
    pub delete_after_days: u32,
}

fn default_delete_days() -> u32 {
    15
}

fn default_share_provider() -> String {
    "r2".to_string()
}

fn default_share_region() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct R2SettingsResponse {
    pub enabled: bool,
    pub provider: String,
    pub account_id: String,
    pub endpoint_url: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_set: bool,
    pub bucket: String,
    pub public_base_url: String,
    pub delete_after_days: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct R2SettingsUpdate {
    pub enabled: bool,
    pub provider: String,
    pub account_id: String,
    pub endpoint_url: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: Option<String>,
    pub bucket: String,
    pub public_base_url: String,
    pub delete_after_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsSettings {
    #[serde(default = "default_obs_integration_mode")]
    pub integration_mode: String,
    #[serde(default = "default_obs_websocket_url")]
    pub websocket_url: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub auto_launch: bool,
    #[serde(default)]
    pub start_replay_on_launch: bool,
}

impl Default for ObsSettings {
    fn default() -> Self {
        Self {
            integration_mode: default_obs_integration_mode(),
            websocket_url: default_obs_websocket_url(),
            password: String::new(),
            auto_launch: false,
            start_replay_on_launch: false,
        }
    }
}

fn default_obs_integration_mode() -> String {
    "manual".to_string()
}

fn default_obs_websocket_url() -> String {
    "ws://127.0.0.1:4455".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsSettingsResponse {
    pub integration_mode: String,
    pub websocket_url: String,
    pub password_set: bool,
    pub auto_launch: bool,
    pub start_replay_on_launch: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsSettingsUpdate {
    pub integration_mode: String,
    pub websocket_url: String,
    pub password: Option<String>,
    pub auto_launch: bool,
    pub start_replay_on_launch: bool,
}

#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
    // Guards load-modify-save so two settings updates firing close together
    // (e.g. OBS + R2 forms saved in the same tick) can't clobber each other.
    lock: std::sync::Arc<parking_lot::Mutex<()>>,
}

impl SettingsStore {
    pub fn open(data_dir: &Path) -> Result<Self, SettingsError> {
        std::fs::create_dir_all(data_dir)?;
        Ok(Self {
            path: data_dir.join("settings.json"),
            lock: std::sync::Arc::new(parking_lot::Mutex::new(())),
        })
    }

    fn load(&self) -> Result<SettingsFile, SettingsError> {
        Ok(super::load_json_or_default(&self.path)?)
    }

    fn save(&self, settings: &SettingsFile) -> Result<(), SettingsError> {
        let json = serde_json::to_string_pretty(settings)?;
        super::atomic_write(&self.path, &json)?;
        Ok(())
    }

    pub fn watch_path(&self) -> Result<Option<String>, SettingsError> {
        Ok(self.load()?.watch_path)
    }

    pub fn set_watch_path(&self, path: Option<&str>) -> Result<(), SettingsError> {
        let _guard = self.lock.lock();
        let mut settings = self.load()?;
        settings.watch_path = path.map(|p| p.to_string());
        self.save(&settings)
    }

    pub fn r2(&self) -> Result<R2Settings, SettingsError> {
        Ok(self.load()?.r2)
    }

    pub fn r2_response(&self) -> Result<R2SettingsResponse, SettingsError> {
        let r2 = self.r2()?;
        Ok(R2SettingsResponse {
            enabled: r2.enabled,
            provider: if r2.provider.trim().is_empty() { default_share_provider() } else { r2.provider },
            account_id: r2.account_id,
            endpoint_url: r2.endpoint_url,
            region: if r2.region.trim().is_empty() { default_share_region() } else { r2.region },
            access_key_id: r2.access_key_id,
            secret_set: !r2.secret_access_key.is_empty(),
            bucket: r2.bucket,
            public_base_url: r2.public_base_url,
            delete_after_days: r2.delete_after_days,
        })
    }

    pub fn set_r2(&self, update: R2SettingsUpdate) -> Result<(), SettingsError> {
        let _guard = self.lock.lock();
        let mut settings = self.load()?;
        settings.r2.enabled = update.enabled;
        let provider = update.provider.trim().to_lowercase();
        settings.r2.provider = provider.clone();
        settings.r2.account_id = update.account_id.trim().to_string();
        let mut endpoint = update.endpoint_url.trim().trim_end_matches('/').to_string();
        if provider == "b2"
            && !endpoint.is_empty()
            && !endpoint.starts_with("http://")
            && !endpoint.starts_with("https://")
        {
            endpoint = format!("https://{endpoint}");
        }
        settings.r2.endpoint_url = endpoint;
        settings.r2.region = update.region.trim().to_string();
        settings.r2.access_key_id = update.access_key_id.trim().to_string();
        if let Some(secret) = update.secret_access_key {
            if !secret.trim().is_empty() {
                settings.r2.secret_access_key = secret.trim().to_string();
            }
        }
        settings.r2.bucket = update.bucket.trim().to_string();
        settings.r2.public_base_url = update.public_base_url.trim().trim_end_matches('/').to_string();
        settings.r2.delete_after_days = update.delete_after_days.max(1);
        self.save(&settings)
    }

    pub fn obs(&self) -> Result<ObsSettings, SettingsError> {
        Ok(self.load()?.obs)
    }

    pub fn obs_response(&self) -> Result<ObsSettingsResponse, SettingsError> {
        let obs = self.obs()?;
        Ok(ObsSettingsResponse {
            integration_mode: if obs.integration_mode.trim().is_empty() {
                default_obs_integration_mode()
            } else {
                obs.integration_mode
            },
            websocket_url: if obs.websocket_url.trim().is_empty() {
                default_obs_websocket_url()
            } else {
                obs.websocket_url
            },
            password_set: !obs.password.is_empty(),
            auto_launch: obs.auto_launch,
            start_replay_on_launch: obs.start_replay_on_launch,
        })
    }

    pub fn set_obs(&self, update: ObsSettingsUpdate) -> Result<(), SettingsError> {
        let _guard = self.lock.lock();
        let mut settings = self.load()?;
        let mode = update.integration_mode.trim().to_lowercase();
        settings.obs.integration_mode = if matches!(mode.as_str(), "off" | "manual" | "managed") {
            mode
        } else {
            default_obs_integration_mode()
        };
        let url = update.websocket_url.trim();
        settings.obs.websocket_url = if url.is_empty() {
            default_obs_websocket_url()
        } else {
            url.to_string()
        };
        if let Some(password) = update.password {
            settings.obs.password = password.trim().to_string();
        }
        settings.obs.auto_launch = update.auto_launch;
        settings.obs.start_replay_on_launch = update.start_replay_on_launch;
        self.save(&settings)
    }
}
