//! Prints the distinct leaf points of BIKE.RES .col files as Rust consts:
//! `shell_points <GAME dir> NAME=file.col ...`.
use mm2_formats::{col, faoe, rs2, Container};
use std::path::Path;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let raw = std::fs::read(Path::new(&a[1]).join("RES").join("BIKE.RES")).unwrap();
    let raw = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, "BIKE.RES").unwrap(), _ => raw };
    let ar = rs2::Archive::parse(&raw).unwrap();
    for arg in &a[2..] {
        let (name, file) = arg.split_once('=').unwrap();
        let e = ar.entries().iter().find(|e| e.name.eq_ignore_ascii_case(file)).unwrap();
        let c = col::parse(ar.data(e)).unwrap();
        let mut list: Vec<[f32; 3]> = Vec::new();
        for p in &c.parts {
            let m = &p.body.matrix;
            for n in &p.body.points {
                if let col::Node::Leaf(q) = n {
                    let v = [0, 1, 2].map(|k| m[k] * q[0] + m[3 + k] * q[1] + m[6 + k] * q[2] + m[9 + k]);
                    if !list.iter().any(|w| (0..3).map(|k| (w[k] - v[k]).powi(2)).sum::<f32>() < 1e-8) { list.push(v); }
                }
            }
        }
        println!("pub(crate) const {name}: [[f32; 3]; {}] = [", list.len());
        for v in &list { println!("    [{:?}, {:?}, {:?}],", v[0], v[1], v[2]); }
        println!("];\n");
    }
}
