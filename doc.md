# 533clip Project Notes

## What This App Is

533clip is a Windows desktop clip manager built around OBS replay buffer output. The goal is to feel like a lightweight Outplayed/Medal-style app:

- OBS handles actual recording/clipping.
- 533clip watches the output folder.
- New clips appear in the app with thumbnails, hover previews, popups, and sounds.
- Clips can be grouped by game/app, tagged, trimmed, favorited, deleted, and shared.
- The app should be simple enough to give to friends.

The app is intentionally local-first. Cloud sharing is optional.

## Current Goals

- Keep OBS performance low.
- Avoid depending on OBS WebSocket unless the user explicitly chooses managed OBS mode.
- Make clip detection fast and reliable.
- Make new clip notifications feel instant.
- Make grouping usable even when automatic game/app detection gets it wrong.
- Make sharing to Discord easy through local export or B2/R2 link upload.

## Tech Stack

- Frontend: React 19, TypeScript, Vite, Zustand.
- Desktop/backend: Tauri 2, Rust.
- File watching: `notify` + `notify-debouncer-full`.
- Video tools: bundled FFmpeg/FFprobe sidecars in `src-tauri/binaries`.
- Cloud uploads: S3-compatible SDK for Cloudflare R2 and Backblaze B2.
- Windows integration: tray icon, single instance, active foreground window detection, startup registry entry.

## Useful Commands

From repo root:

```powershell
npm run build
cargo check --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

Installer output:

```text
C:\Users\adamk\clipvault\src-tauri\target\release\bundle\nsis\533clip_0.1.0_x64-setup.exe
```

Clean Rust build cache:

```powershell
cargo clean --manifest-path src-tauri\Cargo.toml
```

This deletes `src-tauri\target`. It is safe, but next build is slow.

## Important Files

Frontend:

- `src/App.tsx`: main app state, settings tabs, event listeners, library/player wiring.
- `src/components/PlayerModal.tsx`: full-screen clip player, trimming, export/share controls.
- `src/components/ClipCard.tsx`: library card, selection, hover preview, drag behavior.
- `src/components/Sidebar.tsx`: navigation, game/app groups, rename, drag-drop target.
- `src/components/CaptureOverlay.tsx`: always-on-top popup window UI and sound playback.
- `src/components/ObsControlPanel.tsx`: OBS settings, presets, capture mode, hotkeys.
- `src/components/R2SettingsPanel.tsx`: R2/B2 settings, import/export friend config.
- `src/components/DebugPanel.tsx`: debug stats and recent events.
- `src/store/clipStore.ts`: Zustand clip/filter/selection state.
- `src/utils/gameName.ts`: frontend display grouping and aliases.
- `src/styles/global.css`: all styling.

Backend:

- `src-tauri/src/lib.rs`: Tauri setup, commands, tray, overlay window, OBS monitors, sharing commands.
- `src-tauri/src/watcher/service.rs`: recursive watched-folder service and clip event handling.
- `src-tauri/src/watcher/stability.rs`: waits for files to stop growing before importing.
- `src-tauri/src/pipeline/mod.rs`: creates Clip records, emits events, queues probe/thumb jobs.
- `src-tauri/src/queue/worker.rs`: background probe, thumbnail, trim work.
- `src-tauri/src/models/clip.rs`: Clip model and stable clip IDs.
- `src-tauri/src/storage/json_store.rs`: `clips.json` store.
- `src-tauri/src/storage/settings.rs`: app settings, OBS settings, B2/R2 settings.
- `src-tauri/src/storage/tags.rs`: known tag registry.
- `src-tauri/src/ffmpeg/*`: probe, thumbnail, trim, Discord export helpers.
- `src-tauri/src/obs.rs`: OBS config editing, launch/control, presets, audio controls.
- `src-tauri/src/active_window.rs`: foreground app/game detection and stable last app cache.
- `src-tauri/src/sharing.rs`: R2/B2 upload/delete and signed links.

Cloud redirect:

- `redirect-worker/worker.js`: Cloudflare Worker that turns short URLs into signed B2 links.
- `redirect-worker/README.md`: deployment notes.

## Data Storage

Runtime app data is under the Tauri app data dir, then `533clip`.

Important runtime files:

- `clips.json`: all indexed clips and metadata.
- `settings.json`: watched folder, OBS mode, cloud sharing settings.
- `tags.json`: tag registry.
- `thumbnails/`: generated JPEG thumbnails.
- `shares/`: Discord/B2 export outputs.

Do not delete app data unless you want to reset the library.

## Clip Model

Rust model: `src-tauri/src/models/clip.rs`.

Fields:

- `id`
- `file_path`
- `file_name`
- `display_name`
- `game_name`
- `created_at`
- `duration`
- `resolution`
- `thumbnail_path`
- `is_favorite`
- `tags`
- `status`: `processing | ready | failed`
- `share_url`
- `share_key`
- `shared_at`

IDs are stable hashes from file path + creation timestamp.

## Clip Detection Flow

1. User saves replay in OBS.
2. `WatcherService` sees create/modify event in watched folder.
3. Popup fires early, before full import, for faster feedback.
4. Watcher waits until file is stable.
5. Pipeline creates/upserts a `Clip` with status `processing`.
6. Queue runs `ffprobe` to read metadata.
7. Queue runs `ffmpeg` to generate thumbnail.
8. Once duration and thumbnail exist, clip becomes `ready`.
9. Frontend receives `clip://updated` and updates library.

Important recent watcher behavior:

- Recursive watching is enabled.
- Delete/remove events should be ignored.
- MKV/MP4 remux pairs are deduped by stem to avoid double popup/sound.
- Existing clips are scanned when folder is set or restored.

## Popup/Sound Flow

The popup is a separate Tauri webview window:

- Label: `capture-overlay`
- Frontend component: `CaptureOverlay`
- Backend emit functions in `lib.rs`.

Events:

- `game://ready`
- `clip://saved-overlay`
- `recording://state`

Sound is played inside `CaptureOverlay.tsx`, not the main app.

Design note:

- The overlay should stay alive/invisible instead of being destroyed.
- If it dies, backend tries to recreate it before showing.

## Game/App Detection

Backend file: `src-tauri/src/active_window.rs`.

Current strategy:

- A background ready monitor samples foreground window.
- After a target stays stable for a few samples, it becomes the remembered app/game.
- Clip import prefers the remembered stable app over the instant foreground app.

Reason:

- Instant foreground app is unreliable on multi-monitor setups.
- User may hover Discord/Chrome while clipping a game.
- We still want clipping any app, so Chrome/Discord should not be globally blocked.
- The better solution is "stable target lock", not a giant ignore list.

Known issue:

- Automatic detection can still fail if the user never focuses the target long enough before clipping.

Manual fix:

- Multi-select clips.
- Drag selected clips onto a game/app group in the sidebar.
- Backend command: `set_clips_game`.

## Grouping Semantics

Frontend grouping uses `src/utils/gameName.ts`.

- If `clip.gameName` exists, use that.
- Else infer from parent folder.
- Else infer from filename.
- Else fallback is `Ungrouped`.

Current special behavior:

- Sidebar `General` means all clips, not only ungrouped.
- Dedicated game/app tabs still filter to that name.
- Game aliases are stored in localStorage under `533clip-game-aliases`.

## Selection, Bulk Actions, Drag-Drop

Selection lives in `src/store/clipStore.ts`.

Bulk actions:

- Delete selected.
- Tag selected.
- Untag selected.
- Drag selected to a game/app group.

Drag behavior:

- `ClipCard` is draggable.
- If dragged clip is selected, all selected clips move.
- If dragged clip is not selected, only that clip moves.
- `Sidebar` game rows are drop targets.

If drag feels broken, check:

- `ClipCard.tsx` `draggable` and `onDragStart`.
- `Sidebar.tsx` `onDragOver`/`onDrop`.
- `App.tsx` `dragClipIds` and `handleDropClipsToGame`.
- Child images/videos should have `draggable={false}` so they do not steal the drag.

## Trimming

Frontend: `PlayerModal.tsx`.

Backend:

- Command: `queue_trim_clip`
- Job kind: `JobKind::Trim`
- Worker: `queue/worker.rs`
- FFmpeg helper: `ffmpeg/trim.rs`

Expected UX:

- User adjusts timeline range.
- User clicks scissors.
- In-app popup asks:
  - Keep original
  - Delete original
  - Cancel
- New trimmed clip keeps the original clip creation date.
- New trimmed clip inherits game/app and display title.
- New trimmed clip shows popup/sound.
- If delete original is chosen, original is deleted only after trim succeeds.

Known fragile area:

- The player timeline has had several bugs around start/end dragging and playback position.
- If trimming breaks, inspect `PlayerModal.tsx` and `TrimTimeline.tsx`.

## Discord Export

Frontend button in `PlayerModal.tsx`.

Backend command:

- `export_clip_for_discord`

FFmpeg helper:

- `src-tauri/src/ffmpeg/share.rs`

Output:

- App data `shares/{clip_id}-discord.mp4`
- Then `reveal_path` opens Explorer at the exported file.

FFmpeg is bundled in installer:

- `src-tauri/binaries/ffmpeg-x86_64-pc-windows-msvc.exe`
- `src-tauri/binaries/ffprobe-x86_64-pc-windows-msvc.exe`

If Discord export fails on a clean PC, first check sidecar resolution in:

- `src-tauri/src/ffmpeg/command.rs`

## Cloud Sharing

Supported providers:

- Cloudflare R2
- Backblaze B2

Frontend:

- `R2SettingsPanel.tsx`

Backend:

- `sharing.rs`
- commands in `lib.rs`

Behavior:

- Export/compress clip for sharing.
- Upload to S3-compatible bucket.
- Return either public URL or private signed link.
- Copy link to clipboard.

Friend config:

- Export/import config exists.
- It can include B2/R2 keys so a friend can upload using same bucket.
- Be careful with sharing keys publicly.

## OBS Integration

There are three OBS integration modes:

- `manual`: default, safest. 533clip watches folder only.
- `managed`: 533clip can launch/control OBS via config/WebSocket.
- `off`: disables OBS-related monitoring/control.

Why manual default:

- Managed/WebSocket experiments caused performance confusion.
- OBS itself should remain responsible for replay buffer recording.
- 533clip should not make OBS heavier unless user opts in.

OBS features include:

- Launch OBS.
- Start replay buffer.
- Save replay buffer.
- Toggle recording.
- Hotkey editing.
- Replay duration.
- Audio input/mute/volume.
- Capture mode switch: display/game capture.
- Quality presets: high, medium, low, potato, 533.

Important:

- OBS preset editing writes OBS profile config and often requires OBS restart.
- Display capture can use high GPU in OBS. This is usually OBS, not 533clip.

## Settings Tabs

Current settings sections:

- Appearance
- Sharing
- Storage
- OBS
- Audio
- Debug

Settings are split between:

- localStorage for frontend-only preferences.
- `settings.json` via Rust for important app/backend settings.

## Installer / Bundling

Tauri config:

- `src-tauri/tauri.conf.json`

Important:

- `externalBin` includes FFmpeg and FFprobe sidecars.
- Installer is large because FFmpeg binaries are large.

Generated installer path:

```text
src-tauri\target\release\bundle\nsis\533clip_0.1.0_x64-setup.exe
```

## Recent Bugs / History

Things that were fixed or worked around:

- Multiple tray icons.
- OBS WebSocket perf confusion.
- Overlay popup focus stealing.
- Overlay window dying after long usage.
- CMD windows from `tasklist`/Windows commands.
- Missing FFmpeg on friends' PCs.
- Clips stuck forever in `processing`.
- Watch folder switching not persisting reliably.
- Recursive folder watching.
- MKV/MP4 remux deletion.
- Double popup/sound from remux pairs.
- Delete events triggering clip saved popup.
- General tab changed to show all clips.

## Current Risk Areas

Highest-risk code:

- `watcher/service.rs`: clip event timing, delete/modify filtering, dedupe.
- `active_window.rs`: app/game detection accuracy.
- `PlayerModal.tsx` + `TrimTimeline.tsx`: trim UI/playback state.
- `queue/worker.rs`: trim/probe/thumbnail job completion and status changes.
- `ffmpeg/command.rs`: bundled sidecar resolution.
- `ObsControlPanel.tsx` + `obs.rs`: managed OBS control.

## Suggestions For Next Work

1. Add a "manual move to game/app" command in right-click context menu, not only drag-drop.
2. Add an "all apps" mode and "game lock" UI indicator:
   - show current detected/locked app in the toolbar
   - allow clicking it to override before clipping
3. Add debug events for watcher event kind:
   - create
   - modify
   - remove
   - ignored delete
   - duplicate stem skipped
4. Add explicit "Repair library" button:
   - re-probe failed/processing clips
   - regenerate missing thumbnails
5. Add local file open / reveal buttons on each clip card.
6. Add a packaged update/version workflow before sharing widely.

## Development Style Notes

Keep changes conservative. This project has a lot of small behavior fixes accumulated from real testing.

Preferred approach:

- Read current code before refactoring.
- Avoid broad rewrites.
- Preserve local data formats unless migration is added.
- Keep frontend UI simple and dense.
- Always run:

```powershell
cargo check --manifest-path src-tauri\Cargo.toml
npm run build
```

Before giving installer:

```powershell
npm run tauri build
```

## Terms

- "Clip": OBS replay file imported by 533clip.
- "Watch folder": directory 533clip monitors for new OBS outputs.
- "Overlay": always-on-top popup window for ready/saved/recording events.
- "Ready to clip": app/game was detected and 533clip is ready to label clips with it.
- "General": all clips view.
- "Ungrouped": clips that could not be assigned a game/app.

