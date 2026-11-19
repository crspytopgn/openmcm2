//! Maps of the free nationals' and supercross tracks' layouts, with
//! their turns counted: how many, how sharp, and how many are square.
//! Usage: free_layouts <out-dir>
use mm2_course::free;
use std::io::Write;

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("out dir"));
    std::fs::create_dir_all(&out).unwrap();
    if let Some(game) = std::env::args().nth(2) {
        originals(std::path::Path::new(&game), &out);
        return;
    }
    let mut csv = std::fs::File::create(out.join("turns.csv")).unwrap();
    writeln!(csv, "track,angle,radius").unwrap();
    for def in free::SET {
        let Some(points) = free::layout(def) else { continue };
        let turns = free::turns(&points);
        for (a, r) in &turns {
            writeln!(csv, "{},{a:.1},{r:.1}", def.file).unwrap();
        }
        let square = turns.iter().filter(|t| (75.0..=105.0).contains(&t.0.abs())).count();
        let hair = turns.iter().filter(|t| t.0.abs() > 150.0).count();
        let sweep = turns.iter().filter(|t| t.0.abs() < 75.0).count();
        let radii: Vec<i32> = turns.iter().map(|t| t.1 as i32).collect();
        let runs = free::straight_lengths(&points);
        let jumpable = runs.iter().filter(|&&r| r >= 150.0).count();
        let longest = runs.iter().copied().fold(0.0f32, f32::max);
        println!("{} straights>=150ft {jumpable} longest {longest:.0} total {:.0}", def.file, runs.iter().filter(|&&r| r >= 100.0).sum::<f32>());
        println!("{} {:<20} turns {:2}  square(75-105) {:2}  hairpin(>150) {:2}  sweeper(<75) {:2}  radii {:?}", def.file, def.name, turns.len(), square, hair, sweep, radii);
        let mut f = std::fs::File::create(out.join(format!("{}.xy", def.file))).unwrap();
        for p in &points {
            writeln!(f, "{} {}", p[0], p[1]).unwrap();
        }
    }
}

/// The originals' lap tracks, for comparison: `free_layouts <out> <GAME>`.
#[allow(dead_code)]
pub fn originals(game: &std::path::Path, out: &std::path::Path) {
    use mm2_formats::{faoe, rs2, Container};
    let mut csv = std::fs::File::create(out.join("turns.csv")).unwrap();
    writeln!(csv, "track,angle,radius").unwrap();
    for kind in ["NATIONAL", "SX"] {
        let mut names: Vec<_> = std::fs::read_dir(game.join("TERAFORM").join(kind)).unwrap().flatten().map(|e| e.path()).collect();
        names.sort();
        for path in names {
            let file = path.file_name().unwrap().to_string_lossy().to_string();
            if !file.to_ascii_uppercase().ends_with(".ENV") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            let plain = match Container::detect(&bytes) {
                Container::Faoe => faoe::decode(&bytes, &file).unwrap(),
                _ => bytes.clone(),
            };
            let Ok(ar) = rs2::Archive::parse(&plain) else { continue };
            let Some(e) = ar.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".tdf")) else { continue };
            let Ok(def) = mm2_formats::tdf::parse(ar.data(e)) else { continue };
            let points: Vec<[f32; 2]> = def.nodes.iter().flat_map(|n| &n.sections).map(|c| [c.center[0], c.center[2]]).collect();
            if points.len() < 8 {
                continue;
            }
            let stem = format!("{}{}", if kind == "SX" { "SX_" } else { "NAT_" }, &file[..file.len() - 4]);
            let turns = free::turns(&points);
            for (a, r) in &turns {
                writeln!(csv, "{stem},{a:.1},{r:.1}").unwrap();
            }
            let square = turns.iter().filter(|t| (75.0..=105.0).contains(&t.0.abs())).count();
            let hair = turns.iter().filter(|t| t.0.abs() > 150.0).count();
            let sweep = turns.iter().filter(|t| t.0.abs() < 75.0).count();
            let radii: Vec<i32> = turns.iter().map(|t| t.1 as i32).collect();
            println!("{stem:<16} turns {:2}  square {:2}  hairpin {:2}  sweeper {:2}  radii {:?}", turns.len(), square, hair, sweep, radii);
            let mut f = std::fs::File::create(out.join(format!("{stem}.xy"))).unwrap();
            for p in &points {
                writeln!(f, "{} {}", p[0], p[1]).unwrap();
            }
        }
    }
}
