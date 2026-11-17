//! The credits (`CreditsVidDlg`, `credits.dtm`, `0x453f80`): `UI/CREDITS.AVI`
//! over the whole screen with the Back arrow on top; the main menu again
//! when it ends (`0x44c1c0`) or Back is clicked. With the Free Pack and no
//! movie, the pack's `menus/credits.txt` rolls up the screen instead.

use super::{Cx, Go, Screen};
use crate::movie::Movie;
use crate::ui::{Dlg, Event, Gfx, Ui};

pub struct Credits {
    dlg: Dlg,
    movie: Option<Movie>,
    /// The rolling text with no movie: its lines (`# ` for a heading), and
    /// when it started.
    roll: Vec<String>,
    started_at: f32,
    started: bool,
}

/// How fast the rolling credits go up, pixels a second, and the space
/// between lines.
const ROLL_SPEED: f32 = 34.0;
const LINE: f32 = 20.0;

impl Credits {
    pub fn open(cx: &mut Cx) -> Option<Credits> {
        let dlg = Dlg::open(cx.ui, "CREDITS", (0.0, 0.0))?;
        let movie = Movie::open(&cx.game().join("UI/CREDITS.AVI"), false);
        let roll = match movie {
            Some(_) => Vec::new(),
            None => crate::freepack::credits()?,
        };
        Some(Credits {
            dlg,
            movie,
            roll,
            started_at: 0.0,
            started: false,
        })
    }
}

impl Screen for Credits {
    fn layers(&mut self) -> Vec<&mut Dlg> {
        vec![&mut self.dlg]
    }

    fn event(&mut self, _cx: &mut Cx, e: Event) -> Go {
        match e {
            Event::Clicked(n) if n.eq_ignore_ascii_case("Back") => Go::Back,
            _ => Go::Stay,
        }
    }

    fn tick(&mut self, cx: &mut Cx) -> Go {
        if !self.started {
            self.started = true;
            self.started_at = cx.now;
            if let Some(m) = &mut self.movie {
                m.play(cx.ui.audio.as_ref(), cx.now);
            }
        }
        let done = match &self.movie {
            Some(m) => m.done(cx.now),
            // Every line gone off the top.
            None => (cx.now - self.started_at) * ROLL_SPEED > 480.0 + self.roll.len() as f32 * LINE,
        };
        if done {
            return Go::Back;
        }
        Go::Stay
    }

    fn draw(&mut self, ui: &mut Ui, gfx: &mut Gfx, now: f32) {
        match &mut self.movie {
            Some(m) => m.draw(gfx, now, [0.0, 0.0, 640.0, 480.0]),
            None => {
                ui.fill(gfx, [0.0, 0.0, 640.0, 480.0], [0.06, 0.06, 0.055, 1.0]);
                let top = 480.0 - (now - self.started_at).max(0.0) * ROLL_SPEED;
                for (i, line) in self.roll.iter().enumerate() {
                    let y = top + i as f32 * LINE;
                    if !(-LINE..480.0).contains(&y) || line.is_empty() {
                        continue;
                    }
                    let (text, heading) = match line.strip_prefix("# ") {
                        Some(h) => (h, true),
                        None => (line.as_str(), false),
                    };
                    let Some(t) = ui.text(gfx, text, if heading { 15.0 } else { 12.0 }, heading) else { continue };
                    let (w, h) = gfx.sprites.size(t);
                    // Headings in the lime of the new menus.
                    let color = if heading { [0.64, 0.76, 0.29, 1.0] } else { [0.91, 0.89, 0.82, 1.0] };
                    gfx.sprites.quad(t, [0.0, 0.0, 1.0, 1.0], [320.0 - w as f32 / 2.0, y, w as f32, h as f32], color);
                }
            }
        }
        self.dlg.tick(now);
        self.dlg.draw(ui, gfx, now);
    }
}
