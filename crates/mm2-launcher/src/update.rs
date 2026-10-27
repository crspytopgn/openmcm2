//! Updating OpenMCM2 itself from its GitHub releases. Only the program
//! files are replaced (the Mac app, or the launcher, viewer and tools
//! beside each other); the settings, saves and game data in
//! [`settings::user_dir`](crate::settings::user_dir) are never touched.
//!
//! Everything goes over HTTPS. The download is checked against the
//! SHA-256 GitHub gives for it (or a `SHA256SUMS` file in the release)
//! before anything is unpacked, and the old files are put back if any
//! step of the swap fails.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/crspytopgn/openmcm2/releases/latest";
/// Where updates are unpacked, beside the installed files so they can be
/// swapped in by renaming.
const STAGING: &str = ".openmcm2-update";
/// No release is anywhere near this; a bigger download is refused.
const MAX_DOWNLOAD: u64 = 512 << 20;

/// This build's release tag, from the release build (`OPENMCM2_VERSION`,
/// or the release workflow's `VERSION`). Builds made from source have
/// none and never update themselves.
pub fn current() -> Option<&'static str> {
    [option_env!("OPENMCM2_VERSION"), option_env!("VERSION")]
        .into_iter()
        .flatten()
        .find(|v| parse(v).is_some())
}

/// `v1.2.3` as numbers, or none if it isn't a version.
fn parse(tag: &str) -> Option<Vec<u64>> {
    let t = tag.trim().trim_start_matches(['v', 'V']);
    let parts = t.split('.').map(|p| p.parse().ok()).collect::<Option<Vec<u64>>>()?;
    (!parts.is_empty()).then_some(parts)
}

/// Whether `tag` is a later version than `than`.
pub fn newer(tag: &str, than: &str) -> bool {
    match (parse(tag), parse(than)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            a > b
        }
        _ => false,
    }
}

/// The end of this platform's release archive name, as the release
/// workflow names them.
fn platform() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("-macos-arm64.zip")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("-macos-x86_64.zip")
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        Some("-windows-x86_64.zip")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("-linux-x86_64.tar.gz")
    } else {
        None
    }
}

/// What gets replaced: the Mac app bundle, or the folder the launcher,
/// viewer and tools sit in.
#[derive(Debug, Clone, PartialEq)]
pub enum Install {
    MacApp(PathBuf),
    Folder(PathBuf),
}

impl Install {
    /// The running launcher's own install, if it's laid out as a release
    /// puts it.
    pub fn here() -> Result<Install, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.canonicalize().unwrap_or(exe);
        Install::of(&exe)
    }

    fn of(exe: &Path) -> Result<Install, String> {
        let dir = exe.parent().ok_or("the launcher has no folder")?;
        if cfg!(target_os = "macos") {
            let app = dir.parent().and_then(Path::parent).filter(|a| {
                a.extension().is_some_and(|e| e == "app") && dir.ends_with("Contents/MacOS")
            });
            return app.map(|a| Install::MacApp(a.to_path_buf())).ok_or_else(|| "the launcher isn't inside OpenMCM2.app".into());
        }
        let viewer = format!("mm2-viewer{}", std::env::consts::EXE_SUFFIX);
        if !dir.join(viewer).is_file() {
            return Err("mm2-viewer isn't beside the launcher".into());
        }
        Ok(Install::Folder(dir.to_path_buf()))
    }

    /// The folder the update is unpacked in and old files are kept in.
    fn staging(&self) -> PathBuf {
        match self {
            Install::MacApp(app) => app.parent().unwrap_or(Path::new(".")).join(STAGING),
            Install::Folder(dir) => dir.join(STAGING),
        }
    }

    /// The launcher to start once updated.
    pub fn launcher(&self) -> PathBuf {
        match self {
            Install::MacApp(app) => app.join("Contents/MacOS/mm2-launcher"),
            Install::Folder(dir) => dir.join(format!("mm2-launcher{}", std::env::consts::EXE_SUFFIX)),
        }
    }

    /// Clears what an earlier update left: the staging folder and the
    /// old program files Windows wouldn't let go of while they ran.
    pub fn tidy(&self) {
        let _ = fs::remove_dir_all(self.staging());
        if let Install::Folder(dir) = self {
            for e in fs::read_dir(dir).into_iter().flatten().flatten() {
                if e.file_name().to_string_lossy().ends_with(".old") && e.path().is_file() {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
    }
}

/// A release newer than this build.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub tag: String,
    pub page: String,
    pub asset: String,
    url: String,
    size: u64,
    /// The SHA-256 GitHub reports for the asset, in hex.
    sha256: Option<String>,
    /// A `SHA256SUMS` file in the release, for when it doesn't.
    sums: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(600)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .user_agent(concat!("OpenMCM2-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Asks GitHub for the latest release, and returns it if it's newer than
/// this build and has an archive for this platform.
pub fn check() -> Result<Option<Release>, String> {
    let current = current().ok_or("this build wasn't made by a release, so it can't update itself")?;
    let text = agent()
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("couldn't reach GitHub: {e}"))?
        .body_mut()
        .with_config()
        .limit(4 << 20)
        .read_to_string()
        .map_err(|e| format!("couldn't read GitHub's answer: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("GitHub's answer didn't make sense: {e}"))?;
    release_from(&json, current, platform().ok_or("there are no release builds for this computer")?)
}

fn release_from(json: &serde_json::Value, current: &str, ending: &str) -> Result<Option<Release>, String> {
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    let tag = s(json, "tag_name").ok_or("the release has no tag")?;
    if !newer(&tag, current) {
        return Ok(None);
    }
    let assets = json.get("assets").and_then(|a| a.as_array()).cloned().unwrap_or_default();
    let asset = assets
        .iter()
        .find(|a| s(a, "name").is_some_and(|n| n.ends_with(ending)))
        .ok_or_else(|| format!("{tag} has no download for this computer yet"))?;
    let sums = assets
        .iter()
        .find(|a| s(a, "name").is_some_and(|n| n == "SHA256SUMS" || n == "SHA256SUMS.txt"))
        .and_then(|a| s(a, "browser_download_url"));
    Ok(Some(Release {
        page: s(json, "html_url").unwrap_or_else(|| "https://github.com/crspytopgn/openmcm2/releases/latest".into()),
        asset: s(asset, "name").unwrap_or_default(),
        url: s(asset, "browser_download_url").ok_or("the download has no address")?,
        size: asset.get("size").and_then(|x| x.as_u64()).unwrap_or(0),
        sha256: s(asset, "digest").and_then(|d| d.strip_prefix("sha256:").map(str::to_ascii_lowercase)),
        sums,
        tag,
    }))
}

/// How far an update has got, shared with the window.
#[derive(Default)]
pub struct Progress {
    pub total: AtomicU64,
    pub done: AtomicU64,
    pub cancel: AtomicBool,
    /// What it's doing now.
    pub stage: Mutex<String>,
    /// The launcher to restart into once it's done, or what went wrong.
    pub result: Mutex<Option<Result<PathBuf, String>>>,
}

/// Downloads `release` and swaps it in for `install` on a thread.
pub fn start(release: Release, install: Install) -> Arc<Progress> {
    let p = Arc::new(Progress::default());
    let q = p.clone();
    std::thread::spawn(move || {
        let r = run(&release, &install, &q);
        if r.is_err() {
            let _ = fs::remove_dir_all(install.staging());
        }
        *q.result.lock().unwrap() = Some(r);
    });
    p
}

fn run(release: &Release, install: &Install, p: &Progress) -> Result<PathBuf, String> {
    let staging = install.staging();
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)
        .map_err(|e| format!("can't write beside OpenMCM2 in {} ({e}); move it to a folder you own, or update by hand", staging.display()))?;
    let want = match (&release.sha256, &release.sums) {
        (Some(h), _) => h.clone(),
        (None, Some(url)) => checksum_from_sums(url, &release.asset)?,
        (None, None) => return Err("the release gives no checksum for its download, so it wasn't used".into()),
    };
    *p.stage.lock().unwrap() = format!("Downloading {}…", release.tag);
    let archive = staging.join(&release.asset);
    download(&release.url, release.size, &want, &archive, p)?;
    *p.stage.lock().unwrap() = "Installing…".into();
    let unpacked = staging.join("new");
    unpack(&archive, &unpacked)?;
    let top = single_folder(&unpacked)?;
    swap(install, &top, &staging.join("old"))?;
    Ok(install.launcher())
}

fn checksum_from_sums(url: &str, asset: &str) -> Result<String, String> {
    let text = agent()
        .get(url)
        .call()
        .map_err(|e| format!("couldn't fetch the checksums: {e}"))?
        .body_mut()
        .with_config()
        .limit(1 << 20)
        .read_to_string()
        .map_err(|e| format!("couldn't read the checksums: {e}"))?;
    find_sum(&text, asset).ok_or_else(|| format!("the checksums don't list {asset}"))
}

/// The hash for `asset` in a `sha256sum` style list.
fn find_sum(text: &str, asset: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (hash, name) = l.trim().split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == asset && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}

fn download(url: &str, size: u64, sha256: &str, to: &Path, p: &Progress) -> Result<(), String> {
    if size > MAX_DOWNLOAD {
        return Err("the download is far bigger than a release should be".into());
    }
    let mut response = agent().get(url).call().map_err(|e| format!("download failed: {e}"))?;
    p.total.store(size.max(1), Ordering::Relaxed);
    let mut body = response.body_mut().with_config().limit(MAX_DOWNLOAD).reader();
    let mut file = fs::File::create(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut got = 0u64;
    loop {
        if p.cancel.load(Ordering::Relaxed) {
            return Err("update cancelled".into());
        }
        let n = body.read(&mut buf).map_err(|e| format!("download failed: {e}"))?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", to.display()))?;
        got += n as u64;
        p.done.store(got, Ordering::Relaxed);
    }
    file.sync_all().map_err(|e| e.to_string())?;
    if size != 0 && got != size {
        return Err(format!("the download stopped short ({got} of {size} bytes)"));
    }
    let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if hex != sha256 {
        return Err("the download doesn't match its checksum, so it wasn't used".into());
    }
    Ok(())
}

/// Unpacks a release `.zip` or `.tar.gz` into `dest`, refusing anything
/// that would land outside it.
fn unpack(archive: &Path, dest: &Path) -> Result<(), String> {
    let err = |e: &dyn std::fmt::Display| format!("couldn't unpack the update: {e}");
    fs::create_dir_all(dest).map_err(|e| err(&e))?;
    let file = fs::File::open(archive).map_err(|e| err(&e))?;
    if archive.to_string_lossy().ends_with(".tar.gz") {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
        tar.set_preserve_permissions(true);
        for entry in tar.entries().map_err(|e| err(&e))? {
            let mut entry = entry.map_err(|e| err(&e))?;
            let kind = entry.header().entry_type();
            if !(kind.is_file() || kind.is_dir()) {
                return Err(err(&"it holds a link or device file"));
            }
            let path = entry.path().map_err(|e| err(&e))?.into_owned();
            if !safe(&path) {
                return Err(err(&format!("unsafe path {}", path.display())));
            }
            if !entry.unpack_in(dest).map_err(|e| err(&e))? {
                return Err(err(&format!("unsafe path {}", path.display())));
            }
        }
    } else {
        let mut zip = zip::ZipArchive::new(file).map_err(|e| err(&e))?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| err(&e))?;
            if f.is_symlink() {
                return Err(err(&"it holds a link"));
            }
            let rel = f.enclosed_name().filter(|p| safe(p)).ok_or_else(|| err(&"an unsafe path"))?;
            let out = dest.join(rel);
            if f.is_dir() {
                fs::create_dir_all(&out).map_err(|e| err(&e))?;
                continue;
            }
            if let Some(d) = out.parent() {
                fs::create_dir_all(d).map_err(|e| err(&e))?;
            }
            let mut w = fs::File::create(&out).map_err(|e| err(&e))?;
            io::copy(&mut f, &mut w).map_err(|e| err(&e))?;
            #[cfg(unix)]
            if let Some(mode) = f.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&out, fs::Permissions::from_mode(mode & 0o755)).map_err(|e| err(&e))?;
            }
        }
    }
    Ok(())
}

/// Whether an archive path stays inside the folder it's unpacked in.
fn safe(p: &Path) -> bool {
    p.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// The one folder a release archive holds (`openmcm2-vX-platform`).
fn single_folder(dir: &Path) -> Result<PathBuf, String> {
    let entries: Vec<_> =
        fs::read_dir(dir).map_err(|e| e.to_string())?.flatten().filter(|e| e.file_name() != "__MACOSX").collect();
    match entries.as_slice() {
        [one] if one.path().is_dir() => Ok(one.path()),
        _ => Err("the update isn't laid out as a release".into()),
    }
}

/// Puts the unpacked release `top` in place of `install`, keeping the old
/// files in `old` until it has all worked, and putting them back if not.
fn swap(install: &Install, top: &Path, old: &Path) -> Result<(), String> {
    fs::create_dir_all(old).map_err(|e| e.to_string())?;
    match install {
        Install::MacApp(app) => {
            let new = top.join("OpenMCM2.app");
            for f in ["Contents/MacOS/mm2-launcher", "Contents/MacOS/mm2-viewer", "Contents/Info.plist"] {
                if !new.join(f).is_file() {
                    return Err(format!("the update is missing OpenMCM2.app/{f}"));
                }
            }
            mac_prepare(&new)?;
            let kept = old.join("OpenMCM2.app");
            fs::rename(app, &kept).map_err(|e| format!("couldn't move the old app aside: {e}"))?;
            if let Err(e) = fs::rename(&new, app) {
                let _ = fs::rename(&kept, app);
                return Err(format!("couldn't put the new app in place: {e}"));
            }
            Ok(())
        }
        Install::Folder(dir) => {
            let exe = std::env::consts::EXE_SUFFIX;
            for f in ["mm2-launcher", "mm2-viewer"] {
                if !top.join(format!("{f}{exe}")).is_file() {
                    return Err(format!("the update is missing {f}{exe}"));
                }
            }
            // Only plain files sit in a release folder; each replaces the
            // file of the same name and nothing else in the folder.
            let files: Vec<_> = fs::read_dir(top).map_err(|e| e.to_string())?.flatten().filter(|e| e.path().is_file()).collect();
            let mut moved: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
            let mut fail = None;
            for e in &files {
                let dest = dir.join(e.file_name());
                let aside = if dest.exists() {
                    // A running exe can be renamed but not deleted on
                    // Windows: the copy beside it goes at the next start.
                    let mut beside = dir.join(format!("{}.old", e.file_name().to_string_lossy()));
                    if beside.exists() && fs::remove_file(&beside).is_err() {
                        beside = old.join(e.file_name());
                    }
                    if let Err(err) = fs::rename(&dest, &beside) {
                        fail = Some(format!("couldn't move {} aside: {err}", dest.display()));
                        break;
                    }
                    Some(beside)
                } else {
                    None
                };
                moved.push((dest.clone(), aside));
                if let Err(err) = fs::rename(e.path(), &dest) {
                    fail = Some(format!("couldn't put {} in place: {err}", dest.display()));
                    break;
                }
            }
            if let Some(f) = fail {
                for (dest, aside) in moved.into_iter().rev() {
                    if let Some(a) = aside {
                        let _ = fs::remove_file(&dest);
                        let _ = fs::rename(&a, &dest);
                    } else {
                        let _ = fs::remove_file(&dest);
                    }
                }
                return Err(f);
            }
            Ok(())
        }
    }
}

/// Readies a downloaded Mac app to open: no quarantine flag (the launcher's
/// own download doesn't set one, but a copied-in zip may have), and a
/// valid ad-hoc signature so Apple silicon will run it.
fn mac_prepare(app: &Path) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }
    use std::process::Command;
    let _ = Command::new("/usr/bin/xattr").args(["-dr", "com.apple.quarantine"]).arg(app).status();
    let ok = Command::new("/usr/bin/codesign").args(["--verify", "--deep"]).arg(app).status().is_ok_and(|s| s.success());
    if !ok {
        let signed = Command::new("/usr/bin/codesign")
            .args(["--force", "--deep", "--sign", "-"])
            .arg(app)
            .status()
            .is_ok_and(|s| s.success());
        if !signed {
            return Err("the new app's signature didn't check out".into());
        }
    }
    Ok(())
}

/// Starts the updated launcher; the caller then quits.
pub fn restart(install: &Install) -> io::Result<()> {
    use std::process::Command;
    match install {
        Install::MacApp(app) => Command::new("/usr/bin/open").arg("-n").arg(app).spawn().map(drop),
        Install::Folder(_) => Command::new(install.launcher()).spawn().map(drop),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(newer("v0.1.6", "v0.1.5"));
        assert!(newer("v0.2", "v0.1.9"));
        assert!(newer("v0.1.10", "v0.1.9"));
        assert!(!newer("v0.1.5", "v0.1.5"));
        assert!(!newer("v0.1.5.0", "v0.1.5"));
        assert!(!newer("v0.1.4", "v0.1.5"));
        assert!(!newer("nightly", "v0.1.5"));
    }

    #[test]
    fn picks_the_platform_asset() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"tag_name":"v0.1.6","html_url":"https://github.com/x/y/releases/tag/v0.1.6","assets":[
            {"name":"openmcm2-v0.1.6-linux-x86_64.tar.gz","size":10,"browser_download_url":"https://github.com/a.tgz","digest":"sha256:ABCD"},
            {"name":"openmcm2-v0.1.6-windows-x86_64.zip","size":20,"browser_download_url":"https://github.com/a.zip","digest":null}]}"#,
        )
        .unwrap();
        let r = release_from(&json, "v0.1.5", "-linux-x86_64.tar.gz").unwrap().unwrap();
        assert_eq!((r.size, r.sha256.as_deref(), r.sums), (10, Some("abcd"), None));
        let w = release_from(&json, "v0.1.5", "-windows-x86_64.zip").unwrap().unwrap();
        assert_eq!(w.sha256, None);
        assert_eq!(release_from(&json, "v0.1.6", "-linux-x86_64.tar.gz").unwrap(), None);
        assert!(release_from(&json, "v0.1.5", "-macos-arm64.zip").is_err());
    }

    #[test]
    fn sums() {
        let h = "a".repeat(64);
        let text = format!("{h}  openmcm2-v1-linux-x86_64.tar.gz\n{}  *other.zip\n", "b".repeat(64));
        assert_eq!(find_sum(&text, "openmcm2-v1-linux-x86_64.tar.gz"), Some(h));
        assert_eq!(find_sum(&text, "other.zip"), Some("b".repeat(64)));
        assert_eq!(find_sum(&text, "missing.zip"), None);
    }

    #[test]
    fn unsafe_paths() {
        assert!(safe(Path::new("openmcm2/mm2-viewer")));
        assert!(!safe(Path::new("../evil")));
        assert!(!safe(Path::new("/etc/passwd")));
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mm2-update-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A release archive built as the workflow builds it, unpacked and
    /// swapped over an older install, with the settings untouched.
    #[cfg(unix)]
    #[test]
    fn swaps_a_folder_install() {
        let root = tmp("folder");
        let install = root.join("OpenMCM2");
        fs::create_dir_all(install.join("GAME")).unwrap();
        for f in ["mm2-launcher", "mm2-viewer", "README.md", "GAME/UILANG.DLL", "launcher.ini"] {
            fs::write(install.join(f), "old").unwrap();
        }
        let rel = root.join("src/openmcm2-v9-linux-x86_64");
        fs::create_dir_all(&rel).unwrap();
        for f in ["mm2-launcher", "mm2-viewer", "mm2-tools", "README.md"] {
            fs::write(rel.join(f), "new").unwrap();
        }
        let archive = root.join("r.tar.gz");
        let status = std::process::Command::new("tar")
            .arg("czf")
            .arg(&archive)
            .arg("-C")
            .arg(root.join("src"))
            .arg("openmcm2-v9-linux-x86_64")
            .status()
            .unwrap();
        assert!(status.success());
        let inst = Install::Folder(install.clone());
        let unpacked = inst.staging().join("new");
        unpack(&archive, &unpacked).unwrap();
        let top = single_folder(&unpacked).unwrap();
        swap(&inst, &top, &inst.staging().join("old")).unwrap();
        for f in ["mm2-launcher", "mm2-viewer", "mm2-tools", "README.md"] {
            assert_eq!(fs::read_to_string(install.join(f)).unwrap(), "new", "{f}");
        }
        assert_eq!(fs::read_to_string(install.join("GAME/UILANG.DLL")).unwrap(), "old");
        assert_eq!(fs::read_to_string(install.join("launcher.ini")).unwrap(), "old");
        inst.tidy();
        assert!(!install.join("mm2-viewer.old").exists());
        assert!(!inst.staging().exists());
        let _ = fs::remove_dir_all(&root);
    }

    /// A broken release leaves the old files as they were.
    #[test]
    fn keeps_the_old_install_on_a_bad_update() {
        let root = tmp("bad");
        let exe = std::env::consts::EXE_SUFFIX;
        fs::write(root.join(format!("mm2-launcher{exe}")), "old").unwrap();
        fs::write(root.join(format!("mm2-viewer{exe}")), "old").unwrap();
        let top = root.join("top");
        fs::create_dir_all(&top).unwrap();
        fs::write(top.join(format!("mm2-launcher{exe}")), "new").unwrap();
        let inst = Install::Folder(root.clone());
        assert!(swap(&inst, &top, &root.join("old")).is_err());
        assert_eq!(fs::read_to_string(root.join(format!("mm2-launcher{exe}"))).unwrap(), "old");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_a_broken_archive() {
        let root = tmp("broken");
        let archive = root.join("x.zip");
        fs::write(&archive, b"not a zip").unwrap();
        assert!(unpack(&archive, &root.join("out")).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
