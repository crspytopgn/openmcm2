use mm2_formats::{dtm, rs2};
// For every DAT in a UI folder: image file -> whether a control showing it also has its own text.
fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut v: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x.eq_ignore_ascii_case("dat"))).collect();
    v.sort();
    for p in v {
        let raw = std::fs::read(&p).unwrap();
        let Ok(a) = rs2::Archive::parse(&raw) else { continue };
        let Some(e) = a.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".dtm")) else { continue };
        let d = dtm::parse(a.data(e));
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        for c in &d.controls {
            let text = c.props.get("LocalTextId").is_some() || c.props.get("Caption").map_or(false, |t| !t.is_empty()) || c.props.get("Text").map_or(false, |t| !t.is_empty());
            for (k, v) in &c.props.0 {
                if !k.starts_with("Anim") || v == "0" { continue }
                if let Some(an) = d.anim(v) { for f in &an.frames { println!("{}\t{}\t{}\t{}", stem, f, text as u8, c.name); } }
            }
        }
    }
}
