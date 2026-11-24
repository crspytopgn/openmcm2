//! The riders go round Course Builder tracks: a new track of each kind
//! and a few made up at random.

use mm2_course::{auto, env, Course, Mode};
use mm2_preview::{Sim, POSE};

fn ride(name: &str, course: &Course, sx: bool) -> (u32, usize) {
    let bytes = env::write(course).env;
    let mut sim = Sim::new(bytes, 6, sx).unwrap_or_else(|_| panic!("{name}: no sim"));
    let mut crashes = 0;
    let mut was = vec![false; 6];
    for _ in 0..(180 * 30) {
        sim.advance(1.0 / 30.0);
        let p = sim.poses();
        for i in 0..6 {
            let down = p[i * POSE + 15] > 0.5;
            crashes += (down && !was[i]) as usize;
            was[i] = down;
            assert!(
                p[i * POSE..i * POSE + 3].iter().all(|v| v.is_finite()),
                "{name}: lost a rider"
            );
        }
    }
    let laps = sim.laps();
    let most = (0..6).map(|i| laps[i * 3] as u32).max().unwrap();
    let best = (0..6)
        .map(|i| laps[i * 3 + 2])
        .filter(|&t| t > 0.0)
        .fold(f32::MAX, f32::min);
    eprintln!("{name}: most laps {most}, best lap {best:.1} s, crashes {crashes}");
    (most, crashes)
}

#[test]
fn riders_lap_new_tracks() {
    for (name, mode, sx) in [
        ("national", Mode::National, false),
        ("supercross", Mode::Supercross, true),
    ] {
        let (laps, _) = ride(name, &Course::new(mode), sx);
        assert!(laps >= 1, "{name}: nobody finished a lap in three minutes");
    }
    for seed in 1..4 {
        ride(
            &format!("surprise national {seed}"),
            &auto::surprise(Mode::National, seed),
            false,
        );
    }
}
