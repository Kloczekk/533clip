import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { clipDisplayName } from "../utils/clipDisplay";
import type { Clip } from "../types/clip";

interface ClipCardProps {
  clip: Clip;
  selected: boolean;
  onOpen: (clip: Clip) => void;
  onToggleSelect: (id: string) => void;
  onToggleFavorite: (id: string) => void;
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
  onOpen,
  onToggleSelect,
  onToggleFavorite,
}: ClipCardProps) {
  const [thumbSrc, setThumbSrc] = useState<string | null>(null);
  const [thumbError, setThumbError] = useState(false);

  useEffect(() => {
    setThumbSrc(null);
    setThumbError(false);
    if (!clip.thumbnailPath || clip.status === "processing") return;

    void invoke<string>("get_thumbnail_data_url", {
      path: clip.thumbnailPath,
    })
      .then(setThumbSrc)
      .catch(() => setThumbError(true));
  }, [clip.thumbnailPath, clip.status, clip.id]);

  const showProcessing = clip.status === "processing";

  return (
    <article
      className={`clip-card ${clip.isFavorite ? "is-favorite" : ""} ${selected ? "is-selected" : ""}`}
      onClick={() => onOpen(clip)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOpen(clip);
        }
      }}
      role="button"
      tabIndex={0}
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
          <img src={thumbSrc} alt="" className="thumb" loading="lazy" />
        ) : (
          <div className="thumb placeholder">
            {showProcessing ? (
              <span className="spinner" aria-hidden />
            ) : (
              <span className="play-icon">▶</span>
            )}
          </div>
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
