//! Offline uninstall: an embedded PowerShell helper removes the executable
//! after this process exits, which is required by Windows image-file locking.
use utilizer::Result;

#[cfg(windows)]
pub fn schedule(purge: bool) -> Result<()> {
    use std::{os::windows::process::CommandExt, path::PathBuf, process::Command};

    let base =
        PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?);
    let installed = base.join("Programs/Utilizer/utilizer.exe");
    if !installed.is_file() || !same_file::is_same_file(std::env::current_exe()?, &installed)? {
        return Err("Run uninstall using the installed copy of Utilizer. For a portable copy, delete the executable yourself.".into());
    }
    let script = format!(
        "& {{\n{}\n}} -WaitForProcessId {} {}",
        include_str!("../uninstall.ps1"),
        std::process::id(),
        if purge { "-Purge" } else { "" },
    );
    let powershell =
        PathBuf::from(std::env::var_os("SystemRoot").ok_or("SystemRoot is unavailable")?)
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let command = |check: bool| {
        let mut command = Command::new(&powershell);
        command
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(if check {
                format!("{script} -CheckOnly")
            } else {
                script.clone()
            })
            // The helper uses this terminal's output without opening a window.
            .creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        command
    };
    if !command(true).status()?.success() {
        return Err("Uninstall checks failed. Nothing was removed; see the message above.".into());
    }
    command(false).spawn()?;
    println!("Uninstall scheduled. Cleanup finishes after this command exits.");
    Ok(())
}

#[cfg(not(windows))]
pub fn schedule(_purge: bool) -> Result<()> {
    Err("Uninstall is supported on Windows only".into())
}
