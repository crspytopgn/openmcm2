//! A game pad in the launcher's window, for a Steam Deck or a sofa: the
//! D-pad or left stick moves between the buttons and boxes as the arrow
//! keys do (the one picked is outlined), A presses it, and the shoulder
//! buttons step through everything in order as Tab does.

use std::sync::mpsc;
use std::time::Duration;

use eframe::egui::{self, Key, Modifiers};
use gilrs::{Axis, Button, EventType, Gilrs};

/// A key the pad stands for, with Shift or not.
type Press = (Key, bool);

pub struct PadKeys {
    rx: mpsc::Receiver<Press>,
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
            loop {
                let Some(ev) = gilrs.next_event_blocking(Some(Duration::from_millis(250))) else {
                    continue;
                };
                let press = match ev.event {
                    EventType::ButtonPressed(b, _) => button(b),
                    EventType::AxisChanged(a @ (Axis::LeftStickX | Axis::LeftStickY), v, _) => {
                        let i = usize::from(a == Axis::LeftStickY);
                        stick(&mut held[i], a, v)
                    }
                    _ => None,
                };
                if let Some(p) = press {
                    if tx.send(p).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            }
        });
        PadKeys { rx }
    }

    /// Adds the pad's presses to this frame's input, as key presses;
    /// with nothing picked yet, the first goes to the first button.
    /// Presses while the window is in the background (the game is up)
    /// are dropped.
    pub fn feed(&self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let presses: Vec<Press> = self.rx.try_iter().collect();
        if presses.is_empty() || !raw.focused {
            return;
        }
        let mut picked = ctx.memory(|m| m.focused().is_some());
        for (key, shift) in presses {
            let (key, shift) = if picked { (key, shift) } else { (Key::Tab, false) };
            picked = true;
            let modifiers = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
            for pressed in [true, false] {
                raw.events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers });
            }
        }
    }
}

fn button(b: Button) -> Option<Press> {
    Some(match b {
        Button::DPadUp => (Key::ArrowUp, false),
        Button::DPadDown => (Key::ArrowDown, false),
        Button::DPadLeft => (Key::ArrowLeft, false),
        Button::DPadRight => (Key::ArrowRight, false),
        Button::South | Button::Start => (Key::Enter, false),
        Button::RightTrigger => (Key::Tab, false),
        Button::LeftTrigger => (Key::Tab, true),
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
}
