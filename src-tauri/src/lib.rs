mod active_window;
mod ffmpeg;
mod models;
mod obs;
mod pipeline;
mod queue;
mod sharing;
mod storage;
mod trim_paths;
mod watcher;

use crate::models::clip::Clip;
use crate::ffmpeg::export_for_discord;
use crate::queue::JobQueue;
use crate::queue::{init_job_queue, Job, JobKind};
use crate::storage::{
    merge_tags_from_clips, ClipStore, ObsSettingsResponse, ObsSettingsUpdate, R2SettingsResponse,
    R2SettingsUpdate, SettingsStore, TagRegistryStore,
};
use crate::trim_paths::trimmed_output_path;
use crate::watcher::WatcherService;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tracing_subscriber::EnvFilter;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DebugEventPayload {
    pub message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GameReadyPayload {
    pub game_name: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClipSavedOverlayPayload {
    pub game_name: Option<String>,
    pub file_name: String,
    pub display_name: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordingOverlayPayload {
    pub active: bool,
}

pub(crate) fn emit_debug(app: &tauri::AppHandle, message: impl Into<String>) {
    let _ = app.emit(
        "debug://event",
        DebugEventPayload {
            message: message.into(),
        },
    );
}

pub struct AppState {
    pub store: ClipStore,
    pub tags: TagRegistryStore,
    pub settings: SettingsStore,
    pub watcher: WatcherService,
    pub queue: JobQueue,
    pub data_dir: PathBuf,
}

fn normalize_tag(tag: &str) -> Result<String, String> {
    let t = tag.trim().to_lowercase();
    if t.is_empty() {
        return Err("tag cannot be empty".into());
    }
    if t.len() > 32 {
        return Err("tag too long (max 32)".into());
    }
    Ok(t)
}

fn create_capture_overlay_window(app: &tauri::App) -> Result<(), String> {
    if app.get_webview_window("capture-overlay").is_some() {
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(
        app,
        "capture-overlay",
        WebviewUrl::App("index.html?overlay".into()),
    )
    .title("533clip capture")
    .inner_size(340.0, 84.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(false)
    .build()
    .map_err(|e| e.to_string())?;
    let _ = window.set_ignore_cursor_events(true);
    force_overlay_topmost(&window);
    Ok(())
}

#[cfg(windows)]
fn force_overlay_topmost(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowLongPtrW, SetWindowPos, GetWindowLongPtrW, GWL_EXSTYLE, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_EX_NOACTIVATE,
    };

    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            // SWP_NOACTIVATE only stops *this* SetWindowPos call from
            // activating the window — it doesn't stop Windows from still
            // treating the overlay's mere appearance as reason to knock a
            // game out of DirectX exclusive fullscreen (this is what was
            // breaking Roblox input). WS_EX_NOACTIVATE is the persistent
            // window style that makes the window permanently unactivatable,
            // which is what actually keeps exclusive-fullscreen games from
            // losing focus/input when the overlay shows.
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                ex_style | (WS_EX_NOACTIVATE.0 as isize),
            );
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }
}

#[cfg(not(windows))]
fn force_overlay_topmost(window: &tauri::WebviewWindow) {
    let _ = window.set_always_on_top(true);
}

fn position_capture_overlay(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.current_monitor().or_else(|_| window.primary_monitor()) {
        let pos = monitor.position();
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let width = 340.0;
        let margin = 18.0;
        let x = pos.x as f64 / scale + size.width as f64 / scale - width - margin;
        let y = pos.y as f64 / scale + margin;
        let _ = window.set_position(LogicalPosition::new(x, y));
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        force_foreground(&window);
    }
}

// Re-launching from Windows Search/Start while already running routes
// through tauri-plugin-single-instance's callback, which runs inside the
// already-running (backgrounded) process — not the short-lived one Windows
// just spawned in response to the click. Windows only grants
// SetForegroundWindow to whichever process last received user input, so a
// plain set_focus() from a backgrounded process is silently ignored and the
// window just sits there behind whatever's on top. Toggling topmost is the
// standard workaround to force it above the current foreground window.
#[cfg(windows)]
fn force_foreground(window: &tauri::WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetForegroundWindow, SetWindowPos, ShowWindow, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE,
        SWP_NOSIZE, SW_RESTORE,
    };

    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            let _ = SetWindowPos(hwnd, Some(HWND_NOTOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

#[cfg(not(windows))]
fn force_foreground(_window: &tauri::WebviewWindow) {}

fn show_game_ready_overlay(app: &tauri::AppHandle, game_name: String) {
    let window = match app.get_webview_window("capture-overlay") {
        Some(window) => window,
        None => match WebviewWindowBuilder::new(
            app,
            "capture-overlay",
            WebviewUrl::App("index.html?overlay".into()),
        )
        .title("533clip capture")
        .inner_size(340.0, 84.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        {
            Ok(window) => {
                let _ = window.set_ignore_cursor_events(true);
                window
            }
            Err(_) => return,
        },
    };

    position_capture_overlay(&window);
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.set_always_on_top(true);
    let _ = window.show();
    force_overlay_topmost(&window);
    let _ = app.emit(
        "game://ready",
        GameReadyPayload {
            game_name: game_name.clone(),
        },
    );

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        if let Some(window) = app.get_webview_window("capture-overlay") {
            force_overlay_topmost(&window);
        }
    });
}

pub(crate) fn show_clip_saved_overlay(
    app: &tauri::AppHandle,
    game_name: Option<String>,
    file_name: String,
    display_name: Option<String>,
) {
    let window = match app.get_webview_window("capture-overlay") {
        Some(window) => window,
        None => match WebviewWindowBuilder::new(
            app,
            "capture-overlay",
            WebviewUrl::App("index.html?overlay".into()),
        )
        .title("533clip capture")
        .inner_size(340.0, 84.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        {
            Ok(window) => {
                let _ = window.set_ignore_cursor_events(true);
                window
            }
            Err(_) => return,
        },
    };

    position_capture_overlay(&window);
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.set_always_on_top(true);
    let _ = window.show();
    force_overlay_topmost(&window);
    let _ = app.emit(
        "clip://saved-overlay",
        ClipSavedOverlayPayload {
            game_name,
            file_name,
            display_name,
        },
    );

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        if let Some(window) = app.get_webview_window("capture-overlay") {
            force_overlay_topmost(&window);
        }
    });
}

fn show_recording_overlay(app: &tauri::AppHandle, active: bool) {
    let window = match app.get_webview_window("capture-overlay") {
        Some(window) => window,
        None => match WebviewWindowBuilder::new(
            app,
            "capture-overlay",
            WebviewUrl::App("index.html?overlay".into()),
        )
        .title("533clip capture")
        .inner_size(340.0, 84.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        {
            Ok(window) => {
                let _ = window.set_ignore_cursor_events(true);
                window
            }
            Err(_) => return,
        },
    };

    position_capture_overlay(&window);
    let _ = window.set_ignore_cursor_events(true);
    let _ = window.set_always_on_top(true);
    let _ = window.show();
    force_overlay_topmost(&window);
    let _ = app.emit("recording://state", RecordingOverlayPayload { active });
}

fn spawn_game_ready_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut candidate: Option<String> = None;
        let mut candidate_count = 0u8;
        let mut active_game: Option<String> = None;
        loop {
            if app
                .state::<AppState>()
                .settings
                .obs()
                .map(|settings| settings.integration_mode == "off")
                .unwrap_or(false)
            {
                candidate = None;
                candidate_count = 0;
                active_game = None;
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
            if !obs::is_obs_running() {
                candidate = None;
                candidate_count = 0;
                active_game = None;
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            // This calls into unsafe Win32 FFI every 2s for as long as OBS is
            // running. catch_unwind so a panic there (a foreground window
            // from some third-party app violating the usual contract, etc.)
            // can't take the whole process down instead of just skipping
            // that one poll.
            let detected = std::panic::catch_unwind(crate::active_window::detect_game_name)
                .unwrap_or_else(|_| {
                    tracing::warn!("detect_game_name panicked, skipping this poll");
                    None
                });
            if let Some(game_name) = detected {
                if candidate.as_deref() == Some(&game_name) {
                    candidate_count = candidate_count.saturating_add(1);
                } else {
                    candidate = Some(game_name.clone());
                    candidate_count = 1;
                }

                if candidate_count >= 2 && active_game.as_deref() != Some(&game_name) {
                    active_game = Some(game_name.clone());
                    crate::active_window::remember_game_name(&game_name);
                    let _ = app.emit(
                        "game://locked",
                        GameReadyPayload {
                            game_name: game_name.clone(),
                        },
                    );
                    show_game_ready_overlay(&app, game_name);
                }
            } else {
                candidate = None;
                candidate_count = 0;
                active_game = None;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

fn spawn_recording_monitor(app: tauri::AppHandle, settings: SettingsStore) {
    tauri::async_runtime::spawn(async move {
        let mut last: Option<bool> = None;
        let mut last_error: Option<String> = None;
        loop {
            let obs_settings = match settings.obs() {
                Ok(settings) => settings,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };
            if obs_settings.integration_mode != "managed" {
                last = None;
                last_error = None;
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
            if !obs::is_obs_running() {
                last = Some(false);
                last_error = None;
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            match obs::recording_active(&obs_settings).await {
                Ok(active) => {
                    if last.is_some() && last != Some(active) {
                        show_recording_overlay(&app, active);
                    }
                    last = Some(active);
                    last_error = None;
                }
                Err(e) => {
                    // Recording state polling was previously a silent no-op
                    // on any WebSocket/auth failure — no popup would ever
                    // fire and nothing indicated why. Surface it once per
                    // distinct error instead of on every 2.5s poll.
                    if last_error.as_deref() != Some(e.as_str()) {
                        tracing::warn!(error = %e, "recording state check failed");
                        emit_debug(&app, format!("OBS recording check failed: {e}"));
                        last_error = Some(e);
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(2500)).await;
        }
    });
}

async fn wait_for_obs_exit(max_wait: Duration) {
    let deadline = tokio::time::Instant::now() + max_wait;
    while obs::is_obs_running() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[tauri::command]
async fn set_watch_path(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let pb = PathBuf::from(&path);
    state
        .settings
        .set_watch_path(Some(&path))
        .map_err(|e| e.to_string())?;
    state
        .watcher
        .set_watch_path(pb)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn detect_obs_recording_paths() -> Vec<String> {
    obs::detect_recording_paths()
}

/// Native OS drag-out (drop the clip file straight into Discord/Explorer/
/// etc as a real file), not the in-app HTML5 drag used for moving a clip
/// onto a sidebar game row. `DoDragDrop` (what the `drag` crate wraps on
/// Windows) blocks until the user drops or cancels, so this runs on a
/// dedicated OS thread rather than a tokio worker.
#[tauri::command]
async fn start_file_drag(
    window: tauri::WebviewWindow,
    path: String,
    thumbnail: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let pb = PathBuf::from(&path);
        if !pb.is_file() {
            return Err("file not found".to_string());
        }
        let image = thumbnail
            .map(PathBuf::from)
            .filter(|t| t.is_file())
            .map(drag::Image::File)
            .unwrap_or_else(|| drag::Image::Raw(Vec::new()));
        drag::start_drag(
            &window,
            drag::DragItem::Files(vec![pb]),
            image,
            |_result, _pos| {},
            drag::Options::default(),
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("explorer.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .arg(format!("/select,\"{}\"", path))
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("reveal is only implemented on Windows".into())
    }
}

/// Puts a file on the Windows clipboard as CF_HDROP, the same format Explorer
/// uses for copied files, so the exported clip can be pasted (Ctrl+V)
/// directly into a Discord message instead of dragging it in from Explorer.
#[tauri::command]
fn copy_file_to_clipboard(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
        use windows::Win32::UI::Shell::DROPFILES;

        const CF_HDROP: u32 = 15;

        if !std::path::Path::new(&path).is_file() {
            return Err("file not found".into());
        }

        let mut wide: Vec<u16> = std::ffi::OsStr::new(&path).encode_wide().collect();
        wide.push(0); // terminate this file name
        wide.push(0); // terminate the (single-entry) file list

        let header_size = std::mem::size_of::<DROPFILES>();
        let data_size = header_size + wide.len() * std::mem::size_of::<u16>();

        unsafe {
            let hglobal = GlobalAlloc(GMEM_MOVEABLE, data_size).map_err(|e| e.to_string())?;
            let ptr = GlobalLock(hglobal);
            if ptr.is_null() {
                return Err("failed to lock clipboard memory".into());
            }

            std::ptr::write_bytes(ptr as *mut u8, 0, data_size);
            let dropfiles = ptr as *mut DROPFILES;
            (*dropfiles).pFiles = header_size as u32;
            (*dropfiles).fWide = true.into();

            let file_ptr = (ptr as *mut u8).add(header_size) as *mut u16;
            std::ptr::copy_nonoverlapping(wide.as_ptr(), file_ptr, wide.len());

            let _ = GlobalUnlock(hglobal);

            OpenClipboard(None).map_err(|e| e.to_string())?;
            let empty_result = EmptyClipboard();
            let set_result = SetClipboardData(CF_HDROP, Some(HANDLE(hglobal.0)));
            let _ = CloseClipboard();

            empty_result.map_err(|e| e.to_string())?;
            set_result.map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("clipboard file copy is only implemented on Windows".into())
    }
}

#[tauri::command]
fn get_r2_settings(state: tauri::State<'_, AppState>) -> Result<R2SettingsResponse, String> {
    state.settings.r2_response().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_r2_settings(
    state: tauri::State<'_, AppState>,
    settings: R2SettingsUpdate,
) -> Result<R2SettingsResponse, String> {
    state
        .settings
        .set_r2(settings)
        .map_err(|e| e.to_string())?;
    state.settings.r2_response().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_obs_settings(state: tauri::State<'_, AppState>) -> Result<ObsSettingsResponse, String> {
    let mut response = state.settings.obs_response().map_err(|e| e.to_string())?;
    if response.integration_mode == "managed"
        && (response.websocket_url.trim().is_empty() || response.websocket_url == "ws://127.0.0.1:4455")
    {
        if let Some(url) = obs::detect_websocket_url() {
            response.websocket_url = url;
        }
    }
    Ok(response)
}

#[tauri::command]
fn set_obs_settings(
    state: tauri::State<'_, AppState>,
    settings: ObsSettingsUpdate,
) -> Result<ObsSettingsResponse, String> {
    state
        .settings
        .set_obs(settings)
        .map_err(|e| e.to_string())?;
    if state.settings.obs().map_err(|e| e.to_string())?.integration_mode == "managed" {
        let _ = obs::ensure_websocket_enabled_config();
    }
    state.settings.obs_response().map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_obs_replay_hotkey(
    state: tauri::State<'_, AppState>,
    hotkey: String,
) -> Result<Vec<String>, String> {
    let was_running = obs::is_obs_running();
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    let updated = obs::set_replay_save_hotkey(&hotkey)?;
    let _ = obs::disable_auto_remux_profiles();
    if was_running {
        let _ = obs::exit_obs(&settings).await;
        wait_for_obs_exit(Duration::from_secs(8)).await;
        if !obs::is_obs_running() {
            obs::launch_minimized_to_tray(settings.start_replay_on_launch)?;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        if settings.start_replay_on_launch {
            let _ = obs::start_replay_buffer(&settings).await;
        }
    }
    Ok(updated)
}

#[tauri::command]
async fn set_obs_recording_hotkey(
    state: tauri::State<'_, AppState>,
    hotkey: String,
) -> Result<Vec<String>, String> {
    let was_running = obs::is_obs_running();
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    let updated = obs::set_recording_toggle_hotkey(&hotkey)?;
    if was_running {
        let _ = obs::exit_obs(&settings).await;
        wait_for_obs_exit(Duration::from_secs(8)).await;
        if !obs::is_obs_running() {
            obs::launch_minimized_to_tray(settings.start_replay_on_launch)?;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        if settings.start_replay_on_launch {
            let _ = obs::start_replay_buffer(&settings).await;
        }
    }
    Ok(updated)
}

#[tauri::command]
fn get_launch_on_startup() -> Result<bool, String> {
    #[cfg(windows)]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = Command::new("reg")
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "query",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "533clip",
            ])
            .output()
            .map_err(|e| e.to_string())?;
        Ok(output.status.success())
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

#[tauri::command]
fn set_launch_on_startup(enabled: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut cmd = Command::new("reg");
        cmd.creation_flags(CREATE_NO_WINDOW);
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            cmd.args([
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "533clip",
                "/t",
                "REG_SZ",
                "/d",
                &format!("\"{}\"", exe.to_string_lossy()),
                "/f",
            ]);
        } else {
            cmd.args([
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "533clip",
                "/f",
            ]);
        }
        let output = cmd.output().map_err(|e| e.to_string())?;
        if enabled && !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(enabled)
    }
    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

#[tauri::command]
async fn set_obs_replay_duration(
    state: tauri::State<'_, AppState>,
    seconds: u32,
) -> Result<Vec<String>, String> {
    let was_running = obs::is_obs_running();
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    let updated = obs::set_replay_duration(seconds)?;
    if was_running {
        let _ = obs::exit_obs(&settings).await;
        wait_for_obs_exit(Duration::from_secs(8)).await;
        if !obs::is_obs_running() {
            obs::launch_minimized_to_tray(settings.start_replay_on_launch)?;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        if settings.start_replay_on_launch {
            let _ = obs::start_replay_buffer(&settings).await;
        }
    }
    Ok(updated)
}

#[tauri::command]
async fn set_obs_capture_source_mode(
    state: tauri::State<'_, AppState>,
    mode: String,
) -> Result<obs::ObsRuntimeStatus, String> {
    let was_running = obs::is_obs_running();
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    obs::set_capture_source_mode(&mode)?;
    if was_running {
        let _ = obs::exit_obs(&settings).await;
        wait_for_obs_exit(Duration::from_secs(8)).await;
        if !obs::is_obs_running() {
            obs::launch_minimized_to_tray(settings.start_replay_on_launch)?;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        if settings.start_replay_on_launch {
            let _ = obs::start_replay_buffer(&settings).await;
        }
    }
    Ok(obs::runtime_status(&settings).await)
}

#[tauri::command]
async fn obs_status(state: tauri::State<'_, AppState>) -> Result<obs::ObsRuntimeStatus, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    if settings.integration_mode != "managed" {
        return Ok(obs::ObsRuntimeStatus {
            installed: obs::detect_obs_executable().is_some(),
            running: obs::is_obs_running(),
            websocket_connected: false,
            replay_buffer_active: false,
            path: obs::detect_obs_executable().map(|p| p.to_string_lossy().into_owned()),
            error: Some("OBS integration is manual/off. 533clip is only watching the folder.".into()),
        });
    }
    Ok(obs::runtime_status(&settings).await)
}

#[tauri::command]
async fn obs_stats(state: tauri::State<'_, AppState>) -> Result<obs::ObsStats, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    obs::stats(&settings).await
}

#[tauri::command]
async fn obs_launch(state: tauri::State<'_, AppState>) -> Result<obs::ObsRuntimeStatus, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    let remux_changed = obs::disable_auto_remux_profiles()
        .map(|updated| !updated.is_empty())
        .unwrap_or(false);
    if obs::is_obs_running() && remux_changed {
        let _ = obs::exit_obs(&settings).await;
        wait_for_obs_exit(Duration::from_secs(8)).await;
    }
    if !obs::is_obs_running() {
        obs::launch_minimized_to_tray(settings.start_replay_on_launch)?;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Ok(obs::runtime_status(&settings).await)
}

#[tauri::command]
async fn obs_start_replay_buffer(state: tauri::State<'_, AppState>) -> Result<obs::ObsRuntimeStatus, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    obs::start_replay_buffer(&settings).await?;
    Ok(obs::runtime_status(&settings).await)
}

#[tauri::command]
async fn obs_stop_replay_buffer(state: tauri::State<'_, AppState>) -> Result<obs::ObsRuntimeStatus, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    obs::stop_replay_buffer(&settings).await?;
    Ok(obs::runtime_status(&settings).await)
}

#[tauri::command]
async fn obs_toggle_recording(state: tauri::State<'_, AppState>) -> Result<obs::ObsRuntimeStatus, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    obs::toggle_recording(&settings).await?;
    Ok(obs::runtime_status(&settings).await)
}

#[tauri::command]
async fn obs_save_replay_buffer(state: tauri::State<'_, AppState>) -> Result<obs::ObsRuntimeStatus, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    obs::save_replay_buffer(&settings).await?;
    tokio::time::sleep(Duration::from_millis(600)).await;
    Ok(obs::runtime_status(&settings).await)
}

#[tauri::command]
async fn obs_apply_quality_preset(
    state: tauri::State<'_, AppState>,
    preset: String,
) -> Result<obs::ObsRuntimeStatus, String> {
    let was_running = obs::is_obs_running();
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    if was_running {
        let _ = obs::stop_replay_buffer(&settings).await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let _ = obs::exit_obs(&settings).await;
        wait_for_obs_exit(Duration::from_secs(8)).await;
        if obs::is_obs_running() {
            obs::stop_processes();
            wait_for_obs_exit(Duration::from_secs(4)).await;
        }
    }
    obs::apply_quality_preset(&preset)?;
    let _ = obs::disable_auto_remux_profiles();
    if was_running {
        if !obs::is_obs_running() {
            obs::launch_minimized_to_tray(settings.start_replay_on_launch)?;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        if settings.start_replay_on_launch {
            let _ = obs::start_replay_buffer(&settings).await;
        }
    }
    Ok(obs::runtime_status(&settings).await)
}

#[tauri::command]
async fn obs_audio_inputs(state: tauri::State<'_, AppState>) -> Result<Vec<obs::ObsAudioInput>, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    obs::audio_inputs(&settings).await
}

#[tauri::command]
async fn obs_set_audio_mute(
    state: tauri::State<'_, AppState>,
    input_name: String,
    muted: bool,
) -> Result<Vec<obs::ObsAudioInput>, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    obs::set_input_mute(&settings, &input_name, muted).await?;
    obs::audio_inputs(&settings).await
}

#[tauri::command]
async fn obs_set_audio_volume(
    state: tauri::State<'_, AppState>,
    input_name: String,
    volume_mul: f64,
) -> Result<Vec<obs::ObsAudioInput>, String> {
    let settings = state.settings.obs().map_err(|e| e.to_string())?;
    obs::set_input_volume(&settings, &input_name, volume_mul).await?;
    obs::audio_inputs(&settings).await
}

#[tauri::command]
async fn get_watch_path(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state
        .watcher
        .watch_path()
        .await
        .map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn list_clips(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<Vec<Clip>, String> {
    repair_stuck_processing(&app, &state).await?;
    Ok(state.store.list())
}

/// Repairs clips stuck in `Processing` (interrupted probe/thumbnail jobs) by
/// resuming them. Only called automatically from `list_clips`, and only acts
/// on `Processing` clips so a normal library refresh never retries clips the
/// user was already told had failed.
async fn repair_stuck_processing(app: &tauri::AppHandle, state: &AppState) -> Result<usize, String> {
    repair_clips(app, state, false).await
}

/// Full repair pass: resumes stuck `Processing` clips AND retries `Failed`
/// clips whose source file still exists (re-probe metadata, regenerate a
/// missing thumbnail). Explicit, user-triggered action — never runs on its
/// own so a broken clip doesn't retry forever in the background.
#[tauri::command]
async fn repair_processing_clips(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    repair_clips(&app, &state, true).await
}

async fn repair_clips(
    app: &tauri::AppHandle,
    state: &AppState,
    include_failed: bool,
) -> Result<usize, String> {
    use crate::models::clip::ClipStatus;

    let clips = state.store.list();
    let mut repaired = 0usize;
    for clip in clips {
        let is_processing = clip.status == ClipStatus::Processing;
        let is_retryable_failure = include_failed && clip.status == ClipStatus::Failed;
        if !is_processing && !is_retryable_failure {
            continue;
        }

        let path = PathBuf::from(&clip.file_path);
        if !path.is_file() {
            if clip.status != ClipStatus::Failed {
                if let Some(updated) = state
                    .store
                    .update(&clip.id, |c| c.status = ClipStatus::Failed)
                    .map_err(|e| e.to_string())?
                {
                    let _ = app.emit("clip://updated", &updated);
                    repaired += 1;
                }
            }
            continue;
        }

        let thumb_ok = clip
            .thumbnail_path
            .as_ref()
            .is_some_and(|p| PathBuf::from(p).is_file());
        if clip.duration.is_some() && thumb_ok {
            if clip.status != ClipStatus::Ready {
                if let Some(updated) = state
                    .store
                    .update(&clip.id, |c| c.status = ClipStatus::Ready)
                    .map_err(|e| e.to_string())?
                {
                    let _ = app.emit("clip://updated", &updated);
                    repaired += 1;
                }
            }
            continue;
        }

        if is_retryable_failure {
            if let Some(updated) = state
                .store
                .update(&clip.id, |c| c.status = ClipStatus::Processing)
                .map_err(|e| e.to_string())?
            {
                let _ = app.emit("clip://updated", &updated);
            }
        }

        if clip.duration.is_none() {
            state
                .queue
                .enqueue(Job::new(JobKind::Probe {
                    clip_id: clip.id.clone(),
                    path: path.clone(),
                }))
                .await
                .map_err(|e| e.to_string())?;
        }
        if !thumb_ok {
            state
                .queue
                .enqueue(Job::new(JobKind::Thumbnail {
                    clip_id: clip.id.clone(),
                    path,
                }))
                .await
                .map_err(|e| e.to_string())?;
        }
        crate::emit_debug(app, format!("repair queued: {}", clip.file_name));
        repaired += 1;
    }

    // Backfill highlight markers for clips that predate the feature, or that
    // hit the ffmpeg colon-in-Windows-path bug present before that was
    // fixed — only on the explicit "Repair library" action, not on every
    // list_clips refresh, since this scans the whole library.
    if include_failed {
        for clip in state.store.list() {
            if !crate::queue::needs_audio_peaks(&clip) {
                continue;
            }
            let path = PathBuf::from(&clip.file_path);
            if !path.is_file() {
                continue;
            }
            state
                .queue
                .enqueue(Job::new(JobKind::AudioPeaks {
                    clip_id: clip.id.clone(),
                    path,
                }))
                .await
                .map_err(|e| e.to_string())?;
            repaired += 1;
        }
    }

    Ok(repaired)
}

/// Returns a `data:image/jpeg;base64,...` URL the WebView can always display.
#[tauri::command]
fn get_thumbnail_data_url(path: String) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("thumbnail file is empty".into());
    }
    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes)))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TrimQueuedResponse {
    output_path: String,
}

#[tauri::command]
async fn queue_trim_clip(
    state: tauri::State<'_, AppState>,
    clip_id: String,
    start_secs: f64,
    end_secs: f64,
    delete_original: bool,
) -> Result<TrimQueuedResponse, String> {
    let clip = state
        .store
        .get(&clip_id)
        .ok_or_else(|| "clip not found".to_string())?;

    let duration = clip.duration.unwrap_or(0.0);
    if duration <= 0.0 {
        return Err("clip duration unknown — wait for processing to finish".into());
    }
    if start_secs < 0.0 {
        return Err("invalid trim range".into());
    }
    // The timeline UI seeds `end` from the browser <video> element's decoded
    // duration, which commonly disagrees with ffprobe's `clip.duration` by a
    // few hundred ms (sometimes more) on OBS-written MKV/remuxed files. Clamp
    // a small overrun to the real end instead of rejecting a "trim to the
    // end" request the user never actually asked to extend.
    const OVERRUN_TOLERANCE: f64 = 3.0;
    let end_secs = if end_secs > duration && end_secs <= duration + OVERRUN_TOLERANCE {
        duration
    } else {
        end_secs
    };
    if end_secs > duration + 0.05 || end_secs <= start_secs + 0.1 {
        return Err("invalid trim range".into());
    }

    let input = PathBuf::from(&clip.file_path);
    if !input.exists() {
        return Err("source video file is missing".into());
    }

    let output = trimmed_output_path(&input);
    state
        .queue
        .enqueue(Job::new(JobKind::Trim {
            source_clip_id: clip_id,
            input,
            output: output.clone(),
            start_secs,
            end_secs,
            delete_original,
        }))
        .await
        .map_err(|e| e.to_string())?;

    Ok(TrimQueuedResponse {
        output_path: output.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
async fn rename_clip(
    state: tauri::State<'_, AppState>,
    id: String,
    display_name: String,
) -> Result<Clip, String> {
    let name = display_name.trim();
    if name.is_empty() {
        return Err("name cannot be empty".into());
    }
    let clip = state
        .store
        .update(&id, |c| c.display_name = Some(name.to_string()))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    Ok(clip)
}

#[tauri::command]
async fn set_clip_game(
    state: tauri::State<'_, AppState>,
    id: String,
    game_name: String,
) -> Result<Clip, String> {
    let name = game_name.trim();
    if name.is_empty() {
        return Err("game name cannot be empty".into());
    }
    let clip = state
        .store
        .update(&id, |c| c.game_name = Some(name.to_string()))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    Ok(clip)
}

#[tauri::command]
async fn set_clips_game(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
    game_name: String,
) -> Result<Vec<Clip>, String> {
    let name = game_name.trim();
    if name.is_empty() {
        return Err("game name is required".into());
    }
    let updated = state
        .store
        .update_many(&ids, |c| c.game_name = Some(name.to_string()))
        .map_err(|e| e.to_string())?;
    for clip in &updated {
        let _ = app.emit("clip://updated", clip);
    }
    Ok(updated)
}

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Opens an http(s) URL in the user's default browser. Full silent
/// auto-update is out of scope (needs signing/hosting infra); this is the
/// manual "check for updates" escape hatch.
#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only http/https URLs are supported".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("explorer.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("unsupported platform".into())
    }
}

#[tauri::command]
fn get_locked_game() -> Option<String> {
    crate::active_window::last_game_name()
}

/// Lets the user manually lock the app/game before clipping, overriding
/// auto-detection (which can miss the target on multi-monitor setups if the
/// user never focuses it long enough before saving a replay).
#[tauri::command]
fn set_locked_game(app: tauri::AppHandle, game_name: String) -> Result<(), String> {
    let name = game_name.trim();
    if name.is_empty() {
        return Err("game name is required".into());
    }
    crate::active_window::remember_game_name(name);
    let _ = app.emit(
        "game://locked",
        GameReadyPayload {
            game_name: name.to_string(),
        },
    );
    Ok(())
}

#[tauri::command]
fn clear_locked_game(app: tauri::AppHandle) {
    crate::active_window::clear_game_name();
    let _ = app.emit(
        "game://locked",
        GameReadyPayload {
            game_name: String::new(),
        },
    );
}

fn all_tags(state: &AppState) -> Result<Vec<String>, String> {
    let known = state.tags.list().map_err(|e| e.to_string())?;
    let from_clips: Vec<String> = state
        .store
        .list()
        .into_iter()
        .flat_map(|c| c.tags)
        .collect();
    Ok(merge_tags_from_clips(known, from_clips.into_iter()))
}

#[tauri::command]
async fn list_tags(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    all_tags(&state)
}

#[tauri::command]
async fn create_tag(state: tauri::State<'_, AppState>, tag: String) -> Result<Vec<String>, String> {
    let t = normalize_tag(&tag)?;
    state.tags.ensure_tag(&t).map_err(|e| e.to_string())?;
    all_tags(&state)
}

#[tauri::command]
async fn delete_tag(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    tag: String,
) -> Result<Vec<String>, String> {
    let t = normalize_tag(&tag)?;
    state.tags.remove_tag(&t).map_err(|e| e.to_string())?;
    let updated = state
        .store
        .remove_tag_from_all_clips(&t)
        .map_err(|e| e.to_string())?;
    for clip in updated {
        let _ = app.emit("clip://updated", &clip);
    }
    all_tags(&state)
}

#[tauri::command]
async fn add_clip_tag(
    state: tauri::State<'_, AppState>,
    id: String,
    tag: String,
) -> Result<Clip, String> {
    let t = normalize_tag(&tag)?;
    state.tags.ensure_tag(&t).map_err(|e| e.to_string())?;
    let clip = state
        .store
        .update(&id, |c| {
            if !c.tags.contains(&t) {
                c.tags.push(t.clone());
                c.tags.sort();
            }
        })
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    Ok(clip)
}

#[tauri::command]
async fn remove_clip_tag(
    state: tauri::State<'_, AppState>,
    id: String,
    tag: String,
) -> Result<Clip, String> {
    let t = normalize_tag(&tag)?;
    let clip = state
        .store
        .update(&id, |c| {
            c.tags.retain(|x| x != &t);
        })
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    Ok(clip)
}

#[tauri::command]
async fn add_tag_to_clips(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
    tag: String,
) -> Result<Vec<Clip>, String> {
    let t = normalize_tag(&tag)?;
    state.tags.ensure_tag(&t).map_err(|e| e.to_string())?;
    let updated = state
        .store
        .update_many(&ids, |c| {
            if !c.tags.contains(&t) {
                c.tags.push(t.clone());
                c.tags.sort();
            }
        })
        .map_err(|e| e.to_string())?;
    for clip in &updated {
        let _ = app.emit("clip://updated", clip);
    }
    Ok(updated)
}

#[tauri::command]
async fn remove_tag_from_clips(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
    tag: String,
) -> Result<Vec<Clip>, String> {
    let t = normalize_tag(&tag)?;
    let updated = state
        .store
        .update_many(&ids, |c| {
            c.tags.retain(|x| x != &t);
        })
        .map_err(|e| e.to_string())?;
    for clip in &updated {
        let _ = app.emit("clip://updated", clip);
    }
    Ok(updated)
}

#[tauri::command]
async fn delete_clips(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let removed = state.store.remove_many(&ids).map_err(|e| e.to_string())?;
    for clip in removed {
        let _ = app.emit("clip://deleted", clip.id);
    }
    Ok(())
}

#[tauri::command]
async fn delete_clip(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let removed = state
        .store
        .remove(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    let _ = app.emit("clip://deleted", removed.id);
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanupReport {
    removed_missing_clips: usize,
    removed_orphan_thumbnails: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalClipCleanupReport {
    removed_clips: usize,
    kept_favorites: usize,
    freed_bytes: u64,
    total_bytes: u64,
}

#[tauri::command]
async fn cleanup_storage(state: tauri::State<'_, AppState>) -> Result<CleanupReport, String> {
    let removed_missing_clips = state
        .store
        .remove_missing_files()
        .map_err(|e| e.to_string())?;
    let known_thumbs = state.store.thumbnail_paths();
    let thumb_dir = state.data_dir.join("thumbnails");
    let mut removed_orphan_thumbnails = 0usize;

    if let Ok(entries) = std::fs::read_dir(&thumb_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && !known_thumbs.contains(&path)
                && std::fs::remove_file(&path).is_ok()
            {
                removed_orphan_thumbnails += 1;
            }
        }
    }

    Ok(CleanupReport {
        removed_missing_clips,
        removed_orphan_thumbnails,
    })
}

#[tauri::command]
async fn cleanup_old_local_clips(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    older_than_days: u32,
    max_size_gb: Option<f64>,
) -> Result<LocalClipCleanupReport, String> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(older_than_days.max(1) as i64);
    let mut ids = Vec::new();
    let mut kept_favorites = 0usize;
    let mut candidates = Vec::new();
    let mut total_bytes = 0u64;

    for clip in state.store.list() {
        let bytes = std::fs::metadata(&clip.file_path).map(|m| m.len()).unwrap_or(0);
        total_bytes = total_bytes.saturating_add(bytes);
        let Ok(created) = chrono::DateTime::parse_from_rfc3339(&clip.created_at) else {
            continue;
        };
        if clip.is_favorite {
            kept_favorites += 1;
            continue;
        }
        candidates.push((created.with_timezone(&chrono::Utc), clip.id.clone(), bytes));
        if created.with_timezone(&chrono::Utc) >= cutoff {
            continue;
        }
        ids.push(clip.id);
    }

    if let Some(max_size_gb) = max_size_gb {
        let max_bytes = (max_size_gb.max(0.1) * 1024.0 * 1024.0 * 1024.0) as u64;
        let mut projected_total = total_bytes;
        for id in &ids {
            if let Some((_, _, bytes)) = candidates.iter().find(|(_, candidate_id, _)| candidate_id == id) {
                projected_total = projected_total.saturating_sub(*bytes);
            }
        }
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, id, bytes) in candidates {
            if projected_total <= max_bytes {
                break;
            }
            if ids.iter().any(|existing| existing == &id) {
                continue;
            }
            ids.push(id);
            projected_total = projected_total.saturating_sub(bytes);
        }
    }

    let freed_bytes = state
        .store
        .list()
        .into_iter()
        .filter(|clip| ids.iter().any(|id| id == &clip.id))
        .map(|clip| std::fs::metadata(&clip.file_path).map(|m| m.len()).unwrap_or(0))
        .sum();
    let removed = state.store.remove_many(&ids).map_err(|e| e.to_string())?;
    for clip in &removed {
        let _ = app.emit("clip://deleted", clip.id.clone());
    }

    Ok(LocalClipCleanupReport {
        removed_clips: removed.len(),
        kept_favorites,
        freed_bytes,
        total_bytes: total_bytes.saturating_sub(freed_bytes),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareResponse {
    output_path: String,
    url: Option<String>,
    clip: Option<Clip>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FriendSharingConfig {
    app: String,
    version: u8,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    account_id: String,
    #[serde(default)]
    endpoint_url: String,
    #[serde(default)]
    region: String,
    #[serde(default)]
    access_key_id: String,
    #[serde(default)]
    secret_access_key: String,
    #[serde(default)]
    bucket: String,
    #[serde(default)]
    public_base_url: String,
    #[serde(default = "default_friend_delete_days")]
    delete_after_days: u32,
}

fn default_friend_delete_days() -> u32 {
    15
}

#[tauri::command]
fn export_friend_sharing_config(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let r2 = state.settings.r2().map_err(|e| e.to_string())?;
    let config = FriendSharingConfig {
        app: "533clip".to_string(),
        version: 1,
        provider: r2.provider,
        account_id: r2.account_id,
        endpoint_url: r2.endpoint_url,
        region: r2.region,
        access_key_id: r2.access_key_id,
        secret_access_key: r2.secret_access_key,
        bucket: r2.bucket,
        public_base_url: r2.public_base_url,
        delete_after_days: r2.delete_after_days,
    };
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_friend_sharing_config(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<R2SettingsResponse, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let config: FriendSharingConfig = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if config.app != "533clip" {
        return Err("Not a 533clip sharing config".into());
    }
    let current = state.settings.r2().map_err(|e| e.to_string())?;
    state
        .settings
        .set_r2(R2SettingsUpdate {
            enabled: true,
            provider: config.provider,
            account_id: config.account_id,
            endpoint_url: config.endpoint_url,
            region: config.region,
            access_key_id: if config.access_key_id.trim().is_empty() {
                current.access_key_id
            } else {
                config.access_key_id
            },
            secret_access_key: if config.secret_access_key.trim().is_empty() {
                None
            } else {
                Some(config.secret_access_key)
            },
            bucket: config.bucket,
            public_base_url: config.public_base_url,
            delete_after_days: config.delete_after_days,
        })
        .map_err(|e| e.to_string())?;
    state.settings.r2_response().map_err(|e| e.to_string())
}

#[tauri::command]
async fn export_clip_for_discord(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<ShareResponse, String> {
    let clip = state
        .store
        .get(&id)
        .ok_or_else(|| "clip not found".to_string())?;
    let duration = clip
        .duration
        .ok_or_else(|| "clip duration unknown — wait for processing to finish".to_string())?;
    let input = PathBuf::from(&clip.file_path);
    let output = state.data_dir.join("shares").join(format!("{}-discord.mp4", clip.id));
    export_for_discord(&input, &output, duration).map_err(|e| e.to_string())?;
    Ok(ShareResponse {
        output_path: output.to_string_lossy().into_owned(),
        url: None,
        clip: None,
    })
}

#[tauri::command]
async fn upload_clip_to_r2(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<ShareResponse, String> {
    let clip = state
        .store
        .get(&id)
        .ok_or_else(|| "clip not found".to_string())?;
    let r2 = state.settings.r2().map_err(|e| e.to_string())?;
    if !r2.enabled {
        return Err("cloud sharing is disabled".into());
    }
    let duration = clip
        .duration
        .ok_or_else(|| "clip duration unknown — wait for processing to finish".to_string())?;

    let output = state.data_dir.join("shares").join(format!("{}-r2.mp4", clip.id));
    export_for_discord(&PathBuf::from(&clip.file_path), &output, duration)
        .map_err(|e| e.to_string())?;
    let short_id: String = clip.id.chars().filter(|c| c.is_ascii_alphanumeric()).take(10).collect();
    let key = format!("c/{}.mp4", short_id);
    let url = sharing::upload_file(&r2, &key, &output).await?;
    let shared_at = chrono::Utc::now().to_rfc3339();
    let updated = state
        .store
        .update(&clip.id, |c| {
            c.share_key = Some(key.clone());
            c.share_url = Some(url.clone());
            c.shared_at = Some(shared_at.clone());
        })
        .map_err(|e| e.to_string())?;
    let updated = updated.ok_or_else(|| "clip not found after upload".to_string())?;
    Ok(ShareResponse {
        output_path: output.to_string_lossy().into_owned(),
        url: Some(url),
        clip: Some(updated),
    })
}

#[tauri::command]
async fn cleanup_r2_uploads(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    run_r2_cleanup(&state.settings, &state.store).await
}

async fn run_r2_cleanup(settings: &SettingsStore, store: &ClipStore) -> Result<usize, String> {
    let r2 = settings.r2().map_err(|e| e.to_string())?;
    if !r2.enabled {
        return Ok(0);
    }
    let cutoff = chrono::Utc::now() - chrono::Duration::days(r2.delete_after_days as i64);
    let clips = store.list();
    let mut removed = 0usize;
    for clip in clips {
        let Some(shared_at) = &clip.shared_at else {
            continue;
        };
        let Some(key) = &clip.share_key else {
            continue;
        };
        let Ok(t) = chrono::DateTime::parse_from_rfc3339(shared_at) else {
            continue;
        };
        if t.with_timezone(&chrono::Utc) <= cutoff {
            if sharing::delete_object(&r2, key).await.is_ok() {
                let _ = store.update(&clip.id, |c| {
                    c.share_key = None;
                    c.share_url = None;
                    c.shared_at = None;
                });
                removed += 1;
            }
        }
    }
    Ok(removed)
}

/// Runs the same cleanup cleanup_r2_uploads does, but on a timer, so
/// "delete after N days" is actually automatic instead of only happening
/// when the user remembers to click the button in Sharing settings.
fn spawn_r2_cleanup_monitor(app: tauri::AppHandle, settings: SettingsStore, store: ClipStore) {
    tauri::async_runtime::spawn(async move {
        loop {
            match run_r2_cleanup(&settings, &store).await {
                Ok(removed) if removed > 0 => {
                    emit_debug(&app, format!("R2/B2 auto-cleanup: removed {removed} expired upload(s)"));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "R2/B2 auto-cleanup failed");
                }
                _ => {}
            }
            tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}

#[tauri::command]
async fn toggle_favorite(state: tauri::State<'_, AppState>, id: String) -> Result<Clip, String> {
    let clip = state
        .store
        .update(&id, |c| c.is_favorite = !c.is_favorite)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    Ok(clip)
}

fn force_fullscreen_geometry(window: &tauri::WebviewWindow) {
    // Echo back the window's own just-queried outer_position()/inner_size()
    // rather than recomputing geometry from current_monitor(). The latter
    // reads a separately-cached monitor scale factor that can itself be
    // stale (the same WebView2 DPI-cache bug fought elsewhere via the
    // Resized-handler echo below) — feeding fullscreen a value computed
    // from that cache reproduces the exact offset it's meant to fix.
    // Re-applying the size Windows *just reported* forces a fresh re-layout
    // instead of trusting a second, independently-cached number.
    if let Ok(pos) = window.outer_position() {
        let _ = window.set_position(pos);
    }
    if let Ok(size) = window.inner_size() {
        let _ = window.set_size(size);
    }
}

#[tauri::command]
async fn toggle_app_fullscreen(app: tauri::AppHandle) -> Result<bool, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let fullscreen = !window.is_fullscreen().map_err(|e| e.to_string())?;
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    window.set_fullscreen(fullscreen).map_err(|e| e.to_string())?;
    if fullscreen {
        // set_fullscreen(true) alone can leave the window offset from the
        // monitor origin instead of filling it — force explicit geometry
        // rather than relying on the OS/webview getting it right on its own.
        // The transition isn't necessarily settled the instant
        // set_fullscreen() returns, so retry after a short delay too —
        // a single immediate correction wasn't enough.
        force_fullscreen_geometry(&window);
        let window_for_retry = window.clone();
        tauri::async_runtime::spawn(async move {
            // A single 150ms retry wasn't enough on all machines — the
            // fullscreen transition can take longer when the GPU is under
            // load (OBS running alongside). Keep nudging for a bit longer.
            for delay_ms in [150, 350, 700] {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                force_fullscreen_geometry(&window_for_retry);
            }
        });
    }
    Ok(fullscreen)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("five33clip_lib=info".parse().unwrap()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .join("533clip");

            let store = ClipStore::open(&data_dir).map_err(|e| e.to_string())?;
            let tags = TagRegistryStore::open(&data_dir).map_err(|e| e.to_string())?;
            let settings = SettingsStore::open(&data_dir).map_err(|e| e.to_string())?;
            let (queue, pipeline) = init_job_queue(handle.clone(), store.clone(), data_dir.clone());
            let watcher = WatcherService::new(pipeline.clone());

            let saved_watch = settings.watch_path().map_err(|e| e.to_string())?;
            let obs_launch_settings = settings.obs().map_err(|e| e.to_string())?;
            if obs_launch_settings.integration_mode == "managed" {
                let _ = obs::disable_auto_remux_profiles();
                let _ = obs::remove_legacy_533clip_scripts();
            }

            let settings_for_monitor = settings.clone();
            let settings_for_r2_cleanup = settings.clone();
            let store_for_r2_cleanup = store.clone();
            app.manage(AppState {
                store,
                tags,
                settings,
                watcher: watcher.clone(),
                queue,
                data_dir: data_dir.clone(),
            });

            create_capture_overlay_window(app)?;
            spawn_game_ready_monitor(handle.clone());
            spawn_recording_monitor(handle.clone(), settings_for_monitor);
            spawn_r2_cleanup_monitor(handle.clone(), settings_for_r2_cleanup, store_for_r2_cleanup);

            let show = MenuItem::with_id(app, "show", "Show 533clip", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let menu = Menu::with_items(app, &[&show, &quit]).map_err(|e| e.to_string())?;
            TrayIconBuilder::with_id("533clip-main")
                .tooltip("533clip")
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .ok_or("missing app icon")?,
                )
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)
                .map_err(|e| e.to_string())?;

            if let Some(path) = saved_watch {
                let pb = PathBuf::from(&path);
                if pb.is_dir() {
                    if let Err(e) = tauri::async_runtime::block_on(watcher.set_watch_path(pb)) {
                        tracing::warn!(error = %e, "failed to restore saved watch folder");
                    }
                } else {
                    tracing::warn!(path = %path, "saved watch folder no longer exists");
                }
            }

            if obs_launch_settings.integration_mode == "managed" && obs_launch_settings.auto_launch {
                let handle_for_obs_launch = handle.clone();
                tauri::async_runtime::spawn(async move {
                    if !obs::is_obs_running() {
                        if let Err(e) = obs::launch_minimized_to_tray(
                            obs_launch_settings.start_replay_on_launch,
                        ) {
                            // Previously silent — if OBS isn't in one of the
                            // 3 hardcoded install paths detect_obs_executable
                            // checks, "auto-launch OBS on startup" just does
                            // nothing with zero visible feedback anywhere.
                            tracing::warn!(error = %e, "failed to auto-launch OBS");
                            emit_debug(&handle_for_obs_launch, format!("Auto-launch OBS failed: {e}"));
                        }
                    }
                });
            }

            // The WebView2 DPI-scale cache is only known to be correct once
            // the window has been through a real OS resize (that's what the
            // WindowEvent::Resized echo-fix below relies on). A freshly
            // launched window that's never been moved/resized hasn't gone
            // through that yet, so the first fullscreen toggle used a
            // stale/uninitialized scale and came out offset — dragging the
            // window to snap it (a real resize) before pressing F11 fixed
            // it, because that's what primed the cache. Do that priming
            // nudge automatically at startup instead of requiring it.
            if let Some(main_window) = handle.get_webview_window("main") {
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    if let Ok(size) = main_window.inner_size() {
                        let _ = main_window.set_size(size);
                    }
                });
            }

            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                show_main_window(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
            // Windows/WebView2 can cache the wrong DPI scale factor when the
            // main window transitions between maximized and restored/floating
            // states, rendering everything too large until something forces
            // a recalculation. Re-applying the settled size is the standard
            // nudge that fixes it. Guarded against re-triggering on a size
            // we already forced, in case set_size itself echoes a Resized
            // event, so this can't turn into a resize loop.
            if window.label() == "main" {
                if let WindowEvent::Resized(size) = event {
                    // Re-applying size while entering/exiting fullscreen
                    // fights the OS's own fullscreen positioning (offsets
                    // the content instead of filling the screen) — only
                    // needed for the maximize/restore DPI-cache case. Do NOT
                    // also skip on is_maximized(): this echo is specifically
                    // what fixes the DPI-cache sidebar-scaling bug that
                    // happens on maximize — skipping it there reintroduces
                    // that exact bug (reverted a local change that added
                    // this skip; see code review discussion).
                    if window.is_fullscreen().unwrap_or(false) {
                        return;
                    }
                    static LAST_FORCED_SIZE: std::sync::OnceLock<
                        parking_lot::Mutex<Option<(u32, u32)>>,
                    > = std::sync::OnceLock::new();
                    let guard = LAST_FORCED_SIZE.get_or_init(|| parking_lot::Mutex::new(None));
                    let mut last = guard.lock();
                    let current = (size.width, size.height);
                    if *last != Some(current) {
                        *last = Some(current);
                        let _ = window.set_size(*size);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            set_watch_path,
            get_watch_path,
            detect_obs_recording_paths,
            reveal_path,
            copy_file_to_clipboard,
            start_file_drag,
            get_r2_settings,
            set_r2_settings,
            get_obs_settings,
            set_obs_settings,
            set_obs_replay_hotkey,
            set_obs_recording_hotkey,
            set_obs_replay_duration,
            set_obs_capture_source_mode,
            get_launch_on_startup,
            set_launch_on_startup,
            obs_status,
            obs_stats,
            obs_launch,
            obs_start_replay_buffer,
            obs_stop_replay_buffer,
            obs_save_replay_buffer,
            obs_toggle_recording,
            obs_apply_quality_preset,
            obs_audio_inputs,
            obs_set_audio_mute,
            obs_set_audio_volume,
            list_clips,
            repair_processing_clips,
            toggle_favorite,
            rename_clip,
            set_clip_game,
            set_clips_game,
            get_app_version,
            open_external_url,
            get_locked_game,
            set_locked_game,
            clear_locked_game,
            delete_clip,
            delete_clips,
            cleanup_storage,
            cleanup_old_local_clips,
            export_clip_for_discord,
            upload_clip_to_r2,
            cleanup_r2_uploads,
            export_friend_sharing_config,
            import_friend_sharing_config,
            list_tags,
            create_tag,
            delete_tag,
            add_clip_tag,
            remove_clip_tag,
            add_tag_to_clips,
            remove_tag_from_clips,
            get_thumbnail_data_url,
            queue_trim_clip,
            toggle_app_fullscreen
        ])
        .run(tauri::generate_context!())
        .expect("error while running 533clip");
}
