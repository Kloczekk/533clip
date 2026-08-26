import { useCallback, useEffect, useRef, useState } from "react";

const MIN_GAP_SEC = 0.25;

export interface TrimTimelineProps {
  duration: number;
  start: number;
  end: number;
  current: number;
  peaks?: number[];
  waveform?: number[];
  onStartChange: (t: number) => void;
  onEndChange: (t: number) => void;
  onSeek: (t: number) => void;
}

function clamp(n: number, min: number, max: number) {
  return Math.min(Math.max(n, min), max);
}

function formatTime(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${r.toString().padStart(2, "0")}`;
}

export function TrimTimeline({
  duration,
  start,
  end,
  current,
  peaks = [],
  waveform = [],
  onStartChange,
  onEndChange,
  onSeek,
}: TrimTimelineProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<"start" | "end" | null>(null);
  const [dragging, setDragging] = useState<"start" | "end" | null>(null);
  /** While dragging a handle, playhead follows the handle (fixes lag / wrong side). */
  const [scrubPreview, setScrubPreview] = useState<number | null>(null);
  /** Cursor-following ghost line + time tooltip while hovering, mouse-not-down. */
  const [hoverPct, setHoverPct] = useState<number | null>(null);

  const dur = duration > 0 ? duration : 1;
  const startPct = (start / dur) * 100;
  const endPct = (end / dur) * 100;
  const playheadTime = scrubPreview ?? current;
  const playheadPct = clamp((playheadTime / dur) * 100, 0, 100);

  // Bars within this many indices of a peak's nearest bar are drawn in the
  // accent color, fusing "highlight moment" into the waveform shape itself
  // instead of a separate row of dots.
  const highlightedBars = new Set<number>();
  if (waveform.length > 0) {
    for (const peak of peaks) {
      const idx = Math.round((peak / dur) * (waveform.length - 1));
      for (let i = idx - 1; i <= idx + 1; i++) {
        if (i >= 0 && i < waveform.length) highlightedBars.add(i);
      }
    }
  }

  const timeFromClientX = useCallback(
    (clientX: number) => {
      const el = trackRef.current;
      if (!el) return 0;
      const rect = el.getBoundingClientRect();
      const x = clamp(clientX - rect.left, 0, rect.width);
      return (x / rect.width) * dur;
    },
    [dur],
  );

  const applyDrag = useCallback(
    (clientX: number, handle: "start" | "end") => {
      const t = timeFromClientX(clientX);
      if (handle === "start") {
        const next = clamp(t, 0, end - MIN_GAP_SEC);
        setScrubPreview(next);
        onStartChange(next);
      } else {
        const next = clamp(t, start + MIN_GAP_SEC, dur);
        setScrubPreview(next);
        onEndChange(next);
      }
    },
    [timeFromClientX, end, start, dur, onStartChange, onEndChange, onSeek],
  );

  useEffect(() => {
    if (!dragging) return;

    const onMove = (e: PointerEvent) => {
      applyDrag(e.clientX, dragging);
    };
    const onUp = () => {
      dragRef.current = null;
      setDragging(null);
      setScrubPreview(null);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }, [dragging, applyDrag]);

  const beginHandleDrag = (handle: "start" | "end", e: React.PointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragRef.current = handle;
    setDragging(handle);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    applyDrag(e.clientX, handle);
  };

  const onTrackPointerDown = (e: React.PointerEvent) => {
    if (dragRef.current) return;
    if ((e.target as HTMLElement).closest(".trim-handle")) return;
    const t = timeFromClientX(e.clientX);
    setScrubPreview(t);
    onSeek(t);
    setScrubPreview(null);
  };

  const onTrackPointerMove = (e: React.PointerEvent) => {
    if (dragRef.current) return;
    const el = trackRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const x = clamp(e.clientX - rect.left, 0, rect.width);
    setHoverPct((x / rect.width) * 100);
  };

  const onTrackPointerLeave = () => setHoverPct(null);

  return (
    <div className="trim-timeline">
      <div className="trim-time-row">
        <span className="trim-time">{formatTime(start)}</span>
        <span className="trim-time trim-time-center">
          {formatTime(end - start)} selected
        </span>
        <span className="trim-time">{formatTime(end)}</span>
      </div>

      <div
        ref={trackRef}
        className={`trim-track ${dragging ? "is-dragging" : ""} ${hoverPct != null ? "is-hovering" : ""}`}
        onPointerDown={onTrackPointerDown}
        onPointerMove={onTrackPointerMove}
        onPointerLeave={onTrackPointerLeave}
        role="slider"
        aria-label="Trim timeline"
      >
        <div className="trim-track-base" />

        {waveform.length > 0 && (
          <div className="trim-waveform" aria-hidden>
            {waveform.map((amplitude, i) => (
              <span
                key={i}
                className={`trim-waveform-bar ${highlightedBars.has(i) ? "is-highlight" : ""}`}
                style={{ height: `${Math.max(6, amplitude * 100)}%` }}
              />
            ))}
          </div>
        )}

        {hoverPct != null && !dragging && (
          <div className="trim-hover-line" style={{ left: `${hoverPct}%` }} aria-hidden>
            <span className="trim-hover-time">{formatTime((hoverPct / 100) * dur)}</span>
          </div>
        )}

        <div
          className="trim-selection"
          style={{ left: `${startPct}%`, width: `${endPct - startPct}%` }}
        />

        <div className="trim-dim trim-dim-left" style={{ width: `${startPct}%` }} />
        <div
          className="trim-dim trim-dim-right"
          style={{ left: `${endPct}%`, width: `${100 - endPct}%` }}
        />

        <div className="trim-ticks" aria-hidden>
          {Array.from({ length: 20 }, (_, i) => (
            <span key={i} className="trim-tick" style={{ left: `${(i / 20) * 100}%` }} />
          ))}
        </div>

        <div
          className={`trim-playhead ${dragging ? "is-scrubbing" : ""}`}
          style={{ left: `${playheadPct}%` }}
          aria-hidden
        >
          <span className="trim-playhead-cap" />
          <span className="trim-playhead-line" />
        </div>

        <div
          className={`trim-handle trim-handle-start ${dragging === "start" ? "is-active" : ""}`}
          style={{ left: `${startPct}%` }}
          onPointerDown={(e) => beginHandleDrag("start", e)}
          role="slider"
          aria-label="Trim start"
        >
          <span className="trim-handle-grip" />
        </div>
        <div
          className={`trim-handle trim-handle-end ${dragging === "end" ? "is-active" : ""}`}
          style={{ left: `${endPct}%` }}
          onPointerDown={(e) => beginHandleDrag("end", e)}
          role="slider"
          aria-label="Trim end"
        >
          <span className="trim-handle-grip" />
        </div>
      </div>

      <div className="trim-time-row trim-time-row-sub">
        <span className="trim-time-muted">0:00</span>
        <span className="trim-time-muted trim-playhead-time">
          Playhead {formatTime(playheadTime)}
        </span>
        <span className="trim-time-muted">{formatTime(dur)}</span>
      </div>
    </div>
  );
}
