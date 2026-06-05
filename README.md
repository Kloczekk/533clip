# 533clip

High-performance, local-first OBS clip manager (Tauri + React + Rust).

## Prerequisites

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://rustup.rs/)
- [FFmpeg](https://ffmpeg.org/) on your PATH (`ffmpeg` and `ffprobe` in a terminal)

## Run

```powershell
cd C:\Users\adamk\clipvault
npm install
npm run tauri dev
```

## Features (current)

- Watches OBS clip folder (`.mp4`, `.mkv`, `.mov`) with remux-friendly handling
- **Use OBS folder** reads paths from OBS profile `basic.ini`
- Saved watch folder persists in `settings.json` across restarts
- Scans the folder on startup for clips not yet in the library
- Job queue: metadata (ffprobe) + thumbnails (ffmpeg), no CMD windows
- JSON library in app data (`533clip/clips.json`)
- Dashboard grid grouped by date, favorites filter, tags

## OBS setup tips

1. In OBS: **Settings → Output → Recording** — note the path (often `Videos`).
2. In 533clip: **Use OBS folder** (sidebar) or **Browse…** to pick that same folder.
3. Replay buffer / recordings should write there; remux to `.mp4` in OBS if you use `.mkv` temporarily.
4. **WebSocket** (for “Replay saved” toast): OBS → **Tools → WebSocket Server Settings** → enable server, copy password into 533clip sidebar → **OBS WebSocket** → **Save OBS connection**. Green dot = connected.

## Player

- **Volume** and **Speed** (0.25×–1× slow motion) sliders in the clip player; values are remembered between sessions.

## Data location

`%APPDATA%\com.five33clip.app\533clip\` (`clips.json`, `tags.json`, `settings.json`)
