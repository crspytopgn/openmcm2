//! A small note on the main menu when a newer OpenMCM2 is out, for
//! players who start the game straight away and so never see the
//! launcher's update offer. The check runs once, on its own thread as the
//! game starts, and says nothing if GitHub can't be reached. Clicking the
//! note opens the launcher's Updates page (or the release page, without a
//! launcher); its × hides it until the next version. The launcher's
//! Updates page turns it off (`update_notice` in `launcher.ini`).

use std::sync::Mutex;

/// A release newer than this build.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub tag: String,
    pub page: String,
}

static NOTE: Mutex<Option<Note>> = Mutex::new(None);

/// The file in the user folder that keeps the version whose note was
/// closed.
const CLOSED: &str = "update-note.txt";

/// The note to show, if there is one.
pub fn get() -> Option<Note> {
    NOTE.lock().ok()?.clone()
}

/// `MM2_UPDATE_NOTE=v9.9.9` shows the note for that version without
/// asking GitHub (for screenshots and testing). Whether it was set.
pub fn from_env() -> bool {
    let Some(tag) = std::env::var("MM2_UPDATE_NOTE").ok().filter(|t| !t.trim().is_empty()) else {
        return false;
    };
    *NOTE.lock().unwrap() = Some(Note {
        tag: tag.trim().to_string(),
        page: mm2_settings::release::PAGE.into(),
    });
    true
}

/// Asks GitHub in the background whether a newer release is out, unless
/// this is a build made from source, or the player turned the note off.
pub fn check() {
    if from_env() {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use mm2_settings::release;
        let (Some(current), Some(ending)) = (release::current(), release::platform()) else {
            return;
        };
        if !mm2_settings::Settings::load().update_notice {
            return;
        }
        std::thread::spawn(move || {
            let Some(note) = latest(current, ending) else { return };
            if closed().as_deref() != Some(note.tag.as_str()) {
                *NOTE.lock().unwrap() = Some(note);
            }
        });
    }
}

/// The latest release, if it is newer than `current` and has its
/// download for this computer up (so the note never comes before the
/// files do).
#[cfg(not(target_arch = "wasm32"))]
fn latest(current: &str, ending: &str) -> Option<Note> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .user_agent(concat!("OpenMCM2/", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let text = agent
        .get(mm2_settings::release::LATEST)
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .body_mut()
        .with_config()
        .limit(4 << 20)
        .read_to_string()
        .ok()?;
    note_from(&serde_json::from_str(&text).ok()?, current, ending)
}

#[cfg(not(target_arch = "wasm32"))]
fn note_from(json: &serde_json::Value, current: &str, ending: &str) -> Option<Note> {
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    let tag = s(json, "tag_name")?;
    if !mm2_settings::release::newer(&tag, current) {
        return None;
    }
    let assets = json.get("assets")?.as_array()?;
    assets.iter().find(|a| s(a, "name").is_some_and(|n| n.ends_with(ending)))?;
    Some(Note {
        page: s(json, "html_url").unwrap_or_else(|| mm2_settings::release::PAGE.into()),
        tag,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn closed() -> Option<String> {
    let t = std::fs::read_to_string(mm2_settings::user_dir().join(CLOSED)).ok()?;
    Some(t.trim().to_string())
}

/// Hides the note until a later version than this one is out.
pub fn close() {
    let Some(note) = NOTE.lock().unwrap().take() else { return };
    let dir = mm2_settings::user_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Err(e) = std::fs::write(dir.join(CLOSED), format!("{}\n", note.tag)) {
        eprintln!("couldn't keep the closed update note in {}: {e}", dir.display());
    }
}

/// Opens the launcher beside the game on its Updates page, or, without
/// one, the release page in the browser. True if the launcher started,
/// when the game should close so its files can be updated.
pub fn open() -> bool {
    let Some(note) = get() else { return false };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let launcher = std::env::current_exe()
            .ok()
            .map(|me| me.with_file_name(format!("mm2-launcher{}", std::env::consts::EXE_SUFFIX)))
            .filter(|l| l.is_file());
        if let Some(l) = launcher {
            if std::process::Command::new(l).arg("--updates").spawn().is_ok() {
                return true;
            }
        }
        open_page(&note.page);
    }
    #[cfg(target_arch = "wasm32")]
    let _ = note;
    false
}

/// `url` in the player's web browser.
#[cfg(not(target_arch = "wasm32"))]
fn open_page(url: &str) {
    use std::process::Command;
    let r = if cfg!(windows) {
        Command::new("explorer").arg(url).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
    if let Err(e) = r {
        eprintln!("couldn't open {url}: {e}");
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn only_newer_releases_with_this_download() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"tag_name":"v0.5.5","html_url":"https://github.com/x/y/releases/tag/v0.5.5","assets":[
                {"name":"openmcm2-v0.5.5-windows-x86_64.zip"},{"name":"openmcm2-v0.5.5-linux-x86_64.tar.gz"}]}"#,
        )
        .unwrap();
        let n = note_from(&json, "v0.5.4", "-linux-x86_64.tar.gz").unwrap();
        assert_eq!(n.tag, "v0.5.5");
        assert_eq!(n.page, "https://github.com/x/y/releases/tag/v0.5.5");
        assert!(note_from(&json, "v0.5.5", "-linux-x86_64.tar.gz").is_none(), "not newer");
        assert!(note_from(&json, "v0.5.4", "-macos-arm64.zip").is_none(), "no Mac download yet");
    }
}
