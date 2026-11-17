//! Lists what each track takes from the original game's shared files:
//! models, textures, sky, eco-system species and extra resource packs.
//! Usage: stock_refs <GAME/RES dir> <track.env>...   (TSV on stdout)
use mm2_formats::{esb, faoe, rs2, scn, slb, Container};
use std::collections::BTreeSet;

fn plain(raw: Vec<u8>, name: &str) -> Option<Vec<u8>> {
    match Container::detect(&raw) {
        Container::Faoe => faoe::decode(&raw, name).ok(),
        Container::Rs2 => Some(raw),
        _ => None,
    }
}
fn load(dir: &str, name: &str) -> Vec<u8> {
    let p = std::path::Path::new(dir).join(name);
    plain(std::fs::read(&p).unwrap(), name).unwrap()
}
fn has(a: &rs2::Archive, n: &str) -> bool {
    a.get(n).is_some()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let res = &args[0];
    let models = load(res, "MODELS.RES");
    let objects = load(res, "OBJECTS.RES");
    let eco = load(res, "ECO.RES");
    let skies = load(res, "SKIES.RES");
    let (models, objects, eco, skies) = (
        rs2::Archive::parse(&models).unwrap(),
        rs2::Archive::parse(&objects).unwrap(),
        rs2::Archive::parse(&eco).unwrap(),
        rs2::Archive::parse(&skies).unwrap(),
    );
    let pack_bytes: Vec<(String, Vec<u8>)> = ["ASDOME03.TEX", "B1B_02.TEX", "EURO.TEX", "FOOTBALL.TEX"].iter().map(|n| (n.to_string(), load(res, n))).collect();
    let packs: Vec<(String, rs2::Archive)> = pack_bytes.iter().map(|(n, b)| (n.clone(), rs2::Archive::parse(b).unwrap())).collect();
    for path in &args[1..] {
        let name = std::path::Path::new(path).file_name().unwrap().to_string_lossy().to_string();
        let Some(bytes) = plain(std::fs::read(path).unwrap(), &name) else {
            println!("{name}\tERROR\tnot an archive");
            continue;
        };
        let Ok(track) = rs2::Archive::parse(&bytes) else { continue };
        let find = |ext: &str| track.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(ext));
        let Some(scene) = find(".scn").and_then(|e| scn::Scene::parse(track.data(e)).ok()) else {
            println!("{name}\tERROR\tno scene");
            continue;
        };
        let mut out: BTreeSet<(String, String)> = BTreeSet::new();
        let mut tex = BTreeSet::new();
        let mut ms: BTreeSet<String> = scene.models.iter().map(|m| m.model.clone()).collect();
        ms.extend(scene.animations.iter().map(|m| m.model.clone()));
        ms.extend(scene.procedural.iter().map(|m| m.model.clone()));
        for m in &ms {
            let bare = m.rsplit(['\\', '/']).next().unwrap_or(m);
            let stem = bare.rsplit_once('.').map_or(bare, |(s, _)| s).trim();
            let (slt, slb_) = (format!("{stem}.slt"), format!("{stem}.slb"));
            fn look_up<'a>(a: &rs2::Archive<'a>, slt: &str, slb_: &str) -> Option<(bool, &'a [u8])> { a.get(slt).map(|d| (true, d)).or_else(|| a.get(slb_).map(|d| (false, d))) }
            let look = |a| look_up(a, &slt, &slb_);
            let found = if let Some(d) = look(&track) { Some(d) } else if let Some(d) = look(&models).or_else(|| look(&eco)) {
                out.insert(("model".into(), stem.to_string()));
                Some(d)
            } else {
                out.insert(("model-missing".into(), stem.to_string()));
                None
            };
            let mats = match found {
                Some((true, d)) => mm2_formats::slt::parse(d).ok().map(|m| m.materials),
                Some((false, d)) => slb::parse(d).ok().map(|m| m.materials),
                None => None,
            };
            if let Some(mats) = mats {
                tex.extend(mats.into_iter().map(|t| t.texture).filter(|t| !t.is_empty()));
            }
        }
        for t in &tex {
            if !has(&track, t) {
                let in_pack = scene.resource_files.iter().any(|r| packs.iter().any(|(n, a)| n.eq_ignore_ascii_case(r) && has(a, t)));
                let k = if has(&objects, t) || has(&eco, t) { "texture" } else if in_pack { "texture-stadium" } else { "texture-missing" };
                out.insert((k.into(), t.clone()));
            }
        }
        if let Some(c) = &scene.cube_file {
            if !has(&track, c) {
                out.insert((if has(&skies, c) { "sky" } else { "sky-missing" }.into(), c.clone()));
            }
        }
        for t in [&scene.detail_texture, &scene.particle_texture, &scene.stadium].into_iter().flatten() {
            if !has(&track, t) {
                out.insert(("extra".into(), t.clone()));
            }
        }
        for r in &scene.resource_files {
            out.insert(("resource-pack".into(), r.clone()));
        }
        if let Some(e) = find(".esb").and_then(|e| esb::parse(track.data(e)).ok()) {
            for s in &e.species {
                let (n, t) = (format!("{}.slb", s.name), format!("{}.slt", s.name));
                if !has(&track, &n) && !has(&track, &t) {
                    out.insert((if has(&eco, &n) || has(&eco, &t) { "eco" } else { "eco-missing" }.into(), s.name.clone()));
                }
            }
        }
        if out.is_empty() {
            println!("{name}\tnone\t-");
        }
        for (k, v) in out {
            println!("{name}\t{k}\t{v}");
        }
    }
}
