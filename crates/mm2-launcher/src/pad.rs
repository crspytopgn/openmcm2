//! A game pad in the launcher's window, for a Steam Deck or a sofa: the
//! D-pad or left stick moves between the buttons and boxes as the arrow
//! keys do (the one picked is outlined), going on while held, A presses
//! it, B backs out as Escape does (closing a list that is open), the
//! shoulder buttons step through everything in order as Tab does, the
//! triggers go to the page before or after, and the right stick scrolls.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, Modifiers};
use gilrs::{Axis, Button, EventType, Gilrs};

/// A key the pad stands for, with Shift or not.
type Press = (Key, bool);

/// What the pad asks of the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    Key(Press),
    /// Scroll the page this far down (points).
    Scroll(f32),
    /// The page `n` along in the sidebar.
    Page(i32),
}

/// How long a held direction waits before it repeats, then between
/// repeats.
const FIRST: Duration = Duration::from_millis(400);
const REPEAT: Duration = Duration::from_millis(110);
/// How often the thread looks at the right stick and held directions.
const TICK: Duration = Duration::from_millis(30);

pub struct PadKeys {
    rx: mpsc::Receiver<Out>,
}

impl PadKeys {
    /// Reads the pads on a thread of their own, waking the window for
    /// each press.
    pub fn start(ctx: egui::Context) -> PadKeys {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut gilrs = match Gilrs::new() {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("no gamepads in the launcher: {e}");
                    return;
                }
            };
            // Which way each stick axis was last pushed, so a push counts once.
            let mut held = [0i8; 2];
            // The arrow held down, and when it next repeats.
            let mut repeat: Option<(Key, Instant)> = None;
            let mut scroll = 0.0f32;
            loop {
                let mut out = Vec::new();
                if let Some(ev) = gilrs.next_event_blocking(Some(TICK)) {
                    match ev.event {
                        EventType::ButtonPressed(b, _) => {
                            if let Some(o) = button(b) {
                                if let Out::Key((k, false)) = o {
                                    if is_arrow(k) {
                                        repeat = Some((k, Instant::now() + FIRST));
                                    }
                                }
                                out.push(o);
                            }
                        }
                        EventType::ButtonReleased(b, _) => {
                            if let Some(Out::Key((k, _))) = button(b) {
                                if repeat.is_some_and(|(r, _)| r == k) {
                                    repeat = None;
                                }
                            }
                        }
                        EventType::AxisChanged(a @ (Axis::LeftStickX | Axis::LeftStickY), v, _) => {
                            let i = usize::from(a == Axis::LeftStickY);
                            if let Some(p) = stick(&mut held[i], a, v) {
                                repeat = Some((p.0, Instant::now() + FIRST));
                                out.push(Out::Key(p));
                            } else if held[i] == 0 && repeat.is_some_and(|(k, _)| axis_of(k) == Some(a)) && !dpad_held(&gilrs) {
                                repeat = None;
                            }
                        }
                        EventType::AxisChanged(Axis::RightStickY, v, _) => scroll = v,
                        _ => {}
                    }
                }
                if let Some((k, at)) = repeat {
                    if Instant::now() >= at {
                        repeat = Some((k, at + REPEAT));
                        out.push(Out::Key((k, false)));
                    }
                }
                // Past a fifth, up to 1500 points a second; gilrs has
                // a stick's Y up positive.
                if scroll.abs() > 0.2 {
                    out.push(Out::Scroll(-scroll * 1500.0 * TICK.as_secs_f32()));
                }
                for o in out {
                    if tx.send(o).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            }
        });
        PadKeys { rx }
    }

    /// Adds the pad's presses to this frame's input, as key presses and
    /// scrolling; with nothing picked yet, the first goes to the first
    /// button. Presses while the window is in the background (the game
    /// is up) are dropped. The pages asked for come back, and the
    /// scrolling is added to `scroll`.
    pub fn feed(&self, ctx: &egui::Context, raw: &mut egui::RawInput, scroll: &mut f32) -> Vec<i32> {
        let outs: Vec<Out> = self.rx.try_iter().collect();
        let mut pages = Vec::new();
        if outs.is_empty() || !raw.focused {
            return pages;
        }
        let mut picked = ctx.memory(|m| m.focused().is_some());
        for o in outs {
            match o {
                Out::Key((key, shift)) => {
                    let (key, shift) = if picked || key == Key::Escape { (key, shift) } else { (Key::Tab, false) };
                    picked = true;
                    let modifiers = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
                    for pressed in [true, false] {
                        raw.events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers });
                    }
                }
                Out::Scroll(by) => *scroll += by,
                Out::Page(n) => pages.push(n),
            }
        }
        pages
    }
}

fn is_arrow(k: Key) -> bool {
    matches!(k, Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight)
}

/// The stick axis that pushes arrow `k`.
fn axis_of(k: Key) -> Option<Axis> {
    match k {
        Key::ArrowLeft | Key::ArrowRight => Some(Axis::LeftStickX),
        Key::ArrowUp | Key::ArrowDown => Some(Axis::LeftStickY),
        _ => None,
    }
}

fn dpad_held(g: &Gilrs) -> bool {
    g.gamepads()
        .any(|(_, p)| [Button::DPadUp, Button::DPadDown, Button::DPadLeft, Button::DPadRight].iter().any(|&b| p.is_pressed(b)))
}

fn button(b: Button) -> Option<Out> {
    Some(match b {
        Button::DPadUp => Out::Key((Key::ArrowUp, false)),
        Button::DPadDown => Out::Key((Key::ArrowDown, false)),
        Button::DPadLeft => Out::Key((Key::ArrowLeft, false)),
        Button::DPadRight => Out::Key((Key::ArrowRight, false)),
        Button::South | Button::Start => Out::Key((Key::Enter, false)),
        Button::East => Out::Key((Key::Escape, false)),
        Button::RightTrigger => Out::Key((Key::Tab, false)),
        Button::LeftTrigger => Out::Key((Key::Tab, true)),
        Button::RightTrigger2 => Out::Page(1),
        Button::LeftTrigger2 => Out::Page(-1),
        _ => return None,
    })
}

/// A stick axis past two thirds counts as one arrow press; it must come
/// back under a third before it counts again.
fn stick(held: &mut i8, a: Axis, v: f32) -> Option<Press> {
    let way = if v > 0.66 {
        1
    } else if v < -0.66 {
        -1
    } else {
        if v.abs() < 0.33 {
            *held = 0;
        }
        return None;
    };
    if *held == way {
        return None;
    }
    *held = way;
    // gilrs has a stick's Y up positive.
    Some(match (a, way) {
        (Axis::LeftStickX, 1) => (Key::ArrowRight, false),
        (Axis::LeftStickX, _) => (Key::ArrowLeft, false),
        (_, 1) => (Key::ArrowUp, false),
        _ => (Key::ArrowDown, false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_push_counts_once() {
        let mut held = 0;
        assert_eq!(stick(&mut held, Axis::LeftStickY, 0.9), Some((Key::ArrowUp, false)));
        assert_eq!(stick(&mut held, Axis::LeftStickY, 1.0), None);
        assert_eq!(stick(&mut held, Axis::LeftStickY, 0.5), None);
        assert_eq!(stick(&mut held, Axis::LeftStickY, 0.9), None, "not back in the middle yet");
        assert_eq!(stick(&mut held, Axis::LeftStickY, 0.1), None);
        assert_eq!(stick(&mut held, Axis::LeftStickY, -0.8), Some((Key::ArrowDown, false)));
    }

    #[test]
    fn buttons() {
        assert_eq!(button(Button::East), Some(Out::Key((Key::Escape, false))));
        assert_eq!(button(Button::RightTrigger2), Some(Out::Page(1)));
        assert_eq!(axis_of(Key::ArrowUp), Some(Axis::LeftStickY));
    }
}
