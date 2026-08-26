import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ObsSettings {
  integrationMode: string;
  websocketUrl: string;
  passwordSet: boolean;
  autoLaunch: boolean;
  startReplayOnLaunch: boolean;
}

interface ObsStatus {
  installed: boolean;
  running: boolean;
  websocketConnected: boolean;
  replayBufferActive: boolean;
  path?: string | null;
  error?: string | null;
}

interface ObsStats {
  cpuUsage: number;
  memoryUsageMb: number;
  activeFps: number;
  renderSkippedFrames: number;
  renderTotalFrames: number;
  outputSkippedFrames: number;
  outputTotalFrames: number;
}

const blankStatus: ObsStatus = {
  installed: false,
  running: false,
  websocketConnected: false,
  replayBufferActive: false,
};

function skippedPercent(skipped: number, total: number) {
  if (!Number.isFinite(total) || total <= 0) return "0.00%";
  return `${((skipped / total) * 100).toFixed(2)}%`;
}

export function ObsControlPanel() {
  const [settings, setSettings] = useState<ObsSettings>({
    integrationMode: "manual",
    websocketUrl: "ws://127.0.0.1:4455",
    passwordSet: false,
    autoLaunch: false,
    startReplayOnLaunch: false,
  });
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState<ObsStatus>(blankStatus);
  const [stats, setStats] = useState<ObsStats | null>(null);
  const [qualityPreset, setQualityPreset] = useState(() => localStorage.getItem("533clip-obs-quality-preset") ?? "medium");
  const [captureMode, setCaptureMode] = useState(() => localStorage.getItem("533clip-obs-capture-mode") ?? "display");
  const [replayHotkey, setReplayHotkey] = useState(() => localStorage.getItem("533clip-hotkeys") ?? "F8");
  const [recordingHotkey, setRecordingHotkey] = useState(() => localStorage.getItem("533clip-recording-hotkey") ?? "F9");
  const [replayDuration, setReplayDuration] = useState(() => Number(localStorage.getItem("533clip-replay-duration") ?? 45));
  const [capturingReplayHotkey, setCapturingReplayHotkey] = useState(false);
  const [capturingRecordingHotkey, setCapturingRecordingHotkey] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  // OBS WebSocket calls can take a couple seconds each when the connection
  // isn't actually reachable. Without this guard, a slow refresh() could
  // still be in flight when the next 10s interval tick fires, stacking up
  // overlapping WebSocket connection attempts and making the whole panel —
  // and the settings tab it lives in — feel like it's hanging.
  const refreshInFlight = useRef(false);
  const refresh = async () => {
    if (refreshInFlight.current) return status;
    refreshInFlight.current = true;
    try {
      const next = await invoke<ObsStatus>("obs_status");
      setStatus(next);
      if (next.websocketConnected) {
        void invoke<ObsStats>("obs_stats").then(setStats).catch(() => setStats(null));
      } else {
        setStats(null);
      }
      return next;
    } finally {
      refreshInFlight.current = false;
    }
  };

  useEffect(() => {
    void invoke<ObsSettings>("get_obs_settings")
      .then(setSettings)
      .catch((e) => setMessage(String(e)));
    void refresh().catch(() => undefined);
    if (settings.integrationMode === "managed" && localStorage.getItem("533clip-recording-hotkey-applied") !== "true") {
      void invoke<string[]>("set_obs_recording_hotkey", { hotkey: recordingHotkey })
        .then(() => localStorage.setItem("533clip-recording-hotkey-applied", "true"))
        .catch(() => undefined);
    }
    if (settings.integrationMode === "managed" && localStorage.getItem("533clip-replay-hotkey-applied") !== "true") {
      void invoke<string[]>("set_obs_replay_hotkey", { hotkey: replayHotkey })
        .then(() => localStorage.setItem("533clip-replay-hotkey-applied", "true"))
        .catch(() => undefined);
    }
    const timer = window.setInterval(() => {
      void refresh().catch(() => undefined);
    }, 10000);
    return () => window.clearInterval(timer);
  }, []);

  async function persistSettings(nextSettings: ObsSettings, nextPassword: string | null = null, quiet = false) {
    setBusy(true);
    if (!quiet) setMessage(null);
    try {
      const saved = await invoke<ObsSettings>("set_obs_settings", {
        settings: {
          websocketUrl: nextSettings.websocketUrl,
          integrationMode: nextSettings.integrationMode,
          password: nextPassword,
          autoLaunch: nextSettings.autoLaunch,
          startReplayOnLaunch: nextSettings.startReplayOnLaunch,
        },
      });
      setSettings(saved);
      if (nextPassword) setPassword("");
      if (!quiet) setMessage("OBS settings saved");
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function run(command: string, ok: string) {
    setBusy(true);
    setMessage(null);
    try {
      const next = await invoke<ObsStatus>(command);
      setStatus(next);
      setMessage(ok);
    } catch (e) {
      setMessage(String(e));
      void refresh().catch(() => undefined);
    } finally {
      setBusy(false);
    }
  }

  async function applyQualityPreset(preset: string) {
    setQualityPreset(preset);
    localStorage.setItem("533clip-obs-quality-preset", preset);
    setBusy(true);
    setMessage(null);
    try {
      const next = await invoke<ObsStatus>("obs_apply_quality_preset", { preset });
      setStatus(next);
      setMessage(`OBS ${preset} preset applied`);
    } catch (e) {
      setMessage(String(e));
      void refresh().catch(() => undefined);
    } finally {
      setBusy(false);
    }
  }

  function hotkeyFromEvent(e: KeyboardEvent<HTMLButtonElement>, kind: "replay" | "recording") {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      if (kind === "replay") setCapturingReplayHotkey(false);
      else setCapturingRecordingHotkey(false);
      return;
    }
    if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;
    const parts: string[] = [];
    if (e.ctrlKey) parts.push("Ctrl");
    if (e.shiftKey) parts.push("Shift");
    if (e.altKey) parts.push("Alt");
    if (e.metaKey) parts.push("Win");
    parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key.replace("Arrow", ""));
    const next = parts.join("+");
    if (kind === "replay") {
      setReplayHotkey(next);
      localStorage.setItem("533clip-hotkeys", next);
      setCapturingReplayHotkey(false);
    } else {
      setRecordingHotkey(next);
      localStorage.setItem("533clip-recording-hotkey", next);
      setCapturingRecordingHotkey(false);
    }
    setBusy(true);
    setMessage(null);
    const command = kind === "replay" ? "set_obs_replay_hotkey" : "set_obs_recording_hotkey";
    void invoke<string[]>(command, { hotkey: next })
      .then((paths) => setMessage(`OBS ${kind} hotkey set to ${next} in ${paths.length} profile(s)`))
      .catch((e) => setMessage(String(e)))
      .finally(() => setBusy(false));
  }

  function saveReplayDuration(seconds: number) {
    const next = Math.min(600, Math.max(5, seconds || 45));
    setReplayDuration(next);
    localStorage.setItem("533clip-replay-duration", String(next));
    setBusy(true);
    setMessage(null);
    void invoke<string[]>("set_obs_replay_duration", { seconds: next })
      .then((paths) => setMessage(`Replay length set to ${next}s in ${paths.length} profile(s)`))
      .catch((e) => setMessage(String(e)))
      .finally(() => setBusy(false));
  }

  function applyCaptureMode(mode: string) {
    setCaptureMode(mode);
    localStorage.setItem("533clip-obs-capture-mode", mode);
    setBusy(true);
    setMessage(null);
    void invoke<ObsStatus>("set_obs_capture_source_mode", { mode })
      .then((next) => {
        setStatus(next);
        setMessage(`OBS capture source set to ${mode === "game" ? "Game Capture" : "Display Capture"}`);
      })
      .catch((e) => setMessage(String(e)))
      .finally(() => setBusy(false));
  }

  return (
    <section className="settings-section obs-panel">
      <div className="obs-panel-head">
        <div>
          <h2>OBS Control</h2>
          <p>Auto-detects OBS and keeps replay controls synced.</p>
        </div>
      </div>

      <div className="obs-status-strip">
        <span className={status.installed ? "obs-pill good" : "obs-pill"}>OBS {status.installed ? "found" : "missing"}</span>
        <span className={status.running ? "obs-pill good" : "obs-pill"}>{status.running ? "running" : "closed"}</span>
        <span className={status.websocketConnected ? "obs-pill good" : "obs-pill"}>websocket {status.websocketConnected ? "online" : "off"}</span>
        <span className={status.replayBufferActive ? "obs-pill good" : "obs-pill"}>replay {status.replayBufferActive ? "active" : "off"}</span>
      </div>
      <p className="obs-path" title={status.path ?? undefined}>{status.path || "OBS path not detected"}</p>
      {stats && (
        <div className="obs-status-strip">
          <span className="obs-pill good">OBS CPU {stats.cpuUsage.toFixed(1)}%</span>
          <span className="obs-pill good">OBS RAM {stats.memoryUsageMb.toFixed(0)} MB</span>
          <span className="obs-pill good">FPS {stats.activeFps.toFixed(0)}</span>
          <span className={stats.renderSkippedFrames > 0 ? "obs-pill" : "obs-pill good"}>
            render skipped {skippedPercent(stats.renderSkippedFrames, stats.renderTotalFrames)}
          </span>
          <span className={stats.outputSkippedFrames > 0 ? "obs-pill" : "obs-pill good"}>
            encode skipped {skippedPercent(stats.outputSkippedFrames, stats.outputTotalFrames)}
          </span>
        </div>
      )}

      <div className="settings-form-grid obs-form">
        <label>
          <span>Integration</span>
          <select
            className="tag-input"
            value={settings.integrationMode}
            onChange={(e) => {
              const next = { ...settings, integrationMode: e.target.value };
              setSettings(next);
              void persistSettings(next, null, true);
            }}
          >
            <option value="manual">Manual folder watch</option>
            <option value="managed">Managed OBS</option>
            <option value="off">Off</option>
          </select>
        </label>
        <label>
          <span>WebSocket URL</span>
        <input
            className="tag-input"
            disabled={settings.integrationMode !== "managed"}
            value={settings.websocketUrl}
            onChange={(e) => setSettings({ ...settings, websocketUrl: e.target.value })}
            onBlur={() => void persistSettings(settings, null, true)}
          />
        </label>
        <label>
          <span>Password</span>
          <input
            className="tag-input"
            type="password"
            disabled={settings.integrationMode !== "managed"}
            placeholder={settings.passwordSet ? "saved" : "optional"}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            onBlur={() => {
              if (password) void persistSettings(settings, password, true);
            }}
          />
        </label>
        <label className="settings-check">
          <input
            type="checkbox"
            checked={settings.autoLaunch}
            disabled={settings.integrationMode !== "managed"}
            onChange={(e) => {
              const next = { ...settings, autoLaunch: e.target.checked };
              setSettings(next);
              void persistSettings(next, null, true);
            }}
          />
          <span>Launch OBS with 533clip</span>
        </label>
        <label className="settings-check">
          <input
            type="checkbox"
            checked={settings.startReplayOnLaunch}
            disabled={settings.integrationMode !== "managed"}
            onChange={(e) => {
              const next = { ...settings, startReplayOnLaunch: e.target.checked };
              setSettings(next);
              void persistSettings(next, null, true);
            }}
          />
          <span>Start replay buffer on launch</span>
        </label>
        <label>
          <span>Quality preset</span>
          <select
            className="tag-input"
            value={qualityPreset}
            disabled={busy || settings.integrationMode !== "managed"}
            onChange={(e) => void applyQualityPreset(e.target.value)}
          >
            <option value="high">High quality 864p</option>
            <option value="medium">Medium 720p</option>
            <option value="low">Low GPU 540p</option>
            <option value="potato">Potato mode</option>
            <option value="533">533</option>
          </select>
        </label>
        <label>
          <span>Capture source</span>
          <select
            className="tag-input"
            value={captureMode}
            disabled={busy || settings.integrationMode !== "managed"}
            onChange={(e) => applyCaptureMode(e.target.value)}
          >
            <option value="display">Display Capture</option>
            <option value="game">Game Capture</option>
          </select>
        </label>
        <label>
          <span>Clip hotkey</span>
          <button
            type="button"
            className={`hotkey-capture ${capturingReplayHotkey ? "is-capturing" : ""}`}
            disabled={busy || settings.integrationMode !== "managed"}
            onClick={() => setCapturingReplayHotkey(true)}
            onKeyDown={capturingReplayHotkey ? (e) => hotkeyFromEvent(e, "replay") : undefined}
          >
            {capturingReplayHotkey ? "Press button..." : replayHotkey}
          </button>
        </label>
        <label>
          <span>Clip length</span>
          <select
            className="tag-input"
            value={replayDuration}
            disabled={busy || settings.integrationMode !== "managed"}
            onChange={(e) => saveReplayDuration(Number(e.target.value))}
          >
            <option value={15}>15 seconds</option>
            <option value={30}>30 seconds</option>
            <option value={45}>45 seconds</option>
            <option value={60}>60 seconds</option>
            <option value={90}>90 seconds</option>
            <option value={120}>120 seconds</option>
            <option value={180}>180 seconds</option>
          </select>
        </label>
        <label>
          <span>Recording hotkey</span>
          <button
            type="button"
            className={`hotkey-capture ${capturingRecordingHotkey ? "is-capturing" : ""}`}
            disabled={busy || settings.integrationMode !== "managed"}
            onClick={() => setCapturingRecordingHotkey(true)}
            onKeyDown={capturingRecordingHotkey ? (e) => hotkeyFromEvent(e, "recording") : undefined}
          >
            {capturingRecordingHotkey ? "Press button..." : recordingHotkey}
          </button>
        </label>
      </div>

      <div className="settings-actions obs-actions">
        <button type="button" className="btn ghost" disabled={busy || settings.integrationMode !== "managed"} onClick={() => void run("obs_launch", "OBS launched")}>
          Launch
        </button>
        <button type="button" className="btn ghost" disabled={busy || settings.integrationMode !== "managed"} onClick={() => void run("obs_save_replay_buffer", "Replay saved")}>
          Save replay
        </button>
        <button type="button" className="btn ghost" disabled={busy || settings.integrationMode !== "managed"} onClick={() => void run("obs_stop_replay_buffer", "Replay stopped")}>
          Stop replay
        </button>
        <button type="button" className="btn ghost" disabled={busy || settings.integrationMode !== "managed"} onClick={() => void run("obs_toggle_recording", "Recording toggled")}>
          Toggle recording
        </button>
      </div>

      {(message || status.error) && <p className="settings-status">{message || status.error}</p>}
    </section>
  );
}
