//! Dumps an .mcf's model frames and named motions as JSON:
//! `motion_dump <GAME dir> Rider.mcf Turn2D LeanR02 ...` (an .mcf in
//! `RES/BIKE.RES`; its model from there or `RES/OBJECTS.RES`).
use mm2_formats::{anim, faoe, rs2, slb, Container};
use std::path::Path;
fn archive(dir: &Path, name: &str) -> Vec<u8> {
    let raw = std::fs::read(dir.join("RES").join(name)).unwrap();
    match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, name).unwrap(), _ => raw }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = Path::new(&args[1]);
    let objs = archive(dir, "BIKE.RES");
    let a = rs2::Archive::parse(&objs).unwrap();
    let get = |n: &str| a.entries().iter().find(|e| e.name.eq_ignore_ascii_case(n)).map(|e| a.data(e).to_vec());
    let mcf = anim::Mcf::parse(&get(&args[2]).expect("mcf")).unwrap();
    let slbname = mcf.model.replace(".slt", ".slb").replace(".SLT", ".slb");
    let model = get(&slbname).and_then(|b| slb::parse(&b).ok()).or_else(|| {
        let m = archive(dir, "OBJECTS.RES"); let ma = rs2::Archive::parse(&m).unwrap();
        ma.entries().iter().find(|e| e.name.eq_ignore_ascii_case(&slbname)).and_then(|e| slb::parse(ma.data(e)).ok())
    }).expect("model");
    print!("{{\"frames\":[");
    for (i, f) in model.frames.iter().enumerate() {
        print!("{}{{\"name\":{:?},\"parent\":{},\"m\":{:?}}}", if i > 0 { "," } else { "" }, f.name, f.parent.map(|p| p as i64).unwrap_or(-1), f.matrix);
    }
    print!("],\"motions\":{{");
    let mut first = true;
    for want in &args[3..] {
        let Some(info) = mcf.motions.iter().find(|m| m.name.eq_ignore_ascii_case(want)) else { eprintln!("no {want}"); continue };
        let mo = anim::Motion::parse(&get(&info.file).expect("mot")).unwrap();
        print!("{}{:?}:[", if first { "" } else { "," }, want); first = false;
        for (i, fr) in mo.frames.iter().enumerate() {
            print!("{}[", if i > 0 { "," } else { "" });
            for (j, k) in fr.iter().enumerate() {
                print!("{}[{},{:?},{:?},{:?}]", if j > 0 { "," } else { "" }, k.frame, k.look, k.up, k.position);
            }
            print!("]");
        }
        print!("]");
    }
    print!("}},\"info\":{{");
    let mut first = true;
    for want in &args[3..] {
        let Some(info) = mcf.motions.iter().find(|m| m.name.eq_ignore_ascii_case(want)) else { continue };
        print!("{}{:?}:{{\"fps\":{},\"looping\":{},\"frames\":{}}}", if first { "" } else { "," }, want, info.fps, info.looping, info.frame_count);
        first = false;
    }
    println!("}}}}");
}
