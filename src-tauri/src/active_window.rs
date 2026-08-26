#[derive(Debug, Clone)]
pub struct ActiveWindowInfo {
    pub title: String,
    pub process_name: String,
}

static LAST_GAME_NAME: std::sync::OnceLock<parking_lot::Mutex<Option<String>>> =
    std::sync::OnceLock::new();

#[cfg(windows)]
pub fn detect_game_name() -> Option<String> {
    let info = active_window_info()?;
    if is_ignored_process(&strip_exe(&info.process_name).to_lowercase()) {
        return None;
    }
    Some(infer_game_name(&info))
}

#[cfg(not(windows))]
pub fn detect_game_name() -> Option<String> {
    None
}

pub fn last_game_name() -> Option<String> {
    LAST_GAME_NAME
        .get_or_init(|| parking_lot::Mutex::new(None))
        .lock()
        .clone()
}

pub fn remember_game_name(game: &str) {
    if game.trim().is_empty() {
        return;
    }
    *LAST_GAME_NAME
        .get_or_init(|| parking_lot::Mutex::new(None))
        .lock() = Some(game.trim().to_string());
}

pub fn clear_game_name() {
    *LAST_GAME_NAME
        .get_or_init(|| parking_lot::Mutex::new(None))
        .lock() = None;
}

#[cfg(windows)]
fn active_window_info() -> Option<ActiveWindowInfo> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == std::ptr::null_mut() {
            return None;
        }

        let len = GetWindowTextLengthW(hwnd).max(0);
        let mut title_buf = vec![0u16; len as usize + 1];
        let title_len = GetWindowTextW(hwnd, &mut title_buf).max(0) as usize;
        let title = String::from_utf16_lossy(&title_buf[..title_len.min(title_buf.len())])
            .trim()
            .to_string();

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut path_buf = vec![0u16; 4096];
        let mut size = path_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(path_buf.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        if ok.is_err() || size == 0 {
            return None;
        }

        let process_path = String::from_utf16_lossy(&path_buf[..(size as usize).min(path_buf.len())]);
        let process_name = std::path::Path::new(&process_path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        Some(ActiveWindowInfo {
            title,
            process_name,
        })
    }
}

#[cfg(windows)]
fn infer_game_name(info: &ActiveWindowInfo) -> String {
    let exe = strip_exe(&info.process_name);
    let lower = exe.to_lowercase();

    if lower == "robloxplayerbeta" {
        return title_or("Roblox", &info.title);
    }

    if lower == "robloxstudiobeta" || lower == "robloxstudio" {
        return "Roblox Studio".to_string();
    }

    if lower.contains("blender") {
        return "Blender".to_string();
    }
    if lower.contains("kletka") {
        return "Kletka".to_string();
    }

    if lower == "studio" && info.title.to_lowercase().contains("roblox") {
        return "Roblox Studio".to_string();
    }

    if lower == "tmodloader" {
        return "tModLoader".to_string();
    }

    if lower == "terraria" {
        return "Terraria".to_string();
    }

    if is_generic_process(&lower) {
        return title_or(&clean_name(&exe), &info.title);
    }

    clean_name(&exe)
}

#[cfg(windows)]
fn strip_exe(name: &str) -> String {
    name.strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name)
        .to_string()
}

#[cfg(windows)]
fn title_or(fallback: &str, title: &str) -> String {
    let cleaned = clean_title(title);
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

#[cfg(windows)]
fn clean_title(title: &str) -> String {
    let mut value = title
        .replace(" - Roblox", "")
        .replace("Roblox - ", "")
        .replace(" - Microsoft Store", "")
        .trim()
        .to_string();
    if value.eq_ignore_ascii_case("roblox") || value.eq_ignore_ascii_case("terraria") {
        value.clear();
    }
    clean_name(&value)
}

#[cfg(windows)]
fn clean_name(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("blender") {
        return "Blender".to_string();
    }
    if lower.contains("kletka") {
        return "Kletka".to_string();
    }
    if lower.contains("robloxstudiobeta") || lower.contains("robloxstudio") {
        return "Roblox Studio".to_string();
    }
    name.replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[cfg(windows)]
fn is_generic_process(process: &str) -> bool {
    matches!(
        process,
        "javaw"
            | "java"
            | "unityplayer"
            | "unrealcefsubprocess"
            | "gamelaunchhelper"
            | "game"
            | "win64shipping"
    )
}

#[cfg(windows)]
fn is_ignored_process(process: &str) -> bool {
    matches!(
        process,
        "five33clip"
            | "codex"
            | "explorer"
            | "obs64"
            | "obs32"
            | "screenclippinghost"
            | "taskmgr"
            | "blender"
            | "applicationframehost"
            | "search"
            | "searchapp"
            | "shellexperiencehost"
            | "searchhost"
            | "runtimebroker"
            | "systemsettings"
            | "winstore.app"
            | "lockapp"
            | "securityhealthsystray"
            | "widgets"
            | "widgetservice"
            | "phoneexperiencehost"
            | "yourphone"
            | "dllhost"
            | "conhost"
            | "ctfmon"
            | "audiodg"
            | "msedgewebview2"
            | "startmenuexperiencehost"
            | "textinputhost"
            | "powershell"
            | "windowsterminal"
            | "code"
            | "cursor"
    )
}
