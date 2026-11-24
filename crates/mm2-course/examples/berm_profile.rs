//! Cross-sections of a track's sharpest turns, outside to the right:
//! how high the ground stands across the track (ft above the line's
//! middle), to compare berms with the original tracks'.
//! Usage: berm_profile <TRACK.ENV | Mode name | file.course> [turns]
use mm2_course::{bake, copy, Course, Mode};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let what = &args[0];
    let turns: usize = args.get(1).and_then(|t| t.parse().ok()).unwrap_or(6);
    let c = if what.to_ascii_uppercase().ends_with(".ENV") {
        copy::import(&std::fs::read(what).unwrap(), what, None, 0, "x").unwrap()
    } else if what.ends_with(".course") {
        Course::from_json(&std::fs::read_to_string(what).unwrap()).unwrap()
    } else {
        let m = *Mode::ALL.iter().find(|m| m.name().eq_ignore_ascii_case(what)).unwrap();
        let mut c = Course::new(m);
        if let Some(b) = args.get(2) {
            c.line.berms = b.parse().unwrap();
        }
        c
    };
    let p = c.path();
    let height: Box<dyn Fn(f32, f32) -> f32> = match &c.source {
        Some(src) => {
            let src = src.clone();
            Box::new(move |x, z| src.height(x, z))
        }
        None => {
            let f = bake::bake(&c, 1, &bake::land(&c, 1));
            Box::new(move |x, z| f.height(x, z))
        }
    };
    // Sharpest turns, at least 150 ft apart.
    let mut idx: Vec<usize> = (0..p.turn.len()).collect();
    idx.sort_by(|&a, &b| p.turn[b].abs().partial_cmp(&p.turn[a].abs()).unwrap());
    let mut picked: Vec<usize> = Vec::new();
    for i in idx {
        if picked.iter().all(|&j| (p.s[i] - p.s[j]).abs() > 150.0) {
            picked.push(i);
        }
        if picked.len() >= turns {
            break;
        }
    }
    let hw = c.mode.track_width() / 2.0;
    let offs: Vec<f32> = (-8..=12).map(|k| k as f32 * hw / 6.0).collect();
    print!("{:>8} {:>6} |", "s", "radius");
    for o in &offs {
        print!("{:>6.0}", o);
    }
    println!();
    for &i in &picked {
        let side = p.side(i);
        let out = -p.turn[i].signum();
        let at = p.pts[i];
        let h0 = height(at[0], at[1]);
        if std::env::var_os("DBG").is_some() {
            for k in -3..=3 { let q = [at[0] + p.dir[i][0] * k as f32 * 1.5, at[1] + p.dir[i][1] * k as f32 * 1.5]; eprint!(" {:.1}", height(q[0], q[1])); }
            eprintln!(" at {:?}", at);
        }
        if std::env::var_os("CAM").is_some() {
            eprintln!("turn s={:.0} at {:.1},{:.1} h {:.1} out {:.3},{:.3} dir {:.3},{:.3}", p.s[i], at[0], at[1], h0, side[0] * out, side[1] * out, p.dir[i][0], p.dir[i][1]);
        }
        print!("{:>8.0} {:>6.0} |", p.s[i], 1.0 / p.turn[i].abs());
        for o in &offs {
            let d = o * out;
            print!("{:>6.1}", height(at[0] + side[0] * d, at[1] + side[1] * d) - h0);
        }
        println!();
    }
}
#[allow(dead_code)]
fn unused() {}
