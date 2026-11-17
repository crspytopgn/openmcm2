//! The jump kit: shapes that sit on the line, each a height profile along
//! the track from where it begins to where it ends.

use crate::{Jump, Mode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Tabletop,
    Double,
    Triple,
    StepUp,
    StepDown,
    Kicker,
    Roller,
    Whoops,
    Rhythm,
    /// A dip or wash across the track.
    Dip,
}

/// How one stretch of a profile curves from its height before to its own.
#[derive(Debug, Clone, Copy)]
enum Run {
    /// A take-off face: shallow at the bottom, steepest at the lip.
    Face,
    /// An S-curve: a landing, or the back of a take-off.
    Ease,
    Flat,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Tabletop,
        Kind::Double,
        Kind::Triple,
        Kind::StepUp,
        Kind::StepDown,
        Kind::Kicker,
        Kind::Roller,
        Kind::Whoops,
        Kind::Rhythm,
        Kind::Dip,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Tabletop => "Tabletop",
            Kind::Double => "Double",
            Kind::Triple => "Triple",
            Kind::StepUp => "Step-up",
            Kind::StepDown => "Step-down",
            Kind::Kicker => "Kicker",
            Kind::Roller => "Roller",
            Kind::Whoops => "Whoops",
            Kind::Rhythm => "Rhythm",
            Kind::Dip => "Dip",
        }
    }

    /// What it is, for its tooltip.
    pub fn about(self) -> &'static str {
        match self {
            Kind::Tabletop => {
                "A take-off face, a flat top and a landing. Safe to come up short on."
            }
            Kind::Double => "Two humps with a gap between: clear it or case the landing.",
            Kind::Triple => "Three humps; the big jump of a Supercross track.",
            Kind::StepUp => "Jump up onto a higher deck, then roll down off its end.",
            Kind::StepDown => "Jump off a high deck down onto a long landing.",
            Kind::Kicker => "A short, steep lip that throws the bike up.",
            Kind::Roller => "One smooth, round bump to pump or jump.",
            Kind::Whoops => "A row of small, tight bumps to skim across at speed.",
            Kind::Rhythm => "A row of mid-sized bumps to double and triple through.",
            Kind::Dip => "A dip or wash cut across the track.",
        }
    }

    /// A new one of this kind at `at` ft along the line, sized for the mode.
    pub fn make(self, mode: Mode, at: f32) -> Jump {
        let (length, height) = match self {
            Kind::Tabletop => (60.0, 7.0),
            Kind::Double => (70.0, 7.5),
            Kind::Triple => (100.0, 8.0),
            Kind::StepUp => (65.0, 7.0),
            Kind::StepDown => (70.0, 7.0),
            Kind::Kicker => (24.0, 4.5),
            Kind::Roller => (32.0, 3.5),
            Kind::Whoops => (110.0, 2.5),
            Kind::Rhythm => (120.0, 4.5),
            Kind::Dip => (40.0, 4.0),
        };
        let (l, h) = match mode {
            Mode::Supercross => (1.0, 1.0),
            Mode::National => (1.45, 1.2),
            Mode::Enduro => (1.2, 0.9),
            Mode::Baja => (1.7, 1.0),
        };
        Jump {
            kind: self,
            at,
            length: length * l,
            height: height * h,
        }
    }

    /// The stretches of the profile: where each ends (0 to 1 along the
    /// jump), its height there (times the jump's height) and its curve.
    fn runs(self) -> &'static [(f32, f32, Run)] {
        use Run::*;
        match self {
            Kind::Tabletop => &[(0.32, 1.0, Face), (0.68, 1.0, Flat), (1.0, 0.0, Ease)],
            Kind::Double => &[
                (0.26, 1.0, Face),
                (0.34, 0.0, Ease),
                (0.6, 0.0, Flat),
                (0.68, 0.9, Ease),
                (1.0, 0.0, Ease),
            ],
            Kind::Triple => &[
                (0.2, 1.0, Face),
                (0.26, 0.0, Ease),
                (0.38, 0.0, Flat),
                (0.45, 0.75, Ease),
                (0.52, 0.0, Ease),
                (0.66, 0.0, Flat),
                (0.73, 0.9, Ease),
                (1.0, 0.0, Ease),
            ],
            Kind::StepUp => &[
                (0.3, 0.7, Face),
                (0.36, 0.35, Ease),
                (0.44, 1.0, Ease),
                (0.8, 1.0, Flat),
                (1.0, 0.0, Ease),
            ],
            Kind::StepDown => &[
                (0.2, 1.0, Ease),
                (0.42, 1.0, Flat),
                (0.47, 1.05, Face),
                (1.0, 0.0, Ease),
            ],
            Kind::Kicker => &[(0.72, 1.0, Face), (1.0, 0.0, Ease)],
            Kind::Roller | Kind::Whoops | Kind::Rhythm | Kind::Dip => &[],
        }
    }

    /// The height at `u` (0 to 1 along a jump `length` ft long), times
    /// the jump's height.
    pub fn profile(self, u: f32, length: f32) -> f32 {
        if !(0.0..=1.0).contains(&u) {
            return 0.0;
        }
        let bumps = |n: f32, sharp: f32| {
            let n = n.round().max(2.0);
            let v = (std::f32::consts::PI * u * n).sin().abs();
            v.powf(sharp)
        };
        match self {
            Kind::Roller => (std::f32::consts::PI * u).sin().powi(2),
            Kind::Whoops => bumps(length / 14.0, 1.4),
            Kind::Rhythm => bumps(length / 28.0, 1.0),
            Kind::Dip => -(std::f32::consts::PI * u).sin().powi(2),
            _ => {
                let (mut u0, mut h0) = (0.0, 0.0);
                for &(u1, h1, run) in self.runs() {
                    if u <= u1 {
                        let t = ((u - u0) / (u1 - u0).max(1e-4)).clamp(0.0, 1.0);
                        let k = match run {
                            Run::Face => t.powf(1.7),
                            Run::Ease => t * t * (3.0 - 2.0 * t),
                            Run::Flat => 0.0,
                        };
                        return match run {
                            Run::Flat => h0,
                            _ => h0 + (h1 - h0) * k,
                        };
                    }
                    (u0, h0) = (u1, h1);
                }
                0.0
            }
        }
    }
}

impl Jump {
    /// Its height `ds` ft from where it begins.
    pub fn height_at(&self, ds: f32) -> f32 {
        if self.length <= 0.0 {
            return 0.0;
        }
        self.kind.profile(ds / self.length, self.length) * self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_start_and_end_on_the_ground() {
        for k in Kind::ALL {
            for l in [30.0, 100.0] {
                assert!(k.profile(0.0, l).abs() < 0.02, "{k:?} start");
                assert!(k.profile(1.0, l).abs() < 0.02, "{k:?} end");
                let top = (0..=100)
                    .map(|i| k.profile(i as f32 / 100.0, l).abs())
                    .fold(0.0, f32::max);
                assert!(top > 0.5 && top <= 1.06, "{k:?} top {top}");
            }
        }
    }

    #[test]
    fn a_double_has_a_gap() {
        let k = Kind::Double;
        assert!(k.profile(0.26, 70.0) > 0.95);
        assert!(k.profile(0.45, 70.0) < 0.01);
        assert!(k.profile(0.68, 70.0) > 0.85);
    }

    #[test]
    fn faces_are_steepest_at_the_lip() {
        let k = Kind::Kicker;
        let slope = |u: f32| k.profile(u + 0.01, 24.0) - k.profile(u, 24.0);
        assert!(slope(0.6) > slope(0.1) * 2.0);
    }
}
