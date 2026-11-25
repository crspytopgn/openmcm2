//! Lists every entry of a RES archive: `res_ls <GAME dir> BIKE.RES [filter]`.
use mm2_formats::{faoe, rs2, Container};
use std::path::Path;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let raw = std::fs::read(Path::new(&a[1]).join("RES").join(&a[2])).unwrap();
    let raw = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, &a[2]).unwrap(), _ => raw };
    let ar = rs2::Archive::parse(&raw).unwrap();
    let f = a.get(3).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    for e in ar.entries() { if e.name.to_ascii_lowercase().contains(&f) { println!("{}\t{}", e.name, ar.data(e).len()); } }
}
