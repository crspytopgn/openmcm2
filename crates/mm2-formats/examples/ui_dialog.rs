use mm2_formats::{dtm, rs2};
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let raw = std::fs::read(&p).unwrap();
    let a = rs2::Archive::parse(&raw).unwrap();
    let e = a.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".dtm")).unwrap();
    let d = dtm::parse(a.data(e));
    println!("bg {:?} rect {:?}", d.background(), d.rect());
    for c in &d.controls { println!("{}\t{:?}\t{:?}\t{:?}", c.name, d.place(c), c.kind, c.props.0.iter().filter(|(k,_)| !k.starts_with("Text") && !k.starts_with("Font")).collect::<Vec<_>>()); }
    for a in &d.anims { println!("ANIM {} {:?}", a.name, a.frames.iter().take(2).collect::<Vec<_>>()); }
}
