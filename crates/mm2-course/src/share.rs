//! Getting a track ready to share with other players (the leaderboard
//! server's shared tracks), and checking it carries none of a game's own
//! files: only what the player made goes up. A copy of another track keeps
//! the name of that track's file, never the file or the folder it is in;
//! whoever downloads the copy needs that track themselves.

use crate::picture::Embedded;
use crate::Course;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

/// The track as it goes up: a copy's track named by its file alone.
pub fn for_sharing(c: &Course) -> Course {
    let mut out = c.clone();
    out.source = None;
    out.ground = None;
    if let Some(b) = &mut out.base {
        b.file = file_name(&b.file).to_string();
    }
    out
}

/// The last part of a path, either way its slashes lean.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// The files of the player's own packed into the track: models and their
/// pictures, the ground picture, height map and plant maps.
pub fn embedded(c: &Course) -> Vec<&Embedded> {
    let mut out: Vec<&Embedded> = Vec::new();
    for m in &c.models {
        out.push(&m.slt);
        out.extend(&m.textures);
    }
    if let Some(g) = &c.look.ground {
        out.push(&g.picture.file);
    }
    if let Some(h) = &c.land.heightmap {
        out.push(&h.picture);
    }
    out.extend(c.plants.iter().filter_map(|p| p.map.as_ref()).map(|m| &m.file));
    out
}

/// The files in a game's archives, to tell a game's own file packed into
/// a track from one the player made.
#[derive(Default)]
pub struct Stock(HashSet<(usize, u64)>);

fn key(bytes: &[u8]) -> (usize, u64) {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    (bytes.len(), h.finish())
}

impl Stock {
    /// Adds each file in an archive (a `.RES` or a track's `.ENV`), and
    /// the archive itself.
    pub fn add_archive(&mut self, bytes: &[u8]) {
        self.0.insert(key(bytes));
        if let Ok(a) = mm2_formats::rs2::Archive::parse(bytes) {
            for e in a.entries() {
                let data = a.data(e);
                // Tiny files (a few bytes of settings) say nothing.
                if data.len() >= 256 {
                    self.0.insert(key(data));
                }
            }
        }
    }

    pub fn has(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 256 && self.0.contains(&key(bytes))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The names of the files packed into the track that are a game's own.
pub fn stock_files(c: &Course, stock: &Stock) -> Vec<String> {
    embedded(c).into_iter().filter(|e| stock.has(&e.bytes())).map(|e| e.name.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{copy::Base, Mode, UserModel};

    #[test]
    fn a_copy_names_its_track_by_file_alone() {
        let mut c = Course::new(Mode::National);
        c.base = Some(Base { file: "/Users/someone/Games/MM2/TERAFORM/NATIONAL/FARM.ENV".into(), course: 0 });
        assert_eq!(for_sharing(&c).base.unwrap().file, "FARM.ENV");
        c.base = Some(Base { file: "C:\\Games\\MM2\\TERAFORM\\SX\\SX01.ENV".into(), course: 1 });
        let b = for_sharing(&c).base.unwrap();
        assert_eq!((b.file.as_str(), b.course), ("SX01.ENV", 1));
    }

    #[test]
    fn finds_a_games_own_model() {
        let stock_model = vec![7u8; 1000];
        let mine = vec![9u8; 1000];
        let mut stock = Stock::default();
        stock.add_archive(&mm2_formats::rs2::build(&[("BARN.SLT", &stock_model)]));
        let mut c = Course::new(Mode::National);
        c.models.push(UserModel { slt: Embedded::new("barn.slt", &stock_model), textures: vec![Embedded::new("mine.tga", &mine)] });
        assert_eq!(stock_files(&c, &stock), ["barn.slt"]);
        c.models[0].slt = Embedded::new("barn.slt", &mine);
        assert!(stock_files(&c, &stock).is_empty());
    }
}
