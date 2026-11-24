//! The riders keep to the course the builder drew.

use mm2_course::{auto, env, Mode};
use mm2_preview::{Sim, POSE};

#[test]
fn riders_stay_near_the_line() {
    let c = auto::surprise(Mode::National, 7);
    let path = c.path();
    let pts: Vec<[f32; 2]> = path
        .pts
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<f32>>()
        .chunks(2)
        .map(|p| [p[0], p[1]])
        .collect();
    let mut sim = Sim::new(env::write(&c).env, 6, false).unwrap();
    let mut far = Vec::new();
    for f in 0..(90 * 30) {
        sim.advance(1.0 / 30.0);
        if f % 150 == 0 {
            let p = sim.poses();
            let d: Vec<i32> = (0..6)
                .map(|i| {
                    let (x, z) = (p[i * POSE], p[i * POSE + 2]);
                    pts.iter()
                        .map(|q| ((q[0] - x).powi(2) + (q[1] - z).powi(2)).sqrt())
                        .fold(f32::MAX, f32::min) as i32
                })
                .collect();
            far.push(d);
        }
    }
    let all: Vec<i32> = far.into_iter().flatten().collect();
    let near = all.iter().filter(|&&d| d <= 15).count();
    assert!(
        near * 10 >= all.len() * 8,
        "riders strayed from the line: {all:?}"
    );
}
