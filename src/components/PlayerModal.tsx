import { useCallback, useEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { TrimTimeline } from "./TrimTimeline";
import { IconButton } from "./IconButton";
import {
  IconBack,
  IconForward,
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
import type { Clip } from "../types/clip";

interface PlayerModalProps {
  clip: Clip;
  onClose: () => void;
  onUpdate: (clip: Clip) => void;
  onDeleted: (id: string) => void;
  allTags: string[];
  onTagsChange: () => void;
}

export function PlayerModal({
  clip,
  onClose,
  onUpdate,
  onDeleted,
  allTags,
  onTagsChange,
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
  const [toast, setToast] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState(clipDisplayName(clip));
  const [tagDraft, setTagDraft] = useState("");
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

  const duration = videoDuration > 0 ? videoDuration : clip.duration ?? 0;
  const videoSrc = convertFileSrc(clip.filePath);

  useEffect(() => {
    setTitleDraft(clipDisplayName(clip));
  }, [clip.id, clip.displayName, clip.fileName]);

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
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (editingTitle) {
          setEditingTitle(false);
          setTitleDraft(clipDisplayName(clip));
        } else {
          onClose();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, editingTitle, clip]);

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

  useEffect(() => {
    const v = videoRef.current;
    if (!v) return;

    const tick = () => {
      if (v && !v.paused) {
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
  }, [clip.id, duration]);

  const togglePlay = () => {
    const v = videoRef.current;
    if (!v) return;
    if (v.paused) void v.play();
    else v.pause();
  };

  const skip = (delta: number) => seekTo(current + delta);

  const onLoadedMetadata = () => {
    const v = videoRef.current;
    if (!v) return;
    const d = v.duration;
    if (d && Number.isFinite(d) && d > 0) {
      setVideoDuration(d);
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
    onClose();
  }

  async function handleTrim() {
    if (trimming || end <= start + 0.1 || duration <= 0) return;
    setTrimming(true);
    setToast(null);
    try {
      await invoke("queue_trim_clip", {
        clipId: clip.id,
        startSecs: start,
        endSecs: end,
      });
      setToast("Trimming…");
    } catch (e) {
      setTrimming(false);
      setToast(String(e));
    }
  }

  return (
    <div className="player-fullscreen" role="dialog" aria-modal="true">
      <header className="player-topbar">
        <IconButton label="Back to library" onClick={onClose}>
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
      </header>

      <div className="player-tags-bar">
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
        <video
          ref={videoRef}
          src={videoSrc}
          className="player-video-full"
          playsInline
          onClick={togglePlay}
          onLoadedMetadata={onLoadedMetadata}
          onDoubleClick={togglePlay}
        />
      </div>

      <footer className="player-dock">
        <div className="player-transport">
          <IconButton label="Skip to start" onClick={() => seekTo(0)}>
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
          <IconButton label="Skip to end" onClick={() => seekTo(duration)}>
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
        </div>

        {duration > 0 && (
          <div className="player-timeline-wrap">
            <TrimTimeline
              duration={duration}
              start={start}
              end={end}
              current={current}
              onStartChange={setStart}
              onEndChange={setEnd}
              onSeek={seekTo}
            />
          </div>
        )}

        <div className="player-toolbar">
          <IconButton
            label="Apply trim (lossless)"
            variant="accent"
            disabled={trimming || clip.status !== "ready" || duration <= 0}
            onClick={() => void handleTrim()}
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
    </div>
  );
}
