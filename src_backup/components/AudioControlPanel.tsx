import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ObsAudioInput {
  name: string;
  kind: string;
  muted: boolean;
  volumeMul: number;
}

export function AudioControlPanel() {
  const [inputs, setInputs] = useState<ObsAudioInput[]>([]);
  const [micName, setMicName] = useState(() => localStorage.getItem("533clip-mic-source") ?? "");
  const [playbackName, setPlaybackName] = useState(() => localStorage.getItem("533clip-playback-source") ?? "");
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const mic = useMemo(() => inputs.find((input) => input.name === micName), [inputs, micName]);
  const playback = useMemo(
    () => inputs.find((input) => input.name === playbackName),
    [inputs, playbackName],
  );

  async function refresh() {
    setBusy(true);
    try {
      const next = await invoke<ObsAudioInput[]>("obs_audio_inputs");
      setInputs(next);
      if (!micName) {
        const guessed = next.find((input) => /mic|aux|input/i.test(`${input.name} ${input.kind}`));
        if (guessed) setMicName(guessed.name);
      }
      if (!playbackName) {
        const guessed = next.find((input) => /desktop|output|wasapi_output/i.test(`${input.name} ${input.kind}`));
        if (guessed) setPlaybackName(guessed.name);
      }
      setStatus(null);
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    localStorage.setItem("533clip-mic-source", micName);
  }, [micName]);

  useEffect(() => {
    localStorage.setItem("533clip-playback-source", playbackName);
  }, [playbackName]);

  async function setMicMute(muted: boolean) {
    if (!micName) return;
    setBusy(true);
    try {
      setInputs(await invoke<ObsAudioInput[]>("obs_set_audio_mute", { inputName: micName, muted }));
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function setMicVolume(volumeMul: number) {
    if (!micName) return;
    setInputs((items) => items.map((item) => item.name === micName ? { ...item, volumeMul } : item));
    try {
      setInputs(await invoke<ObsAudioInput[]>("obs_set_audio_volume", { inputName: micName, volumeMul }));
    } catch (e) {
      setStatus(String(e));
    }
  }

  return (
    <section className="settings-section clean-settings-panel audio-panel">
      <div className="obs-panel-head">
        <div>
          <h2>Audio Sources</h2>
          <p>Control OBS mic and desktop audio from 533clip.</p>
        </div>
        <button type="button" className="btn ghost small" disabled={busy} onClick={() => void refresh()}>
          Refresh
        </button>
      </div>

      <div className="settings-form-grid">
        <label>
          <span>Mic source</span>
          <select className="tag-input" value={micName} onChange={(e) => setMicName(e.target.value)}>
            <option value="">Pick mic source</option>
            {inputs.map((input) => (
              <option key={input.name} value={input.name}>{input.name}</option>
            ))}
          </select>
        </label>
        <label>
          <span>Desktop audio source</span>
          <select className="tag-input" value={playbackName} onChange={(e) => setPlaybackName(e.target.value)}>
            <option value="">Pick desktop source</option>
            {inputs.map((input) => (
              <option key={input.name} value={input.name}>{input.name}</option>
            ))}
          </select>
        </label>
      </div>

      <div className="audio-control-row">
        <button
          type="button"
          className={`btn ghost ${mic?.muted ? "danger-lite" : ""}`}
          disabled={!micName || busy}
          onClick={() => void setMicMute(!mic?.muted)}
        >
          {mic?.muted ? "Unmute mic" : "Mute mic"}
        </button>
        <label className="audio-slider">
          <span>Mic gain</span>
          <input
            type="range"
            min={0}
            max={400}
            step={5}
            value={Math.round((mic?.volumeMul ?? 1) * 100)}
            disabled={!micName}
            onChange={(e) => void setMicVolume(Number(e.target.value) / 100)}
          />
          <strong>{Math.round((mic?.volumeMul ?? 1) * 100)}%</strong>
        </label>
      </div>

      {playback && <p className="settings-status">Playback source: {playback.name}</p>}
      {status && <p className="settings-status">{status}</p>}
    </section>
  );
}
