import { IconTrash } from "./Icons";
import { IconButton } from "./IconButton";

interface LibraryToolbarProps {
  searchQuery: string;
  onSearchChange: (q: string) => void;
  selectedCount: number;
  onDeleteSelected: () => void;
  onClearSelection: () => void;
}

export function LibraryToolbar({
  searchQuery,
  onSearchChange,
  selectedCount,
  onDeleteSelected,
  onClearSelection,
}: LibraryToolbarProps) {
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
      {selectedCount > 0 && (
        <div className="selection-bar">
          <span className="selection-count">{selectedCount} selected</span>
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
