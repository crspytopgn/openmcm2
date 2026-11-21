//! Downloading and updating the Free Pack from its GitHub release.
//!
//! The release (tag `freepack` in the openmcm2 repo, made by
//! `tools/freepack/pack.py`) holds `freepack.json`, a manifest of every
//! file with its size and SHA-256 and the part zip it is in, and the part
//! zips. The pack's folder keeps the manifest it was last brought up to
//! date with, so an update downloads only the parts holding a changed
//! file, and removes files the pack no longer has.
//!
//! The new look menus alone (art, layout, words and sounds) come from the
//! same release: a folder kept with only the menus' files in its list
//! (`install_menus`) downloads and updates just their parts, for players
//! with the original graphics who want the new menus.

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The manifest's address; `MM2_FREEPACK_URL` points elsewhere (a test
/// server).
const MANIFEST_URL: &str = "https://github.com/crspytopgn/openmcm2/releases/download/freepack/freepack.json";
/// The manifest the pack's folder was last brought up to date with.
const LOCAL: &str = "freepack.json";
/// Where a download keeps its parts until it's done.
const STAGING: &str = ".download";
/// In `STAGING`: the part zips already unpacked by a download that then
/// stopped (their names hold a hash of what's in them, so a name is
/// enough), and the list the folder had before it started.
const DONE: &str = "done.txt";
const BEFORE: &str = "before.json";
/// The most a part may be.
const MAX_PART: u64 = 2 << 30;

pub fn manifest_url() -> String {
    std::env::var("MM2_FREEPACK_URL").unwrap_or_else(|_| MANIFEST_URL.to_string())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub part: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub file: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub version: String,
    pub parts: Vec<Part>,
    pub files: Vec<Entry>,
}

/// Whether a pack file belongs to the new look menus: their art (`ui/`),
/// layout and words (`menus/`) and sounds (`sound/ui/`). The same list
/// the browser page uses.
pub fn is_menu_file(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.starts_with("ui/") || p.starts_with("menus/") || p.starts_with("sound/ui/")
}

impl Manifest {
    /// Only the new look menus' files, and the parts holding them.
    pub fn menus_only(&self) -> Manifest {
        let files: Vec<Entry> = self.files.iter().filter(|f| is_menu_file(&f.path)).cloned().collect();
        let used: HashSet<&str> = files.iter().map(|f| f.part.as_str()).collect();
        Manifest { version: self.version.clone(), parts: self.parts.iter().filter(|p| used.contains(p.name.as_str())).cloned().collect(), files }
    }

    /// Whether this list holds only the menus' files (a folder kept by
    /// `install_menus`).
    pub fn is_menus_only(&self) -> bool {
        !self.files.is_empty() && self.files.iter().all(|f| is_menu_file(&f.path))
    }

    pub fn parse(text: &str) -> Result<Manifest, String> {
        let json: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("the Free Pack's list didn't make sense: {e}"))?;
        let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let n = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
        let list = |k: &str| json.get(k).and_then(|x| x.as_array()).cloned().unwrap_or_default();
        if json.get("format").and_then(|x| x.as_u64()) != Some(1) {
            return Err("this Free Pack needs a newer OpenMCM2: update it first (Updates)".into());
        }
        let files: Vec<Entry> = list("files")
            .iter()
            .map(|f| Entry { path: s(f, "path"), size: n(f, "size"), sha256: s(f, "sha256"), part: s(f, "part") })
            .collect();
        // Only plain paths inside the pack's folder.
        if files.iter().any(|f| f.path.is_empty() || f.path.starts_with('/') || f.path.split('/').any(|p| p == ".." || p.is_empty() || p.contains('\\'))) {
            return Err("the Free Pack's list has a file outside the pack, so it wasn't used".into());
        }
        Ok(Manifest {
            version: s(&json, "version"),
            parts: list("parts").iter().map(|p| Part { name: s(p, "name"), file: s(p, "file"), size: n(p, "size"), sha256: s(p, "sha256") }).collect(),
            files,
        })
    }

    /// The parts to fetch to bring a pack kept with `have` up to this one:
    /// those holding a file that is new or changed, or missing from `dir`.
    pub fn needed(&self, have: Option<&Manifest>, dir: &Path) -> Vec<Part> {
        let had: HashMap<&str, &str> = have.map(|m| m.files.iter().map(|f| (f.path.as_str(), f.sha256.as_str())).collect()).unwrap_or_default();
        let parts: HashSet<&str> = self
            .files
            .iter()
            .filter(|f| had.get(f.path.as_str()) != Some(&f.sha256.as_str()) || fs::metadata(dir.join(&f.path)).map(|m| m.len()).ok() != Some(f.size))
            .map(|f| f.part.as_str())
            .collect();
        self.parts.iter().filter(|p| parts.contains(p.name.as_str())).cloned().collect()
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3600)))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        // A stalled try ends, and the next one carries on from the bytes
        // already on disk, so even a slow line gets there.
        .timeout_recv_body(Some(Duration::from_secs(300)))
        .user_agent(concat!("OpenMCM2-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// The newest manifest, asked for up to three times.
pub fn fetch() -> Result<Manifest, String> {
    let mut last = String::new();
    for wait in [0, 2, 5] {
        std::thread::sleep(Duration::from_secs(wait));
        let got = agent()
            .get(&manifest_url())
            .call()
            .map_err(|e| format!("couldn't reach the Free Pack download: {e}"))
            .and_then(|mut r| r.body_mut().with_config().limit(32 << 20).read_to_string().map_err(|e| format!("couldn't read the Free Pack's list: {e}")));
        match got {
            Ok(text) => return Manifest::parse(&text),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// The manifest the pack in `dir` was kept up to date with.
pub fn local(dir: &Path) -> Option<Manifest> {
    Manifest::parse(&fs::read_to_string(dir.join(LOCAL)).ok()?).ok()
}

/// What a newer pack would take to download, if there is one.
pub struct Update {
    pub version: String,
    pub bytes: u64,
}

/// The newest list for the folder kept with `have`: the whole pack, or
/// only its menus for a menus-only folder.
fn newest_for(have: Option<&Manifest>, menus: bool) -> Result<Manifest, String> {
    let new = fetch()?;
    Ok(if menus || have.is_some_and(|h| h.is_menus_only()) { new.menus_only() } else { new })
}

/// Looks for a newer pack than the one in `dir` (or newer menus, for a
/// menus-only folder).
pub fn check(dir: &Path) -> Result<Option<Update>, String> {
    let have = local(dir);
    let new = newest_for(have.as_ref(), false)?;
    if have.as_ref().is_some_and(|h| h.version == new.version) {
        return Ok(None);
    }
    let parts = new.needed(have.as_ref(), dir);
    Ok((!parts.is_empty()).then(|| Update { version: new.version.clone(), bytes: parts.iter().map(|p| p.size).sum() }))
}

#[derive(Default)]
pub struct Progress {
    pub total: AtomicU64,
    pub done: AtomicU64,
    pub cancel: AtomicBool,
    pub stage: Mutex<String>,
    /// The pack's folder, or what went wrong.
    pub result: Mutex<Option<Result<PathBuf, String>>>,
}

/// Downloads the newest pack into `dir`, or brings the one there up to
/// date, on a thread of its own.
pub fn install(dir: PathBuf) -> Arc<Progress> {
    start(dir, false)
}

/// Downloads only the new look menus into `dir`, or brings them up to
/// date: a small part of the pack, for playing with the original graphics.
pub fn install_menus(dir: PathBuf) -> Arc<Progress> {
    start(dir, true)
}

fn start(dir: PathBuf, menus: bool) -> Arc<Progress> {
    let p = Arc::new(Progress::default());
    let q = p.clone();
    std::thread::spawn(move || {
        let r = run(&dir, &q, menus).map(|_| dir.clone());
        // After a failure the half-downloaded part and the list of finished
        // ones stay, so trying again carries on where it stopped.
        if r.is_ok() {
            let _ = fs::remove_dir_all(dir.join(STAGING));
        }
        *q.result.lock().unwrap() = Some(r);
    });
    p
}

fn run(dir: &Path, p: &Progress, menus: bool) -> Result<(), String> {
    let what = if menus { "the new look menus" } else { "the Free Pack" };
    *p.stage.lock().unwrap() = format!("Checking {what}…");
    let staging = dir.join(STAGING);
    fs::create_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    // The folder's list moves aside while it's being changed (a stopped
    // update isn't taken for a finished one), and is read back from there
    // when trying again.
    if let Ok(text) = fs::read_to_string(dir.join(LOCAL)) {
        fs::write(staging.join(BEFORE), text).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    let _ = fs::remove_file(dir.join(LOCAL));
    let have = fs::read_to_string(staging.join(BEFORE)).ok().and_then(|t| Manifest::parse(&t).ok());
    let new = newest_for(have.as_ref(), menus)?;
    let done: HashSet<String> = fs::read_to_string(staging.join(DONE)).unwrap_or_default().lines().map(str::to_string).collect();
    let parts: Vec<Part> = new.needed(have.as_ref(), dir).into_iter().filter(|x| !done.contains(&x.file)).collect();
    let base = manifest_url();
    let base = base.rsplit_once('/').map_or(base.as_str(), |(b, _)| b).to_string();
    let wanted: HashMap<&str, &Entry> = new.files.iter().map(|f| (f.path.as_str(), f)).collect();
    p.total.store(parts.iter().map(|x| x.size).sum::<u64>().max(1), Ordering::Relaxed);
    for (i, part) in parts.iter().enumerate() {
        let stage = format!("Downloading {what} {}: part {} of {}", new.version, i + 1, parts.len());
        *p.stage.lock().unwrap() = format!("{stage}…");
        let zip = staging.join(&part.file);
        download(&format!("{base}/{}", part.file), part, &zip, p, &stage)?;
        unpack(&zip, dir, &wanted)?;
        let _ = fs::remove_file(&zip);
        let mut list = fs::OpenOptions::new().create(true).append(true).open(staging.join(DONE)).map_err(|e| format!("{}: {e}", staging.display()))?;
        writeln!(list, "{}", part.file).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    // Files the pack no longer has.
    if let Some(have) = &have {
        for f in have.files.iter().filter(|f| !wanted.contains_key(f.path.as_str())) {
            let _ = fs::remove_file(dir.join(&f.path));
        }
    }
    let text = serde_json::to_string(&json_of(&new)).map_err(|e| e.to_string())?;
    fs::write(dir.join(LOCAL), text).map_err(|e| format!("{}: {e}", dir.display()))
}

fn json_of(m: &Manifest) -> serde_json::Value {
    serde_json::json!({
        "format": 1,
        "version": m.version,
        "parts": m.parts.iter().map(|p| serde_json::json!({"name": p.name, "file": p.file, "size": p.size, "sha256": p.sha256})).collect::<Vec<_>>(),
        "files": m.files.iter().map(|f| serde_json::json!({"path": f.path, "size": f.size, "sha256": f.sha256, "part": f.part})).collect::<Vec<_>>(),
    })
}

/// Why a try at a part stopped: worth trying again, or not.
enum Fail {
    Again(String),
    Stop(String),
}

/// Downloads a part into `to`, checked against its hash. A dropped or
/// stalled connection, a busy server or a damaged download is tried again
/// with growing waits (about two and a half minutes in all), each try
/// carrying on from the bytes already on disk.
pub fn download(url: &str, part: &Part, to: &Path, p: &Progress, stage: &str) -> Result<(), String> {
    if part.size > MAX_PART {
        return Err("a Free Pack part is far bigger than it should be".into());
    }
    let base = p.done.load(Ordering::Relaxed);
    let waits = [2u64, 5, 10, 20, 40, 60];
    let mut last = String::new();
    for attempt in 0..=waits.len() {
        if attempt > 0 {
            let wait = waits[attempt - 1];
            *p.stage.lock().unwrap() = format!("{stage}: {last}. Trying again in {wait} s…");
            for _ in 0..wait * 10 {
                if p.cancel.load(Ordering::Relaxed) {
                    return Err("download cancelled".into());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            *p.stage.lock().unwrap() = format!("{stage}…");
        }
        match try_download(url, part, to, p, base) {
            Ok(()) => return Ok(()),
            Err(Fail::Stop(e)) => return Err(e),
            Err(Fail::Again(e)) => last = e,
        }
    }
    Err(format!(
        "{} kept failing to download ({last}), even after trying again for a few minutes. Check your internet connection and try again: what already came down is kept, so it carries on where it stopped.",
        part.file
    ))
}

fn try_download(url: &str, part: &Part, to: &Path, p: &Progress, base: u64) -> Result<(), Fail> {
    let disk = |e: std::io::Error| Fail::Stop(format!("{}: {e}", to.display()));
    let mut have = fs::metadata(to).map(|m| m.len()).unwrap_or(0);
    if have > part.size {
        fs::remove_file(to).map_err(disk)?;
        have = 0;
    }
    p.done.store(base + have, Ordering::Relaxed);
    if have < part.size {
        let mut req = agent().get(url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let mut response = match req.call() {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(404)) => {
                return Err(Fail::Stop(format!(
                    "{} isn't on the download server. A new version of the pack may be on its way up: wait a few minutes and try again.",
                    part.file
                )))
            }
            // The bytes kept don't fit what the server has: start the part over.
            Err(ureq::Error::StatusCode(416)) => {
                let _ = fs::remove_file(to);
                return Err(Fail::Again("the download had to start over".into()));
            }
            Err(e) => return Err(Fail::Again(e.to_string())),
        };
        // A server that ignores the range sends the whole part again.
        let resumed = have > 0 && response.status().as_u16() == 206;
        if !resumed {
            have = 0;
            p.done.store(base, Ordering::Relaxed);
        }
        let mut file = fs::OpenOptions::new().create(true).write(true).append(resumed).truncate(!resumed).open(to).map_err(disk)?;
        let mut body = response.body_mut().with_config().limit(MAX_PART).reader();
        let mut buf = vec![0u8; 1 << 16];
        loop {
            if p.cancel.load(Ordering::Relaxed) {
                return Err(Fail::Stop("download cancelled".into()));
            }
            let n = body.read(&mut buf).map_err(|e| Fail::Again(e.to_string()))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(disk)?;
            have += n as u64;
            p.done.store(base + have, Ordering::Relaxed);
        }
    }
    let mut hash = Sha256::new();
    let mut file = fs::File::open(to).map_err(disk)?;
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(disk)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    if hex(hash.finalize().as_slice()) != part.sha256 {
        let _ = fs::remove_file(to);
        p.done.store(base, Ordering::Relaxed);
        return Err(Fail::Again(format!("{} came down damaged (wrong checksum)", part.file)));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Puts the part's files the manifest lists into `dir`, each checked
/// against its hash and written beside its place first.
fn unpack(zip: &Path, dir: &Path, wanted: &HashMap<&str, &Entry>) -> Result<(), String> {
    let mut z = zip::ZipArchive::new(fs::File::open(zip).map_err(|e| e.to_string())?).map_err(|e| format!("couldn't open a Free Pack part: {e}"))?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().map(|n| n.to_string()).unwrap_or_default();
        let Some(entry) = wanted.get(name.as_str()) else { continue };
        let out = dir.join(&entry.path);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let tmp = out.with_extension("part-new");
        let mut file = fs::File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; 1 << 16];
        loop {
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
        drop(file);
        if hex(hash.finalize().as_slice()) != entry.sha256 {
            let _ = fs::remove_file(&tmp);
            return Err(format!("{} doesn't match its checksum, so it wasn't used", entry.path));
        }
        fs::rename(&tmp, &out).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = r#"{"format":1,"version":"2","parts":[{"name":"a","file":"a-1.zip","size":10,"sha256":"x"},{"name":"b","file":"b-1.zip","size":20,"sha256":"y"}],
        "files":[{"path":"a/one.png","size":3,"sha256":"11","part":"a"},{"path":"b/two.glb","size":4,"sha256":"22","part":"b"}]}"#;

    #[test]
    fn needs_only_changed_parts() {
        let dir = std::env::temp_dir().join(format!("fp-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("a")).unwrap();
        fs::create_dir_all(dir.join("b")).unwrap();
        fs::write(dir.join("a/one.png"), b"abc").unwrap();
        fs::write(dir.join("b/two.glb"), b"abcd").unwrap();
        let new = Manifest::parse(TEXT).unwrap();
        assert_eq!(new.needed(None, &dir).len(), 2);
        let mut have = new.clone();
        have.files[1].sha256 = "old".into();
        let parts = new.needed(Some(&have), &dir);
        assert_eq!(parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["b"]);
        assert!(new.needed(Some(&new), &dir).is_empty());
        // A file gone from the folder comes back.
        fs::remove_file(dir.join("a/one.png")).unwrap();
        assert_eq!(new.needed(Some(&new), &dir)[0].name, "a");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn menus_alone() {
        let text = r#"{"format":1,"version":"3","parts":[{"name":"ui-1","file":"u.zip","size":10,"sha256":"x"},{"name":"sound","file":"s.zip","size":20,"sha256":"y"},{"name":"bikes","file":"b.zip","size":30,"sha256":"z"}],
            "files":[{"path":"ui/MAINMENU/a.png","size":3,"sha256":"11","part":"ui-1"},{"path":"menus/MAINMENU.dtm","size":3,"sha256":"12","part":"ui-1"},{"path":"sound/ui/click.wav","size":4,"sha256":"22","part":"sound"},{"path":"sound/engine/125/a.wav","size":4,"sha256":"23","part":"sound"},{"path":"bikes/a.glb","size":5,"sha256":"33","part":"bikes"}]}"#;
        let full = Manifest::parse(text).unwrap();
        assert!(!full.is_menus_only());
        let m = full.menus_only();
        assert!(m.is_menus_only());
        assert_eq!(m.files.len(), 3);
        assert_eq!(m.parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["ui-1", "sound"]);
        // Kept and read back, it's still the menus alone.
        assert!(Manifest::parse(&json_of(&m).to_string()).unwrap().is_menus_only());
    }

    #[test]
    fn refuses_paths_outside() {
        assert!(Manifest::parse(&TEXT.replace("a/one.png", "../x")).is_err());
        assert!(Manifest::parse(&TEXT.replace("\"format\":1", "\"format\":2")).is_err());
    }

    /// Downloads the pack `MM2_FREEPACK_URL` names into `MM2_FREEPACK_DIR`:
    /// `cargo test -p mm2-launcher downloads_a_pack -- --ignored --nocapture`
    /// (`MM2_FREEPACK_MENUS=1`: the new look menus alone).
    #[test]
    #[ignore]
    fn downloads_a_pack() {
        let dir = PathBuf::from(std::env::var("MM2_FREEPACK_DIR").expect("MM2_FREEPACK_DIR"));
        let p = if std::env::var_os("MM2_FREEPACK_MENUS").is_some() { install_menus(dir.clone()) } else { install(dir.clone()) };
        loop {
            if let Some(r) = p.result.lock().unwrap().take() {
                r.expect("download");
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        println!("downloaded {} bytes; version {}", p.done.load(Ordering::Relaxed), local(&dir).unwrap().version);
    }
}
