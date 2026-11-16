//! Full screen or a window, chosen on Options' Graphics page (not the
//! game's: it listed the 16-bit modes of a card of the time, and changed
//! them on its next start). The page asks; the window changes on the next
//! frame, and the choice is saved to the launcher's `launcher.ini` for the
//! next start.

use mm2_settings::{fits, Settings, SHAPES};
use std::sync::Mutex;
use winit::window::{Fullscreen, Window};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Full,
    /// A window this many pixels inside.
    Window(u32, u32),
}

impl Mode {
    /// As the page lists it, in the game's style ("640 X 480").
    pub fn text(self) -> String {
        match self {
            Mode::Full => "Full Screen".into(),
            Mode::Window(w, h) => format!("{w} X {h}"),
        }
    }
}

struct State {
    now: Mode,
    monitor: Option<(u32, u32)>,
    asked: Option<Mode>,
}

static STATE: Mutex<State> = Mutex::new(State {
    now: Mode::Full,
    monitor: None,
    asked: None,
});

/// The window as it was last frame.
pub fn current() -> Mode {
    STATE.lock().unwrap().now
}

/// What the page offers: full screen, then the window sizes that fit on
/// the screen, smallest first, and the window's own size if it is none
/// of them.
pub fn modes() -> Vec<Mode> {
    let s = STATE.lock().unwrap();
    offered(s.now, s.monitor)
}

fn offered(now: Mode, monitor: Option<(u32, u32)>) -> Vec<Mode> {
    let mut sizes: Vec<(u32, u32)> = SHAPES
        .iter()
        .flat_map(|s| s.2.iter().map(|z| (z.0, z.1)))
        .filter(|&z| fits(z, monitor))
        .collect();
    if let Mode::Window(w, h) = now {
        if !sizes.contains(&(w, h)) {
            sizes.push((w, h));
        }
    }
    sizes.sort();
    std::iter::once(Mode::Full)
        .chain(sizes.into_iter().map(|(w, h)| Mode::Window(w, h)))
        .collect()
}

/// Asks for `mode` from the next frame.
pub fn request(mode: Mode) {
    STATE.lock().unwrap().asked = Some(mode);
}

/// Each frame: notes the window, and makes the change asked for.
pub fn tick(window: &Window) {
    // In points, as the launcher and `--window` count a window: pixels on
    // most screens, a half each way on a Retina screen.
    let scale = window.scale_factor();
    let points = |z: winit::dpi::PhysicalSize<u32>| {
        let z = z.to_logical::<f64>(scale);
        (z.width.round() as u32, z.height.round() as u32)
    };
    let monitor = window.current_monitor().map(|m| points(m.size()));
    let mut s = STATE.lock().unwrap();
    s.monitor = monitor;
    s.now = match window.fullscreen() {
        Some(_) => Mode::Full,
        None => {
            let (w, h) = points(window.inner_size());
            Mode::Window(w, h)
        }
    };
    let Some(mode) = s.asked.take() else {
        return;
    };
    if mode == s.now {
        return;
    }
    match mode {
        Mode::Full => window.set_fullscreen(Some(Fullscreen::Borderless(None))),
        Mode::Window(w, h) => {
            window.set_fullscreen(None);
            let _ = window.request_inner_size(winit::dpi::LogicalSize::new(w, h));
            // In the middle of the screen, the title bar a little higher
            // to make room for it.
            if let Some(m) = window.current_monitor() {
                let (mw, mh) = (m.size().width as f64, m.size().height as f64);
                let (pw, ph) = (w as f64 * scale, h as f64 * scale);
                let x = m.position().x + ((mw - pw) / 2.0) as i32;
                let y = m.position().y + ((mh - ph) / 2.0 - 20.0 * scale).max(0.0) as i32;
                window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
            }
        }
    }
    let mut settings = Settings::load();
    match mode {
        Mode::Full => settings.fullscreen = true,
        Mode::Window(w, h) => {
            settings.fullscreen = false;
            settings.window = (w, h);
        }
    }
    if let Err(e) = settings.save() {
        eprintln!("display: couldn't save {}: {e}", Settings::path().display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_screen_then_the_sizes_that_fit() {
        let modes = offered(Mode::Full, Some((1920, 1080)));
        assert_eq!(modes[0], Mode::Full);
        assert_eq!(modes[1], Mode::Window(800, 600));
        assert!(modes.contains(&Mode::Window(1600, 900)));
        assert!(!modes.contains(&Mode::Window(1920, 1080)));
        // A window dragged to its own size keeps it on the list.
        let modes = offered(Mode::Window(1111, 777), Some((1920, 1080)));
        assert!(modes.contains(&Mode::Window(1111, 777)));
    }
}
