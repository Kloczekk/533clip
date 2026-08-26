use std::path::PathBuf;
use std::process::Command;

/// Spawns FFmpeg/FFprobe without showing a console window on Windows.
pub fn hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(resolve_program(program));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn resolve_program(program: &str) -> PathBuf {
    let exe_name = if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_string()
    };
    let sidecar_name = if cfg!(windows) {
        format!("{program}-x86_64-pc-windows-msvc.exe")
    } else {
        program.to_string()
    };

    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidates.push(dir.join(&sidecar_name));
            candidates.push(dir.join(&exe_name));
            candidates.push(dir.join("resources").join(&sidecar_name));
            candidates.push(dir.join("resources").join(&exe_name));
            if let Some(parent) = dir.parent() {
                candidates.push(parent.join("resources").join(&sidecar_name));
                candidates.push(parent.join("resources").join(&exe_name));
            }
        }
    }
    candidates.push(PathBuf::from("src-tauri").join("binaries").join(&sidecar_name));
    candidates.push(PathBuf::from("src-tauri").join("binaries").join(&exe_name));

    candidates
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(program))
}
