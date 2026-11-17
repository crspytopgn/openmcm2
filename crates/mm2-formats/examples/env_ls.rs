//! Lists a track's files. Usage: env_ls <track.env>
use mm2_formats::{faoe, rs2, Container};
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
    let raw = std::fs::read(&p).unwrap();
    let plain = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, &name).unwrap(), _ => raw };
    let ar = rs2::Archive::parse(&plain).unwrap();
    for e in ar.entries() { println!("{}\t{}", e.name, ar.data(e).len()); }
}
