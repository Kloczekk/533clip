# 533clip

Lightweight, local-first clip manager for storing, viewing, trimming, and tagging gameplay clips without bloat.

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

## Features

- Watches a clip folder for new `.mp4`, `.mkv`, and `.mov` files
- Shows an in-app popup when a new clip is detected
- Runs in the system tray when the window is closed
- Settings tab for clip folder setup
- **Use OBS folder** can read the OBS recording path from profile `basic.ini`
- Saved watch folder persists in `settings.json` across restarts
- Scans the folder on startup for clips not yet in the library
- Job queue for metadata (`ffprobe`), thumbnails (`ffmpeg`), and trims
- JSON library in app data (`533clip/clips.json`)
- Dashboard grid grouped by date, favorites filter, search, and tags
- Player with trim timeline, volume, playback speed, and previous/next clip switching

## Setup

1. Open **Settings** in 533clip.
2. Pick the folder where your clips are saved.
3. Use **Use OBS folder** if you want 533clip to detect your OBS recording folder automatically.
4. Save clips into that folder. 533clip will detect them directly; OBS WebSocket is not required.

## Data Location

`%APPDATA%\com.five33clip.app\533clip\` (`clips.json`, `tags.json`, `settings.json`)
