//! Prints each model's bounding box (model space, after frame transforms)
//! for MODELS.RES and ECO.RES: `model_bounds <GAME dir>`.
use mm2_formats::{faoe, rs2, slb, slt, Container};
use std::path::Path;
fn archive(dir: &Path, name: &str) -> Vec<u8> {
    let raw = std::fs::read(dir.join("RES").join(name)).unwrap();
    match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, name).unwrap(), _ => raw }
}
fn main() {
    let dir = std::env::args().nth(1).expect("GAME dir");
    let dir = Path::new(&dir);
    println!("res\tname\tminx\tminy\tminz\tmaxx\tmaxy\tmaxz\ttris");
    for res in ["MODELS.RES", "ECO.RES"] {
        let bytes = archive(dir, res);
        let a = rs2::Archive::parse(&bytes).unwrap();
        for e in a.entries() {
            let n = e.name.to_ascii_lowercase();
            let m = if n.ends_with(".slb") { slb::parse(a.data(e)).ok() }
                    else if n.ends_with(".slt") { slt::parse(a.data(e)).ok() } else { None };
            let Some(m) = m else { continue };
            let w = m.world_matrices();
            let mut lo = [f32::MAX; 3]; let mut hi = [f32::MIN; 3]; let mut tris = 0;
            if let Some(l) = m.lods.first() {
                for s in &l.surfaces {
                    tris += s.faces.len();
                    for (i, p) in s.positions.iter().enumerate() {
                        let mm = &w[s.frame_of(i).min(w.len().saturating_sub(1))];
                        let q = [0, 1, 2].map(|c| p[0]*mm[c] + p[1]*mm[4+c] + p[2]*mm[8+c] + mm[12+c]);
                        for c in 0..3 { lo[c] = lo[c].min(q[c]); hi[c] = hi[c].max(q[c]); }
                    }
                }
            }
            println!("{res}\t{n}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{tris}", lo[0],lo[1],lo[2],hi[0],hi[1],hi[2]);
        }
    }
}
