//! The pad in the menus, besides the pointer: a held D-pad or left stick
//! goes on stepping from button to button, and an edit box is typed in
//! a letter at a time, as on an arcade machine's high-score table.

#![cfg_attr(not(feature = "gamepad"), allow(dead_code))]

/// How long a held direction waits before it repeats, then between
/// repeats, in seconds.
const FIRST: f32 = 0.4;
const REPEAT: f32 = 0.11;

/// The left stick counts as a direction past half way.
const PUSHED: f32 = 0.5;

/// A way across (−1 left, 1 right) and down (−1 up, 1 down).
pub type Way = (i8, i8);

/// A direction held down, stepping once at once and then again while
/// it is held.
#[derive(Default)]
pub struct Repeat {
    held: Option<Way>,
    wait: f32,
}

impl Repeat {
    /// The way held this frame (or none), `dt` seconds after the last:
    /// the way to step now, if any.
    pub fn step(&mut self, way: Option<Way>, dt: f32) -> Option<Way> {
        let Some(way) = way else {
            self.held = None;
            return None;
        };
        if self.held != Some(way) {
            self.held = Some(way);
            self.wait = FIRST;
            return Some(way);
        }
        self.wait -= dt;
        if self.wait > 0.0 {
            return None;
        }
        self.wait = (self.wait + REPEAT).max(0.0);
        Some(way)
    }
}

/// The way the left stick points, along its longer axis, if it is
/// pushed far enough.
pub fn stick_way(v: [f32; 2]) -> Option<Way> {
    let (x, y) = (v[0], v[1]);
    if x.abs().max(y.abs()) < PUSHED {
        return None;
    }
    Some(if x.abs() > y.abs() { (x.signum() as i8, 0) } else { (0, y.signum() as i8) })
}

/// The letters an edit box steps through, in order.
const CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 .-_!?'&";

/// The letter after `c` (or before, going `up` false), round the end.
pub fn next_char(c: char, up: bool) -> char {
    let all: Vec<char> = CHARS.chars().collect();
    let n = all.len() as i32;
    let at = all.iter().position(|&a| a == c).map_or(if up { -1 } else { 0 }, |p| p as i32);
    all[(at + if up { 1 } else { -1 }).rem_euclid(n) as usize]
}

/// The letter a new place starts on after `text`: a capital first, then
/// small letters after a letter, a nought after a figure.
pub fn new_char(text: &str) -> char {
    match text.chars().last() {
        None => 'A',
        Some(c) if c.is_ascii_alphabetic() => 'a',
        Some(c) if c.is_ascii_digit() => '0',
        _ => 'A',
    }
}

/// What the pad does in an edit box being typed in.
#[derive(Debug, PartialEq)]
pub enum Typing {
    /// The last letter to the one after (true) or before it.
    Change(bool),
    /// A new letter on the end.
    Add,
    /// The last letter away.
    Remove,
}

/// The D-pad's way, typing.
pub fn typing(way: Way) -> Typing {
    match way {
        (0, d) => Typing::Change(d < 0),
        (1, _) => Typing::Add,
        _ => Typing::Remove,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_way_repeats() {
        let mut r = Repeat::default();
        assert_eq!(r.step(Some((0, 1)), 0.016), Some((0, 1)), "at once");
        assert_eq!(r.step(Some((0, 1)), 0.2), None);
        assert_eq!(r.step(Some((0, 1)), 0.21), Some((0, 1)), "after the first wait");
        assert_eq!(r.step(Some((0, 1)), 0.05), None);
        assert_eq!(r.step(Some((0, 1)), 0.07), Some((0, 1)), "then quicker");
        assert_eq!(r.step(Some((1, 0)), 0.016), Some((1, 0)), "a new way at once");
        assert_eq!(r.step(None, 0.016), None);
        assert_eq!(r.step(Some((1, 0)), 0.016), Some((1, 0)), "pressed again");
    }

    #[test]
    fn the_stick_points_its_longer_way() {
        assert_eq!(stick_way([0.3, 0.2]), None);
        assert_eq!(stick_way([0.7, 0.4]), Some((1, 0)));
        assert_eq!(stick_way([-0.2, -0.9]), Some((0, -1)));
    }

    #[test]
    fn letters_go_round() {
        assert_eq!(next_char('A', true), 'B');
        assert_eq!(next_char('A', false), '&');
        assert_eq!(next_char('&', true), 'A');
        assert_eq!(next_char('Z', true), 'a');
        assert_eq!(new_char(""), 'A');
        assert_eq!(new_char("Mi"), 'a');
        assert_eq!(new_char("12"), '0');
        assert_eq!(typing((0, -1)), Typing::Change(true));
        assert_eq!(typing((1, 0)), Typing::Add);
        assert_eq!(typing((-1, 0)), Typing::Remove);
    }
}
