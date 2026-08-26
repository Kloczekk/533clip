import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type MouseEvent as ReactMouseEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { AudioControlPanel } from "./components/AudioControlPanel";
import { ClipContextMenu } from "./components/ClipContextMenu";
import { DebugPanel } from "./components/DebugPanel";
import {
  IconBug,
  IconCamera,
  IconCloud,
  IconDatabase,
  IconKeyboard,
  IconPalette,
  IconSpeaker,
} from "./components/Icons";
import { LibraryToolbar } from "./components/LibraryToolbar";
import { ObsControlPanel } from "./components/ObsControlPanel";
import { PlayerModal } from "./components/PlayerModal";
import { R2SettingsPanel } from "./components/R2SettingsPanel";
import { Sidebar } from "./components/Sidebar";
import type { AppView } from "./components/Sidebar";
import { TagPanel } from "./components/TagPanel";
import { VirtualizedClipGrid } from "./components/VirtualizedClipGrid";
import { useClipStore } from "./store/clipStore";
import type { Clip } from "./types/clip";
import { filterClips } from "./utils/filterClips";
import { GameIcon } from "./components/GameIcon";
import { gameNameForClip, setGameAlias } from "./utils/gameName";
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
  const animatedBgRef = useRef<HTMLVideoElement>(null);

  const [activeView, setActiveView] = useState<AppView>("library");
  const [playingClipId, setPlayingClipId] = useState<string | null>(null);
  const [allTags, setAllTags] = useState<string[]>([]);
  const [errorToast, setErrorToast] = useState<string | null>(null);
  const [clipContextMenu, setClipContextMenu] = useState<{ x: number; y: number; clipId: string } | null>(null);
  const [lockedGame, setLockedGame] = useState<string | null>(null);
  const [theme, setTheme] = useState(() => localStorage.getItem("533clip-theme") ?? "amber");
  const [bgStyle, setBgStyle] = useState(() => localStorage.getItem("533clip-bg") ?? "animated");
  const [customBg, setCustomBg] = useState(() => localStorage.getItem("533clip-custom-bg") ?? "");
  const [settingsTab, setSettingsTab] = useState<"appearance" | "sharing" | "storage" | "obs" | "audio" | "shortcuts" | "debug">("appearance");
  const [radius, setRadius] = useState(() => Number(localStorage.getItem("533clip-radius") ?? 8));
  const [sidebarPosition, setSidebarPosition] = useState(() => localStorage.getItem("533clip-sidebar") ?? "left");
  const [playerTheme, setPlayerTheme] = useState(() => localStorage.getItem("533clip-player-theme") ?? "minimal");
  const [clipLayout, setClipLayout] = useState(() => localStorage.getItem("533clip-clip-layout") ?? "grid");
  const [fontScale] = useState(() => Number(localStorage.getItem("533clip-font-scale") ?? 100));
  const [startupBehavior, setStartupBehavior] = useState(() => localStorage.getItem("533clip-startup") ?? "normal");
  const [launchOnStartup, setLaunchOnStartup] = useState(false);
const [notificationStyle, setNotificationStyle] = useState(() => localStorage.getItem("533clip-notification-style") ?? "outplayed");
  const [hoverPreviewEnabled, setHoverPreviewEnabled] = useState(() => localStorage.getItem("533clip-hover-preview") !== "false");
  const [animatedBg, setAnimatedBg] = useState(() => localStorage.getItem("533clip-animated-bg") !== "false");
  const [customBgVideo, setCustomBgVideo] = useState(() => localStorage.getItem("533clip-custom-bg-video") ?? "/backgrounds/default-animated.mp4");
  const [clipSound, setClipSound] = useState(() => localStorage.getItem("533clip-clip-sound") ?? "chime");
  const [customClipSound, setCustomClipSound] = useState(() => localStorage.getItem("533clip-custom-clip-sound") ?? "");
  const [localCleanupDays, setLocalCleanupDays] = useState(() => Number(localStorage.getItem("533clip-local-cleanup-days") ?? 30));
  const [localCleanupMaxGb, setLocalCleanupMaxGb] = useState(() => Number(localStorage.getItem("533clip-local-cleanup-max-gb") ?? 20));
  const [localCleanupStatus, setLocalCleanupStatus] = useState<string | null>(null);
  const [localCleanupBusy, setLocalCleanupBusy] = useState(false);
  const [activePreviewId, setActivePreviewId] = useState<string | null>(null);
  const [appSuspended, setAppSuspended] = useState(false);
  const [gameAliasVersion, setGameAliasVersion] = useState(0);
  const [dragClipIds, setDragClipIds] = useState<string[]>([]);

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
    void invoke<string | null>("get_locked_game").then((game) => {
      if (game) setLockedGame(game);
    });
    void loadClips();
    void loadTags();
  }, [setWatchPath, loadClips, loadTags]);

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "F11") return;
      const target = e.target as HTMLElement | null;
      if (target?.tagName === "INPUT" || target?.tagName === "TEXTAREA" || target?.isContentEditable) return;
      e.preventDefault();
      void invoke<boolean>("toggle_app_fullscreen");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (!errorToast) return;
    const id = window.setTimeout(() => setErrorToast(null), 6000);
    return () => window.clearTimeout(id);
  }, [errorToast]);

  useEffect(() => {
    localStorage.setItem("533clip-theme", theme);
    localStorage.setItem("533clip-bg", bgStyle);
    localStorage.setItem("533clip-custom-bg", customBg);
    localStorage.setItem("533clip-radius", String(radius));
    localStorage.setItem("533clip-sidebar", sidebarPosition);
    localStorage.setItem("533clip-player-theme", playerTheme);
    localStorage.setItem("533clip-clip-layout", clipLayout);
    localStorage.setItem("533clip-font-scale", String(fontScale));
    localStorage.setItem("533clip-startup", startupBehavior);
    localStorage.setItem("533clip-notification-style", notificationStyle);
    localStorage.setItem("533clip-hover-preview", String(hoverPreviewEnabled));
    localStorage.setItem("533clip-notification-theme", theme);
    localStorage.setItem("533clip-animated-bg", String(animatedBg));
    localStorage.setItem("533clip-custom-bg-video", customBgVideo);
    localStorage.setItem("533clip-clip-sound", clipSound);
    localStorage.setItem("533clip-custom-clip-sound", customClipSound);
  }, [theme, bgStyle, customBg, radius, sidebarPosition, playerTheme, clipLayout, fontScale, startupBehavior, notificationStyle, hoverPreviewEnabled, animatedBg, customBgVideo, clipSound, customClipSound]);

  useEffect(() => {
    document.documentElement.style.setProperty("--font-scale", String(fontScale / 100));
  }, [fontScale]);

  useEffect(() => {
    if (startupBehavior === "tray") {
      setTimeout(() => void getCurrentWindow().hide(), 400);
    } else if (startupBehavior === "minimized") {
      setTimeout(() => void getCurrentWindow().minimize(), 400);
    }
  }, []);

  useEffect(() => {
    let alive = true;
    const win = getCurrentWindow();
    const sync = async () => {
      try {
        const [visible, minimized, focused] = await Promise.all([
          win.isVisible(),
          win.isMinimized(),
          win.isFocused(),
        ]);
        if (!alive) return;
        setAppSuspended(!visible || minimized || document.hidden || !focused);
      } catch {
        if (alive) setAppSuspended(document.hidden || !document.hasFocus());
      }
    };
    const unlistenFocus = win.onFocusChanged(() => void sync());
    // Focus/blur/visibilitychange below cover virtually every transition;
    // this is just a defensive backstop for whatever they miss, so it
    // doesn't need to run often.
    const interval = window.setInterval(() => void sync(), 5000);
    window.addEventListener("blur", sync);
    window.addEventListener("focus", sync);
    document.addEventListener("visibilitychange", sync);
    void sync();
    return () => {
      alive = false;
      window.clearInterval(interval);
      window.removeEventListener("blur", sync);
      window.removeEventListener("focus", sync);
      document.removeEventListener("visibilitychange", sync);
      void unlistenFocus.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    void invoke<boolean>("get_launch_on_startup").then(setLaunchOnStartup).catch(() => undefined);
  }, []);

  useEffect(() => {
    if (appSuspended) {
      setActivePreviewId(null);
    }
  }, [appSuspended]);

  useEffect(() => {
    const video = animatedBgRef.current;
    if (!video) return;
    const syncPlayback = () => {
      const shouldPause = appSuspended || document.hidden || !document.hasFocus();
      if (shouldPause) {
        video.pause();
      } else if (animatedBg && customBgVideo) {
        void video.play().catch(() => undefined);
      }
    };
    document.addEventListener("visibilitychange", syncPlayback);
    window.addEventListener("blur", syncPlayback);
    window.addEventListener("focus", syncPlayback);
    syncPlayback();
    return () => {
      document.removeEventListener("visibilitychange", syncPlayback);
      window.removeEventListener("blur", syncPlayback);
      window.removeEventListener("focus", syncPlayback);
    };
  }, [animatedBg, customBgVideo, appSuspended]);

  useEffect(() => {
    const clear = () => setActivePreviewId(null);
    window.addEventListener("blur", clear);
    document.addEventListener("visibilitychange", clear);
    return () => {
      window.removeEventListener("blur", clear);
      document.removeEventListener("visibilitychange", clear);
    };
  }, []);

  useEffect(() => {
    const unlistenGameLocked = listen<{ gameName: string }>("game://locked", (event) => {
      setLockedGame(event.payload.gameName.trim() || null);
    });
    const unlistenUpdated = listen<Clip>("clip://updated", (event) => {
      upsertClip(event.payload);
      void loadTags();
    });
    const unlistenDeleted = listen<string>("clip://deleted", (event) => {
      removeClip(event.payload);
      setPlayingClipId((id) => (id === event.payload ? null : id));
    });
    return () => {
      void unlistenGameLocked.then((fn) => fn());
      void unlistenUpdated.then((fn) => fn());
      void unlistenDeleted.then((fn) => fn());
    };
  }, [upsertClip, removeClip, loadTags]);

  const filtered = useMemo(
    () => filterClips(clips, filter, searchQuery),
    [clips, filter, searchQuery, gameAliasVersion],
  );
  const groups = useMemo(() => groupClipsByDate(filtered), [filtered]);
  // groupClipsByDate re-sorts by date internally (session grouping), which
  // can differ from `filtered`'s own array order (new clips can land at
  // whatever position the realtime clip://updated upsert happened to put
  // them). Arrow-key prev/next must walk the SAME order the grid actually
  // renders, or it jumps to a clip that isn't the visually adjacent one.
  const orderedClips = useMemo(() => groups.flatMap((g) => g.clips), [groups]);
  const favoriteCount = clips.filter((c) => c.isFavorite).length;
  const gameCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const clip of clips) {
      const game = gameNameForClip(clip);
      counts.set(game, (counts.get(game) ?? 0) + 1);
    }
    counts.set("General", clips.length);
    return [...counts.entries()]
      .map(([name, count]) => ({ name, count }))
      .sort((a, b) => {
        if (a.name === "General") return -1;
        if (b.name === "General") return 1;
        return b.count - a.count || a.name.localeCompare(b.name);
      })
      .slice(0, 12);
  }, [clips, gameAliasVersion]);
  const selectedCount = Object.values(selectedIds).filter(Boolean).length;
  const activeTag = filter.kind === "tag" ? filter.tag : null;
  const playingIndex = playingClipId
    ? orderedClips.findIndex((c) => c.id === playingClipId)
    : -1;

  function notifyError(e: unknown) {
    setErrorToast(String(e instanceof Error ? e.message : e));
  }

  async function applyWatchFolder(path: string) {
    try {
      await invoke("set_watch_path", { path });
      setWatchPath(path);
      void loadClips();
    } catch (e) {
      notifyError(e);
    }
  }

  async function pickWatchFolder() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select clip output folder",
      defaultPath: watchPath ?? undefined,
    });
    if (!selected || Array.isArray(selected)) return;
    await applyWatchFolder(selected);
  }

  async function detectObsFolder() {
    try {
      const paths = await invoke<string[]>("detect_obs_recording_paths");
      if (paths.length === 0) {
        await pickWatchFolder();
        return;
      }
      const match =
        paths.find((p) => watchPath && p.toLowerCase() === watchPath.toLowerCase()) ??
        paths[0];
      await applyWatchFolder(match);
    } catch (e) {
      notifyError(e);
    }
  }

  async function handleToggleFavorite(id: string) {
    try {
      const updated = await invoke<Clip>("toggle_favorite", { id });
      upsertClip(updated);
    } catch (e) {
      notifyError(e);
    }
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
    try {
      await invoke("delete_clips", { ids });
      clearSelection();
    } catch (e) {
      notifyError(e);
    }
  }

  function selectedClipIds() {
    return Object.entries(selectedIds)
      .filter(([, v]) => v)
      .map(([id]) => id);
  }

  async function handleTagSelected(tag: string) {
    const ids = selectedClipIds();
    if (ids.length === 0) return;
    try {
      const updated = await invoke<Clip[]>("add_tag_to_clips", { ids, tag });
      updated.forEach(upsertClip);
      void loadTags();
    } catch (e) {
      notifyError(e);
    }
  }

  async function handleUntagSelected(tag: string) {
    const ids = selectedClipIds();
    if (ids.length === 0) return;
    try {
      const updated = await invoke<Clip[]>("remove_tag_from_clips", { ids, tag });
      updated.forEach(upsertClip);
      void loadTags();
    } catch (e) {
      notifyError(e);
    }
  }

  function handleDragStartClip(id: string) {
    const selected = Object.entries(selectedIds)
      .filter(([, v]) => v)
      .map(([clipId]) => clipId);
    setDragClipIds(selected.includes(id) ? selected : [id]);
  }

  async function handleOverrideLockedGame(game: string | null) {
    try {
      if (game) {
        await invoke("set_locked_game", { gameName: game });
        setLockedGame(game);
      } else {
        await invoke("clear_locked_game");
        setLockedGame(null);
      }
    } catch (e) {
      notifyError(e);
    }
  }

  function handleClipContextMenu(id: string, e: ReactMouseEvent) {
    if (!selectedIds[id]) {
      clearSelection();
      toggleSelected(id);
    }
    setClipContextMenu({ x: e.clientX, y: e.clientY, clipId: id });
  }

  async function handleDropClipsToGame(game: string, draggedId?: string) {
    const selected = selectedClipIds();
    const ids =
      dragClipIds.length > 0
        ? dragClipIds
        : draggedId && selected.includes(draggedId)
          ? selected
          : draggedId
            ? [draggedId]
            : selected;
    if (ids.length === 0) return;
    try {
      const updated = await invoke<Clip[]>("set_clips_game", { ids, gameName: game });
      updated.forEach(upsertClip);
      setGameAliasVersion((v) => v + 1);
    } catch (e) {
      notifyError(e);
    } finally {
      setDragClipIds([]);
      clearSelection();
    }
  }

  async function handleCleanupOldLocalClips() {
    const days = Math.max(1, localCleanupDays);
    const ok = await ask(
      `Delete non-favorite clips older than ${days} day(s)? Favorite clips stay kept.`,
      { title: "Clean old clips", kind: "warning" },
    );
    if (!ok) return;
    setLocalCleanupBusy(true);
    setLocalCleanupStatus(null);
    try {
      localStorage.setItem("533clip-local-cleanup-days", String(days));
      localStorage.setItem("533clip-local-cleanup-max-gb", String(localCleanupMaxGb));
      const report = await invoke<{ removedClips: number; keptFavorites: number; freedBytes: number; totalBytes: number }>("cleanup_old_local_clips", {
        olderThanDays: days,
        maxSizeGb: localCleanupMaxGb,
      });
      await loadClips();
      const freedGb = (report.freedBytes / 1024 / 1024 / 1024).toFixed(2);
      const totalGb = (report.totalBytes / 1024 / 1024 / 1024).toFixed(2);
      setLocalCleanupStatus(`Removed ${report.removedClips} clip(s), freed ${freedGb} GB. Kept ${report.keptFavorites} favourite(s). Total now ${totalGb} GB.`);
    } catch (e) {
      setLocalCleanupStatus(String(e));
    } finally {
      setLocalCleanupBusy(false);
    }
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
      setActiveView("library");
    }
  }

  const headerTitle =
    filter.kind === "favorites"
      ? "Favorites"
      : filter.kind === "tag"
        ? `#${filter.tag}`
        : filter.kind === "game"
          ? filter.game
        : "Library";
  const headerGame = filter.kind === "game" ? filter.game : null;
  const customStyle: CSSProperties = {
    "--ui-radius": `${radius}px`,
    "--font-scale": `${fontScale / 100}`,
    ...(bgStyle === "custom" && customBg
      ? { "--custom-bg": `url("${convertFileSrc(customBg)}")` }
      : {}),
  } as CSSProperties;

  return (
    <div
      className={`app-shell theme-${theme} bg-${bgStyle} sidebar-${sidebarPosition} player-${playerTheme} layout-${clipLayout} notification-${notificationStyle} ${appSuspended ? "app-suspended" : ""}`}
      style={customStyle}
    >
      {animatedBg && customBgVideo && !appSuspended && (
        <video
          ref={animatedBgRef}
          className="animated-bg-video"
          src={customBgVideo.startsWith("/") ? customBgVideo : convertFileSrc(customBgVideo)}
          autoPlay
          loop
          muted
          playsInline
        />
      )}
      {clipContextMenu && (
        <ClipContextMenu
          x={clipContextMenu.x}
          y={clipContextMenu.y}
          count={selectedIds[clipContextMenu.clipId] ? Math.max(1, selectedCount) : 1}
          games={gameCounts.filter((g) => g.name !== "General").map((g) => g.name)}
          tags={allTags}
          onMoveToGame={(game) => {
            void handleDropClipsToGame(game, clipContextMenu.clipId);
            setClipContextMenu(null);
          }}
          onAddTag={(tag) => {
            void handleTagSelected(tag);
            setClipContextMenu(null);
          }}
          onClose={() => setClipContextMenu(null)}
        />
      )}
      {errorToast && (
        <div className="app-error-toast" role="alert">
          <span className="app-error-toast-icon" aria-hidden>
            ⚠
          </span>
          <p className="app-error-toast-message">{errorToast}</p>
          <button
            type="button"
            className="app-error-toast-close"
            aria-label="Dismiss"
            onClick={() => setErrorToast(null)}
          >
            ×
          </button>
        </div>
      )}
      <Sidebar
        activeView={activeView}
        filter={filter}
        clipCount={clips.length}
        favoriteCount={favoriteCount}
        gameCounts={gameCounts}
        watchPath={watchPath}
        onViewChange={setActiveView}
        onFilterChange={setFilter}
        onRenameGame={(from, to) => {
          setGameAlias(from, to);
          setGameAliasVersion((v) => v + 1);
          if (filter.kind === "game" && filter.game === from) {
            setFilter({ kind: "game", game: to });
          }
        }}
        onDropClipsToGame={(game, draggedId) => void handleDropClipsToGame(game, draggedId)}
      />

      <main className="main-content">
        <header className="main-header">
          <h1>
            {activeView === "settings" ? (
              "Settings"
            ) : (
              <>
                {headerGame && <GameIcon name={headerGame} size={20} className="header-game-icon" />}
                {headerTitle}
              </>
            )}
          </h1>
          {activeView === "settings" ? (
            <p className="main-subtitle">Clip detection, tray behavior, and folder setup</p>
          ) : (
            <p className="main-subtitle">
              {filtered.length} clip{filtered.length === 1 ? "" : "s"}
              {searchQuery.trim() ? ` - matching "${searchQuery.trim()}"` : ""}
            </p>
          )}
        </header>

        {activeView === "settings" ? (
          <div className="settings-view">
            <div className="settings-tabs">
              {(
                [
                  { id: "appearance", label: "Appearance", icon: <IconPalette size={16} /> },
                  { id: "sharing", label: "Sharing", icon: <IconCloud size={16} /> },
                  { id: "storage", label: "Storage", icon: <IconDatabase size={16} /> },
                  { id: "obs", label: "OBS", icon: <IconCamera size={16} /> },
                  { id: "audio", label: "Audio", icon: <IconSpeaker size={16} /> },
                  { id: "shortcuts", label: "Shortcuts", icon: <IconKeyboard size={16} /> },
                  { id: "debug", label: "Debug", icon: <IconBug size={16} /> },
                ] as const
              ).map((tab) => (
                <button
                  key={tab.id}
                  type="button"
                  className={`settings-tab ${settingsTab === tab.id ? "active" : ""}`}
                  onClick={() => setSettingsTab(tab.id)}
                >
                  {tab.icon}
                  <span>{tab.label}</span>
                </button>
              ))}
            </div>

            {settingsTab === "appearance" && (
              <>
                <section className="settings-section appearance-section clean-settings-panel">
                  <h2>Appearance</h2>
                  <p>Change 533clip colors, layout, backgrounds, and player style.</p>
                  <p className="settings-subhead">Theme</p>
                  <div className="appearance-grid">
                    <label>
                      <span>Color</span>
                      <select className="tag-input" value={theme} onChange={(e) => setTheme(e.target.value)}>
                        <option value="violet">Violet</option>
                        <option value="emerald">Emerald</option>
                        <option value="sky">Sky</option>
                        <option value="rose">Rose</option>
                        <option value="amber">Amber</option>
                      </select>
                    </label>
                    <label>
                      <span>Background</span>
                      <select
                        className="tag-input"
                        value={bgStyle}
                        onChange={(e) => {
                          const value = e.target.value;
                          setBgStyle(value);
                          if (value !== "animated") setAnimatedBg(false);
                          if (value === "custom") {
                            void open({ multiple: false, filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }] }).then((file) => {
                              if (typeof file === "string") setCustomBg(file);
                            });
                          } else if (value === "animated") {
                            setAnimatedBg(true);
                            void open({ multiple: false, filters: [{ name: "Video", extensions: ["mp4", "webm"] }] }).then((file) => {
                              if (typeof file === "string") setCustomBgVideo(file);
                            });
                          }
                        }}
                      >
                        <option value="solid">Solid</option>
                        <option value="arena">Arena lights</option>
                        <option value="neon">Neon city</option>
                        <option value="space">Deep space</option>
                        <option value="forest">Forest</option>
                        <option value="custom">Custom image</option>
                        <option value="animated">Animated video</option>
                      </select>
                    </label>
                    <label>
                      <span>Rounded corners</span>
                      <select className="tag-input" value={radius} onChange={(e) => setRadius(Number(e.target.value))}>
                        <option value={0}>0%</option>
                        <option value={6}>25%</option>
                        <option value={12}>50%</option>
                        <option value={18}>75%</option>
                        <option value={24}>100%</option>
                      </select>
                    </label>
                  </div>

                  <p className="settings-subhead">Interface</p>
                  <div className="appearance-grid">
                    <label>
                      <span>Sidebar</span>
                      <select className="tag-input" value={sidebarPosition} onChange={(e) => setSidebarPosition(e.target.value)}>
                        <option value="left">Left</option>
                        <option value="top">Top</option>
                      </select>
                    </label>
                    <label>
                      <span>Player</span>
                      <select className="tag-input" value={playerTheme} onChange={(e) => setPlayerTheme(e.target.value)}>
                        <option value="minimal">Minimal</option>
                        <option value="cinematic">Cinematic</option>
                      </select>
                    </label>
                    <label>
                      <span>Clip layout</span>
                      <select className="tag-input" value={clipLayout} onChange={(e) => setClipLayout(e.target.value)}>
                        <option value="grid">Grid</option>
                        <option value="list">List</option>
                        <option value="compact">Compact</option>
                      </select>
                    </label>
                    <label className="settings-check appearance-check">
                      <input
                        type="checkbox"
                        checked={hoverPreviewEnabled}
                        onChange={(e) => setHoverPreviewEnabled(e.target.checked)}
                      />
                      <span>Hover previews</span>
                    </label>
                    <label>
                      <span>Startup</span>
                      <select className="tag-input" value={startupBehavior} onChange={(e) => setStartupBehavior(e.target.value)}>
                        <option value="normal">Normal</option>
                        <option value="minimized">Minimized</option>
                        <option value="tray">Tray only</option>
                      </select>
                    </label>
                    <label className="settings-check appearance-check">
                      <input
                        type="checkbox"
                        checked={launchOnStartup}
                        onChange={(e) => {
                          const next = e.target.checked;
                          setLaunchOnStartup(next);
                          void invoke<boolean>("set_launch_on_startup", { enabled: next })
                            .then(setLaunchOnStartup)
                            .catch((err) => {
                              setLaunchOnStartup(!next);
                              notifyError(err);
                            });
                        }}
                      />
                      <span>Open on Windows startup</span>
                    </label>
                  </div>

                  <p className="settings-subhead">Notifications</p>
                  <div className="appearance-grid">
                    <label>
                      <span>Style</span>
                      <select className="tag-input" value={notificationStyle} onChange={(e) => setNotificationStyle(e.target.value)}>
                        <option value="compact">Compact</option>
                        <option value="outplayed">Outplayed</option>
                        <option value="minimal">Minimal</option>
                      </select>
                    </label>
                    <label>
                      <span>Clip sound</span>
                      <select
                        className="tag-input"
                        value={clipSound}
                        onChange={(e) => {
                          const value = e.target.value;
                          setClipSound(value);
                          if (value === "custom") {
                            void open({ multiple: false, filters: [{ name: "Audio", extensions: ["mp3", "wav", "ogg"] }] }).then((file) => {
                              if (typeof file === "string") setCustomClipSound(file);
                            });
                          }
                        }}
                      >
                        <option value="chime">Chime</option>
                        <option value="pop">Pop</option>
                        <option value="soft">Soft</option>
                        <option value="custom">Custom file</option>
                        <option value="off">Off</option>
                      </select>
                      {clipSound === "custom" && (
                        <button
                          type="button"
                          className="inline-file-pick"
                          onClick={() => void open({ multiple: false, filters: [{ name: "Audio", extensions: ["mp3", "wav", "ogg"] }] }).then((file) => {
                            if (typeof file === "string") setCustomClipSound(file);
                          })}
                        >
                          {customClipSound ? customClipSound.split(/[\\/]/).pop() : "Choose sound..."}
                        </button>
                      )}
                    </label>
                  </div>
                </section>
              </>
            )}

            {settingsTab === "sharing" && (
              <>
            <R2SettingsPanel />
              </>
            )}

            {settingsTab === "storage" && (
              <>
              <section className="settings-section clean-settings-panel debug-panel">
                <h2>Clip Detection</h2>
                <p>533clip watches this folder directly and imports new OBS replay files.</p>
                <div className="settings-path">
                  <span>{watchPath ?? "No folder selected"}</span>
                </div>
                <div className="settings-actions">
                  <button type="button" className="btn ghost" onClick={() => void detectObsFolder()}>
                    Use OBS folder
                  </button>
                  <button type="button" className="btn ghost" onClick={() => void pickWatchFolder()}>
                    Browse...
                  </button>
                </div>
              </section>
              <section className="settings-section clean-settings-panel debug-panel">
                <h2>Storage Cleanup</h2>
                <p>Clean old local clips by date and folder size. Favourite clips are always kept.</p>
                <div className="settings-form-grid">
                  <label>
                    <span>Delete clips older than</span>
                    <select
                      className="tag-input"
                      value={localCleanupDays}
                      onChange={(e) => setLocalCleanupDays(Number(e.target.value))}
                    >
                      <option value={7}>7 days</option>
                      <option value={14}>14 days</option>
                      <option value={30}>30 days</option>
                      <option value={60}>60 days</option>
                      <option value={90}>90 days</option>
                      <option value={180}>180 days</option>
                    </select>
                  </label>
                  <label>
                    <span>Keep folder under</span>
                    <select
                      className="tag-input"
                      value={localCleanupMaxGb}
                      onChange={(e) => setLocalCleanupMaxGb(Number(e.target.value))}
                    >
                      <option value={5}>5 GB</option>
                      <option value={10}>10 GB</option>
                      <option value={20}>20 GB</option>
                      <option value={50}>50 GB</option>
                      <option value={100}>100 GB</option>
                      <option value={250}>250 GB</option>
                    </select>
                  </label>
                </div>
                <div className="settings-actions">
                  <button
                    type="button"
                    className="btn ghost"
                    disabled={localCleanupBusy}
                    onClick={() => void handleCleanupOldLocalClips()}
                  >
                    {localCleanupBusy ? "Cleaning..." : "Clean storage"}
                  </button>
                </div>
                {localCleanupStatus && <p className="settings-status">{localCleanupStatus}</p>}
              </section>
              </>
            )}

            {settingsTab === "audio" && <AudioControlPanel />}

            {settingsTab === "obs" && <ObsControlPanel />}

            {settingsTab === "shortcuts" && (
              <section className="settings-section clean-settings-panel">
                <h2>Keyboard Shortcuts</h2>
                <p>Work inside the clip player. Disabled while typing in a text field.</p>

                <p className="settings-subhead">Playback</p>
                <div className="shortcuts-grid">
                  <ShortcutRow keys={["Space", "K"]} label="Play / pause" />
                  <ShortcutRow keys={["J"]} label="Back 5 seconds" />
                  <ShortcutRow keys={["L"]} label="Forward 5 seconds" />
                  <ShortcutRow keys={["Home"]} label="Jump to start" />
                  <ShortcutRow keys={["End"]} label="Jump to end" />
                  <ShortcutRow keys={["↑"]} label="Volume up" />
                  <ShortcutRow keys={["↓"]} label="Volume down" />
                </div>

                <p className="settings-subhead">Navigation</p>
                <div className="shortcuts-grid">
                  <ShortcutRow keys={["←"]} label="Previous clip" />
                  <ShortcutRow keys={["→"]} label="Next clip" />
                  <ShortcutRow keys={["Esc"]} label="Close player" />
                </div>

                <p className="settings-subhead">Actions</p>
                <div className="shortcuts-grid">
                  <ShortcutRow keys={["F"]} label="Toggle favorite" />
                </div>
              </section>
            )}

            {settingsTab === "debug" && (
              <>
                <DebugPanel
                  watchPath={watchPath}
                  clips={clips}
                  onRefresh={() => {
                    void loadClips();
                    void loadTags();
                  }}
                  onCleanup={() => {
                    void invoke<{ removedMissingClips: number; removedOrphanThumbnails: number }>(
                      "cleanup_storage",
                    )
                      .then((report) => {
                        setErrorToast(
                          `Cleanup: removed ${report.removedMissingClips} missing clip(s), ${report.removedOrphanThumbnails} orphan thumbnail(s)`,
                        );
                        void loadClips();
                      })
                      .catch(notifyError);
                  }}
                  onRepair={() => {
                    void invoke<number>("repair_processing_clips")
                      .then((count) => {
                        setErrorToast(`Repair: requeued ${count} clip(s)`);
                        void loadClips();
                      })
                      .catch(notifyError);
                  }}
                />
              </>
            )}
          </div>
        ) : (
          <>
            <LibraryToolbar
              searchQuery={searchQuery}
              onSearchChange={setSearchQuery}
              selectedCount={selectedCount}
              onDeleteSelected={() => void handleDeleteSelected()}
              onTagSelected={(tag) => void handleTagSelected(tag)}
              onUntagSelected={(tag) => void handleUntagSelected(tag)}
              onClearSelection={clearSelection}
              lockedGame={lockedGame}
              knownGames={gameCounts.filter((g) => g.name !== "General").map((g) => g.name)}
              onOverrideGame={(game) => void handleOverrideLockedGame(game)}
            />

            <TagPanel
              tags={allTags}
              activeTag={activeTag}
              onSelectTag={handleTagSelect}
              onCreateTag={(t) => void handleCreateTag(t)}
              onDeleteTag={(t) => void handleDeleteTag(t)}
            />

            {groups.length === 0 ? (
              <div className="clips-scroll">
                <div className="empty-state">
                  <p className="empty-title">No clips here</p>
                  <p className="empty-body">
                    Save a replay into your watched folder or try another search / tag filter.
                  </p>
                </div>
              </div>
            ) : (
              <VirtualizedClipGrid
                groups={groups}
                selectedIds={selectedIds}
                onOpen={(c) => setPlayingClipId(c.id)}
                onToggleSelect={toggleSelected}
                onToggleFavorite={(id) => void handleToggleFavorite(id)}
                onDragStartClip={handleDragStartClip}
                layout={clipLayout}
                activePreviewId={activePreviewId}
                hoverPreviewEnabled={hoverPreviewEnabled && !appSuspended}
                onPreviewChange={setActivePreviewId}
                onContextMenu={handleClipContextMenu}
              />
            )}
          </>
        )}
      </main>

      {playingClip && (
        <PlayerModal
          key={playingClip.id}
          clip={playingClip}
          onClose={() => setPlayingClipId(null)}
          onUpdate={(c) => {
            upsertClip(c);
            void loadTags();
          }}
          onDeleted={removeClip}
          allTags={allTags}
          onTagsChange={loadTags}
          hasPrevious={playingIndex > 0}
          hasNext={playingIndex >= 0 && playingIndex < orderedClips.length - 1}
          onPrevious={() => {
            if (playingIndex > 0) setPlayingClipId(orderedClips[playingIndex - 1].id);
          }}
          onNext={() => {
            if (playingIndex >= 0 && playingIndex < orderedClips.length - 1) {
              setPlayingClipId(orderedClips[playingIndex + 1].id);
            }
          }}
          playerTheme={playerTheme}
          suspended={appSuspended}
        />
      )}
    </div>
  );
}

function ShortcutRow({ keys, label }: { keys: string[]; label: string }) {
  return (
    <div className="shortcut-row">
      <span className="shortcut-keys">
        {keys.map((k) => (
          <kbd key={k}>{k}</kbd>
        ))}
      </span>
      <span className="shortcut-label">{label}</span>
    </div>
  );
}

export default App;
