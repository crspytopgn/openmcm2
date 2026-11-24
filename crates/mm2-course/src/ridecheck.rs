//! The ride check: how a track actually rides, from the computer riders'
//! laps on it (the game's own physics and AI, taped frame by frame with
//! `mm2-viewer TRACK --screenshot x --ride 180 --riders 6 --ai-csv FILE`).
//!
//! Every jump is found from where the bikes leave the ground, and each
//! attempt is judged against the jump's landing as the ground lies: short
//! (cased onto the face or the deck), clean (down the landing), long
//! (over it onto the flat), or crashed. Crashes elsewhere are counted where
//! they happen, and each tight turn by how far out the riders take it (do
//! they use the berm?) and how much speed they keep. Fun is scored too, so
//! a fix never makes a track tamer than the originals: air time a lap, how
//! many jumps and big jumps, and how many are hard (cleared most of the
//! time, but not always).

use crate::path::{Grid, Path};

/// One taped frame of one rider.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub t: f32,
    pub bike: usize,
    pub pos: [f32; 3],
    pub speed: f32,
    pub airborne: bool,
    pub crashed: bool,
}

/// Reads the `--ai-csv` tape.
pub fn read_tape(text: &str) -> Vec<Frame> {
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or_default().split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (Some(t), Some(b), Some(x), Some(y), Some(z), Some(sp), Some(air), Some(cr)) =
        (col("t"), col("bike"), col("x"), col("y"), col("z"), col("speed"), col("airborne"), col("crashed"))
    else {
        return Vec::new();
    };
    lines
        .filter_map(|l| {
            let v: Vec<&str> = l.split(',').collect();
            let f = |i: usize| v.get(i)?.trim().parse::<f32>().ok();
            Some(Frame {
                t: f(t)?,
                bike: f(b)? as usize,
                pos: [f(x)?, f(y)?, f(z)?],
                speed: f(sp)?,
                airborne: f(air)? != 0.0,
                crashed: f(cr)? != 0.0,
            })
        })
        .collect()
}

/// How an attempt at a jump went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    Short,
    Clean,
    Long,
    Crashed,
}

/// One jump, from all its attempts.
#[derive(Debug, Clone, Default)]
pub struct Jump {
    /// Where the bikes leave the ground, ft along the line.
    pub at: f32,
    /// Where its landing slopes down, ft along: start and end.
    pub landing: Option<(f32, f32)>,
    pub tries: usize,
    pub short: usize,
    pub clean: usize,
    pub long: usize,
    pub crashed: usize,
    /// Means over the attempts: take-off speed (ft/s), air time (s),
    /// distance flown (ft).
    pub speed: f32,
    pub air: f32,
    pub flown: f32,
    /// How far, ft, the short ones fell short of the landing and the long
    /// ones flew past it, on average.
    pub short_by: f32,
    pub long_by: f32,
}

impl Jump {
    /// The share of attempts that went cleanly.
    pub fn clean_share(&self) -> f32 {
        self.clean as f32 / self.tries.max(1) as f32
    }

    /// Whether riders crash on it often enough to show a player: a third
    /// of them or more (flying long or landing short without crashing is
    /// everyday riding in this game, the original tracks are full of it).
    pub fn worrying(&self) -> bool {
        self.tries >= 3 && self.crashed * 3 >= self.tries
    }

    /// What's wrong with it, if anything, as a fixer would act on it.
    pub fn trouble(&self) -> Option<Trouble> {
        if self.tries < 3 {
            return None;
        }
        let n = self.tries as f32;
        let (short, long, crashed) = (self.short as f32 / n, self.long as f32 / n, self.crashed as f32 / n);
        if crashed > 0.45 {
            Some(Trouble::Crashes)
        } else if short > 0.5 {
            Some(Trouble::Short)
        } else if long > 0.5 {
            Some(Trouble::Long)
        } else {
            None
        }
    }
}

/// A jump or a spot that needs fixing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// Most riders come up short: lengthen the run-up (more speed) or
    /// bring the landing nearer, before ever making it smaller.
    Short,
    /// Most fly past the landing: lengthen the landing.
    Long,
    /// Most crash here.
    Crashes,
}

/// A tight turn and how it's ridden.
#[derive(Debug, Clone, Default)]
pub struct Turn {
    pub at: f32,
    pub radius: f32,
    /// How far towards the outside the riders go at the apex, in half
    /// track widths (1 is the edge: riding the berm).
    pub out: f32,
    /// The slowest they go through it, ft/s, on average.
    pub slowest: f32,
    pub crashes: usize,
}

/// Fun and danger in one lap's terms.
#[derive(Debug, Clone, Default)]
pub struct Score {
    /// Rider-laps the tape covers.
    pub laps: f32,
    /// Seconds in the air a lap.
    pub air: f32,
    /// Jumps a lap with half a second or more of air, and big ones with
    /// 1.2 s or more.
    pub jumps: f32,
    pub big: f32,
    /// Distinct jumps that are hard: cleared 40 to 85 % of the time.
    pub hard: usize,
    /// Crashes a lap.
    pub crashes: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub jumps: Vec<Jump>,
    pub turns: Vec<Turn>,
    /// Crashes away from the jumps, ft along, with how many.
    pub crash_spots: Vec<(f32, usize)>,
    pub score: Score,
}

/// Checks the ride on a track with line `path` and ground `height`, with
/// a track `half` ft each side, from the tape's `frames`.
pub fn check(path: &Path, half: f32, height: &dyn Fn(f32, f32) -> f32, frames: &[Frame]) -> Report {
    let grid = Grid::new(path, 70.0);
    let len = path.length.max(1.0);
    let along = |p: [f32; 3]| grid.nearest([p[0], p[2]]).map(|(n, _)| (n.s, n.side));
    let ahead = |a: f32, b: f32| path.ahead(a, b);
    // The ground along the middle of the line, every STEP ft.
    let step = crate::path::STEP;
    let profile: Vec<f32> = path.pts.iter().map(|p| height(p[0], p[1])).collect();
    let at_s = |s: f32| profile[path.index_at(s.rem_euclid(len)).min(profile.len() - 1)];
    // The landing after a take-off at `s`: the first stretch past it that
    // slopes down (the landing ramp) from near the lip's height, within
    // 260 ft; the back of the take-off itself (a double's) is passed over.
    let landing_after = |s0: f32| -> Option<(f32, f32)> {
        let lip = (0..4).map(|k| at_s(s0 + k as f32 * step)).fold(f32::MIN, f32::max);
        let mut start = None;
        let mut d = 14.0;
        while d < 260.0 {
            let s = s0 + d;
            let slope = (at_s(s + step) - at_s(s - step)) / (2.0 * step);
            match start {
                None if slope < -0.12 && at_s(s) > lip - 4.0 => start = Some(d),
                Some(a) if slope > -0.06 => return Some((s0 + a, s0 + d)),
                _ => {}
            }
            d += step;
        }
        start.map(|a| (s0 + a, s0 + d))
    };
    let mut bikes: Vec<usize> = frames.iter().map(|f| f.bike).collect();
    bikes.sort_unstable();
    bikes.dedup();
    struct Try {
        at: f32,
        speed: f32,
        air: f32,
        flown: f32,
        land: f32,
        crashed: bool,
    }
    let mut tries: Vec<Try> = Vec::new();
    let mut crashes: Vec<f32> = Vec::new();
    let mut distance = 0.0f32;
    let mut apex: Vec<(f32, f32, f32)> = Vec::new(); // (s, out side, speed)
    for &b in &bikes {
        let fr: Vec<&Frame> = frames.iter().filter(|f| f.bike == b).collect();
        let mut prev_s: Option<f32> = None;
        let mut up: Option<(usize, f32, f32)> = None; // frame, s, speed
        let mut was_crashed = false;
        for (k, f) in fr.iter().enumerate() {
            let Some((s, side)) = along(f.pos) else { continue };
            if let Some(p) = prev_s {
                let d = ahead(p, s);
                if d < 60.0 {
                    distance += d;
                }
            }
            prev_s = Some(s);
            apex.push((s, side, f.speed));
            if f.crashed && !was_crashed {
                crashes.push(s);
            }
            was_crashed = f.crashed;
            match (f.airborne, up) {
                (true, None) if !f.crashed => {
                    let prev = if k > 0 { fr[k - 1] } else { f };
                    let (s0, v0) = along(prev.pos).map_or((s, f.speed), |(s0, _)| (s0, prev.speed));
                    up = Some((k, s0, v0));
                }
                (false, Some((k0, s0, v0))) => {
                    let air = f.t - fr[k0].t;
                    let flown = ahead(s0, s);
                    if air >= 0.25 && flown < 300.0 {
                        // Crashed in the air or within a second of landing.
                        let crashed = fr[k0..].iter().take_while(|g| g.t <= f.t + 1.0).any(|g| g.crashed);
                        tries.push(Try { at: s0, speed: v0, air, flown, land: s, crashed });
                    }
                    up = None;
                }
                _ => {}
            }
        }
    }

    // Jumps: take-offs within 25 ft of each other are one jump.
    let mut order: Vec<usize> = (0..tries.len()).collect();
    order.sort_by(|&a, &b| tries[a].at.total_cmp(&tries[b].at));
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for i in order {
        match groups.last_mut() {
            Some(g) if ahead(tries[*g.last().unwrap()].at, tries[i].at) < 25.0 => g.push(i),
            _ => groups.push(vec![i]),
        }
    }
    // Round the loop: the last group may be the first's.
    if path.closed && groups.len() > 1 {
        let (first, last) = (groups[0][0], *groups.last().unwrap().last().unwrap());
        if ahead(tries[last].at, tries[first].at) < 25.0 {
            let tail = groups.pop().unwrap();
            groups[0].splice(0..0, tail);
        }
    }
    let mut jumps: Vec<Jump> = groups
        .iter()
        .map(|g| {
            let at = tries[g[g.len() / 2]].at;
            let landing = landing_after(at);
            let mut j = Jump { at, landing, tries: g.len(), ..Default::default() };
            for &i in g {
                let t = &tries[i];
                j.speed += t.speed;
                j.air += t.air;
                j.flown += t.flown;
                let lands = ahead(at, t.land);
                let outcome = if t.crashed {
                    Landing::Crashed
                } else {
                    match landing {
                        Some((a, _)) if lands < ahead(at, a) - 4.0 => Landing::Short,
                        Some((_, b)) if lands > ahead(at, b) + 10.0 => Landing::Long,
                        _ => Landing::Clean,
                    }
                };
                match outcome {
                    Landing::Short => {
                        j.short += 1;
                        j.short_by += ahead(at, landing.map_or(at, |l| l.0)) - lands;
                    }
                    Landing::Clean => j.clean += 1,
                    Landing::Long => {
                        j.long += 1;
                        j.long_by += lands - ahead(at, landing.map_or(at, |l| l.1));
                    }
                    Landing::Crashed => j.crashed += 1,
                }
            }
            let n = g.len() as f32;
            j.speed /= n;
            j.air /= n;
            j.flown /= n;
            j.short_by /= j.short.max(1) as f32;
            j.long_by /= j.long.max(1) as f32;
            j
        })
        .collect();
    jumps.sort_by(|a, b| a.at.total_cmp(&b.at));

    // Crashes away from the jumps, in 40 ft spots.
    let near_jump = |s: f32| jumps.iter().any(|j| { let d = ahead(j.at, s); d < 200.0 && j.landing.is_none_or(|(_, b)| d < ahead(j.at, b) + 30.0) });
    let mut spots: Vec<(f32, usize)> = Vec::new();
    let mut lone: Vec<f32> = crashes.iter().copied().filter(|&s| !near_jump(s)).collect();
    lone.sort_by(f32::total_cmp);
    for s in lone {
        match spots.last_mut() {
            Some((at, n)) if ahead(*at, s) < 40.0 => *n += 1,
            _ => spots.push((s, 1)),
        }
    }

    // Tight turns: the apex of each stretch turning tighter than 90 ft.
    let turn = path.smooth(&path.turn, 20.0);
    let mut turns = Vec::new();
    let m = turn.len();
    let mut i = 0;
    while i < m {
        if turn[i].abs() > 1.0 / 90.0 {
            let mut j = i;
            let mut best = i;
            while j < m && turn[j].abs() > 1.0 / 90.0 && turn[j].signum() == turn[i].signum() {
                if turn[j].abs() > turn[best].abs() {
                    best = j;
                }
                j += 1;
            }
            let s = path.s[best];
            let sign = -turn[best].signum();
            let near: Vec<&(f32, f32, f32)> = apex.iter().filter(|a| { let d = ahead(s - 15.0, a.0); d < 30.0 }).collect();
            if !near.is_empty() {
                let out = near.iter().map(|a| a.1 * sign).sum::<f32>() / near.len() as f32 / half;
                let slowest = near.iter().map(|a| a.2).sum::<f32>() / near.len() as f32;
                let crashes = crashes.iter().filter(|&&c| { let d = ahead(s - 40.0, c); d < 80.0 }).count();
                turns.push(Turn { at: s, radius: 1.0 / turn[best].abs(), out, slowest, crashes });
            }
            i = j;
        } else {
            i += 1;
        }
    }

    let laps = (distance / len).max(0.1);
    let score = Score {
        laps,
        air: tries.iter().map(|t| t.air).sum::<f32>() / laps,
        jumps: tries.iter().filter(|t| t.air >= 0.5).count() as f32 / laps,
        big: tries.iter().filter(|t| t.air >= 1.2).count() as f32 / laps,
        hard: jumps.iter().filter(|j| j.tries >= 4 && (0.4..0.85).contains(&j.clean_share())).count(),
        crashes: crashes.len() as f32 / laps,
    };
    Report { jumps, turns, crash_spots: spots, score }
}

impl Report {
    /// A short text report.
    pub fn text(&self) -> String {
        use std::fmt::Write;
        let s = &self.score;
        let mut out = String::new();
        let _ = writeln!(out, "laps {:.1}  air {:.1} s/lap  jumps {:.1}/lap  big {:.1}/lap  hard jumps {}  crashes {:.2}/lap", s.laps, s.air, s.jumps, s.big, s.hard, s.crashes);
        for j in &self.jumps {
            let land = j.landing.map_or("no landing".to_string(), |(a, b)| format!("landing {a:.0}-{b:.0}"));
            let _ = writeln!(
                out,
                "jump {:6.0} ft  {land:18} tries {:2}  clean {:2} short {:2} long {:2} crash {:2}  {:3.0} mph  air {:.2} s  {:3.0} ft{}",
                j.at,
                j.tries,
                j.clean,
                j.short,
                j.long,
                j.crashed,
                j.speed * 0.6818,
                j.air,
                j.flown,
                j.trouble().map_or(String::new(), |t| format!("  <- {t:?}"))
            );
        }
        for t in &self.turns {
            let _ = writeln!(out, "turn {:6.0} ft  radius {:4.0}  out {:5.2}  slowest {:3.0} mph  crashes {}", t.at, t.radius, t.out, t.slowest * 0.6818, t.crashes);
        }
        for (s, n) in &self.crash_spots {
            if *n >= 2 {
                let _ = writeln!(out, "crashes {n} at {s:.0} ft");
            }
        }
        out
    }
}

/// The ground along the middle of the line from `a` to `b` ft, every 4 ft
/// (for looking at a jump by eye).
pub fn profile(path: &Path, height: &dyn Fn(f32, f32) -> f32, a: f32, b: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut s = a;
    while s <= b {
        let p = path.at(s.rem_euclid(path.length.max(1.0)));
        out.push((s, height(p[0], p[1])));
        s += 4.0;
    }
    out
}
