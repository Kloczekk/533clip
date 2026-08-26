import { useState } from "react";
import type { LibraryFilter } from "../utils/filterClips";
import { GameIcon } from "./GameIcon";

export type AppView = "library" | "settings";

interface SidebarProps {
  activeView: AppView;
  filter: LibraryFilter;
  clipCount: number;
  favoriteCount: number;
  gameCounts: { name: string; count: number }[];
  watchPath: string | null;
  onViewChange: (view: AppView) => void;
  onFilterChange: (filter: LibraryFilter) => void;
  onRenameGame: (from: string, to: string) => void;
  onDropClipsToGame: (game: string, draggedId?: string) => void;
}

export function Sidebar({
  activeView,
  filter,
  clipCount,
  favoriteCount,
  gameCounts,
  watchPath,
  onViewChange,
  onFilterChange,
  onRenameGame,
  onDropClipsToGame,
}: SidebarProps) {
  const tagActive = filter.kind === "tag";
  const [editingGame, setEditingGame] = useState<string | null>(null);
  const [gameDraft, setGameDraft] = useState("");

  function startEdit(name: string) {
    setEditingGame(name);
    setGameDraft(name);
  }

  function saveEdit() {
    if (editingGame && gameDraft.trim() && gameDraft.trim() !== editingGame) {
      onRenameGame(editingGame, gameDraft.trim());
    }
    setEditingGame(null);
    setGameDraft("");
  }

  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark">533</span>
        <span className="brand-name">clip</span>
      </div>

      <nav className="side-nav">
        <button
          type="button"
          className={`nav-item ${
            activeView === "library" && filter.kind === "all" && !tagActive ? "active" : ""
          }`}
          onClick={() => {
            onViewChange("library");
            onFilterChange({ kind: "all" });
          }}
        >
          <span className="nav-icon">□</span>
          <span className="nav-label">Library</span>
          <span className="nav-count">{clipCount}</span>
        </button>
        <button
          type="button"
          className={`nav-item ${
            activeView === "library" && filter.kind === "favorites" ? "active" : ""
          }`}
          onClick={() => {
            onViewChange("library");
            onFilterChange({ kind: "favorites" });
          }}
        >
          <span className="nav-icon">★</span>
          <span className="nav-label">Favorites</span>
          <span className="nav-count">{favoriteCount}</span>
        </button>
        <button
          type="button"
          className={`nav-item ${activeView === "settings" ? "active" : ""}`}
          onClick={() => onViewChange("settings")}
        >
          <span className="nav-icon">⚙</span>
          <span className="nav-label">Settings</span>
        </button>

        {gameCounts.length > 0 && (
          <div className="nav-group">
            <p className="footer-label">Games</p>
            {gameCounts.map((game) => (
              editingGame === game.name ? (
                <div key={game.name} className="nav-game-edit">
                  <input
                    className="tag-input"
                    value={gameDraft}
                    autoFocus
                    onChange={(e) => setGameDraft(e.target.value)}
                    onBlur={saveEdit}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") saveEdit();
                      if (e.key === "Escape") setEditingGame(null);
                    }}
                  />
                </div>
              ) : (
                <div
                  key={game.name}
                  className="nav-game-row"
                  onDragOver={(e) => {
                    e.preventDefault();
                    e.dataTransfer.dropEffect = "move";
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    onDropClipsToGame(game.name, e.dataTransfer.getData("text/plain") || undefined);
                  }}
                >
                  <button
                    type="button"
                    className={`nav-item compact ${
                      activeView === "library" && filter.kind === "game" && filter.game === game.name
                        ? "active"
                        : ""
                    }`}
                    onClick={() => {
                      onViewChange("library");
                      onFilterChange({ kind: "game", game: game.name });
                    }}
                  >
                    <span className="nav-label">
                      <GameIcon name={game.name} size={14} className="nav-game-icon" />
                      {game.name}
                    </span>
                    <span className="nav-count">{game.count}</span>
                  </button>
                  <button type="button" className="nav-game-rename" onClick={() => startEdit(game.name)}>
                    Edit
                  </button>
                </div>
              )
            ))}
          </div>
        )}
      </nav>

      <div className="sidebar-footer">
        <p className="footer-label">Clip folder</p>
        <p className="footer-path" title={watchPath ?? undefined}>
          {watchPath ? shortenPath(watchPath) : "Not set - open Settings"}
        </p>
      </div>
    </aside>
  );
}

function shortenPath(path: string): string {
  if (path.length <= 36) return path;
  const parts = path.split(/[/\\]/);
  const tail = parts.slice(-2).join("/");
  return `.../${tail}`;
}
