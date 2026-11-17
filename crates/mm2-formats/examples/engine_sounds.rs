//! Lists each engine class's samples and their lengths, from ENGINE.RES.
//! Usage: engine_sounds <RES dir> [out dir to write the WAVs]
use mm2_formats::{faoe, rs2, Container};
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let p = std::path::Path::new(&args[0]).join("ENGINE.RES");
    let raw = std::fs::read(&p).unwrap();
    let plain = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, "ENGINE.RES").unwrap(), _ => raw };
    let ar = rs2::Archive::parse(&plain).unwrap();
    let ini = ar.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Audio_16.ini")).unwrap();
    let text = String::from_utf8_lossy(ar.data(ini)).to_string();
    println!("{text}");
    for e in ar.entries() {
        if e.name.to_ascii_lowercase().ends_with(".wav") {
            let w = mm2_formats::wav::parse(ar.data(e)).unwrap();
            println!("{}\t{:.2}s\t{} Hz", e.name, w.frames() as f32 / w.rate as f32, w.rate);
            if let Some(out) = args.get(1) { std::fs::write(std::path::Path::new(out).join(&e.name), ar.data(e)).unwrap(); }
        }
    }
}
