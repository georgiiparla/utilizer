#![cfg(windows)]
//! Real Windows process handoff, using an installed copy in a temporary profile.
use std::{fs, path::Path, process::Command};

use tempfile::Builder;

fn user_path() -> Option<String> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Environment")
        .unwrap()
        .get_value("Path")
        .ok()
}

fn install(base: &Path) -> std::path::PathBuf {
    let installed = base.join("Programs/Utilizer/utilizer.exe");
    fs::create_dir_all(installed.parent().unwrap()).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_utilizer"), &installed).unwrap();
    installed
}

#[test]
fn installed_command_removes_itself_offline_and_purge_removes_both_folders() {
    let profile = Builder::new()
        .prefix("Utilizer's temporary profile ")
        .tempdir()
        .unwrap();
    let installed = install(profile.path());
    let data = profile.path().join("Utilizer");
    fs::create_dir_all(data.join("cache/nested")).unwrap();
    fs::write(data.join("utilizer.sqlite3"), "saved playlists").unwrap();
    fs::write(data.join("cache/nested/file"), "cache").unwrap();
    let sentinel = profile.path().join("outside.txt");
    fs::write(&sentinel, "unrelated").unwrap();
    // This temporary installation was never added to the real user's PATH.
    // The script must leave that PATH byte-for-byte unchanged.
    let original = user_path();
    assert!(
        !original
            .as_deref()
            .unwrap_or("")
            .contains(&profile.path().to_string_lossy().to_string())
    );

    let invalid = Command::new(&installed)
        .args(["uninstall", "--unknown"])
        .env("LOCALAPPDATA", profile.path())
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(installed.is_file());

    let result = Command::new(&installed)
        .arg("uninstall")
        .env("LOCALAPPDATA", profile.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("Utilizer uninstalled for this user."),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!installed.parent().unwrap().exists());
    assert_eq!(
        fs::read_to_string(data.join("utilizer.sqlite3")).unwrap(),
        "saved playlists"
    );
    assert!(data.join("cache/nested/file").is_file());
    assert_eq!(user_path(), original);

    let installed = install(profile.path());
    let result = Command::new(&installed)
        .args(["uninstall", "--purge"])
        .env("LOCALAPPDATA", profile.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout)
            .contains("Application and local data folders removed"),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!installed.parent().unwrap().exists());
    assert!(!data.exists());
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "unrelated");
    assert_eq!(user_path(), original);

    let portable = Command::new(env!("CARGO_BIN_EXE_utilizer"))
        .args(["uninstall", "--purge"])
        .env("LOCALAPPDATA", profile.path())
        .output()
        .unwrap();
    assert!(!portable.status.success());
    assert!(String::from_utf8_lossy(&portable.stderr).contains("installed copy"));
    assert!(!data.exists(), "Uninstall must not bootstrap the application");
}
