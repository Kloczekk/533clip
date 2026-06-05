import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { ClipCard } from "./components/ClipCard";
import { LibraryToolbar } from "./components/LibraryToolbar";
import { PlayerModal } from "./components/PlayerModal";
import { Sidebar } from "./components/Sidebar";
import { TagPanel } from "./components/TagPanel";
import { ReplaySavedToast } from "./components/ReplaySavedToast";
import { useClipStore } from "./store/clipStore";
import type { Clip } from "./types/clip";
import { filterClips } from "./utils/filterClips";
import { groupClipsByDate } from "./utils/groupClips";

function App() {
  const watchPath = useClipStore((s) => s.watchPath);
  const clips = useClipStore((s) => s.clips);
  const filter = useClipStore((s) => s.filter);
  const searchQuery = useClipStore((s) => s.searchQuery);
  const selectedIds = useClipStore((s) => s.selectedIds);
  const setWatchPath = useClipStore((s) => s.setWatchPath);
  const setFilter = useClipStore((s) => s.setFilter);
  const setSearchQuery = useClipStore((s) => s.setSearchQuery);
  const setClips = useClipStore((s) => s.setClips);
  const upsertClip = useClipStore((s) => s.upsertClip);
  const removeClip = useClipStore((s) => s.removeClip);
  const toggleSelected = useClipStore((s) => s.toggleSelected);
  const clearSelection = useClipStore((s) => s.clearSelection);

  const [playingClipId, setPlayingClipId] = useState<string | null>(null);
  const [allTags, setAllTags] = useState<string[]>([]);

  const playingClip = playingClipId
    ? clips.find((c) => c.id === playingClipId) ?? null
    : null;

  const loadClips = useCallback(async () => {
    const list = await invoke<Clip[]>("list_clips");
    setClips(list);
  }, [setClips]);

  const loadTags = useCallback(async () => {
    const tags = await invoke<string[]>("list_tags");
    setAllTags(tags);
  }, []);

  useEffect(() => {
    void invoke<string | null>("get_watch_path").then((path) => {
      if (path) setWatchPath(path);
    });
    void loadClips();
    void loadTags();
  }, [setWatchPath, loadClips, loadTags]);

  useEffect(() => {
    const unlistenUpdated = listen<Clip>("clip://updated", (event) => {
      upsertClip(event.payload);
      void loadTags();
    });
    const unlistenDeleted = listen<string>("clip://deleted", (event) => {
      removeClip(event.payload);
      setPlayingClipId((id) => (id === event.payload ? null : id));
    });
    return () => {
      void unlistenUpdated.then((fn) => fn());
      void unlistenDeleted.then((fn) => fn());
    };
  }, [upsertClip, removeClip, loadTags]);

  const filtered = useMemo(
    () => filterClips(clips, filter, searchQuery),
    [clips, filter, searchQuery],
  );
  const groups = groupClipsByDate(filtered);
  const favoriteCount = clips.filter((c) => c.isFavorite).length;

  const selectedCount = Object.values(selectedIds).filter(Boolean).length;
  const activeTag = filter.kind === "tag" ? filter.tag : null;

  async function applyWatchFolder(path: string) {
    await invoke("set_watch_path", { path });
    setWatchPath(path);
    void loadClips();
  }

  async function pickWatchFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select OBS clip output folder",
      defaultPath: watchPath ?? undefined,
    });
    if (!selected || Array.isArray(selected)) return;
    await applyWatchFolder(selected);
  }

  async function detectObsFolder() {
    const paths = await invoke<string[]>("detect_obs_recording_paths");
    if (paths.length === 0) {
      await pickWatchFolder();
      return;
    }
    const match =
      paths.find((p) => watchPath && p.toLowerCase() === watchPath.toLowerCase()) ??
      paths[0];
    await applyWatchFolder(match);
  }

  async function handleToggleFavorite(id: string) {
    const updated = await invoke<Clip>("toggle_favorite", { id });
    upsertClip(updated);
  }

  async function handleDeleteSelected() {
    const ids = Object.entries(selectedIds)
      .filter(([, v]) => v)
      .map(([id]) => id);
    if (ids.length === 0) return;
    const ok = await ask(`Delete ${ids.length} clip(s) permanently?`, {
      title: "Delete clips",
      kind: "warning",
    });
    if (!ok) return;
    await invoke("delete_clips", { ids });
    clearSelection();
  }

  async function handleCreateTag(tag: string) {
    const tags = await invoke<string[]>("create_tag", { tag });
    setAllTags(tags);
  }

  async function handleDeleteTag(tag: string) {
    const ok = await ask(
      `Delete tag "#${tag}" everywhere? It will be removed from all clips.`,
      { title: "Delete tag", kind: "warning" },
    );
    if (!ok) return;
    const tags = await invoke<string[]>("delete_tag", { tag });
    setAllTags(tags);
    if (filter.kind === "tag" && filter.tag === tag) {
      setFilter({ kind: "all" });
    }
  }

  function handleTagSelect(tag: string | null) {
    if (tag === null) {
      setFilter({ kind: "all" });
    } else {
      setFilter({ kind: "tag", tag });
    }
  }

  const headerTitle =
    filter.kind === "favorites"
      ? "Favorites"
      : filter.kind === "tag"
        ? `#${filter.tag}`
        : "Library";

  return (
    <div className="app-shell">
      <ReplaySavedToast />
      <Sidebar
        filter={filter}
        clipCount={clips.length}
        favoriteCount={favoriteCount}
        watchPath={watchPath}
        onFilterChange={setFilter}
        onChooseFolder={() => void pickWatchFolder()}
        onDetectObsFolder={() => void detectObsFolder()}
      />

      <main className="main-content">
        <header className="main-header">
          <h1>{headerTitle}</h1>
          <p className="main-subtitle">
            {filtered.length} clip{filtered.length === 1 ? "" : "s"}
            {searchQuery.trim() ? ` · matching "${searchQuery.trim()}"` : ""}
          </p>
        </header>

        <LibraryToolbar
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          selectedCount={selectedCount}
          onDeleteSelected={() => void handleDeleteSelected()}
          onClearSelection={clearSelection}
        />

        <TagPanel
          tags={allTags}
          activeTag={activeTag}
          onSelectTag={handleTagSelect}
          onCreateTag={(t) => void handleCreateTag(t)}
          onDeleteTag={(t) => void handleDeleteTag(t)}
        />

        <div className="clips-scroll">
          {groups.length === 0 ? (
            <div className="empty-state">
              <p className="empty-title">No clips here</p>
              <p className="empty-body">
                Save a replay in OBS or try another search / tag filter.
              </p>
            </div>
          ) : (
            groups.map((group) => (
              <section key={group.label} className="clip-section">
                <h2 className="section-title">{group.label}</h2>
                <div className="clip-row">
                  {group.clips.map((clip) => (
                    <ClipCard
                      key={clip.id}
                      clip={clip}
                      selected={!!selectedIds[clip.id]}
                      onOpen={(c) => setPlayingClipId(c.id)}
                      onToggleSelect={toggleSelected}
                      onToggleFavorite={(id) => void handleToggleFavorite(id)}
                    />
                  ))}
                </div>
              </section>
            ))
          )}
        </div>
      </main>

      {playingClip && (
        <PlayerModal
          clip={playingClip}
          onClose={() => setPlayingClipId(null)}
          onUpdate={(c) => {
            upsertClip(c);
            void loadTags();
          }}
          onDeleted={removeClip}
          allTags={allTags}
          onTagsChange={loadTags}
        />
      )}
    </div>
  );
}

export default App;
