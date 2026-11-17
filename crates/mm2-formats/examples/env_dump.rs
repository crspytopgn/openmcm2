//! Lists an `.ENV`'s files and prints the text ones. Usage: env_dump <track.env> [file]
use mm2_formats::{faoe, rs2, Container};
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let want = std::env::args().nth(2);
    let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
    let raw = std::fs::read(&p).unwrap();
    let plain = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, &name).unwrap(), _ => raw };
    let ar = rs2::Archive::parse(&plain).unwrap();
    for e in ar.entries() {
        println!("{} {}", e.name, e.size);
        let l = e.name.to_ascii_lowercase();
        if want.as_deref().is_some_and(|w| w.eq_ignore_ascii_case(&e.name)) || (want.is_none() && (l.ends_with(".wpt"))) {
            println!("{}", String::from_utf8_lossy(ar.data(e)));
        }
    }
}
