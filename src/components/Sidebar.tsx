import type { LibraryFilter } from "../utils/filterClips";
import { ObsWebSocketPanel } from "./ObsWebSocketPanel";

interface SidebarProps {
  filter: LibraryFilter;
  clipCount: number;
  favoriteCount: number;
  watchPath: string | null;
  onFilterChange: (filter: LibraryFilter) => void;
  onChooseFolder: () => void;
  onDetectObsFolder: () => void;
}

export function Sidebar({
  filter,
  clipCount,
  favoriteCount,
  watchPath,
  onFilterChange,
  onChooseFolder,
  onDetectObsFolder,
}: SidebarProps) {
  const tagActive = filter.kind === "tag";

  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark">533</span>
        <span className="brand-name">clip</span>
      </div>

      <nav className="side-nav">
        <button
          type="button"
          className={`nav-item ${filter.kind === "all" && !tagActive ? "active" : ""}`}
          onClick={() => onFilterChange({ kind: "all" })}
        >
          <span className="nav-icon">▦</span>
          <span className="nav-label">Library</span>
          <span className="nav-count">{clipCount}</span>
        </button>
        <button
          type="button"
          className={`nav-item ${filter.kind === "favorites" ? "active" : ""}`}
          onClick={() => onFilterChange({ kind: "favorites" })}
        >
          <span className="nav-icon">★</span>
          <span className="nav-label">Favorites</span>
          <span className="nav-count">{favoriteCount}</span>
        </button>
      </nav>

      <div className="sidebar-footer">
        <p className="footer-label">OBS clip folder</p>
        <p className="footer-path" title={watchPath ?? undefined}>
          {watchPath ? shortenPath(watchPath) : "Not set — pick your OBS recording path"}
        </p>
        <div className="footer-actions">
          <button type="button" className="btn ghost" onClick={onDetectObsFolder}>
            Use OBS folder
          </button>
          <button type="button" className="btn ghost" onClick={onChooseFolder}>
            Browse…
          </button>
        </div>
        <p className="footer-hint">
          Match OBS → Settings → Output → Recording path (often Videos).
        </p>
        <ObsWebSocketPanel />
      </div>
    </aside>
  );
}

function shortenPath(path: string): string {
  if (path.length <= 36) return path;
  const parts = path.split(/[/\\]/);
  const tail = parts.slice(-2).join("/");
  return `…/${tail}`;
}
