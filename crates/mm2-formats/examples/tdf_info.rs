//! Prints a track's course: nodes, sections, links, lines. Usage: tdf_info <track.env>
use mm2_formats::{faoe, rs2, tdf, Container};
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
    let raw = std::fs::read(&p).unwrap();
    let plain = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, &name).unwrap(), _ => raw };
    let ar = rs2::Archive::parse(&plain).unwrap();
    for e in ar.entries() {
        if e.name.to_ascii_lowercase().ends_with(".tdf") {
            let d = tdf::parse(ar.data(e)).unwrap();
            println!("{} nodes {} start {:?} finish {:?}", e.name, d.nodes.len(), d.start, d.finish);
            for (i, n) in d.nodes.iter().enumerate() {
                let s = &n.sections;
                println!(" node {i} flags {:#x} width {} sections {} links {:?} first {:?} last {:?}", n.flags, n.width, s.len(), n.links, s.first().map(|c| c.center), s.last().map(|c| c.center));
            }
            let all: Vec<[f32; 3]> = d.nodes.iter().flat_map(|n| n.sections.iter().flat_map(|s| s.edges)).collect();
            let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
            for p in &all { lo = [lo[0].min(p[0]), lo[1].min(p[2])]; hi = [hi[0].max(p[0]), hi[1].max(p[2])]; }
            println!(" box x {:.0}..{:.0} z {:.0}..{:.0}", lo[0], hi[0], lo[1], hi[1]);
            let raw = ar.data(e);
            println!(" first bytes {:?}", &raw[..raw.len().min(40)]);
        }
    }
}
