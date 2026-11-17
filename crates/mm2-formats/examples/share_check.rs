// share_check OLD NEW: every .glb in NEW must give the same model and the
// same picture bytes per material as the one in OLD.
use std::path::{Path, PathBuf};
fn walk(d: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() { walk(&p, out) } else if p.extension().is_some_and(|x| x == "glb") { out.push(p) }
    }
}
fn load(p: &Path) -> (String, Vec<Vec<u8>>) {
    let d = std::fs::read(p).unwrap();
    let (m, ims) = mm2_formats::glb::parse(&d, "x:").unwrap();
    let pics = m.materials.iter().map(|mat| {
        ims.iter().find(|i| i.name == mat.texture).map(|i| match &i.uri {
            Some(u) => std::fs::read(p.parent().unwrap().join(u)).unwrap(),
            None => i.data.clone(),
        }).unwrap_or_default()
    }).collect();
    let mut m2 = m.clone();
    for mat in m2.materials.iter_mut() { mat.texture.clear(); }
    (format!("{:?}", m2.lods.iter().map(|l| l.surfaces.len()).collect::<Vec<_>>()) + &format!("{:?}", m2), pics)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (old, new) = (PathBuf::from(&a[1]), PathBuf::from(&a[2]));
    let mut files = vec![]; walk(&new, &mut files);
    let (mut ok, mut bad) = (0, 0);
    for f in &files {
        let rel = f.strip_prefix(&new).unwrap();
        if load(&old.join(rel)) == load(f) { ok += 1 } else { bad += 1; println!("DIFF {}", rel.display()) }
    }
    println!("same {ok} differ {bad}");
}
