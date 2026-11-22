//! Counts the plants on Baja tracks: originals and free ones.
//! Usage: eco_count <GAME>
use mm2_course::free;
use mm2_formats::{esb, faoe, rs2, Container};
use std::collections::BTreeMap;

fn report(name: &str, eco: &esb::EcoSystem, span: f32, lines: &[Vec<[f32; 2]>]) {
    let mut per: BTreeMap<String, usize> = BTreeMap::new();
    let unit = span / 65536.0;
    let mut tall = 0;
    let mut near = [0usize; 3];
    for o in &eco.objects {
        let sp = eco.species(o.species);
        let n = sp.map(|s| s.name.clone()).unwrap_or_default();
        *per.entry(n).or_default() += 1;
        let h = sp.map(|s| s.height[0]).unwrap_or(0.0);
        if h > 8.0 { tall += 1; }
        let (x, z) = (o.position[0] as f32 * unit, o.position[2] as f32 * unit);
        let d = lines.iter().flatten().map(|p| (p[0]-x).powi(2)+(p[1]-z).powi(2)).fold(f32::MAX, f32::min).sqrt();
        for (k, r) in [50.0, 150.0, 400.0].iter().enumerate() { if d < *r { near[k] += 1; } }
    }
    if let Ok(dir) = std::env::var("ECO_DUMP") {
        use std::io::Write;
        let mut f = std::fs::File::create(format!("{dir}/{name}.csv")).unwrap();
        for o in &eco.objects { let h = eco.species(o.species).map(|s| s.height[0]).unwrap_or(0.0); writeln!(f, "{},{},{h}", o.position[0] as f32 * unit, o.position[2] as f32 * unit).unwrap(); }
        let mut g = std::fs::File::create(format!("{dir}/{name}.lines")).unwrap();
        for l in lines { for p in l { writeln!(g, "{},{}", p[0], p[1]).unwrap(); } writeln!(g).unwrap(); }
    }
    println!("{name}: span {span:.0} ft, {} plants ({:.0} per sq mi), tall(>8ft) {tall}, within 50/150/400 ft of a course {:?}", eco.objects.len(), eco.objects.len() as f32 / (span*span) * 27_878_400.0, near);
    println!("   {:?}", per);
}

fn one(name: &str, plain: &[u8]) {
    let ar = rs2::Archive::parse(plain).unwrap();
    let mut span = 0.0; let mut eco = None; let mut lines = Vec::new();
    for e in ar.entries() {
        let n = e.name.to_ascii_lowercase();
        if n.ends_with(".trn") { if let Ok(t) = mm2_formats::trn::parse(ar.data(e)) { span = (t.size as f32 - 1.0) * t.scale; } }
        if n.ends_with(".esb") { eco = Some(esb::parse(ar.data(e)).unwrap()); }
        if n.ends_with(".tdf") { if let Ok(d) = mm2_formats::tdf::parse(ar.data(e)) { lines.push(d.nodes.iter().flat_map(|n| &n.sections).map(|c| [c.center[0], c.center[2]]).collect()); } }
        if n.ends_with(".scn") { let t = String::from_utf8_lossy(ar.data(e)); println!("   scn lines {}", t.lines().count()); }
    }
    if let Some(e) = eco { report(name, &e, span, &lines); }
}

fn main() {
    let game = std::path::PathBuf::from(std::env::args().nth(1).expect("GAME"));
    let only = std::env::args().nth(2);
    if only.as_deref().map_or(true, |o| o.starts_with("BAJA")) {
    for i in 1..=5 {
        let path = game.join(format!("TERAFORM/BAJA/BAJA0{i}.ENV"));
        let bytes = std::fs::read(&path).unwrap();
        let plain = match Container::detect(&bytes) { Container::Faoe => faoe::decode(&bytes, "x.ENV").unwrap(), _ => bytes.clone() };
        one(&format!("BAJA0{i}"), &plain);
    }
    }
    for def in free::SET.iter().filter(|d| d.file.starts_with("FPBAJA")) {
        if only.as_deref().is_some_and(|o| o != def.file) { continue; }
        let files = free::make(def, &|_| None).unwrap();
        one(def.file, &files[0].1);
    }
}
