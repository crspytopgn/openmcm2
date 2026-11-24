//! Asking the player for files of their own (the Course Builder's height
//! maps, pictures and models): a file dialog on a computer, the browser's
//! file picker on the web, whose answer comes a moment later.

/// A file picked: its name and bytes.
pub type Picked = (String, Vec<u8>);

/// Asks for files ending in one of `exts` (without dots). On a computer
/// the answer is here at once (empty if the player gave up); in a browser
/// it is None, and [`poll`] has it later.
pub fn ask(title: &str, exts: &[&str], many: bool) -> Option<Vec<Picked>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let d = rfd::FileDialog::new().set_title(title).add_filter("Files", exts);
        let paths = if many { d.pick_files().unwrap_or_default() } else { d.pick_file().into_iter().collect() };
        Some(
            paths
                .into_iter()
                .filter_map(|p| Some((p.file_name()?.to_string_lossy().to_string(), std::fs::read(&p).ok()?)))
                .collect(),
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = title;
        let accept: Vec<String> = exts.iter().map(|e| format!(".{e}")).collect();
        crate::web::pick_files(&accept.join(","), many);
        None
    }
}

/// The browser's answer to [`ask`], once the files are read.
pub fn poll() -> Option<Vec<Picked>> {
    #[cfg(target_arch = "wasm32")]
    {
        crate::web::picked()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

/// Keeps `bytes` as a file the player chooses (named `name` to start
/// with): where it went, or None if they gave up. In a browser it goes to
/// the downloads.
pub fn save(name: &str, bytes: &[u8]) -> Result<Option<String>, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new().set_file_name(name).save_file() else {
            return Ok(None);
        };
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(Some(path.display().to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        if crate::web::save_file(name, bytes) {
            Ok(Some(format!("{name} to your downloads")))
        } else {
            Err("the browser wouldn't download it".into())
        }
    }
}
