//! Extra tracks made by players, fetched from a track site when the
//! player picks one. Nothing is bundled: each track comes straight from
//! the site that hosts it, and only when asked for. A track's `.env`
//! goes into the game folder the port lists that event's tracks from
//! (`TERAFORM/<EVENT>`), and `tracks.ini` in the user folder records what
//! was added so it can be taken out again.

use crate::game::find_ci;
use crate::settings::user_dir;
use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// One kind of track a site lists, and where the port looks for it.
pub struct Kind {
    pub label: &'static str,
    /// The site's own name for the category, in its listing and links.
    pub category: &'static str,
    /// The folder under `TERAFORM` the port reads this event from.
    pub folder: &'static str,
}

/// A site tracks can be fetched from.
pub struct Site {
    pub name: &'static str,
    pub home: &'static str,
    pub kinds: &'static [Kind],
    /// The listing page for a category or search word, pages from 1.
    list_url: fn(&str, u32) -> String,
    /// The tracks on a listing page, and how many pages there are.
    parse: fn(&Site, &str) -> Listing,
}

/// Where tracks come from. Another site is another entry here with its
/// own listing address and page reader.
pub const SITES: &[Site] = &[Site {
    name: "MDX Racing",
    home: "https://mdxracing.com/new/index.php",
    kinds: &[
        Kind { label: "Nationals", category: "National_Tracks", folder: "NATIONAL" },
        Kind { label: "Supercross", category: "SuperCross_Tracks", folder: "SX" },
        Kind { label: "Baja", category: "BAJA_Maps", folder: "BAJA" },
        Kind { label: "Enduro", category: "ENDURO", folder: "ENDURO" },
        Kind { label: "Stunt", category: "Stunt_Maps", folder: "QUARRIES" },
    ],
    // The site lists nothing for `page=1` on a one-page category, so the
    // first page goes without it.
    list_url: |q, page| match page {
        1 => format!("https://mdxracing.com/new/index3.php?q={}", encode(q, false)),
        _ => format!("https://mdxracing.com/new/index3.php?page={page}&c=&sc=&q={}", encode(q, false)),
    },
    parse: parse_mdx,
}];

#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub name: String,
    pub author: String,
    pub url: String,
    pub picture: Option<String>,
    /// The folder under `TERAFORM` it goes in.
    pub folder: &'static str,
}

impl Track {
    /// Only zips can be unpacked here; a few tracks are `.rar`.
    pub fn can_add(&self) -> bool {
        self.url.to_ascii_lowercase().ends_with(".zip")
    }
}

#[derive(Clone, Debug, Default)]
pub struct Listing {
    pub tracks: Vec<Track>,
    pub pages: u32,
}

/// The biggest track download taken (the largest seen are under 20 MB).
const MAX_DOWNLOAD: u64 = 300 << 20;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(600)))
        .timeout_connect(Some(Duration::from_secs(20)))
        .user_agent(concat!("OpenMCM2-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Runs `f` up to three times, as track sites drop connections now and then.
fn retry<T>(mut f: impl FnMut() -> Result<T, String>, cancel: &AtomicBool) -> Result<T, String> {
    let mut last = String::new();
    for wait in [0, 2, 5] {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        std::thread::sleep(Duration::from_secs(wait));
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// One listing page from `site`, for a category or a search word.
pub fn list(site: &'static Site, query: &str, page: u32) -> Result<Listing, String> {
    let url = (site.list_url)(query, page);
    let text = retry(
        || {
            agent()
                .get(&url)
                .call()
                .map_err(|e| format!("couldn't reach {}: {e}", site.name))?
                .body_mut()
                .with_config()
                .limit(8 << 20)
                .read_to_vec()
                .map_err(|e| format!("couldn't read {}'s page: {e}", site.name))
        },
        &AtomicBool::new(false),
    )?;
    // Old sites write Latin-1, so each byte past ASCII is its own character.
    let text: String = match String::from_utf8(text) {
        Ok(t) => t,
        Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
    };
    Ok((site.parse)(site, &text))
}

/// Reads an MDX Racing listing: each track is a bold "NAME Track By
/// AUTHOR", then a link to `../mcm2/Tracks/<category>/<file>` around its
/// picture. Links into categories that aren't tracks (bike skins in a
/// search) are left out.
fn parse_mdx(site: &Site, html: &str) -> Listing {
    let base = "https://mdxracing.com/";
    let mut tracks = Vec::new();
    let mut from = 0;
    while let Some(i) = html[from..].find("href='../mcm2/Tracks/").map(|i| i + from) {
        let start = i + "href='../".len();
        let Some(len) = html[start..].find('\'') else { break };
        let rel = &html[start..start + len];
        let before = &html[from..i];
        from = start + len;
        let category = rel.trim_start_matches("mcm2/Tracks/").split('/').next().unwrap_or("");
        let Some(kind) = site.kinds.iter().find(|k| k.category.eq_ignore_ascii_case(category)) else { continue };
        let lower = rel.to_ascii_lowercase();
        if ![".zip", ".rar", ".7z"].iter().any(|e| lower.ends_with(e)) {
            continue;
        }
        let file = rel.rsplit('/').next().unwrap_or(rel);
        let stem = file.rsplit_once('.').map_or(file, |s| s.0).to_string();
        let title = before
            .rfind("<b>")
            .and_then(|b| before[b + 3..].find("</b>").map(|e| unescape(&before[b + 3..b + 3 + e])))
            .unwrap_or_default();
        let (name, author) = match title.to_ascii_lowercase().rfind(" track by") {
            Some(k) => (title[..k].trim().to_string(), title[k + " track by".len()..].trim().to_string()),
            None => (title.trim().to_string(), String::new()),
        };
        let rest = &html[from..];
        let end = rest.find("</a>").unwrap_or(rest.len());
        let picture = rest[..end].find("src='../").and_then(|s| {
            let p = &rest[s + "src='../".len()..end];
            p.find('\'').map(|e| format!("{base}{}", encode(&p[..e], true)))
        });
        tracks.push(Track {
            name: if name.is_empty() { stem } else { name },
            author,
            url: format!("{base}{}", encode(rel, true)),
            picture,
            folder: kind.folder,
        });
    }
    let pages = html
        .match_indices("page=")
        .filter_map(|(i, _)| {
            let d: String = html[i + 5..].chars().take_while(char::is_ascii_digit).collect();
            d.parse().ok()
        })
        .max()
        .unwrap_or(1);
    Listing { tracks, pages }
}

/// Percent-encodes `s` for a URL; `/` is kept when it's a path.
fn encode(s: &str, path: bool) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) || (path && b == b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&#039;", "'").replace("&#39;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// What has been added, by download address: the files put in the game
/// folder, relative to it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Added(pub BTreeMap<String, Vec<String>>);

impl Added {
    fn path() -> PathBuf {
        user_dir().join("tracks.ini")
    }

    pub fn load() -> Added {
        let text = std::fs::read_to_string(Added::path()).unwrap_or_default();
        Added(
            text.lines()
                .filter_map(|l| l.split_once('\t'))
                .map(|(url, files)| (url.to_string(), files.split('|').filter(|f| !f.is_empty()).map(str::to_string).collect()))
                .collect(),
        )
    }

    fn save(&self) -> std::io::Result<()> {
        let text: String = self.0.iter().map(|(url, files)| format!("{url}\t{}\n", files.join("|"))).collect();
        std::fs::create_dir_all(user_dir())?;
        std::fs::write(Added::path(), text)
    }

    /// Whether `url`'s track is in the game folder `game`.
    pub fn has(&self, game: &Path, url: &str) -> bool {
        self.0.get(url).is_some_and(|files| !files.is_empty() && files.iter().all(|f| game.join(f).is_file()))
    }

    /// The number of added tracks still in `game`.
    pub fn count(&self, game: &Path) -> usize {
        self.0.keys().filter(|u| self.has(game, u)).count()
    }

    /// Takes `url`'s track out of `game` again.
    pub fn remove(&mut self, game: &Path, url: &str) -> Result<(), String> {
        for f in self.0.get(url).cloned().unwrap_or_default() {
            let p = game.join(&f);
            if p.exists() {
                std::fs::remove_file(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            }
        }
        self.0.remove(url);
        self.save().map_err(|e| e.to_string())
    }
}

/// How an added track's download is going, shared with the window.
#[derive(Default)]
pub struct Job {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
    /// The end: the files added, or why it failed.
    pub result: Mutex<Option<Result<Vec<String>, String>>>,
}

/// Downloads `track` and puts it in the game folder `game` on a thread
/// of its own.
pub fn add(track: Track, game: PathBuf) -> Arc<Job> {
    let job = Arc::new(Job::default());
    let j = job.clone();
    std::thread::spawn(move || {
        let r = retry(|| download(&track.url, &j), &j.cancel).and_then(|bytes| install(&bytes, &track, &game));
        if let Ok(files) = &r {
            // Two downloads can end together; one writes the list at a time.
            static LIST: Mutex<()> = Mutex::new(());
            let _held = LIST.lock().unwrap();
            let mut added = Added::load();
            added.0.insert(track.url.clone(), files.clone());
            if let Err(e) = added.save() {
                *j.result.lock().unwrap() = Some(Err(format!("added, but couldn't note it down: {e}")));
                return;
            }
        }
        *j.result.lock().unwrap() = Some(r);
    });
    job
}

fn download(url: &str, j: &Job) -> Result<Vec<u8>, String> {
    let mut response = agent().get(url).call().map_err(|e| format!("download failed: {e}"))?;
    let size = response.body().content_length().unwrap_or(0);
    if size > MAX_DOWNLOAD {
        return Err("it's far bigger than a track should be".into());
    }
    j.total.store(size.max(1), Ordering::Relaxed);
    j.done.store(0, Ordering::Relaxed);
    let mut body = response.body_mut().with_config().limit(MAX_DOWNLOAD).reader();
    let mut out = Vec::with_capacity(size as usize);
    let mut buf = vec![0u8; 1 << 16];
    loop {
        if j.cancel.load(Ordering::Relaxed) {
            return Err("cancelled".into());
        }
        let n = body.read(&mut buf).map_err(|e| format!("download failed: {e}"))?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        j.done.store(out.len() as u64, Ordering::Relaxed);
    }
    if size != 0 && out.len() as u64 != size {
        return Err(format!("the download stopped short ({} of {size} bytes)", out.len()));
    }
    Ok(out)
}

/// Puts each `.env` in the zip `bytes` into `game/TERAFORM/<folder>`.
/// A track never replaces a file it didn't add itself, so the game's own
/// tracks are safe from one with the same name.
fn install(bytes: &[u8], track: &Track, game: &Path) -> Result<Vec<String>, String> {
    if !bytes.starts_with(b"PK") {
        return Err(if bytes.starts_with(b"Rar!") {
            "it's a .rar archive, which can't be unpacked here".into()
        } else {
            "the site didn't send a track (it may have been taken down)".into()
        });
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("the track's zip is broken: {e}"))?;
    let teraform = find_ci(game, "TERAFORM").ok_or("the game folder has no TERAFORM folder")?;
    let dir = find_ci(&teraform, track.folder).unwrap_or_else(|| teraform.join(track.folder));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let ours = Added::load();
    let ours: Vec<&String> = ours.0.values().flatten().collect();
    let mut files = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| format!("the track's zip is broken: {e}"))?;
        let Some(name) = f.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())) else { continue };
        if f.is_dir() || !name.to_ascii_lowercase().ends_with(".env") || f.size() > MAX_DOWNLOAD {
            continue;
        }
        let out = dir.join(&name);
        let rel = out.strip_prefix(game).unwrap_or(&out).to_string_lossy().replace('\\', "/");
        let clash = std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .flatten()
            .any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(&name));
        if clash && !ours.contains(&&rel) {
            return Err(format!("you already have a track file called {name}, so it was left alone"));
        }
        let mut data = Vec::new();
        f.read_to_end(&mut data).map_err(|e| format!("couldn't unpack {name}: {e}"))?;
        let tmp = out.with_extension("env.part");
        std::fs::write(&tmp, &data).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &out).map_err(|e| format!("{}: {e}", out.display()))?;
        files.push(rel);
    }
    if files.is_empty() {
        return Err("there's no track (.env) in the download".into());
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const PAGE: &str = "<font color=#00FF00 size=2><br><b>Rat&#039;s Hole Mx Park Track By Mikemach</b><br></a></font>
        <a href='../mcm2/Tracks/National_Tracks/Rats Hole Mx Park.zip' download data-id='1929'>
        <img class='item-pic' src='../mcm2/Tracks/National_Tracks/Rats Hole Mx Park.jpg' /><font>  </a></font></a>
        <b>Skin Track By Someone</b><a href='../mcm2/Tracks/Bike_Skins/Bike_Skin_A/x.zip' download>
        <img src='../mcm2/Tracks/Bike_Skins/x.jpg' /></a>
        <font><b>Stunt Park Track By </b></font><a href='../mcm2/Tracks/Stunt_Maps/Stunt Park.rar' download></a>
        <a href='index3.php?page=1&c=&sc=&q=National_Tracks'>1</a><a href='index3.php?page=58&c=&sc=&q=National_Tracks'>58</a>";

    #[test]
    fn reads_an_mdx_page() {
        let l = parse_mdx(&SITES[0], PAGE);
        assert_eq!(l.pages, 58);
        assert_eq!(l.tracks.len(), 2);
        let t = &l.tracks[0];
        assert_eq!(t.name, "Rat's Hole Mx Park");
        assert_eq!(t.author, "Mikemach");
        assert_eq!(t.url, "https://mdxracing.com/mcm2/Tracks/National_Tracks/Rats%20Hole%20Mx%20Park.zip");
        assert_eq!(t.picture.as_deref(), Some("https://mdxracing.com/mcm2/Tracks/National_Tracks/Rats%20Hole%20Mx%20Park.jpg"));
        assert_eq!(t.folder, "NATIONAL");
        assert!(t.can_add());
        assert_eq!(l.tracks[1].name, "Stunt Park");
        assert_eq!(l.tracks[1].folder, "QUARRIES");
        assert!(!l.tracks[1].can_add());
    }

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn installs_the_env_and_never_replaces_a_stock_track() {
        let game = std::env::temp_dir().join(format!("mm2-tracks-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&game);
        std::fs::create_dir_all(game.join("TERAFORM/ENDURO")).unwrap();
        std::fs::write(game.join("TERAFORM/ENDURO/FARM.env"), b"stock").unwrap();
        let track = |folder| Track { name: "x".into(), author: String::new(), url: "u".into(), picture: None, folder };
        let files = install(&zip_of(&[("readme.txt", b"hi"), ("sub/ar.env", b"track")]), &track("NATIONAL"), &game).unwrap();
        assert_eq!(files, ["TERAFORM/NATIONAL/ar.env"]);
        assert_eq!(std::fs::read(game.join("TERAFORM/NATIONAL/ar.env")).unwrap(), b"track");
        let err = install(&zip_of(&[("farm.ENV", b"mdx")]), &track("ENDURO"), &game).unwrap_err();
        assert!(err.contains("already have"), "{err}");
        assert_eq!(std::fs::read(game.join("TERAFORM/ENDURO/FARM.env")).unwrap(), b"stock");
        assert!(install(b"<html>404</html>", &track("SX"), &game).unwrap_err().contains("taken down"));
        assert!(install(&zip_of(&[("a.txt", b"")]), &track("SX"), &game).unwrap_err().contains("no track"));
        let _ = std::fs::remove_dir_all(&game);
    }

    /// Lists and adds a real track; run with `--ignored` when online.
    #[test]
    #[ignore]
    fn live_mdx() {
        for (i, k) in SITES[0].kinds.iter().enumerate() {
            let l = list(&SITES[0], k.category, 1).unwrap();
            println!("{}: {} tracks, {} pages, first {:?}", k.label, l.tracks.len(), l.pages, l.tracks.first().map(|t| &t.name));
            assert!(!l.tracks.is_empty(), "{i}");
        }
        let found = list(&SITES[0], "forest", 1).unwrap();
        println!("search forest: {} tracks", found.tracks.len());
        let game = std::env::temp_dir().join(format!("mm2-tracks-live-{}", std::process::id()));
        std::fs::create_dir_all(game.join("TERAFORM/SX")).unwrap();
        let track = list(&SITES[0], "SuperCross_Tracks", 1).unwrap().tracks.into_iter().find(Track::can_add).unwrap();
        let job = Arc::new(Job::default());
        let bytes = retry(|| download(&track.url, &job), &job.cancel).unwrap();
        let files = install(&bytes, &track, &game).unwrap();
        println!("{} -> {files:?}", track.name);
        assert!(game.join(&files[0]).metadata().unwrap().len() > 1000);
        std::fs::remove_dir_all(&game).unwrap();
    }
}
