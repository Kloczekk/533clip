import { create } from "zustand";
import type { LibraryFilter } from "../utils/filterClips";

interface ClipStore {
  watchPath: string | null;
  clips: import("../types/clip").Clip[];
  filter: LibraryFilter;
  searchQuery: string;
  selectedIds: Record<string, boolean>;
  setWatchPath: (path: string) => void;
  setFilter: (filter: LibraryFilter) => void;
  setSearchQuery: (q: string) => void;
  setClips: (clips: import("../types/clip").Clip[]) => void;
  upsertClip: (clip: import("../types/clip").Clip) => void;
  removeClip: (id: string) => void;
  toggleSelected: (id: string) => void;
  clearSelection: () => void;
  selectAll: (ids: string[]) => void;
}

export const useClipStore = create<ClipStore>((set) => ({
  watchPath: null,
  clips: [],
  filter: { kind: "all" },
  searchQuery: "",
  selectedIds: {},
  setWatchPath: (path) => set({ watchPath: path }),
  setFilter: (filter) => set({ filter, selectedIds: {} }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setClips: (clips) => set({ clips }),
  upsertClip: (clip) =>
    set((state) => {
      const idx = state.clips.findIndex((c) => c.id === clip.id);
      if (idx === -1) {
        return { clips: [clip, ...state.clips] };
      }
      const next = [...state.clips];
      next[idx] = clip;
      return { clips: next };
    }),
  removeClip: (id) =>
    set((state) => {
      const { [id]: _, ...selectedIds } = state.selectedIds;
      return {
        clips: state.clips.filter((c) => c.id !== id),
        selectedIds,
      };
    }),
  toggleSelected: (id) =>
    set((state) => ({
      selectedIds: {
        ...state.selectedIds,
        [id]: !state.selectedIds[id],
      },
    })),
  clearSelection: () => set({ selectedIds: {} }),
  selectAll: (ids) =>
    set({
      selectedIds: Object.fromEntries(ids.map((id) => [id, true])),
    }),
}));
