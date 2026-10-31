//! A desktop shortcut that starts the game straight away, skipping the
//! launcher: `mm2-viewer` with no arguments, which starts itself with the
//! launcher's saved settings.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The shortcut's name on the desktop.
const NAME: &str = "Play MM2";

/// Puts a shortcut to `viewer` on the desktop, replacing one made before;
/// where it went.
pub fn make(viewer: &Path) -> Result<PathBuf, String> {
    if cfg!(windows) {
        windows(viewer)
    } else if cfg!(target_os = "macos") {
        mac(viewer)
    } else {
        linux(viewer)
    }
}

/// A `.lnk` made through the shell's own COM object, by PowerShell; the
/// paths go in by environment so nothing needs escaping. The desktop is
/// asked for, as it may be in OneDrive.
fn windows(viewer: &Path) -> Result<PathBuf, String> {
    const SCRIPT: &str = "$s = New-Object -ComObject WScript.Shell; \
        $p = Join-Path $s.SpecialFolders.Item('Desktop') ($env:MM2_LNK_NAME + '.lnk'); \
        $l = $s.CreateShortcut($p); \
        $l.TargetPath = $env:MM2_LNK_TARGET; \
        $l.WorkingDirectory = Split-Path $env:MM2_LNK_TARGET; \
        $l.IconLocation = $env:MM2_LNK_TARGET + ',0'; \
        $l.Description = 'Motocross Madness 2 (OpenMCM2)'; \
        $l.Save(); \
        Write-Output $p";
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("MM2_LNK_NAME", NAME)
        .env("MM2_LNK_TARGET", viewer);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| format!("Couldn't run PowerShell: {e}"))?;
    let made = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || made.is_empty() {
        return Err(format!("Couldn't make the shortcut: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(PathBuf::from(made))
}

/// The desktop folder: the user's own name for it where there is one.
fn desktop() -> Result<PathBuf, String> {
    if let Ok(out) = Command::new("xdg-user-dir").arg("DESKTOP").output() {
        let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join("Desktop"))
        .ok_or_else(|| "Can't find the desktop folder.".to_string())
}

/// A double-clickable `.command` script, which Terminal runs.
fn mac(viewer: &Path) -> Result<PathBuf, String> {
    let at = viewer.display().to_string().replace('\'', "'\\''");
    write_runnable(desktop()?.join(format!("{NAME}.command")), &format!("#!/bin/sh\nexec '{at}'\n"))
}

/// A `.desktop` entry.
fn linux(viewer: &Path) -> Result<PathBuf, String> {
    let text = format!(
        "[Desktop Entry]\nType=Application\nName={NAME}\nComment=Motocross Madness 2 (OpenMCM2)\nExec=\"{}\"\nTerminal=false\nCategories=Game;\n",
        viewer.display(),
    );
    write_runnable(desktop()?.join(format!("{NAME}.desktop")), &text)
}

fn write_runnable(path: PathBuf, text: &str) -> Result<PathBuf, String> {
    std::fs::write(&path, text).map_err(|e| format!("Couldn't write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    }
    Ok(path)
}
