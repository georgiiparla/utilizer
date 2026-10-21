//! App-window launch, disabled by --no-open-pwa.
use std::path::{Path, PathBuf};
use utilizer::Result;

fn installed_shortcut(programs: &Path) -> Option<PathBuf> {
    ["", "Chrome Apps", "Edge Apps"]
        .into_iter()
        .map(|folder| programs.join(folder).join("Utilizer.lnk"))
        .find(|path| path.is_file())
}

#[cfg(windows)]
pub fn open() -> Result<()> {
    use std::{
        env,
        process::Command,
        sync::Mutex,
        time::{Duration, Instant},
    };

    // Startup and tray clicks share this gate while the browser creates a window.
    static LAST_LAUNCH: Mutex<Option<Instant>> = Mutex::new(None);
    let mut last_launch = LAST_LAUNCH.lock().map_err(|_| "App launch lock failed")?;
    if focus_existing()? {
        *last_launch = None;
        return Ok(());
    }
    if last_launch.is_some_and(|last| last.elapsed() < Duration::from_secs(10)) {
        return Ok(());
    }

    for base in [env::var_os("APPDATA"), env::var_os("ProgramData")]
        .into_iter()
        .flatten()
    {
        let programs = PathBuf::from(base).join("Microsoft/Windows/Start Menu/Programs");
        if let Some(shortcut) = installed_shortcut(&programs) {
            let windows = env::var_os("SystemRoot").ok_or("SystemRoot is unavailable")?;
            Command::new(PathBuf::from(windows).join("explorer.exe"))
                .arg(shortcut)
                .spawn()?;
            *last_launch = Some(Instant::now());
            return Ok(());
        }
    }
    // App mode also works before the user installs the PWA. Never fall back to a tab.
    let mut last_error = None;
    for (variable, relative) in [
        ("ProgramFiles(x86)", "Microsoft/Edge/Application/msedge.exe"),
        ("ProgramFiles", "Microsoft/Edge/Application/msedge.exe"),
        ("LOCALAPPDATA", "Microsoft/Edge/Application/msedge.exe"),
        ("ProgramFiles", "Google/Chrome/Application/chrome.exe"),
        ("ProgramFiles(x86)", "Google/Chrome/Application/chrome.exe"),
        ("LOCALAPPDATA", "Google/Chrome/Application/chrome.exe"),
    ] {
        let Some(base) = env::var_os(variable) else {
            continue;
        };
        let executable = PathBuf::from(base).join(relative);
        if executable.is_file() {
            match Command::new(executable)
                .arg(format!("--app={}", utilizer::web::URL))
                .arg("--window-size=760,760")
                .spawn()
            {
                Ok(_) => {
                    *last_launch = Some(Instant::now());
                    return Ok(());
                }
                Err(error) => last_error = Some(error),
            }
        }
    }
    Err(last_error.map_or_else(
        || "Install Edge or Chrome to open an app window".into(),
        Into::into,
    ))
}

#[cfg(windows)]
fn is_app_title(title: &str) -> bool {
    matches!(
        title,
        "Utilizer" | "Utilizer · Starting positions" | "Utilizer - Starting positions"
    )
}

#[cfg(windows)]
fn focus_existing() -> Result<bool> {
    use std::{io, ptr::null_mut};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HWND},
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, FlashWindow, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId,
            IsIconic, IsWindowVisible, SW_RESTORE, SetForegroundWindow, ShowWindowAsync,
        },
    };

    unsafe extern "system" fn find_window(window: HWND, data: isize) -> i32 {
        let mut title = [0u16; 256];
        let mut class = [0u16; 64];
        // SAFETY: the HWND comes from EnumWindows and both buffers have the
        // indicated capacity. Invalid/disappearing windows simply fail to match.
        let (visible, title_len, class_len) = unsafe {
            (
                IsWindowVisible(window),
                GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32),
                GetClassNameW(window, class.as_mut_ptr(), class.len() as i32),
            )
        };
        if visible == 0
            || title_len <= 0
            || class_len <= 0
            || !is_app_title(&String::from_utf16_lossy(&title[..title_len as usize]))
            || String::from_utf16_lossy(&class[..class_len as usize]) != "Chrome_WidgetWin_1"
        {
            return 1;
        }
        let mut pid = 0;
        // SAFETY: pid is valid writable storage; the limited-query handle is
        // used only here and closed on every successful OpenProcess call.
        let process = unsafe {
            GetWindowThreadProcessId(window, &mut pid);
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)
        };
        if process.is_null() {
            return 1;
        }
        let mut path = [0u16; 32768];
        let mut length = path.len() as u32;
        // SAFETY: process is live, length describes the buffer, and the handle
        // is closed after querying regardless of success.
        let queried = unsafe {
            let queried = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length);
            CloseHandle(process);
            queried
        };
        if queried == 0 {
            return 1;
        }
        let path = String::from_utf16_lossy(&path[..length as usize]);
        let browser = Path::new(&path).file_name().and_then(|name| name.to_str());
        if !browser.is_some_and(|name| {
            name.eq_ignore_ascii_case("msedge.exe") || name.eq_ignore_ascii_case("chrome.exe")
        }) {
            return 1;
        }
        // SAFETY: data points to the HWND slot owned by focus_existing, which
        // outlives this synchronous EnumWindows callback.
        unsafe { *(data as *mut HWND) = window };
        0
    }

    let mut window: HWND = null_mut();
    // SAFETY: EnumWindows calls find_window synchronously, with a valid pointer
    // to our output slot. It stops when a matching browser app window is found.
    let enumerated = unsafe { EnumWindows(Some(find_window), (&mut window as *mut HWND) as isize) };
    if window.is_null() {
        return if enumerated == 0 {
            Err(io::Error::last_os_error().into())
        } else {
            Ok(false)
        };
    }
    // SAFETY: this HWND was identified as Utilizer's browser app window. Restore
    // only minimized windows, preserving normal/maximized sizing. If Windows
    // denies focus, notify through the taskbar instead of opening a duplicate.
    unsafe {
        if IsIconic(window) != 0 {
            ShowWindowAsync(window, SW_RESTORE);
        }
        if SetForegroundWindow(window) == 0 {
            FlashWindow(window, 1);
        }
    }
    Ok(true)
}

#[cfg(not(windows))]
pub fn open() -> Result<()> {
    Err("App-window launch is supported on Windows only".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn app_titles_exclude_regular_browser_windows_and_other_apps() {
        for title in [
            "Utilizer",
            "Utilizer · Starting positions",
            "Utilizer - Starting positions",
        ] {
            assert!(is_app_title(title));
        }
        for title in [
            "",
            "Other app",
            "Utilizer - Microsoft Edge",
            "Utilizer - Google Chrome",
            "Utilizer documentation",
        ] {
            assert!(!is_app_title(title));
        }
    }

    #[test]
    fn shortcuts_are_limited_to_the_named_app_in_known_folders() {
        let directory = tempfile::tempdir().unwrap();
        assert!(installed_shortcut(directory.path()).is_none());
        std::fs::write(directory.path().join("Other.lnk"), []).unwrap();
        assert!(installed_shortcut(directory.path()).is_none());
        let folder = directory.path().join("Edge Apps");
        std::fs::create_dir(&folder).unwrap();
        let shortcut = folder.join("Utilizer.lnk");
        std::fs::write(&shortcut, []).unwrap();
        assert_eq!(installed_shortcut(directory.path()), Some(shortcut));
    }
}
