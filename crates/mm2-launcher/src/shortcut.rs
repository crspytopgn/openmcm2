//! A desktop shortcut that starts the game straight away with the
//! launcher's current settings, skipping the launcher.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The shortcut's name on the desktop.
const NAME: &str = "OpenMCM2";

/// One argument as a Windows or desktop-entry command line wants it:
/// quoted when it has spaces.
fn quoted(a: &str) -> String {
    if a.contains(' ') {
        format!("\"{a}\"")
    } else {
        a.to_string()
    }
}

/// Puts a shortcut on the desktop that runs `viewer` with `args` from
/// `dir`, replacing one made before; where it went.
pub fn make(viewer: &Path, args: &[String], dir: Option<&Path>) -> Result<PathBuf, String> {
    let line = args.iter().map(|a| quoted(a)).collect::<Vec<_>>().join(" ");
    if cfg!(windows) {
        windows(viewer, &line, dir)
    } else if cfg!(target_os = "macos") {
        mac(viewer, args, dir)
    } else {
        linux(viewer, &line, dir)
    }
}

/// A `.lnk` made through the shell's own COM object, by PowerShell; the
/// paths go in by environment so nothing needs escaping. The desktop is
/// asked for, as it may be in OneDrive.
fn windows(viewer: &Path, line: &str, dir: Option<&Path>) -> Result<PathBuf, String> {
    const SCRIPT: &str = "$s = New-Object -ComObject WScript.Shell; \
        $p = Join-Path $s.SpecialFolders.Item('Desktop') ($env:MM2_LNK_NAME + '.lnk'); \
        $l = $s.CreateShortcut($p); \
        $l.TargetPath = $env:MM2_LNK_TARGET; \
        $l.Arguments = $env:MM2_LNK_ARGS; \
        $l.WorkingDirectory = $env:MM2_LNK_DIR; \
        $l.IconLocation = $env:MM2_LNK_TARGET + ',0'; \
        $l.Description = 'Motocross Madness 2 (OpenMCM2)'; \
        $l.Save(); \
        Write-Output $p";
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("MM2_LNK_NAME", NAME)
        .env("MM2_LNK_TARGET", viewer)
        .env("MM2_LNK_ARGS", line)
        .env("MM2_LNK_DIR", dir.unwrap_or(Path::new("")));
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
fn mac(viewer: &Path, args: &[String], dir: Option<&Path>) -> Result<PathBuf, String> {
    let sh = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let mut text = String::from("#!/bin/sh\n");
    if let Some(d) = dir {
        text += &format!("cd {} || exit 1\n", sh(&d.display().to_string()));
    }
    text += &format!("exec {}", sh(&viewer.display().to_string()));
    for a in args {
        text += " ";
        text += &sh(a);
    }
    text += "\n";
    write_runnable(desktop()?.join(format!("{NAME}.command")), &text)
}

/// A `.desktop` entry.
fn linux(viewer: &Path, line: &str, dir: Option<&Path>) -> Result<PathBuf, String> {
    let mut text = format!(
        "[Desktop Entry]\nType=Application\nName={NAME}\nComment=Motocross Madness 2 (OpenMCM2)\nExec={} {line}\nTerminal=false\nCategories=Game;\n",
        quoted(&viewer.display().to_string()),
    );
    if let Some(d) = dir {
        text += &format!("Path={}\n", d.display());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_arguments_with_spaces_are_quoted() {
        assert_eq!(quoted("--easier"), "--easier");
        assert_eq!(quoted("D:\\Games\\Motocross Madness 2"), "\"D:\\Games\\Motocross Madness 2\"");
    }
}
