//! Bounds of a BIKE.RES .col: `col_info <GAME dir> name.col`.
use mm2_formats::{col, faoe, rs2, Container};
use std::path::Path;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let raw = std::fs::read(Path::new(&a[1]).join("RES").join("BIKE.RES")).unwrap();
    let raw = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, "BIKE.RES").unwrap(), _ => raw };
    let ar = rs2::Archive::parse(&raw).unwrap();
    for n in &a[2..] {
        let e = ar.entries().iter().find(|e| e.name.eq_ignore_ascii_case(n)).unwrap();
        let c = col::parse(ar.data(e)).unwrap();
        println!("{n}: {} parts", c.parts.len());
        for p in &c.parts {
            let v = &p.body.vertices;
            let mut lo = [f32::MAX; 3]; let mut hi = [f32::MIN; 3];
            for q in v { for k in 0..3 { lo[k] = lo[k].min(q[k]); hi[k] = hi[k].max(q[k]); } }
            let t = &p.body.matrix[9..12];
            println!("  frame {:?} verts {} tris {} pts {} t {:?} lo {:.2?} hi {:.2?}", p.frame, v.len(), p.body.triangles.len(), p.body.points.len(), t, lo, hi);
        }
    }
}
