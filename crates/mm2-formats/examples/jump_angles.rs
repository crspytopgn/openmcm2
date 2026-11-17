//! Jump face and landing angles along each track's course line, to
//! compare free tracks with the originals. Usage: jump_angles <track.env>...
//! Per jump: height, lip angle (steepest 4 ft of the takeoff), face angle
//! (rise over the face), landing angle (steepest 4 ft going down).
use mm2_formats::{faoe, rs2, tdf, trn, Container};

fn main() {
    let mut all: Vec<(String, Vec<[f32; 4]>)> = Vec::new();
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let raw = std::fs::read(&p).unwrap();
        let plain = match Container::detect(&raw) { Container::Faoe => match faoe::decode(&raw, &name) { Ok(p) => p, Err(_) => continue }, _ => raw };
        let Ok(ar) = rs2::Archive::parse(&plain) else { continue };
        let t = ar.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(".trn")).and_then(|e| trn::parse(ar.data(e)).ok());
        let Some(t) = t else { continue };
        let s = t.scale;
        let h = |x: f32, z: f32| {
            let (fx, fz) = ((x / s).clamp(0.0, (t.size - 2) as f32), (z / s).clamp(0.0, (t.size - 2) as f32));
            let (ix, iz) = (fx as usize, fz as usize);
            let (tx, tz) = (fx.fract(), fz.fract());
            let a = t.height(ix, iz) * (1.0 - tx) + t.height(ix + 1, iz) * tx;
            let b = t.height(ix, iz + 1) * (1.0 - tx) + t.height(ix + 1, iz + 1) * tx;
            (a * (1.0 - tz) + b * tz) * s
        };
        let mut jumps = Vec::new();
        for e in ar.entries() {
            if !e.name.to_ascii_lowercase().ends_with(".tdf") { continue }
            let d = tdf::parse(ar.data(e)).unwrap();
            for n in &d.nodes {
                let c: Vec<[f32; 3]> = n.sections.iter().map(|c| c.center).collect();
                if c.len() < 3 { continue }
                // Profile every 1 ft.
                let mut prof = Vec::new();
                for w in c.windows(2) {
                    let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt();
                    let k = d.ceil().max(1.0) as usize;
                    for j in 0..k { let f = j as f32 / k as f32; prof.push(h(w[0][0] + (w[1][0] - w[0][0]) * f, w[0][2] + (w[1][2] - w[0][2]) * f)); }
                }
                if let Some(dir) = std::env::var_os("DUMP") {
                    let txt: String = prof.iter().map(|v| format!("{v:.2}\n")).collect();
                    std::fs::write(std::path::Path::new(&dir).join(format!("{name}.prof")), txt).unwrap();
                }
                let deg = |rise: f32, run: f32| (rise / run).atan().to_degrees();
                let mut i = 0;
                while i + 100 < prof.len() {
                    let base = prof[i];
                    let (mut pj, mut peak) = (i..i + 40).map(|j| (j, prof[j])).fold((i, f32::MIN), |a, b| if b.1 > a.1 { b } else { a });
                    if peak - base > 3.0 && pj > i {
                        // Climb to the true top, and take the base as the lowest point in the 60 ft before it.
                        while pj + 1 < prof.len() && prof[pj + 1] >= prof[pj] { pj += 1; }
                        peak = prof[pj];
                        let base = prof[pj.saturating_sub(60)..pj].iter().copied().fold(f32::MAX, f32::min);
                        let after = prof[pj..(pj + 70).min(prof.len())].iter().copied().fold(f32::MAX, f32::min);
                        if peak - after > 3.0 {
                            // Face start: last point at base level before the peak.
                            let start = (pj.saturating_sub(60)..pj).rev().find(|&j| prof[j] <= base + 0.3).unwrap_or(i);
                            let lip = (start + 4..=pj).map(|j| deg(prof[j] - prof[j - 4], 4.0)).fold(0.0, f32::max);
                            let face = deg(peak - prof[start], (pj - start).max(1) as f32);
                            let end = (pj + 70).min(prof.len());
                            let land = (pj + 4..end).map(|j| deg(prof[j - 4] - prof[j], 4.0)).fold(0.0, f32::max);
                            jumps.push([peak - base, lip, face, land]);
                            i = pj + 10;
                            continue;
                        }
                    }
                    i += 1;
                }
            }
        }
        all.push((name, jumps));
    }
    let med = |v: &mut Vec<f32>, q: f32| { v.sort_by(|a, b| a.partial_cmp(b).unwrap()); if v.is_empty() { 0.0 } else { v[((v.len() - 1) as f32 * q).round() as usize] } };
    println!("{:10} {:>3} {:>6} {:>12} {:>12} {:>12}", "track", "n", "h med", "lip med/p90", "face med/p90", "land med/p90");
    if std::env::var_os("BINS").is_some() {
        let groups: Vec<(&str, Vec<[f32; 4]>)> = vec![
            ("orig", all.iter().filter(|(n, _)| !n.starts_with("FP")).flat_map(|(_, j)| j.clone()).collect()),
            ("free", all.iter().filter(|(n, _)| n.starts_with("FP")).flat_map(|(_, j)| j.clone()).collect()),
        ];
        for (g, j) in &groups {
            for (lo, hi) in [(3.0, 5.0), (5.0, 8.0), (8.0, 12.0), (12.0, 99.0)] {
                let sel: Vec<&[f32; 4]> = j.iter().filter(|x| x[0] >= lo && x[0] < hi).collect();
                let col = |k: usize| sel.iter().map(|x| x[k]).collect::<Vec<f32>>();
                let (mut b, mut c, mut d) = (col(1), col(2), col(3));
                println!("{g} h {lo:>2}-{hi:<2} n {:>4}  lip {:>3.0}/{:<3.0} face {:>3.0}/{:<3.0} land {:>3.0}/{:<3.0}", sel.len(), med(&mut b, 0.5), med(&mut b, 0.9), med(&mut c, 0.5), med(&mut c, 0.9), med(&mut d, 0.5), med(&mut d, 0.9));
            }
        }
        return;
    }
    for (name, j) in &all {
        let col = |k: usize| j.iter().map(|x| x[k]).collect::<Vec<f32>>();
        let (mut a, mut b, mut c, mut d) = (col(0), col(1), col(2), col(3));
        println!("{:10} {:>3} {:>6.1} {:>5.0}/{:<6.0} {:>5.0}/{:<6.0} {:>5.0}/{:<6.0}", name, j.len(), med(&mut a, 0.5), med(&mut b, 0.5), med(&mut b, 0.9), med(&mut c, 0.5), med(&mut c, 0.9), med(&mut d, 0.5), med(&mut d, 0.9));
    }
}
