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
    /// Full screen as the screen is.
    Full,
    /// Full screen with the screen switched to this many pixels, one of
    /// its own modes.
    FullAt(u32, u32),
    /// A window this many pixels inside.
    Window(u32, u32),
}

impl Mode {
    /// As the page lists it, in the game's style ("640 X 480").
    pub fn text(self) -> String {
        match self {
            Mode::Full => "Full Screen".into(),
            Mode::FullAt(w, h) => format!("Full Screen {w} X {h}"),
            Mode::Window(w, h) => format!("{w} X {h} Window"),
        }
    }
}

struct State {
    now: Mode,
    monitor: Option<(u32, u32)>,
    /// The screen's own modes' sizes, in pixels.
    screen_modes: Vec<(u32, u32)>,
    asked: Option<Mode>,
}

static STATE: Mutex<State> = Mutex::new(State {
    now: Mode::Full,
    monitor: None,
    screen_modes: Vec::new(),
    asked: None,
});

/// The window as it was last frame.
pub fn current() -> Mode {
    STATE.lock().unwrap().now
}

/// What the page offers: full screen, then full screen at the screen's
/// other resolutions (the usual sizes it has, biggest first), then the
/// window sizes that fit on the screen, smallest first, and the window's
/// own size if it is none of them.
pub fn modes() -> Vec<Mode> {
    let s = STATE.lock().unwrap();
    offered(s.now, s.monitor, &s.screen_modes)
}

/// The full screen resolutions to offer out of the screen's `own` modes:
/// those of the usual sizes (a screen lists dozens of odd ones), biggest
/// first, and the one in use.
fn full_sizes(now: Mode, own: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let usual = |z: (u32, u32)| SHAPES.iter().flat_map(|s| s.2).any(|u| (u.0, u.1) == z);
    let mut sizes: Vec<(u32, u32)> = own.iter().copied().filter(|&z| usual(z)).collect();
    if let Mode::FullAt(w, h) = now {
        sizes.push((w, h));
    }
    sizes.sort_by_key(|z| std::cmp::Reverse(z.0 as u64 * z.1 as u64));
    sizes.dedup();
    sizes
}

fn offered(now: Mode, monitor: Option<(u32, u32)>, screen_modes: &[(u32, u32)]) -> Vec<Mode> {
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
        .chain(full_sizes(now, screen_modes).into_iter().map(|(w, h)| Mode::FullAt(w, h)))
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
    let screen = window.current_monitor();
    let monitor = screen.as_ref().map(|m| points(m.size()));
    let mut s = STATE.lock().unwrap();
    if s.monitor != monitor || s.screen_modes.is_empty() {
        s.screen_modes = screen.as_ref().map_or(Vec::new(), |m| m.video_modes().map(|v| (v.size().width, v.size().height)).collect());
    }
    s.monitor = monitor;
    s.now = match window.fullscreen() {
        Some(Fullscreen::Exclusive(m)) => Mode::FullAt(m.size().width, m.size().height),
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
        Mode::FullAt(w, h) => {
            // The screen's fastest mode of that size; as it is if none.
            let best = screen
                .iter()
                .flat_map(|m| m.video_modes())
                .filter(|v| (v.size().width, v.size().height) == (w, h))
                .max_by_key(|v| (v.refresh_rate_millihertz(), v.bit_depth()));
            window.set_fullscreen(Some(best.map_or(Fullscreen::Borderless(None), Fullscreen::Exclusive)));
        }
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
        Mode::Full => {
            settings.fullscreen = true;
            settings.screen = (0, 0);
        }
        Mode::FullAt(w, h) => {
            settings.fullscreen = true;
            settings.screen = (w, h);
        }
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
        let modes = offered(Mode::Full, Some((1920, 1080)), &[]);
        assert_eq!(modes[0], Mode::Full);
        assert_eq!(modes[1], Mode::Window(800, 600));
        assert!(modes.contains(&Mode::Window(1600, 900)));
        assert!(modes.contains(&Mode::Window(1440, 900)));
        assert!(!modes.contains(&Mode::Window(1920, 1080)));
        // A window dragged to its own size keeps it on the list.
        let modes = offered(Mode::Window(1111, 777), Some((1920, 1080)), &[]);
        assert!(modes.contains(&Mode::Window(1111, 777)));
    }

    #[test]
    fn the_screens_own_resolutions_for_full_screen() {
        let own = [(3840, 2160), (1920, 1080), (1920, 1080), (2560, 1440), (1704, 960), (1024, 768)];
        let modes = offered(Mode::Full, Some((3840, 2160)), &own);
        // The usual sizes, biggest first, each once; the odd 1704 x 960 not.
        assert_eq!(&modes[1..5], &[Mode::FullAt(3840, 2160), Mode::FullAt(2560, 1440), Mode::FullAt(1920, 1080), Mode::FullAt(1024, 768)]);
        assert!(!modes.contains(&Mode::FullAt(1704, 960)));
        assert_eq!(modes[5], Mode::Window(800, 600));
    }
}
