import { useEffect, useRef, useState } from "react";
import { IconTag, IconTrash } from "./Icons";
import { IconButton } from "./IconButton";
import { GameIcon } from "./GameIcon";

interface LibraryToolbarProps {
  searchQuery: string;
  onSearchChange: (q: string) => void;
  selectedCount: number;
  onDeleteSelected: () => void;
  onTagSelected: (tag: string) => void;
  onUntagSelected: (tag: string) => void;
  onClearSelection: () => void;
  lockedGame?: string | null;
  knownGames?: string[];
  onOverrideGame?: (game: string | null) => void;
}

export function LibraryToolbar({
  searchQuery,
  onSearchChange,
  selectedCount,
  onDeleteSelected,
  onTagSelected,
  onUntagSelected,
  onClearSelection,
  lockedGame,
  knownGames = [],
  onOverrideGame,
}: LibraryToolbarProps) {
  const [tagDraft, setTagDraft] = useState("");
  const [gamePickerOpen, setGamePickerOpen] = useState(false);
  const [customGame, setCustomGame] = useState("");
  const pickerRef = useRef<HTMLDivElement>(null);
  const submitTag = (mode: "tag" | "untag") => {
    const tag = tagDraft.trim();
    if (!tag) return;
    if (mode === "tag") onTagSelected(tag);
    else onUntagSelected(tag);
  };

  useEffect(() => {
    if (!gamePickerOpen) return;
    const onPointerDown = (e: PointerEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) setGamePickerOpen(false);
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    return () => window.removeEventListener("pointerdown", onPointerDown, true);
  }, [gamePickerOpen]);

  return (
    <div className="library-toolbar">
      <input
        type="search"
        className="search-input"
        placeholder="Search clips, tags…"
        value={searchQuery}
        onChange={(e) => onSearchChange(e.target.value)}
        aria-label="Search clips"
      />
      {onOverrideGame && (
        <div className="game-lock-control" ref={pickerRef}>
          <button
            type="button"
            className={`game-lock-pill ${lockedGame ? "is-locked" : "is-idle"}`}
            onClick={() => setGamePickerOpen((v) => !v)}
            title="Click to change which game/app new clips are labeled with"
          >
            <span className="game-lock-dot" aria-hidden />
            {lockedGame ? (
              <>
                <GameIcon name={lockedGame} size={14} className="clip-context-menu-icon" />
                {`Ready to clip: ${lockedGame}`}
              </>
            ) : (
              "No game detected"
            )}
          </button>
          {gamePickerOpen && (
            <div className="game-lock-picker">
              <p className="game-lock-picker-title">Label new clips as…</p>
              <button
                type="button"
                className="clip-context-menu-item"
                onClick={() => {
                  onOverrideGame(null);
                  setGamePickerOpen(false);
                }}
              >
                Auto-detect
              </button>
              {knownGames.map((game) => (
                <button
                  key={game}
                  type="button"
                  className="clip-context-menu-item"
                  onClick={() => {
                    onOverrideGame(game);
                    setGamePickerOpen(false);
                  }}
                >
                  <GameIcon name={game} size={14} className="clip-context-menu-icon" />
                  {game}
                </button>
              ))}
              <form
                className="clip-context-menu-new"
                onSubmit={(e) => {
                  e.preventDefault();
                  const name = customGame.trim();
                  if (!name) return;
                  onOverrideGame(name);
                  setCustomGame("");
                  setGamePickerOpen(false);
                }}
              >
                <input
                  type="text"
                  value={customGame}
                  placeholder="Custom name"
                  onChange={(e) => setCustomGame(e.target.value)}
                />
                <button type="submit" className="btn ghost small">
                  Set
                </button>
              </form>
            </div>
          )}
        </div>
      )}
      {selectedCount > 0 && (
        <div className="selection-bar">
          <span className="selection-count">{selectedCount} selected</span>
          <div className="bulk-tag-control">
            <input
              type="text"
              className="tag-input"
              placeholder="tag"
              value={tagDraft}
              onChange={(e) => setTagDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") submitTag("tag");
              }}
            />
            <button type="button" className="btn ghost small" onClick={() => submitTag("tag")}>
              <IconTag size={14} />
              Tag
            </button>
            <button type="button" className="btn ghost small" onClick={() => submitTag("untag")}>
              Untag
            </button>
          </div>
          <button type="button" className="btn-link" onClick={onClearSelection}>
            Clear
          </button>
          <IconButton label={`Delete ${selectedCount} clips`} variant="danger" onClick={onDeleteSelected}>
            <IconTrash size={18} />
          </IconButton>
        </div>
      )}
    </div>
  );
}
