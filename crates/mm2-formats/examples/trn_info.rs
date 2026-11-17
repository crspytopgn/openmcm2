//! Prints a track's terrain size, scale, heights and textures, and the
//! start of its scene. Usage: trn_info <track.env>
use mm2_formats::{faoe, rs2, trn, Container};

fn main() {
    let p = std::env::args().nth(1).expect("track file");
    let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
    let raw = std::fs::read(&p).unwrap();
    let plain = match Container::detect(&raw) {
        Container::Faoe => faoe::decode(&raw, &name).unwrap(),
        _ => raw,
    };
    let ar = rs2::Archive::parse(&plain).unwrap();
    for e in ar.entries() {
        let lower = e.name.to_ascii_lowercase();
        if lower.ends_with(".trn") {
            let t = trn::parse(ar.data(e)).unwrap();
            let (mn, mx) = t
                .heights
                .iter()
                .fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)));
            let mut sizes: Vec<u32> = t.tiles.iter().map(|t| t.size).collect();
            sizes.sort();
            sizes.dedup();
            let mut s = t.surfaces.clone();
            s.sort();
            s.dedup();
            println!(
                "{} size {} scale {} heights {mn}..{mx} textures {} (first {} px) tiles {} tile sizes {:?} surfaces {:?}",
                e.name,
                t.size,
                t.scale,
                t.textures.len(),
                t.textures[0].size,
                t.tiles.len(),
                sizes,
                s
            );
        }
        if lower.ends_with(".scn") {
            let d = ar.data(e);
            println!("--- {}\n{}", e.name, String::from_utf8_lossy(&d[..d.len().min(3000)]));
        }
    }
}
