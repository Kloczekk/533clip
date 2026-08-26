import { useCallback, useEffect, useRef, useState, type CSSProperties, type WheelEvent } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask } from "@tauri-apps/plugin-dialog";
import { TrimTimeline } from "./TrimTimeline";
import { IconButton } from "./IconButton";
import {
  IconBack,
  IconForward,
  IconFullscreen,
  IconFullscreenExit,
  IconPause,
  IconPlay,
  IconRewind,
  IconScissors,
  IconSkipBack,
  IconSkipForward,
  IconStar,
  IconTrash,
} from "./Icons";
import { clipDisplayName } from "../utils/clipDisplay";
import { gameNameForClip } from "../utils/gameName";
import type { Clip } from "../types/clip";

const EMPTY_ARRAY: number[] = [];

interface PlayerModalProps {
  clip: Clip;
  onClose: () => void;
  onUpdate: (clip: Clip) => void;
  onDeleted: (id: string) => void;
  allTags: string[];
  onTagsChange: () => void;
  hasPrevious: boolean;
  hasNext: boolean;
  onPrevious: () => void;
  onNext: () => void;
  playerTheme: string;
  suspended?: boolean;
}

export function PlayerModal({
  clip,
  onClose,
  onUpdate,
  onDeleted,
  allTags,
  onTagsChange,
  hasPrevious,
  hasNext,
  onPrevious,
  onNext,
  playerTheme,
  suspended = false,
}: PlayerModalProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const titleInputRef = useRef<HTMLInputElement>(null);
  const rafRef = useRef<number>(0);

  const [videoDuration, setVideoDuration] = useState(clip.duration ?? 0);
  const [start, setStart] = useState(0);
  const [end, setEnd] = useState(clip.duration ?? 0);
  const [current, setCurrent] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [trimming, setTrimming] = useState(false);
  const [sharing, setSharing] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [trimChoiceOpen, setTrimChoiceOpen] = useState(false);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState(clipDisplayName(clip));
  const [gameDraft, setGameDraft] = useState(gameNameForClip(clip));
  const [tagDraft, setTagDraft] = useState("");
  const [shareProvider, setShareProvider] = useState("r2");
  const [volume, setVolume] = useState(() => {
    const saved = localStorage.getItem("533clip-volume");
    const n = saved ? Number(saved) : 1;
    return Number.isFinite(n) ? Math.min(1, Math.max(0, n)) : 1;
  });
  const [playbackRate, setPlaybackRate] = useState(() => {
    const saved = localStorage.getItem("533clip-playback-rate");
    const n = saved ? Number(saved) : 1;
    return Number.isFinite(n) ? n : 1;
  });
  const [videoZoom, setVideoZoom] = useState(() => {
    const saved = localStorage.getItem("533clip-player-zoom");
    const n = saved ? Number(saved) : 1;
    return Number.isFinite(n) ? Math.min(4, Math.max(1, n)) : 1;
  });
  const [zoomOrigin, setZoomOrigin] = useState({ x: 50, y: 50 });
  const [appFullscreen, setAppFullscreen] = useState(false);

  // ffprobe's duration (clip.duration) is the source of truth — it's what the
  // library grid displays and what queue_trim_clip validates against on the
  // backend. The browser's own <video> duration detection is well known to
  // misjudge raw/un-indexed MKV (exactly what OBS's un-remuxed replay-buffer
  // output is), so preferring it here made the player disagree with the rest
  // of the app about how long a clip actually is. Only fall back to it when
  // ffprobe hasn't reported a duration yet.
  const duration = clip.duration && clip.duration > 0 ? clip.duration : videoDuration;
  const videoSrc = convertFileSrc(clip.filePath);
  // `?? []` would allocate a new array every render, which sits in the
  // keydown effect's dependency array below and was rebinding the listener
  // on every render (60x/sec during playback) instead of only when the
  // clip's own peaks/waveform actually change.
  const peaks = clip.audioPeaks ?? EMPTY_ARRAY;
  const waveform = clip.waveform ?? EMPTY_ARRAY;

  useEffect(() => {
    setTitleDraft(clipDisplayName(clip));
    setGameDraft(gameNameForClip(clip));
  }, [clip.id, clip.displayName, clip.fileName, clip.gameName]);

  useEffect(() => {
    const d = clip.duration ?? 0;
    if (d > 0) {
      setVideoDuration(d);
      setStart(0);
      setEnd(d);
      setCurrent(0);
    }
  }, [clip.id, clip.duration]);

  useEffect(() => {
    if (!suspended) return;
    const v = videoRef.current;
    if (v) v.pause();
    cancelAnimationFrame(rafRef.current);
    setPlaying(false);
  }, [suspended]);

  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;
    v.volume = volume;
    v.muted = volume === 0;
    v.playbackRate = playbackRate;
  }, [volume, playbackRate, clip.id]);

  useEffect(() => {
    localStorage.setItem("533clip-volume", String(volume));
  }, [volume]);

  useEffect(() => {
    localStorage.setItem("533clip-playback-rate", String(playbackRate));
  }, [playbackRate]);

  useEffect(() => {
    localStorage.setItem("533clip-player-zoom", String(videoZoom));
  }, [videoZoom]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const isTyping =
        target?.tagName === "INPUT" ||
        target?.tagName === "TEXTAREA" ||
        target?.isContentEditable;
      if (isTyping) return;
      if (e.key === "Escape") {
        if (editingTitle) {
          setEditingTitle(false);
          setTitleDraft(clipDisplayName(clip));
        } else {
          closePlayer();
        }
      } else if (e.key === "ArrowLeft" && hasPrevious) {
        e.preventDefault();
        onPrevious();
      } else if (e.key === "ArrowRight" && hasNext) {
        e.preventDefault();
        onNext();
      } else if (e.key === " " || e.key === "k") {
        e.preventDefault();
        togglePlay();
      } else if (e.key === "j" || e.key === "l") {
        // Read live currentTime rather than closing over `current` (updates
        // 60x/second during playback while this effect only rebinds on the
        // coarse deps below — a stale `current` would make seeks drift).
        e.preventDefault();
        const liveCurrent = videoRef.current?.currentTime ?? current;
        seekWithinTrim(liveCurrent + (e.key === "j" ? -5 : 5));
      } else if (e.key === "Home") {
        e.preventDefault();
        seekWithinTrim(start);
      } else if (e.key === "End") {
        e.preventDefault();
        seekWithinTrim(end);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setVolume((v) => Math.min(1, v + 0.05));
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setVolume((v) => Math.max(0, v - 0.05));
      } else if (e.key === "f") {
        e.preventDefault();
        void handleFavorite();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [editingTitle, clip, hasPrevious, hasNext, onPrevious, onNext, duration, peaks, start, end]);

  useEffect(() => {
    // The library grid stays mounted behind this full-screen overlay, so
    // whichever clip card was clicked to open the player keeps DOM focus.
    // Its own onKeyDown treats Space/Enter as "open this clip" — left
    // focused, pressing Space to play here re-fires that and snaps the
    // player back to the original clip instead of the one just switched to.
    (document.activeElement as HTMLElement | null)?.blur();
  }, []);

  useEffect(() => {
    const win = getCurrentWindow();
    void win.isFullscreen().then(setAppFullscreen).catch(() => undefined);
    void invoke<{ provider: string }>("get_r2_settings")
      .then((settings) => setShareProvider(settings.provider || "r2"))
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    const unlistenComplete = listen<{ sourceClipId: string }>("trim://complete", (e) => {
      if (e.payload.sourceClipId !== clip.id) return;
      setTrimming(false);
      setToast("Trim saved as new clip");
    });
    const unlistenFailed = listen<{ sourceClipId: string; error?: string }>(
      "trim://failed",
      (e) => {
        if (e.payload.sourceClipId !== clip.id) return;
        setTrimming(false);
        setToast(e.payload.error ?? "Trim failed");
      },
    );
    return () => {
      void unlistenComplete.then((fn) => fn());
      void unlistenFailed.then((fn) => fn());
    };
  }, [clip.id]);

  const seekTo = useCallback(
    (time: number) => {
      const v = videoRef.current;
      if (!v || duration <= 0) return;
      const t = Math.min(Math.max(time, 0), duration);
      v.currentTime = t;
      setCurrent(t);
    },
    [duration],
  );

  /** Like seekTo, but clamped to the current trim selection — used for every
   * seek control (click, transport buttons, keyboard) except the ones that
   * are actively defining a *new* trim range (dragging a handle, picking a
   * highlight window), so you can never land on a frame that's about to be
   * cut away. */
  const seekWithinTrim = useCallback(
    (time: number) => {
      seekTo(Math.min(Math.max(time, start), end));
    },
    [seekTo, start, end],
  );

  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;

    const tick = () => {
      if (v && !v.paused) {
        if (duration > 0 && v.currentTime >= end) {
          v.pause();
          v.currentTime = end;
          setCurrent(end);
          return;
        }
        setCurrent(v.currentTime);
        rafRef.current = requestAnimationFrame(tick);
      }
    };

    const onPlay = () => {
      setPlaying(true);
      cancelAnimationFrame(rafRef.current);
      rafRef.current = requestAnimationFrame(tick);
    };
    const onPause = () => {
      setPlaying(false);
      cancelAnimationFrame(rafRef.current);
      setCurrent(v.currentTime);
    };
    const onSeeked = () => setCurrent(v.currentTime);

    v.addEventListener("play", onPlay);
    v.addEventListener("pause", onPause);
    v.addEventListener("seeked", onSeeked);

    return () => {
      cancelAnimationFrame(rafRef.current);
      v.removeEventListener("play", onPlay);
      v.removeEventListener("pause", onPause);
      v.removeEventListener("seeked", onSeeked);
    };
  }, [clip.id, duration, start, end]);

  useEffect(() => {
    const v = videoRef.current;
    if (!v || duration <= 0 || playing) return;
    if (v.currentTime < start || v.currentTime > end) {
      v.currentTime = start;
      setCurrent(start);
    }
  }, [start, end, duration, playing]);

  const togglePlay = () => {
    const v = videoRef.current;
    if (!v) return;
    if (v.paused) {
      if (duration > 0 && (v.currentTime < start || v.currentTime >= end - 0.05)) {
        v.currentTime = start;
        setCurrent(start);
      }
      void v.play();
    } else {
      v.pause();
    }
  };

  const skip = (delta: number) => seekWithinTrim(current + delta);

  const handleStartChange = useCallback(
    (time: number) => {
      setStart(time);
      const v = videoRef.current;
      if (v) {
        v.pause();
        v.currentTime = time;
        setCurrent(time);
      } else {
        setCurrent(time);
      }
    },
    [],
  );

  const handleEndChange = useCallback(
    (time: number) => {
      setEnd(time);
      const v = videoRef.current;
      if (v) {
        v.pause();
        v.currentTime = start;
        setCurrent(start);
      } else {
        setCurrent(start);
      }
    },
    [start],
  );

  const onLoadedMetadata = () => {
    const v = videoRef.current;
    if (!v) return;
    const d = v.duration;
    if (!d || !Number.isFinite(d) || d <= 0) return;
    setVideoDuration(d);
    // Only reset the trim range to the browser's own duration guess when
    // ffprobe hasn't given us a real one yet — otherwise this can silently
    // shrink/grow the trim end away from the accurate, already-seeded value.
    if (!clip.duration || clip.duration <= 0) {
      setEnd(d);
      setStart(0);
    }
  };

  async function saveTitle() {
    const name = titleDraft.trim();
    if (!name || name === clipDisplayName(clip)) {
      setEditingTitle(false);
      setTitleDraft(clipDisplayName(clip));
      return;
    }
    try {
      const updated = await invoke<Clip>("rename_clip", { id: clip.id, displayName: name });
      onUpdate(updated);
      setEditingTitle(false);
    } catch (e) {
      setToast(String(e));
    }
  }

  async function handleFavorite() {
    const updated = await invoke<Clip>("toggle_favorite", { id: clip.id });
    onUpdate(updated);
  }

  async function saveGameName() {
    const gameName = gameDraft.trim();
    if (!gameName || gameName === gameNameForClip(clip)) return;
    try {
      // Only this one clip's own game_name should change. setGameAlias
      // would remap the clip's *current* displayed group name (which can be
      // a shared bucket like "Ungrouped" or an existing populated game) to
      // the new name for every clip in that group — not what a single-clip
      // rename here should do.
      const updated = await invoke<Clip>("set_clip_game", { id: clip.id, gameName });
      onUpdate(updated);
    } catch (e) {
      setToast(String(e));
    }
  }

  async function handleAddTag(raw: string) {
    const t = raw.trim();
    if (!t) return;
    try {
      const updated = await invoke<Clip>("add_clip_tag", { id: clip.id, tag: t });
      onUpdate(updated);
      onTagsChange();
      setTagDraft("");
    } catch (e) {
      setToast(String(e));
    }
  }

  async function handleRemoveTag(tag: string) {
    const updated = await invoke<Clip>("remove_clip_tag", { id: clip.id, tag });
    onUpdate(updated);
    onTagsChange();
  }

  async function handleDelete() {
    const ok = await ask("Delete this clip permanently? The video file will be removed from disk.", {
      title: "Delete clip",
      kind: "warning",
    });
    if (!ok) return;
    await invoke("delete_clip", { id: clip.id });
    onDeleted(clip.id);
    closePlayer();
  }

  async function handleTrim(deleteOriginal: boolean) {
    setTrimming(true);
    setTrimChoiceOpen(false);
    setToast(null);
    try {
      await invoke("queue_trim_clip", {
        clipId: clip.id,
        startSecs: start,
        endSecs: end,
        deleteOriginal,
      });
      setToast("Trimming…");
    } catch (e) {
      setTrimming(false);
      setToast(String(e));
    }
  }

  async function handleExportDiscord() {
    setSharing(true);
    setToast("Creating Discord export...");
    try {
      const result = await invoke<{ outputPath: string }>("export_clip_for_discord", {
        id: clip.id,
      });
      try {
        await invoke("copy_file_to_clipboard", { path: result.outputPath });
        setToast("Copied — paste it (Ctrl+V) straight into Discord");
      } catch {
        // Clipboard copy is Windows-only / can fail; fall back to just
        // showing the file so it can still be dragged in manually.
        await invoke("reveal_path", { path: result.outputPath });
        setToast("Discord export ready");
      }
    } catch (e) {
      setToast(String(e));
    } finally {
      setSharing(false);
    }
  }

  async function handleUploadR2() {
    setSharing(true);
    setToast(`Creating ${shareProvider === "b2" ? "B2" : "R2"} link...`);
    try {
      const result = await invoke<{ outputPath: string; url?: string; clip?: Clip }>("upload_clip_to_r2", {
        id: clip.id,
      });
      if (result.clip) onUpdate(result.clip);
      if (result.url) {
        await navigator.clipboard.writeText(result.url);
        setToast(`${shareProvider === "b2" ? "B2" : "R2"} link copied`);
      } else {
        setToast(`Uploaded: ${result.outputPath}`);
      }
    } catch (e) {
      setToast(String(e));
    } finally {
      setSharing(false);
    }
  }

  function handleVideoWheel(e: WheelEvent<HTMLVideoElement>) {
    e.preventDefault();
    const rect = e.currentTarget.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 100;
    const y = ((e.clientY - rect.top) / rect.height) * 100;
    const direction = e.deltaY < 0 ? 1 : -1;
    setZoomOrigin({
      x: Math.min(100, Math.max(0, x)),
      y: Math.min(100, Math.max(0, y)),
    });
    setVideoZoom((value) => {
      const next = value + direction * 0.18;
      return Math.min(4, Math.max(1, next));
    });
  }

  async function toggleAppFullscreen() {
    const applied = await invoke<boolean>("toggle_app_fullscreen");
    setAppFullscreen(applied);
    if (applied) {
      // Video zoom persists across clips/sessions via localStorage by
      // design, but a leftover zoom from a previous scroll-wheel adjustment
      // makes the video look cropped/offset in fullscreen with no window
      // geometry to blame — reset it so fullscreen always starts unzoomed.
      setVideoZoom(1);
      setZoomOrigin({ x: 50, y: 50 });
    }
  }

  function closePlayer() {
    const v = videoRef.current;
    if (v) {
      v.pause();
      v.removeAttribute("src");
      v.load();
    }
    onClose();
  }

  return (
    <div className={`player-fullscreen player-theme-${playerTheme}`} role="dialog" aria-modal="true">
      <header className="player-topbar">
        <IconButton label="Back to library" onClick={closePlayer}>
          <IconBack size={22} />
        </IconButton>

        {editingTitle ? (
          <input
            ref={titleInputRef}
            className="player-title-input"
            value={titleDraft}
            onChange={(e) => setTitleDraft(e.target.value)}
            onBlur={() => void saveTitle()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void saveTitle();
            }}
            autoFocus
          />
        ) : (
          <button
            type="button"
            className="player-title-btn"
            onClick={() => {
              setEditingTitle(true);
              setTimeout(() => titleInputRef.current?.select(), 0);
            }}
            title="Click to rename"
          >
            {clipDisplayName(clip)}
          </button>
        )}

        <div className="player-topbar-spacer" />
        {toast && <span className="player-toast">{toast}</span>}
        <IconButton label={appFullscreen ? "Exit fullscreen" : "Fullscreen"} onClick={() => void toggleAppFullscreen()}>
          {appFullscreen ? <IconFullscreenExit size={20} /> : <IconFullscreen size={20} />}
        </IconButton>
      </header>

      <div className="player-tags-bar">
        <div className="player-game-edit">
          <span>Game</span>
          <input
            type="text"
            className="tag-input"
            value={gameDraft}
            onChange={(e) => setGameDraft(e.target.value)}
            onBlur={() => void saveGameName()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void saveGameName();
            }}
          />
        </div>
        <div className="player-tags-list">
          {clip.tags.map((tag) => (
            <button
              key={tag}
              type="button"
              className="tag-chip small removable"
              onClick={() => void handleRemoveTag(tag)}
              title="Remove tag"
            >
              #{tag} ×
            </button>
          ))}
          {allTags
            .filter((t) => !clip.tags.includes(t))
            .slice(0, 12)
            .map((tag) => (
              <button
                key={tag}
                type="button"
                className="tag-chip small"
                onClick={() => void handleAddTag(tag)}
              >
                + #{tag}
              </button>
            ))}
        </div>
        <div className="player-tag-add">
          <input
            type="text"
            className="tag-input"
            placeholder="Add tag…"
            value={tagDraft}
            onChange={(e) => setTagDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleAddTag(tagDraft);
            }}
          />
        </div>
      </div>

      <div className="player-stage">
        <button
          type="button"
          className="player-side-nav player-side-nav-left"
          disabled={!hasPrevious}
          aria-label="Previous clip"
          onClick={onPrevious}
        >
          <IconBack size={34} />
        </button>
        <video
          ref={videoRef}
          src={videoSrc}
          className="player-video-full"
          style={{
            "--player-zoom": videoZoom,
            "--player-zoom-x": `${zoomOrigin.x}%`,
            "--player-zoom-y": `${zoomOrigin.y}%`,
          } as CSSProperties}
          playsInline
          onClick={togglePlay}
          onWheel={handleVideoWheel}
          onLoadedMetadata={onLoadedMetadata}
          onDoubleClick={togglePlay}
        />
        <button
          type="button"
          className="player-side-nav player-side-nav-right"
          disabled={!hasNext}
          aria-label="Next clip"
          onClick={onNext}
        >
          <IconForward size={34} />
        </button>
      </div>

      <div className="player-bottom-hotzone" aria-hidden="true" />
      <footer className="player-dock">
        <div className="player-transport">
          <IconButton label="Skip to start" onClick={() => seekWithinTrim(start)}>
            <IconSkipBack />
          </IconButton>
          <IconButton label="Back 5 seconds" onClick={() => skip(-5)}>
            <IconRewind />
          </IconButton>
          <IconButton label={playing ? "Pause" : "Play"} variant="accent" onClick={togglePlay}>
            {playing ? <IconPause size={24} /> : <IconPlay size={24} />}
          </IconButton>
          <IconButton label="Forward 5 seconds" onClick={() => skip(5)}>
            <IconForward />
          </IconButton>
          <IconButton label="Skip to end" onClick={() => seekWithinTrim(end)}>
            <IconSkipForward />
          </IconButton>
        </div>

        <div className="player-playback-controls">
          <label className="playback-control">
            <span className="playback-label">Volume</span>
            <input
              type="range"
              className="playback-slider"
              min={0}
              max={1}
              step={0.05}
              value={volume}
              onChange={(e) => setVolume(Number(e.target.value))}
              aria-valuetext={`${Math.round(volume * 100)}%`}
            />
            <span className="playback-value">{Math.round(volume * 100)}%</span>
          </label>
          <label className="playback-control">
            <span className="playback-label">Speed</span>
            <input
              type="range"
              className="playback-slider"
              min={0.25}
              max={1}
              step={0.25}
              value={playbackRate}
              onChange={(e) => setPlaybackRate(Number(e.target.value))}
              aria-valuetext={`${playbackRate}×`}
            />
            <span className="playback-value">{playbackRate}×</span>
          </label>
          {videoZoom > 1.01 && (
            <button type="button" className="btn ghost small" onClick={() => setVideoZoom(1)}>
              Reset zoom
            </button>
          )}
        </div>

        {duration > 0 && (
          <div className="player-timeline-wrap">
            <TrimTimeline
              duration={duration}
              start={start}
              end={end}
              current={current}
              peaks={peaks}
              waveform={waveform}
              onStartChange={handleStartChange}
              onEndChange={handleEndChange}
              onSeek={seekWithinTrim}
            />
          </div>
        )}

        <div className="player-toolbar">
          <button
            type="button"
            className="btn ghost small"
            onClick={() => void invoke("reveal_path", { path: clip.filePath }).catch((e) => setToast(String(e)))}
          >
            Show in folder
          </button>
          <button
            type="button"
            className="btn ghost small"
            disabled={sharing || clip.status !== "ready"}
            onClick={() => void handleExportDiscord()}
          >
            {sharing ? "Working..." : "Discord Export"}
          </button>
          <button
            type="button"
            className="btn ghost small"
            disabled={sharing || clip.status !== "ready"}
            onClick={() => void handleUploadR2()}
          >
            {sharing ? "Creating link..." : `Upload ${shareProvider === "b2" ? "B2" : "R2"}`}
          </button>
          {clip.shareUrl && (
            <button
              type="button"
              className="btn ghost small"
              onClick={() => {
                void navigator.clipboard.writeText(clip.shareUrl!);
                setToast("Link copied");
              }}
            >
              Copy Link
            </button>
          )}
          <IconButton
            label="Apply trim"
            variant="accent"
            disabled={trimming || clip.status !== "ready" || duration <= 0}
            onClick={() => {
              if (trimming || end <= start + 0.1 || duration <= 0) return;
              setTrimChoiceOpen(true);
            }}
          >
            <IconScissors />
          </IconButton>
          <IconButton
            label={clip.isFavorite ? "Remove favorite" : "Add favorite"}
            active={clip.isFavorite}
            onClick={() => void handleFavorite()}
          >
            <IconStar filled={clip.isFavorite} />
          </IconButton>
          <IconButton label="Delete clip" variant="danger" onClick={() => void handleDelete()}>
            <IconTrash />
          </IconButton>
        </div>
      </footer>

      {trimChoiceOpen && (
        <div className="trim-choice-backdrop" role="dialog" aria-modal="true">
          <div className="trim-choice">
            <h3>Save trimmed clip</h3>
            <p>Keep the original full clip or delete it after the trimmed copy is saved.</p>
            <div className="trim-choice-actions">
              <button type="button" className="btn ghost" onClick={() => void handleTrim(false)}>
                Keep original
              </button>
              <button type="button" className="btn ghost danger-lite" onClick={() => void handleTrim(true)}>
                Delete original
              </button>
            </div>
            <button type="button" className="btn-link" onClick={() => setTrimChoiceOpen(false)}>
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
