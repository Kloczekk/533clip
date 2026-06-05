import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface ObsWsSettings {
  enabled: boolean;
  host: string;
  port: number;
  passwordSet: boolean;
}

interface ObsConnectionStatus {
  connected: boolean;
  error?: string | null;
}

export function ObsWebSocketPanel() {
  const [settings, setSettings] = useState<ObsWsSettings | null>(null);
  const [connected, setConnected] = useState(false);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    const s = await invoke<ObsWsSettings>("get_obs_websocket_settings");
    setSettings(s);
    const status = await invoke<ObsConnectionStatus>("get_obs_websocket_status");
    setConnected(status.connected);
    setStatusError(status.error ?? null);
  }, []);

  useEffect(() => {
    void load();
    const unlistenConn = listen<ObsConnectionStatus>("obs://connection", (e) => {
      setConnected(e.payload.connected);
      setStatusError(e.payload.error ?? null);
    });
    return () => {
      void unlistenConn.then((fn) => fn());
    };
  }, [load]);

  async function syncFromObs() {
    setSaving(true);
    try {
      const updated = await invoke<ObsWsSettings>("import_obs_websocket_settings");
      setSettings(updated);
      setPassword("");
      setStatusError(null);
    } catch (e) {
      setStatusError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function save() {
    if (!settings) return;
    setSaving(true);
    try {
      const payload: {
        enabled: boolean;
        host: string;
        port: number;
        password?: string | null;
      } = {
        enabled: settings.enabled,
        host: settings.host,
        port: settings.port,
      };
      if (password.length > 0) {
        payload.password = password;
      }
      const updated = await invoke<ObsWsSettings>("set_obs_websocket_settings", payload);
      setSettings(updated);
      setPassword("");
    } finally {
      setSaving(false);
    }
  }

  if (!settings) return null;

  return (
    <div className="obs-ws-panel">
      <div className="obs-ws-header">
        <p className="footer-label">OBS WebSocket</p>
        <span
          className={`obs-ws-dot ${connected ? "online" : "offline"}`}
          title={connected ? "Connected to OBS" : "Not connected"}
        />
      </div>
      <p className="footer-hint">
        Enable in OBS: Tools → WebSocket Server Settings. Shows “Replay saved” like OBS.
      </p>
      {statusError && <p className="obs-ws-error">{statusError}</p>}
      <label className="obs-ws-row">
        <input
          type="checkbox"
          checked={settings.enabled}
          onChange={(e) => setSettings({ ...settings, enabled: e.target.checked })}
        />
        <span>Connect on startup</span>
      </label>
      <div className="obs-ws-row split">
        <input
          type="text"
          className="tag-input"
          placeholder="Host"
          value={settings.host}
          onChange={(e) => setSettings({ ...settings, host: e.target.value })}
        />
        <input
          type="number"
          className="tag-input obs-ws-port"
          min={1}
          max={65535}
          value={settings.port}
          onChange={(e) =>
            setSettings({ ...settings, port: Number(e.target.value) || 4455 })
          }
        />
      </div>
      <input
        type="password"
        className="tag-input"
        placeholder={
          settings.passwordSet ? "Password (leave blank to keep)" : "WebSocket password"
        }
        value={password}
        onChange={(e) => setPassword(e.target.value)}
      />
      <div className="obs-ws-actions">
        <button
          type="button"
          className="btn ghost"
          disabled={saving}
          onClick={() => void syncFromObs()}
        >
          Sync from OBS
        </button>
        <button type="button" className="btn ghost" disabled={saving} onClick={() => void save()}>
          Save
        </button>
        <button
          type="button"
          className="btn ghost"
          onClick={() => void invoke("reconnect_obs_websocket")}
        >
          Retry
        </button>
      </div>
    </div>
  );
}
