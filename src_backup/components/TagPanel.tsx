import { useState } from "react";

interface TagPanelProps {
  tags: string[];
  activeTag: string | null;
  onSelectTag: (tag: string | null) => void;
  onCreateTag: (tag: string) => void;
  onDeleteTag: (tag: string) => void;
}

export function TagPanel({
  tags,
  activeTag,
  onSelectTag,
  onCreateTag,
  onDeleteTag,
}: TagPanelProps) {
  const [draft, setDraft] = useState("");

  function submit() {
    const t = draft.trim();
    if (!t) return;
    onCreateTag(t);
    setDraft("");
  }

  return (
    <div className="tag-panel">
      <p className="tag-panel-label">Tags</p>
      <div className="tag-chips">
        <button
          type="button"
          className={`tag-chip ${activeTag === null ? "active" : ""}`}
          onClick={() => onSelectTag(null)}
        >
          All tags
        </button>
        {tags.map((tag) => (
          <div key={tag} className="tag-chip-wrap">
            <button
              type="button"
              className={`tag-chip ${activeTag === tag ? "active" : ""}`}
              onClick={() => onSelectTag(tag)}
            >
              #{tag}
            </button>
            <button
              type="button"
              className="tag-chip-delete"
              title={`Delete tag #${tag}`}
              aria-label={`Delete tag ${tag}`}
              onClick={(e) => {
                e.stopPropagation();
                onDeleteTag(tag);
              }}
            >
              ×
            </button>
          </div>
        ))}
      </div>
      <div className="tag-create">
        <input
          type="text"
          className="tag-input"
          placeholder="New tag…"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") submit();
          }}
        />
        <button type="button" className="btn ghost small" onClick={submit}>
          Add
        </button>
      </div>
    </div>
  );
}
