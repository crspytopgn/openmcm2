//! The files the game reads and writes: the disk's, or in a browser, the
//! page's own store (`web/mm2fs.js`). There the game folder is the copy
//! the player picked, at `/game`, and their saves live at `/user`.

#[cfg(not(target_arch = "wasm32"))]
pub use std::fs::*;

#[cfg(not(target_arch = "wasm32"))]
mod paths {
    use std::path::Path;

    pub fn is_file(p: impl AsRef<Path>) -> bool {
        p.as_ref().is_file()
    }

    pub fn is_dir(p: impl AsRef<Path>) -> bool {
        p.as_ref().is_dir()
    }

    pub fn found(p: impl AsRef<Path>) -> bool {
        p.as_ref().exists()
    }
}

#[cfg(target_arch = "wasm32")]
mod paths {
    use std::ffi::OsString;
    use std::io;
    use std::path::{Component, Path, PathBuf};
    use wasm_bindgen::prelude::*;

    pub use std::fs::File;

    #[wasm_bindgen(js_namespace = mm2fs)]
    extern "C" {
        /// The file's bytes, or undefined.
        #[wasm_bindgen(js_name = read)]
        fn js_read(path: &str) -> JsValue;
        /// The names in a folder, folders ending in `/`, or undefined.
        #[wasm_bindgen(js_name = list)]
        fn js_list(path: &str) -> JsValue;
        /// 0 nothing, 1 a file, 2 a folder.
        #[wasm_bindgen(js_name = kind)]
        fn js_kind(path: &str) -> u8;
        #[wasm_bindgen(js_name = write)]
        fn js_write(path: &str, data: &[u8]) -> bool;
        /// Removes a file, or a folder and all in it.
        #[wasm_bindgen(js_name = remove)]
        fn js_remove(path: &str) -> bool;
    }

    /// `/a/b/c`, with `.` and `..` worked out.
    fn key(p: &Path) -> String {
        let mut parts: Vec<String> = Vec::new();
        for c in p.components() {
            match c {
                Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
                Component::ParentDir => {
                    parts.pop();
                }
                _ => {}
            }
        }
        format!("/{}", parts.join("/"))
    }

    fn missing(p: &Path) -> io::Error {
        io::Error::new(io::ErrorKind::NotFound, format!("{}: not found", p.display()))
    }

    pub fn is_file(p: impl AsRef<Path>) -> bool {
        js_kind(&key(p.as_ref())) == 1
    }

    pub fn is_dir(p: impl AsRef<Path>) -> bool {
        js_kind(&key(p.as_ref())) == 2
    }

    pub fn found(p: impl AsRef<Path>) -> bool {
        js_kind(&key(p.as_ref())) != 0
    }

    pub fn read(p: impl AsRef<Path>) -> io::Result<Vec<u8>> {
        let p = p.as_ref();
        let v = js_read(&key(p));
        if v.is_undefined() || v.is_null() {
            return Err(missing(p));
        }
        Ok(js_sys::Uint8Array::new(&v).to_vec())
    }

    pub fn read_to_string(p: impl AsRef<Path>) -> io::Result<String> {
        String::from_utf8(read(p)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn write(p: impl AsRef<Path>, data: impl AsRef<[u8]>) -> io::Result<()> {
        match js_write(&key(p.as_ref()), data.as_ref()) {
            true => Ok(()),
            false => Err(io::Error::new(io::ErrorKind::PermissionDenied, "the game folder cannot be written")),
        }
    }

    pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
        let bytes = read(from)?;
        write(to, &bytes)?;
        Ok(bytes.len() as u64)
    }

    /// Folders come with the files written in them.
    pub fn create_dir_all(_: impl AsRef<Path>) -> io::Result<()> {
        Ok(())
    }

    pub fn remove_file(p: impl AsRef<Path>) -> io::Result<()> {
        let p = p.as_ref();
        js_remove(&key(p)).then_some(()).ok_or_else(|| missing(p))
    }

    pub fn remove_dir_all(p: impl AsRef<Path>) -> io::Result<()> {
        remove_file(p)
    }

    pub struct DirEntry {
        path: PathBuf,
    }

    impl DirEntry {
        pub fn path(&self) -> PathBuf {
            self.path.clone()
        }

        pub fn file_name(&self) -> OsString {
            self.path.file_name().unwrap_or_default().to_owned()
        }
    }

    pub fn read_dir(p: impl AsRef<Path>) -> io::Result<std::vec::IntoIter<io::Result<DirEntry>>> {
        let p = p.as_ref();
        let v = js_list(&key(p));
        if v.is_undefined() || v.is_null() {
            return Err(missing(p));
        }
        let entries: Vec<io::Result<DirEntry>> = js_sys::Array::from(&v)
            .iter()
            .filter_map(|n| n.as_string())
            .map(|n| Ok(DirEntry { path: p.join(n.trim_end_matches('/')) }))
            .collect();
        Ok(entries.into_iter())
    }
}

pub use paths::*;
