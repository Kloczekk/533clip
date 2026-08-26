import { useEffect, useMemo, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { ClipCard } from "./ClipCard";
import type { Clip } from "../types/clip";
import type { ClipGroup } from "../utils/groupClips";

const CARD_WIDTH = 220;
const GAP = 14;
const HEADER_HEIGHT = 34;
const ROW_HEIGHT = 190;
const OVERSCAN = 500;

interface VirtualizedClipGridProps {
  groups: ClipGroup[];
  selectedIds: Record<string, boolean>;
  onOpen: (clip: Clip) => void;
  onToggleSelect: (id: string) => void;
  onToggleFavorite: (id: string) => void;
  onDragStartClip: (id: string) => void;
  layout: string;
  activePreviewId: string | null;
  hoverPreviewEnabled: boolean;
  onPreviewChange: (id: string | null) => void;
  onContextMenu?: (id: string, e: ReactMouseEvent) => void;
}

type VirtualItem =
  | { type: "header"; label: string; top: number; height: number }
  | { type: "row"; key: string; clips: Clip[]; top: number; height: number };

export function VirtualizedClipGrid({
  groups,
  selectedIds,
  onOpen,
  onToggleSelect,
  onToggleFavorite,
  onDragStartClip,
  layout,
  activePreviewId,
  hoverPreviewEnabled,
  onPreviewChange,
  onContextMenu,
}: VirtualizedClipGridProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const resize = () => {
      setViewportHeight(el.clientHeight);
      setWidth(el.clientWidth);
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const cardWidth = layout === "compact" ? 180 : CARD_WIDTH;
  const rowHeight = layout === "compact" ? 158 : layout === "list" ? 102 : ROW_HEIGHT;
  const columns =
    layout === "list" ? 1 : Math.max(1, Math.floor((width + GAP) / (cardWidth + GAP)));
  const selectionMode = Object.values(selectedIds).some(Boolean);

  const { items, totalHeight } = useMemo(() => {
    const next: VirtualItem[] = [];
    let top = 0;

    for (const group of groups) {
      next.push({ type: "header", label: group.label, top, height: HEADER_HEIGHT });
      top += HEADER_HEIGHT;

      for (let i = 0; i < group.clips.length; i += columns) {
        const clips = group.clips.slice(i, i + columns);
        next.push({
          type: "row",
          key: `${group.label}-${i}`,
          clips,
          top,
          height: rowHeight,
        });
        top += rowHeight;
      }

      top += GAP;
    }

    return { items: next, totalHeight: top };
  }, [groups, columns, rowHeight]);

  const start = Math.max(0, scrollTop - OVERSCAN);
  const end = scrollTop + viewportHeight + OVERSCAN;
  const visible = items.filter((item) => item.top + item.height >= start && item.top <= end);

  return (
    <div
      ref={scrollRef}
      className="clips-scroll virtual-scroll"
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
    >
      <div className="virtual-spacer" style={{ height: totalHeight }}>
        {visible.map((item) =>
          item.type === "header" ? (
            <h2
              key={`header-${item.label}`}
              className="section-title virtual-item"
              style={{ transform: `translateY(${item.top}px)` }}
            >
              {item.label}
            </h2>
          ) : (
            <div
              key={item.key}
              className="clip-row virtual-item virtual-row"
              style={{ transform: `translateY(${item.top}px)` }}
            >
              {item.clips.map((clip) => (
                <ClipCard
                  key={clip.id}
                  clip={clip}
                  selected={!!selectedIds[clip.id]}
                  selectionMode={selectionMode}
                  layout={layout}
                  onOpen={onOpen}
                  onToggleSelect={onToggleSelect}
                  onToggleFavorite={onToggleFavorite}
                  onDragStartClip={onDragStartClip}
                  activePreviewId={activePreviewId}
                  hoverPreviewEnabled={hoverPreviewEnabled}
                  onPreviewChange={onPreviewChange}
                  onContextMenu={onContextMenu}
                />
              ))}
            </div>
          ),
        )}
      </div>
    </div>
  );
}
