//! A glb's frames and materials as the engine reads them. Usage: glb_info <file.glb>
fn main() {
    for p in std::env::args().skip(1) {
        let data = std::fs::read(&p).unwrap();
        const BIKE: [&str; 8] = ["Bike", "HandleBars", "KickStart", "NullFrame", "SwingArm", "Shocks", "TireFront", "TireRear"];
        const RIDER: [&str; 16] = ["Pelvis", "LwrTorso", "UprLegL", "UprLegR", "UprTorso", "Head", "UprArmL", "UprArmR", "LwrArmL", "HandL", "LwrArmR", "HandR", "LwrLegL", "FootL", "LwrLegR", "FootR"];
        let (m, images) = if p.ends_with(".slb") {
            (mm2_formats::slb::parse(&data).unwrap(), Vec::new())
        } else if p.contains("rider") {
            mm2_formats::glb::parse_framed(&data, "fp:x:", &RIDER).unwrap()
        } else if p.contains("bike") {
            mm2_formats::glb::parse_framed(&data, "fp:x:", &BIKE).unwrap()
        } else {
            mm2_formats::glb::parse(&data, "fp:x:").unwrap()
        };
        let world = m.world_matrices();
        println!("== {p}: {} frames, {} materials, {} images", m.frames.len(), m.materials.len(), images.len());
        for (i, f) in m.frames.iter().enumerate() {
            let t = &f.matrix;
            let w = &world[i];
            println!("  {i} {:<12} parent {:?} local {:.3} {:.3} {:.3}  x {:.2} {:.2} {:.2} y {:.2} {:.2} {:.2}  world {:.3} {:.3} {:.3}", f.name, f.parent, t[12], t[13], t[14], t[0], t[1], t[2], t[4], t[5], t[6], w[12], w[13], w[14]);
        }
        for (i, mat) in m.materials.iter().enumerate() { println!("  mat {i} {:?}", mat.texture); }
        for l in m.lods.iter().take(1) { for s in &l.surfaces { let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in &s.positions { for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); } }
            println!("  surface {} verts, materials {:?}, groups {:?} bounds {lo:.2?}..{hi:.2?}", s.positions.len(), s.materials, s.groups); } }
        for im in &images { println!("  image {} {}", im.name, im.data.len()); }
    }
}
