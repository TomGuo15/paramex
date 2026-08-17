//! Windows shell glue: log-file location, file logging, single-instance mutex,
//! and the native startup-error box (`app_main.py`). OS calls are `cfg(windows)`
//! with non-Windows fallbacks so the crate compiles + tests anywhere.

use std::path::{Path, PathBuf};

/// `<base>/ParamEx/app.log`. Pure — the caller resolves `%LOCALAPPDATA%`
/// (`app_main.py:31-39`).
fn log_path_under(base: &Path) -> PathBuf {
    base.join("ParamEx").join("app.log")
}

/// `%LOCALAPPDATA%\ParamEx\app.log`, falling back to the home dir when
/// `LOCALAPPDATA` is unset (`app_main.py:32` `base = LOCALAPPDATA or home`).
fn resolve_log_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(dirs_home)
        .unwrap_or_else(|| PathBuf::from("."));
    log_path_under(&base)
}

/// Home dir from the environment (std only; avoids the `dirs` crate). Windows:
/// `USERPROFILE`; otherwise `HOME`.
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Initialise `tracing` → the app-log file (append). Returns the path so the
/// caller can name it in a startup-error box. `Mutex<File>` is a `MakeWriter`,
/// so no `tracing-appender` dependency is needed.
pub fn init_logging() -> PathBuf {
    use std::fs::OpenOptions;
    use std::sync::Mutex;

    let path = resolve_log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) {
        // try_init: never panic if something already set a subscriber.
        let _ = tracing_subscriber::fmt()
            .with_writer(Mutex::new(file))
            .with_ansi(false)
            .with_target(true)
            .try_init();
    }
    path
}

/// True if another ParamEx instance already holds the single-instance mutex
/// (`app_main.py:121-128`). Non-Windows: always `false` (single-instance is a
/// Windows-only affordance).
///
/// Call at most once, at startup: the first call acquires + holds the named mutex for the process lifetime.
#[cfg(not(windows))]
pub fn already_running() -> bool {
    false
}

// Module-scope static so the lifetime intent is explicit: the HANDLE must live
// for the entire process (not just the function call).
#[cfg(windows)]
use std::sync::OnceLock;
#[cfg(windows)]
static SINGLE_INSTANCE_GUARD: OnceLock<usize> = OnceLock::new();

/// Call at most once, at startup: the first call acquires + holds the named mutex for the process lifetime.
#[cfg(windows)]
pub fn already_running() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
    use windows::Win32::System::Threading::CreateMutexW;

    // Hold the HANDLE for the whole process: a named mutex object lives only
    // while ≥1 handle is open. windows-rs HANDLE does not CloseHandle on drop,
    // so leaking it into a 'static is the correct lifetime (#2710).
    // CreateMutexW returns Ok even when the mutex already exists — the only
    // "already running" signal is GetLastError() == ERROR_ALREADY_EXISTS, read
    // IMMEDIATELY after the call.
    // `Local\` scopes the mutex to the caller's Terminal-Server session — the
    // documented per-session single-instance intent. `Global\` was wrong: it needs
    // SeCreateGlobalPrivilege (so the create silently fails for a standard
    // non-admin user, defeating the guard) and over-blocks a legitimate second
    // RDP session for admins.
    let handle: HANDLE =
        match unsafe { CreateMutexW(None, false, w!("Local\\ParamEx_SingleInstance")) } {
            Ok(h) => h,
            Err(_) => return false, // genuine OS failure → behave as first instance
        };
    let already = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let _ = SINGLE_INSTANCE_GUARD.set(handle.0 as usize);
    already
}

/// Native blocking error box naming the log path (`app_main.py:106-112`,
/// `_message_box`). Non-Windows: stderr.
#[cfg(not(windows))]
pub fn show_startup_error(log_path: &Path) {
    eprintln!(
        "ParamEx failed to start. See the log at: {}",
        log_path.display()
    );
}

#[cfg(windows)]
pub fn show_startup_error(log_path: &Path) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let title = HSTRING::from("ParamEx");
    let body = HSTRING::from(format!(
        "ParamEx failed to start. See the log at:\n{}",
        log_path.display()
    ));
    unsafe {
        // MB_ICONERROR (not Python's generic MB_ICONINFORMATION): the WebView2 info box
        // that used the info icon was dropped in the egui port, so the only box left is a
        // fatal startup failure, which warrants an error icon.
        let _ = MessageBoxW(None, &body, &title, MB_OK | MB_ICONERROR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_path_is_appdata_paramex_app_log() {
        let base = PathBuf::from(r"C:\Users\x\AppData\Local");
        assert_eq!(
            log_path_under(&base),
            PathBuf::from(r"C:\Users\x\AppData\Local\ParamEx\app.log"),
        );
    }
}
