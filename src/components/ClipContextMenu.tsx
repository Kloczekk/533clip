import { useEffect, useRef, useState } from "react";
import { GameIcon } from "./GameIcon";

interface ClipContextMenuProps {
  x: number;
  y: number;
  count: number;
  games: string[];
  tags: string[];
  onMoveToGame: (game: string) => void;
  onAddTag: (tag: string) => void;
  onClose: () => void;
}

export function ClipContextMenu({ x, y, count, games, tags, onMoveToGame, onAddTag, onClose }: ClipContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [creating, setCreating] = useState(false);
  const [newGame, setNewGame] = useState("");
  const [taggingOpen, setTaggingOpen] = useState(false);
  const [creatingTag, setCreatingTag] = useState(false);
  const [newTag, setNewTag] = useState("");

  useEffect(() => {
    const onPointerDown = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKey);
    window.addEventListener("scroll", onClose, true);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", onClose, true);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  const style = {
    left: Math.min(x, window.innerWidth - 240),
    top: Math.min(y, window.innerHeight - 40),
  };

  return (
    <div ref={ref} className="clip-context-menu" style={style}>
      <p className="clip-context-menu-title">
        Move {count > 1 ? `${count} clips` : "clip"} to…
      </p>
      <div className="clip-context-menu-list">
        {games.length === 0 && !creating && (
          <p className="clip-context-menu-empty">No game groups yet</p>
        )}
        {games.map((game) => (
          <button
            key={game}
            type="button"
            className="clip-context-menu-item"
            onClick={() => onMoveToGame(game)}
          >
            <GameIcon name={game} size={14} className="clip-context-menu-icon" />
            {game}
          </button>
        ))}
      </div>
      {creating ? (
        <form
          className="clip-context-menu-new"
          onSubmit={(e) => {
            e.preventDefault();
            const name = newGame.trim();
            if (name) onMoveToGame(name);
          }}
        >
          <input
            autoFocus
            type="text"
            value={newGame}
            placeholder="New game/app name"
            onChange={(e) => setNewGame(e.target.value)}
          />
          <button type="submit" className="btn ghost small">
            Move
          </button>
        </form>
      ) : (
        <button
          type="button"
          className="clip-context-menu-item clip-context-menu-new-btn"
          onClick={() => setCreating(true)}
        >
          + New game/app…
        </button>
      )}

      <button
        type="button"
        className="clip-context-menu-item clip-context-menu-new-btn"
        onClick={() => setTaggingOpen((v) => !v)}
      >
        {taggingOpen ? "Hide tags" : "Tag…"}
      </button>
      {taggingOpen && (
        <>
          <p className="clip-context-menu-title">
            Tag {count > 1 ? `${count} clips` : "clip"}
          </p>
          <div className="clip-context-menu-list">
            {tags.length === 0 && !creatingTag && (
              <p className="clip-context-menu-empty">No tags yet</p>
            )}
            {tags.map((tag) => (
              <button
                key={tag}
                type="button"
                className="clip-context-menu-item"
                onClick={() => onAddTag(tag)}
              >
                #{tag}
              </button>
            ))}
          </div>
          {creatingTag ? (
            <form
              className="clip-context-menu-new"
              onSubmit={(e) => {
                e.preventDefault();
                const name = newTag.trim();
                if (name) onAddTag(name);
              }}
            >
              <input
                autoFocus
                type="text"
                value={newTag}
                placeholder="New tag"
                onChange={(e) => setNewTag(e.target.value)}
              />
              <button type="submit" className="btn ghost small">
                Add
              </button>
            </form>
          ) : (
            <button
              type="button"
              className="clip-context-menu-item clip-context-menu-new-btn"
              onClick={() => setCreatingTag(true)}
            >
              + New tag…
            </button>
          )}
        </>
      )}
    </div>
  );
}
