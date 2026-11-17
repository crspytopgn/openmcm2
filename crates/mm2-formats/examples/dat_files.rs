//! Lists the files in a UI .DAT. Usage: dat_files <file.DAT>
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let raw = std::fs::read(&p).unwrap();
    let ar = mm2_formats::rs2::Archive::parse(&raw).unwrap();
    for e in ar.entries() { println!("{}", e.name); }
}
