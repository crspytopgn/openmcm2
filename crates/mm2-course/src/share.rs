//! Getting a track ready to share with other players (the leaderboard
//! server's shared tracks). Only the builder's recipe goes up: the line,
//! jumps, brush strokes, paint, stock objects, plants, rivers, flights and
//! look, a few KB. Files of the player's own packed into the track (their
//! models, ground picture and plant maps) stay on their computer, and a
//! copy of another track names that track's file, never the file or the
//! folder it is in: whoever downloads the copy needs that track themselves.

use crate::Course;

/// The most a shared recipe can be, bytes of JSON (the server's limit
/// too). A dressed Baja track is about 40 KB.
pub const MOST_BYTES: usize = 128 * 1024;

/// The track as it goes up, and what of the player's own was left out
/// (to tell them). A track raised from a height map of their own can't
/// go: its hills are in the picture.
pub fn for_sharing(c: &Course) -> Result<(Course, Vec<&'static str>), String> {
    if c.land.heightmap.is_some() {
        return Err("Its hills come from a height map picture of yours, which can't be shared".into());
    }
    let mut out = c.clone();
    let mut left = Vec::new();
    out.source = None;
    out.ground = None;
    if let Some(b) = &mut out.base {
        b.file = file_name(&b.file).to_string();
    }
    if !out.models.is_empty() {
        let mine: Vec<String> = out.models.iter().map(|m| m.name().to_ascii_lowercase()).collect();
        out.objects.retain(|o| !mine.contains(&o.model.to_ascii_lowercase()));
        out.models.clear();
        left.push("your own models");
    }
    if out.look.ground.take().is_some() {
        left.push("your ground picture");
    }
    let mut maps = false;
    for p in &mut out.plants {
        maps |= p.map.take().is_some();
    }
    if maps {
        left.push("your plant maps");
    }
    Ok((out, left))
}

/// The last part of a path, either way its slashes lean.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::Embedded;
    use crate::{copy::Base, Mode, Object, UserModel};

    #[test]
    fn a_copy_names_its_track_by_file_alone() {
        let mut c = Course::new(Mode::National);
        c.base = Some(Base { file: "/Users/someone/Games/MM2/TERAFORM/NATIONAL/FARM.ENV".into(), course: 0 });
        assert_eq!(for_sharing(&c).unwrap().0.base.unwrap().file, "FARM.ENV");
        c.base = Some(Base { file: "C:\\Games\\MM2\\TERAFORM\\SX\\SX01.ENV".into(), course: 1 });
        let b = for_sharing(&c).unwrap().0.base.unwrap();
        assert_eq!((b.file.as_str(), b.course), ("SX01.ENV", 1));
    }

    #[test]
    fn only_the_recipe_goes() {
        let mut c = Course::new(Mode::National);
        c.models.push(UserModel { slt: Embedded::new("barn.slt", &[7u8; 1000]), textures: vec![Embedded::new("barn.tga", &[9u8; 1000])] });
        c.objects.push(Object { model: "BARN.slt".into(), x: 1.0, z: 2.0, yaw: 0.0, scale: 1.0 });
        c.objects.push(Object { model: "TripleBale03.slt".into(), x: 1.0, z: 2.0, yaw: 0.0, scale: 1.0 });
        let (out, left) = for_sharing(&c).unwrap();
        assert!(out.models.is_empty());
        assert_eq!(out.objects.len(), 1);
        assert_eq!(left, ["your own models"]);
        assert!(out.to_json_compact().len() < 8 * 1024);
    }
}
