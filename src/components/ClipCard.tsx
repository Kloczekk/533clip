import { useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { clipDisplayName } from "../utils/clipDisplay";
import type { Clip } from "../types/clip";
import { IconFolder } from "./Icons";

interface ClipCardProps {
  clip: Clip;
  selected: boolean;
  selectionMode?: boolean;
  layout?: string;
  onOpen: (clip: Clip) => void;
  onToggleSelect: (id: string) => void;
  onToggleFavorite: (id: string) => void;
  onDragStartClip: (id: string) => void;
  activePreviewId: string | null;
  hoverPreviewEnabled: boolean;
  onPreviewChange: (id: string | null) => void;
  onContextMenu?: (id: string, e: ReactMouseEvent) => void;
}

function formatDuration(seconds?: number): string {
  if (seconds == null || !Number.isFinite(seconds)) return "—";
  const s = Math.floor(seconds);
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${r.toString().padStart(2, "0")}`;
}

export function ClipCard({
  clip,
  selected,
  selectionMode = false,
  layout = "grid",
  onOpen,
  onToggleSelect,
  onToggleFavorite,
  onDragStartClip,
  activePreviewId,
  hoverPreviewEnabled,
  onPreviewChange,
  onContextMenu,
}: ClipCardProps) {
  const [thumbError, setThumbError] = useState(false);
  const previewRef = useRef<HTMLVideoElement>(null);
  const dragStart = useRef<{ x: number; y: number; id: number } | null>(null);
  // High enough that a normal click's mouse jitter (very common, especially
  // on trackpads) doesn't accidentally kick off a native OS drag.
  const DRAG_THRESHOLD = 12;
  const thumbSrc = useMemo(
    () => clip.thumbnailPath && clip.status !== "processing" ? convertFileSrc(clip.thumbnailPath) : null,
    [clip.thumbnailPath, clip.status],
  );

  useEffect(() => {
    setThumbError(false);
  }, [clip.thumbnailPath, clip.id]);

  const showProcessing = clip.status === "processing";
  const preview = hoverPreviewEnabled && activePreviewId === clip.id && clip.status === "ready";

  useEffect(() => {
    const video = previewRef.current;
    if (!video || !preview) return;
    video.currentTime = Math.min(1, Math.max(0, clip.duration ? clip.duration * 0.08 : 0));
    void video.play().catch(() => undefined);
  }, [preview, clip.duration, clip.filePath]);

  return (
    <article
      className={`clip-card clip-card-${layout} ${clip.isFavorite ? "is-favorite" : ""} ${selected ? "is-selected" : ""}`}
      onClick={() => {
        if (selectionMode) onToggleSelect(clip.id);
        else onOpen(clip);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          if (selectionMode) onToggleSelect(clip.id);
          else onOpen(clip);
        }
      }}
      role="button"
      tabIndex={0}
      draggable={false}
      onDragStart={(e) => e.preventDefault()}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        if ((e.target as HTMLElement).closest("button, input, label")) return;
        dragStart.current = { x: e.clientX, y: e.clientY, id: e.pointerId };
      }}
      onPointerMove={(e) => {
        const start = dragStart.current;
        if (!start || start.id !== e.pointerId) return;
        const dx = e.clientX - start.x;
        const dy = e.clientY - start.y;
        if (Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
        dragStart.current = null;
        // Real OS drag-out (drop onto Discord/Explorer/etc as a file), not
        // an in-app move — use the right-click menu for moving to a game.
        // Clear hover-preview first: Windows takes over mouse input for the
        // duration of the OS drag, so onMouseLeave may never fire and the
        // preview video would otherwise keep playing underneath it.
        onPreviewChange(null);
        onDragStartClip(clip.id);
        void invoke("start_file_drag", {
          path: clip.filePath,
          thumbnail: clip.thumbnailPath ?? null,
        }).catch(() => undefined);
      }}
      onPointerUp={() => {
        dragStart.current = null;
      }}
      onMouseEnter={() => {
        if (hoverPreviewEnabled && clip.status === "ready") onPreviewChange(clip.id);
      }}
      onMouseLeave={() => {
        if (activePreviewId === clip.id) onPreviewChange(null);
      }}
      onContextMenu={(e) => {
        if (!onContextMenu) return;
        e.preventDefault();
        e.stopPropagation();
        onContextMenu(clip.id, e);
      }}
    >
      <div className="thumb-wrap">
        <label
          className="clip-checkbox"
          onClick={(e) => e.stopPropagation()}
          onPointerDown={(e) => e.stopPropagation()}
        >
          <input
            type="checkbox"
            checked={selected}
            onChange={() => onToggleSelect(clip.id)}
            aria-label={`Select ${clipDisplayName(clip)}`}
          />
        </label>

        {thumbSrc && !thumbError ? (
          <img
            src={thumbSrc}
            alt=""
            className="thumb"
            loading="lazy"
            draggable={false}
            onError={() => setThumbError(true)}
          />
        ) : (
          <div className="thumb placeholder">
            {showProcessing ? (
              <span className="spinner" aria-hidden />
            ) : (
              <span className="play-icon">▶</span>
            )}
          </div>
        )}
        {preview && (
          <video
            ref={previewRef}
            className="thumb preview-video"
            src={convertFileSrc(clip.filePath)}
            muted
            loop
            playsInline
            autoPlay
            preload="metadata"
            draggable={false}
          />
        )}
        <span className="duration-pill">{formatDuration(clip.duration)}</span>
        {showProcessing && <span className="status-pill processing">Processing</span>}
        {clip.status === "failed" && <span className="status-pill failed">Failed</span>}
        <button
          type="button"
          className={`fav-btn-overlay ${clip.isFavorite ? "active" : ""}`}
          onClick={(e) => {
            e.stopPropagation();
            onToggleFavorite(clip.id);
          }}
          aria-label={clip.isFavorite ? "Remove favorite" : "Add favorite"}
        >
          {clip.isFavorite ? "★" : "☆"}
        </button>
        <button
          type="button"
          className="reveal-btn-overlay"
          onClick={(e) => {
            e.stopPropagation();
            void invoke("reveal_path", { path: clip.filePath }).catch(() => undefined);
          }}
          aria-label="Show in folder"
          title="Show in folder"
        >
          <IconFolder size={14} />
        </button>
      </div>
      <div className="clip-meta">
        <p className="clip-name" title={clip.fileName}>
          {clipDisplayName(clip)}
        </p>
        {clip.resolution && <p className="clip-sub">{clip.resolution}</p>}
        {clip.tags.length > 0 && (
          <p className="clip-tags">{clip.tags.map((t) => `#${t}`).join(" ")}</p>
        )}
      </div>
    </article>
  );
}
