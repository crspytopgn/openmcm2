use mm2_formats::{faoe, rs2, texture, Container};
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let raw = std::fs::read(dir.join("RES/GLOBAL.RES")).unwrap();
    let b = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, "GLOBAL.RES").unwrap(), _ => raw };
    let a = rs2::Archive::parse(&b).unwrap();
    for e in a.entries() {
        let n = e.name.to_ascii_lowercase();
        if !n.ends_with(".tga") { continue }
        match texture::parse_overlay(a.data(e)) { Ok(t) => println!("{}\t{}x{}", e.name, t.width, t.height), Err(_) => println!("{}\t?", e.name) }
    }
}
