//! Berms: the dirt banked up round the outside of a turn, shaped as a
//! raced-on motocross track's are, shared by the Course Builder's tracks,
//! the rebuilt demo tracks and the Free Pack's own.
//!
//! Across the track a berm is a bowl: the inside of the turn stays flat,
//! the face curves up ever steeper towards the outside (as the original
//! game's berms do, measured off its Nationals and Supercross tracks), the
//! dirt the bikes push outward piles into a rounded lip just past the
//! track's edge, and behind the lip it falls away at the slope loose dirt
//! settles at until it meets the ground. Along the turn it builds up into
//! the bend and dies away after it, and how tall it stands follows how
//! tight the turn is: a hairpin gets a big wall, a fast sweeper a gentle
//! bank, a straight none. Raced on, the lip is uneven and blown out in
//! places, and a broad rut is worn into the face where the bikes rail it.

use crate::land;

/// How strongly a turn of `curve` (radians a foot, smoothed along the line)
/// is banked, 0 to 1: fully on a hairpin of 40 ft radius or tighter, about
/// two fifths on a 150 ft sweeper, nothing much past 600 ft.
pub fn strength(curve: f32) -> f32 {
    let (wide, tight) = (1.0 / 900.0, 1.0 / 40.0);
    ((curve.abs() - wide) / (tight - wide)).clamp(0.0, 1.0).powf(0.6)
}

/// Where the berm stands against a spot on the ground, from [`shape`].
#[derive(Debug, Clone, Copy)]
pub struct Rise {
    /// Height above the track bed, ft. On the track it is added to the
    /// bed; past the edge it can go below 0, where the berm's back has
    /// fallen away, and the ground there is the higher of it and the land.
    pub height: f32,
    /// How much of the spot is the berm's bare dirt, 0 to 1: its face, its
    /// lip and its back down to where it meets the ground.
    pub dirt: f32,
    /// How deep the bikes' rut is worn here, ft (for painting it darker).
    pub rut: f32,
}

/// The berm's shape at `out` ft from the middle of the track towards the
/// outside of the turn (negative is the inside), on a track `half` ft
/// each side, for a berm whose lip crests `tall` ft above the bed, at
/// `along` ft round the lap (for its wear) with `seed` for the track.
pub fn shape(out: f32, half: f32, tall: f32, along: f32, seed: u32) -> Rise {
    let none = Rise { height: if out > half { f32::MIN } else { 0.0 }, dirt: 0.0, rut: 0.0 };
    if tall < 0.05 || out <= 0.0 {
        return none;
    }
    let half = half.max(4.0);
    // Raced on: the lip a little higher here, lower there, and blown out
    // in places where riders have gone over the top, the whole berm
    // wandering in and out a foot or two.
    let u = |v: f32| v * 0.5 + 0.5;
    let wobble = land::fbm(along / 38.0, 1.7, seed ^ 0xbe41, 2);
    let blown = smoothstep(0.62, 0.85, u(land::noise(along / 70.0, 4.3, seed ^ 0xbe42)));
    let crest = tall * (0.9 + 0.22 * wobble) * (1.0 - 0.4 * blown);
    let out = (out - wobble * half * 0.06).max(0.0);
    // The face: a bowl from flat in the middle to 0.62 of the crest at the
    // edge, steepest at the top.
    const POWER: f32 = 2.2;
    let edge = 0.62 * crest;
    let lip = (half * 0.3).max(4.0);
    let fall = 0.62;
    let (height, dirt) = if out <= half {
        (edge * (out / half).powf(POWER), 1.0)
    } else if out <= half + lip {
        // The lip: on up from the face, rounding over at its crest.
        let t = (out - half) / lip;
        let s0 = POWER * edge / half * lip;
        let (h00, h10, h01) = (2.0 * t * t * t - 3.0 * t * t + 1.0, t * t * t - 2.0 * t * t + t, -2.0 * t * t * t + 3.0 * t * t);
        (edge * h00 + s0 * h10 + crest * h01, 1.0)
    } else {
        // The back: rounding over the crest, then the slope loose dirt
        // settles at, on down until it meets the ground.
        let x = out - half - lip;
        let round = lip * 0.7;
        let h = if x <= round { crest - fall * x * x / (2.0 * round) } else { crest - fall * round / 2.0 - fall * (x - round) };
        (h, smoothstep(-0.6, 0.4, h) * (1.0 - smoothstep(0.0, 6.0, x - crest / fall)))
    };
    // The rut the bikes wear into the face where they rail the berm, and
    // a shallower one lower down, both wandering a little.
    let rut_at = |k: f32, s: u32| half * (k + 0.05 * land::fbm(along / 45.0, k * 9.0, seed ^ s, 2));
    let groove = |at: f32, w: f32| (-((out - at) / w).powi(2)).exp();
    let rut = (0.09 * groove(rut_at(0.74, 0xbe43), half * 0.16) + 0.04 * groove(rut_at(0.45, 0xbe44), half * 0.14)) * crest.min(6.0);
    Rise { height: height - rut, dirt, rut }
}

/// The ground where a berm meets the land past the track's edge: the
/// higher of the land (`ground`, already blended out from the track) and
/// the berm standing on the bed (`bed + rise`), rounded where they meet.
pub fn meet(ground: f32, bed: f32, rise: f32, tall: f32) -> f32 {
    if rise == f32::MIN {
        return ground;
    }
    let k = (tall * 0.35).clamp(0.0, 2.0);
    let b = bed + rise;
    if k <= 0.0 {
        return ground.max(b);
    }
    let h = (k - (ground - b).abs()).max(0.0) / k;
    ground.max(b) + h * h * k * 0.25
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_berm_rises_to_a_lip_then_falls_away() {
        let half = 22.0;
        let at = |o: f32| shape(o, half, 6.0, 300.0, 1).height;
        assert_eq!(at(-10.0), 0.0);
        assert!(at(5.0) < 0.5);
        // The face steepens up to the edge, and the lip stands higher still.
        assert!(at(11.0) - at(5.0) < at(22.0) - at(16.0));
        assert!(at(22.0 + 6.0) > at(22.0));
        // Well past the lip it is below the bed: the land takes over.
        assert!(at(60.0) < 0.0);
        assert_eq!(meet(1.0, 0.0, at(60.0), 6.0), 1.0);
    }

    #[test]
    fn tight_turns_bank_more_than_sweepers() {
        assert!(strength(1.0 / 30.0) > 0.99);
        assert!(strength(1.0 / 150.0) > 0.3 && strength(1.0 / 150.0) < 0.5);
        assert_eq!(strength(0.0), 0.0);
    }

    #[test]
    fn the_flat_meets_the_berm_without_a_step() {
        // On a straight nothing changes, and the blend into the land is
        // left as it was.
        let r = shape(30.0, 22.0, 0.0, 0.0, 1);
        assert_eq!(meet(2.5, 0.0, r.height, 0.0), 2.5);
        // Across the edge the height runs on smoothly.
        let a = shape(21.9, 22.0, 6.0, 300.0, 1).height;
        let b = shape(22.1, 22.0, 6.0, 300.0, 1).height;
        assert!((a - b).abs() < 0.1, "{a} {b}");
    }
}
