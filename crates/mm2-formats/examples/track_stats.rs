//! Course and terrain numbers for comparing tracks: course length, width,
//! turn radii, climbs, jumps along the line, terrain relief, textures,
//! plants and models. Usage: track_stats <track.env>...
use mm2_formats::{esb, faoe, rs2, scn, tdf, trn, Container};

fn main() {
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let raw = std::fs::read(&p).unwrap();
        let plain = match Container::detect(&raw) { Container::Faoe => faoe::decode(&raw, &name).unwrap(), _ => raw };
        let ar = rs2::Archive::parse(&plain).unwrap();
        let mut terrain = None;
        let (mut plants, mut models, mut species) = (0, 0, 0);
        for e in ar.entries() {
            let l = e.name.to_ascii_lowercase();
            if l.ends_with(".trn") { terrain = trn::parse(ar.data(e)).ok(); }
            if l.ends_with(".esb") { if let Ok(x) = esb::parse(ar.data(e)) { plants = x.objects.len(); species = x.species.len(); } }
            if l.ends_with(".scn") {
                let ini = scn::Ini::parse(&String::from_utf8_lossy(ar.data(e)));
                models = ini.section("StaticModels").and_then(|s| s.get("NumberOfStaticModels")).and_then(|n| n.trim().parse::<usize>().ok()).unwrap_or(0);
            }
        }
        let Some(t) = terrain else { continue };
        let s = t.scale;
        let h = |x: f32, z: f32| {
            let (fx, fz) = ((x / s).clamp(0.0, (t.size - 2) as f32), (z / s).clamp(0.0, (t.size - 2) as f32));
            let (ix, iz) = (fx as usize, fz as usize);
            let (tx, tz) = (fx.fract(), fz.fract());
            let a = t.height(ix, iz) * (1.0 - tx) + t.height(ix + 1, iz) * tx;
            let b = t.height(ix, iz + 1) * (1.0 - tx) + t.height(ix + 1, iz + 1) * tx;
            (a * (1.0 - tz) + b * tz) * s
        };
        let (mn, mx) = t.heights.iter().fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        let mut uniq = std::collections::HashSet::new();
        let mut sizes = std::collections::BTreeMap::new();
        for tile in &t.tiles { uniq.insert(tile.texture); *sizes.entry(tile.size).or_insert(0) += 1; }
        let tex_px: Vec<u32> = t.textures.iter().map(|x| x.size).collect();
        println!("== {name}: {} x {} ft, relief {:.0} ft, {} tiles {:?}, {} textures (px {:?}..{:?}), {} plants of {} species, {} models",
            ((t.size - 1) as f32 * s) as i32, ((t.size - 1) as f32 * s) as i32, (mx - mn) * s, t.tiles.len(), sizes, uniq.len(),
            tex_px.iter().min(), tex_px.iter().max(), plants, species, models);
        for e in ar.entries() {
            if !e.name.to_ascii_lowercase().ends_with(".tdf") { continue }
            let d = tdf::parse(ar.data(e)).unwrap();
            for n in &d.nodes {
                let c: Vec<[f32; 3]> = n.sections.iter().map(|c| c.center).collect();
                if c.len() < 3 { continue }
                let mut len = 0.0;
                let widths: Vec<f32> = n.sections.iter().map(|c| ((c.edges[0][0] - c.edges[1][0]).powi(2) + (c.edges[0][2] - c.edges[1][2]).powi(2)).sqrt()).collect();
                for w in c.windows(2) { len += ((w[1][0] - w[0][0]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt(); }
                // Profile every 2 ft along the line.
                let mut prof = Vec::new();
                for w in c.windows(2) {
                    let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt();
                    let k = (d / 2.0).ceil().max(1.0) as usize;
                    for j in 0..k { let f = j as f32 / k as f32; prof.push(h(w[0][0] + (w[1][0] - w[0][0]) * f, w[0][2] + (w[1][2] - w[0][2]) * f)); }
                }
                // Jumps: a rise of 3+ ft within 30 ft then a fall of 3+ ft within 60 ft.
                let mut jumps = Vec::new();
                let mut i = 0;
                while i + 45 < prof.len() {
                    let base = prof[i];
                    let (peak_j, peak) = (i..i + 15).map(|j| (j, prof[j])).fold((i, f32::MIN), |a, b| if b.1 > a.1 { b } else { a });
                    if peak - base > 3.0 && peak_j > i {
                        let after = prof[peak_j..(peak_j + 30).min(prof.len())].iter().copied().fold(f32::MAX, f32::min);
                        if peak - after > 3.0 { jumps.push((peak - base).max(peak - after)); i = peak_j + 10; continue; }
                    }
                    i += 1;
                }
                // Turns: heading change per 40 ft, the tightest radius.
                let mut tight = f32::MAX;
                let step = (40.0 / (len / c.len() as f32)).max(1.0) as usize;
                for k in step..c.len().saturating_sub(step) {
                    let a = (c[k][2] - c[k - step][2]).atan2(c[k][0] - c[k - step][0]);
                    let b = (c[k + step][2] - c[k][2]).atan2(c[k + step][0] - c[k][0]);
                    let mut dd = (b - a).abs(); if dd > std::f32::consts::PI { dd = std::f32::consts::TAU - dd; }
                    let seg = ((c[k + step][0] - c[k - step][0]).powi(2) + (c[k + step][2] - c[k - step][2]).powi(2)).sqrt() / 2.0;
                    if dd > 0.01 { tight = tight.min(seg / dd); }
                }
                let (pmn, pmx) = prof.iter().fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
                let climb: f32 = prof.windows(2).map(|w| (w[1] - w[0]).max(0.0)).sum();
                jumps.sort_by(|a, b| b.partial_cmp(a).unwrap());
                println!("  {} len {:.0} ft, width {:.0}..{:.0} (avg {:.0}), tightest {:.0} ft, height range {:.0} ft, climb {:.0} ft, {} jumps, biggest {:?}",
                    e.name, len, widths.iter().copied().fold(f32::MAX, f32::min), widths.iter().copied().fold(0.0, f32::max), widths.iter().sum::<f32>() / widths.len() as f32,
                    tight, pmx - pmn, climb, jumps.len(), jumps.iter().take(4).map(|j| (j * 10.0).round() / 10.0).collect::<Vec<_>>());
            }
        }
    }
}
