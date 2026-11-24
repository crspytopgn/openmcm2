//! Writes a "Surprise me" course: surprise <mode> <seed> <out.course>
use mm2_course::{auto, Mode};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let m = *Mode::ALL.iter().find(|m| m.name().eq_ignore_ascii_case(&a[0])).unwrap();
    std::fs::write(&a[2], auto::surprise(m, a[1].parse().unwrap()).to_json()).unwrap();
}
