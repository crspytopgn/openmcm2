use mm2_formats::{dtm, rs2};
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let raw = std::fs::read(&p).unwrap();
    let a = rs2::Archive::parse(&raw).unwrap();
    let e = a.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".dtm")).unwrap();
    let d = dtm::parse(a.data(e));
    println!("bg {:?} rect {:?}", d.background(), d.rect());
    for c in &d.controls { println!("{}\t{:?}\t{:?}", c.name, d.place(c), c.kind); }
}
