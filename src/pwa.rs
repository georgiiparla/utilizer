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
    use std::{env, process::Command};

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
                .arg("--window-size=960,720")
                .spawn()
            {
                Ok(_) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
        }
    }
    Err(last_error.map_or_else(
        || "Install Edge or Chrome to open an app window".into(),
        Into::into,
    ))
}

#[cfg(not(windows))]
pub fn open() -> Result<()> {
    Err("App-window launch is supported on Windows only".into())
}

#[cfg(test)]
mod tests {
    use super::*;

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
