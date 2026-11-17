//! Prints one file from a track. Usage: env_cat <track.env> <name>
use mm2_formats::{faoe, rs2, Container};
use std::io::Write;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let name = std::path::Path::new(&a[1]).file_name().unwrap().to_string_lossy().to_string();
    let raw = std::fs::read(&a[1]).unwrap();
    let plain = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, &name).unwrap(), _ => raw };
    let ar = rs2::Archive::parse(&plain).unwrap();
    let e = ar.entries().iter().find(|e| e.name.eq_ignore_ascii_case(&a[2])).expect("no such file");
    std::io::stdout().write_all(ar.data(e)).unwrap();
}
