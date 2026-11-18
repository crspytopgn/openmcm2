//! Lists an .mcf's motions in BIKE.RES: `mcf_list <GAME dir> Rider.mcf`.
use mm2_formats::{anim, faoe, rs2, Container};
use std::path::Path;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let raw = std::fs::read(Path::new(&a[1]).join("RES").join("BIKE.RES")).unwrap();
    let raw = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, "BIKE.RES").unwrap(), _ => raw };
    let ar = rs2::Archive::parse(&raw).unwrap();
    if a.len() < 3 { for e in ar.entries() { if e.name.to_ascii_lowercase().ends_with(".mcf") { println!("{}", e.name); } } return; }
    let e = ar.entries().iter().find(|e| e.name.eq_ignore_ascii_case(&a[2])).unwrap();
    let m = anim::Mcf::parse(ar.data(e)).unwrap();
    for mo in m.motions { println!("{}\t{}\t{}\t{}", mo.name, mo.fps, mo.frame_count, mo.looping); }
}
