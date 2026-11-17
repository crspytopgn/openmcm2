use mm2_formats::{rs2, tga};
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut v: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x.eq_ignore_ascii_case("dat"))).collect();
    v.sort();
    for p in v {
        let raw = std::fs::read(&p).unwrap();
        let Ok(a) = rs2::Archive::parse(&raw) else { continue };
        for e in a.entries() {
            let n = e.name.to_ascii_lowercase();
            if !n.ends_with(".tga") { continue }
            match tga::parse(a.data(e)) { Ok(t) => println!("{}\t{}\t{}x{}", p.file_name().unwrap().to_string_lossy(), e.name, t.width, t.height), Err(_) => println!("{}\t{}\t?", p.file_name().unwrap().to_string_lossy(), e.name) }
        }
    }
}
