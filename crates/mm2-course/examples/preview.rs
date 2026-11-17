//! A shaded top-down picture of a new course of each mode (or of a
//! `.course` file), for checking the land and track by eye.
//! Usage: preview <out-dir> [file.course]
use mm2_course::{bake, env, Course, Mode};
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = std::path::Path::new(&args[0]);
    let courses: Vec<Course> = match args.get(1) {
        Some(f) => vec![Course::from_json(&std::fs::read_to_string(f).unwrap()).unwrap()],
        None => Mode::ALL.iter().map(|&m| Course::new(m)).collect(),
    };
    for c in courses {
        let every = 2;
        let f = bake::bake(&c, every, &bake::land(&c, every));
        let n = f.n;
        let mut img = Vec::with_capacity(n * n * 3);
        for z in (0..n).rev() {
            for x in 0..n {
                let h = |x: usize, z: usize| f.heights[z.min(n - 1) * n + x.min(n - 1)];
                let (dx, dz) = (
                    h(x + 1, z) - h(x.saturating_sub(1), z),
                    h(x, z + 1) - h(x, z.saturating_sub(1)),
                );
                let light = (0.75 - (dx * 0.6 + dz * 0.4) / (2.0 * f.step) * 1.5).clamp(0.2, 1.3);
                let col = env::colour(mm2_course::Surface::from_zone(f.surfaces[z * n + x]));
                for c in col {
                    img.push(((c * light).clamp(0.0, 1.0) * 255.0) as u8);
                }
            }
        }
        let path = out.join(format!("{}.ppm", c.mode.name()));
        let mut file = std::fs::File::create(&path).unwrap();
        write!(file, "P6\n{n} {n}\n255\n").unwrap();
        file.write_all(&img).unwrap();
        let lo = f.heights.iter().copied().fold(f32::MAX, f32::min);
        let hi = f.heights.iter().copied().fold(f32::MIN, f32::max);
        println!("{}: {lo:.0} to {hi:.0} ft", path.display());
    }
}
