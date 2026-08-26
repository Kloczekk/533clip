import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface CapturePayload {
  gameName?: string;
  fileName?: string;
  displayName?: string;
  kind?: "ready" | "saved" | "recording-start" | "recording-stop";
}

const HIDE_MS = 2400;

function playPresetSound(kind: string) {
  if (kind === "off") return;
  if (kind === "custom") {
    const file = localStorage.getItem("533clip-custom-clip-sound");
    if (file) {
      const audio = new Audio(convertFileSrc(file));
      audio.volume = 0.75;
      void audio.play().catch(() => undefined);
    }
    return;
  }
  const ctx = new AudioContext();
  const gain = ctx.createGain();
  gain.connect(ctx.destination);
  gain.gain.setValueAtTime(0.0001, ctx.currentTime);
  gain.gain.exponentialRampToValueAtTime(kind === "soft" ? 0.045 : kind === "pop" ? 0.09 : 0.075, ctx.currentTime + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + (kind === "soft" ? 0.42 : 0.2));

  const tones =
    kind === "pop"
      ? [190, 115]
      : kind === "soft"
        ? [440, 554, 659]
        : [660, 880];
  tones.forEach((freq, i) => {
    const osc = ctx.createOscillator();
    osc.type = kind === "pop" ? "square" : kind === "soft" ? "triangle" : "sine";
    osc.frequency.value = freq;
    osc.connect(gain);
    osc.start(ctx.currentTime + i * (kind === "soft" ? 0.07 : 0.035));
    osc.stop(ctx.currentTime + (kind === "soft" ? 0.34 : 0.14) + i * 0.04);
  });
  window.setTimeout(() => void ctx.close(), 700);
}

export function CaptureOverlay() {
  const [clip, setClip] = useState<CapturePayload | null>(null);
  // Separate from `clip`: `clip` is kept populated through the fade-out (so
  // the text never flashes to the "nothing selected" fallback strings while
  // the card is animating away), while `visible` alone drives the CSS class.
  const [visible, setVisible] = useState(false);
  // Bumped on every event and used as the card's `key`, forcing React to
  // remount it so the pop-in animation replays even when a new notification
  // arrives while the previous one is still showing (back-to-back "ready"
  // then "saved" events otherwise never retrigger the CSS animation, since
  // the wrapper's "is-visible" class never actually toggles off in between).
  const [popKey, setPopKey] = useState(0);
  const [style, setStyle] = useState(() => localStorage.getItem("533clip-notification-style") ?? "outplayed");
  const [theme, setTheme] = useState(() => localStorage.getItem("533clip-notification-theme") ?? "violet");
  const hideTimer = useRef<number | null>(null);

  useEffect(() => {
    const win = getCurrentWindow();
    const show = async (payload: CapturePayload) => {
      setStyle(localStorage.getItem("533clip-notification-style") ?? "outplayed");
      setTheme(localStorage.getItem("533clip-notification-theme") ?? "violet");
      if (hideTimer.current != null) {
        window.clearTimeout(hideTimer.current);
      }
      setClip(payload);
      setVisible(true);
      setPopKey((k) => k + 1);
      if (payload.kind === "saved") {
        playPresetSound(localStorage.getItem("533clip-clip-sound") ?? "chime");
      }
      void win.show();
      hideTimer.current = window.setTimeout(() => {
        setVisible(false);
      }, HIDE_MS);
    };
    const unlistenReady = listen<CapturePayload>("game://ready", (event) => {
      void show({ ...event.payload, kind: "ready" });
    });
    const unlistenSaved = listen<CapturePayload>("clip://saved-overlay", (event) => {
      void show({ ...event.payload, kind: "saved" });
    });
    const unlistenRecording = listen<{ active: boolean }>("recording://state", (event) => {
      void show({ kind: event.payload.active ? "recording-start" : "recording-stop" });
    });

    return () => {
      if (hideTimer.current != null) {
        window.clearTimeout(hideTimer.current);
      }
      void unlistenReady.then((fn) => fn());
      void unlistenSaved.then((fn) => fn());
      void unlistenRecording.then((fn) => fn());
    };
  }, []);

  return (
    <div className={`capture-overlay theme-${theme} capture-overlay-${style} ${visible ? "is-visible" : ""}`}>
      <div key={popKey} className="capture-overlay-card">
        <div className="capture-overlay-mark">533</div>
        <div>
          <p className="capture-overlay-title">
            {clip?.kind === "saved"
              ? `Saved ${clip?.displayName?.trim() || clip?.gameName?.trim() || "clip"}`
              : clip?.kind === "recording-start"
                ? "Recording started"
                : clip?.kind === "recording-stop"
                  ? "Recording stopped"
              : `Ready to clip ${clip?.gameName?.trim() || "game"}`}
          </p>
          <p className="capture-overlay-file">
            {clip?.kind === "saved"
              ? "Clip added to 533clip"
              : clip?.kind?.startsWith("recording")
                ? "OBS recording toggled"
                : "533clip is watching"}
          </p>
        </div>
      </div>
    </div>
  );
}
