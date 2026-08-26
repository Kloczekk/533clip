import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";

interface R2Settings {
  enabled: boolean;
  provider: string;
  accountId: string;
  endpointUrl: string;
  region: string;
  accessKeyId: string;
  secretSet: boolean;
  bucket: string;
  publicBaseUrl: string;
  deleteAfterDays: number;
}

export function R2SettingsPanel() {
  const [settings, setSettings] = useState<R2Settings | null>(null);
  const [secret, setSecret] = useState("");
  const [status, setStatus] = useState<string | null>(null);

  const load = useCallback(async () => {
    setSettings(await invoke<R2Settings>("get_r2_settings"));
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function save() {
    if (!settings) return;
    const updated = await invoke<R2Settings>("set_r2_settings", {
      settings: {
        enabled: settings.enabled,
        provider: settings.provider,
        accountId: settings.accountId,
        endpointUrl: settings.endpointUrl,
        region: settings.region,
        accessKeyId: settings.accessKeyId,
        secretAccessKey: secret || null,
        bucket: settings.bucket,
        publicBaseUrl: settings.publicBaseUrl,
        deleteAfterDays: settings.deleteAfterDays,
      },
    });
    setSettings(updated);
    setSecret("");
    setStatus(
      updated.publicBaseUrl
        ? "Sharing settings saved"
        : "Private signed links enabled",
    );
  }

  async function cleanup() {
    const removed = await invoke<number>("cleanup_r2_uploads");
    setStatus(`Deleted ${removed} old ${settings?.provider === "b2" ? "B2" : "R2"} upload(s)`);
  }

  async function exportFriendConfig() {
    const path = await saveDialog({
      title: "Export 533clip sharing config",
      defaultPath: "533clip-sharing-config.json",
      filters: [{ name: "533clip Sharing Config", extensions: ["json"] }],
    });
    if (!path) return;
    await invoke("export_friend_sharing_config", { path });
    setStatus("Friend config exported without keys");
  }

  async function importFriendConfig() {
    const selected = await open({
      title: "Import 533clip sharing config",
      multiple: false,
      filters: [{ name: "533clip Sharing Config", extensions: ["json"] }],
    });
    if (!selected || Array.isArray(selected)) return;
    const updated = await invoke<R2Settings>("import_friend_sharing_config", { path: selected });
    setSettings(updated);
    setSecret("");
    setStatus("Friend config imported. Paste your Backblaze key ID and secret.");
  }

  if (!settings) return null;

  return (
    <section className="settings-section clean-settings-panel">
      <div className="obs-panel-head">
        <div>
          <h2>Cloud Sharing</h2>
          <p>Compressed links for Discord and auto-cleanup.</p>
        </div>
        <label className="settings-toggle">
          <input
            type="checkbox"
            checked={settings.enabled}
            onChange={(e) => setSettings({ ...settings, enabled: e.target.checked })}
          />
          <span>{settings.enabled ? "On" : "Off"}</span>
        </label>
      </div>
      <div className="settings-form-grid">
        <label>
          <span>Provider</span>
          <select
            className="tag-input"
            value={settings.provider || "r2"}
            onChange={(e) => {
              const provider = e.target.value;
              setSettings({
                ...settings,
                provider,
                region: provider === "b2" ? "us-west-004" : "auto",
                endpointUrl: provider === "b2" ? settings.endpointUrl : "",
              });
            }}
          >
            <option value="b2">Backblaze B2</option>
            <option value="r2">Cloudflare R2</option>
          </select>
        </label>
        <label>
          <span>{settings.provider === "b2" ? "Endpoint" : "Account ID"}</span>
          <input
            className="tag-input"
            placeholder={settings.provider === "b2" ? "https://s3.us-west-004.backblazeb2.com" : ""}
            value={settings.provider === "b2" ? settings.endpointUrl : settings.accountId}
            onChange={(e) =>
              setSettings(
                settings.provider === "b2"
                  ? { ...settings, endpointUrl: e.target.value }
                  : { ...settings, accountId: e.target.value },
              )
            }
          />
        </label>
        <label>
          <span>Bucket</span>
          <input
            className="tag-input"
            value={settings.bucket}
            onChange={(e) => setSettings({ ...settings, bucket: e.target.value })}
          />
        </label>
        <label>
          <span>Region</span>
          <input
            className="tag-input"
            placeholder={settings.provider === "b2" ? "us-west-004" : "auto"}
            value={settings.region}
            onChange={(e) => setSettings({ ...settings, region: e.target.value })}
          />
        </label>
        <label>
          <span>Access Key ID</span>
          <input
            className="tag-input"
            value={settings.accessKeyId}
            onChange={(e) => setSettings({ ...settings, accessKeyId: e.target.value })}
          />
        </label>
        <label>
          <span>Secret Key</span>
          <input
            className="tag-input"
            placeholder={settings.secretSet ? "saved" : ""}
            type="password"
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
          />
        </label>
        <label className="settings-wide">
          <span>Public URL (optional)</span>
          <input
            className="tag-input"
            placeholder="Leave empty for private signed links"
            value={settings.publicBaseUrl}
            onChange={(e) => setSettings({ ...settings, publicBaseUrl: e.target.value })}
          />
        </label>
        <label className="settings-number">
          <span>Delete uploads after</span>
          <input
            className="tag-input"
            type="number"
            min={1}
            max={365}
            value={settings.deleteAfterDays}
            onChange={(e) =>
              setSettings({ ...settings, deleteAfterDays: Number(e.target.value) || 15 })
            }
          />
          <span>days</span>
        </label>
      </div>
      <div className="settings-actions">
        <button type="button" className="btn ghost" onClick={() => void save()}>
          Save
        </button>
        <button type="button" className="btn ghost" onClick={() => void cleanup()}>
          Cleanup
        </button>
        <button type="button" className="btn ghost" onClick={() => void exportFriendConfig()}>
          Export Friend Config
        </button>
        <button type="button" className="btn ghost" onClick={() => void importFriendConfig()}>
          Import Config
        </button>
      </div>
      {status && <p className="settings-status">{status}</p>}
    </section>
  );
}
