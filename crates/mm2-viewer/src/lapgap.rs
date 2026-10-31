//! Live lap timing (the fork's `--lap-gap`, not the game's): how far
//! ahead of or behind their best lap a rider is, at each course point as
//! they reach it, and each lap's time and gap as it ends. Lap 1 runs from
//! the start gate rather than the line, so it is never compared or kept;
//! the best is kept through a restart (R).

/// How long a finished lap's time and gap stay up, s.
pub const LAST_SHOWN: f32 = 4.0;

/// Whether the launcher asked for it.
pub fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::args().any(|a| a == "--lap-gap"))
}

#[derive(Debug, Clone, Default)]
pub struct LapGap {
    /// The lap time at each course point reached this lap, by point.
    this: Vec<Option<f32>>,
    /// The best compared lap's time and its times at each point.
    best: Option<(f32, Vec<Option<f32>>)>,
    /// This lap began at the line, so it counts.
    timed: bool,
    /// The gap at the last point reached; less is ahead.
    pub gap: Option<f32>,
    /// The last lap's time, its gap to the best before it, and the race
    /// clock when it ended.
    pub last: Option<(f32, Option<f32>, f32)>,
}

/// What the line under the panel shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shown {
    /// The lap just ended: its time and gap.
    Lap(f32, Option<f32>),
    Gap(f32),
}

impl LapGap {
    /// The rider reached course point `point`, `time` into the lap; the
    /// first time this lap counts.
    pub fn mark(&mut self, point: usize, time: f32) {
        if !self.timed {
            return;
        }
        if self.this.len() <= point {
            self.this.resize(point + 1, None);
        }
        if self.this[point].is_some() {
            return;
        }
        self.this[point] = Some(time);
        if let Some(Some(best)) = self.best.as_ref().and_then(|(_, b)| b.get(point)) {
            self.gap = Some(time - best);
        }
    }

    /// A lap of `time` ended at race clock `clock`.
    pub fn lap(&mut self, time: f32, clock: f32) {
        let marks = std::mem::take(&mut self.this);
        if self.timed {
            let gap = self.best.as_ref().map(|(b, _)| time - b);
            self.last = Some((time, gap, clock));
            if self.best.as_ref().is_none_or(|(b, _)| time < *b) {
                self.best = Some((time, marks));
            }
        }
        self.timed = true;
        self.gap = None;
    }

    /// For a restart: the best lap only.
    pub fn restarted(&self) -> LapGap {
        LapGap {
            best: self.best.clone(),
            ..LapGap::default()
        }
    }

    /// What to show at race clock `clock`: a lap for a while after it
    /// ends, then the gap once there is one.
    pub fn shown(&self, clock: f32) -> Option<Shown> {
        match self.last {
            Some((time, gap, at)) if clock - at < LAST_SHOWN => Some(Shown::Lap(time, gap)),
            _ => self.gap.map(Shown::Gap),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lap_one_is_not_compared_and_the_best_is_kept() {
        let mut g = LapGap::default();
        // Lap 1, from the gate: nothing.
        g.mark(3, 5.0);
        g.lap(60.0, 60.0);
        assert_eq!(g.shown(60.0), None);
        // Lap 2 is the first best.
        g.mark(3, 10.0);
        g.mark(4, 20.0);
        assert_eq!(g.gap, None);
        g.lap(50.0, 110.0);
        assert_eq!(g.shown(111.0), Some(Shown::Lap(50.0, None)));
        // Lap 3: a second up at point 3, the first time there only.
        g.mark(3, 9.0);
        g.mark(3, 12.0);
        assert_eq!(g.shown(120.0), Some(Shown::Gap(-1.0)));
        g.mark(4, 21.5);
        assert_eq!(g.gap, Some(1.5));
        // Slower: the best stays lap 2's.
        g.lap(52.0, 162.0);
        assert_eq!(g.shown(163.0), Some(Shown::Lap(52.0, Some(2.0))));
        let mut r = g.restarted();
        assert_eq!(r.shown(0.0), None);
        // After a restart lap 1 again doesn't count, then lap 2 compares.
        r.lap(70.0, 70.0);
        r.mark(4, 19.0);
        assert_eq!(r.gap, Some(-1.0));
    }
}
