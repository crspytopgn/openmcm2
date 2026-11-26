//! The screens before the main menu (`Intro1Dlg` `0x499850`, `Intro2Dlg`
//! `0x454240`, `Intro3Dlg` `0x4543b0`): the publisher's and the
//! developer's logos and the legal notice, each for a few seconds. A
//! click or a key skips them all (`0x4543a0`, `0x454570`). Then the intro
//! movie (`IntroVid1Dlg`, `0x4540d0`): `UI/INTRO.AVI` over the whole
//! screen, and the main menu when it ends or is skipped (`0x44c1c0`).
//!
//! With the Extras' "Your own intro video", the player's own video file
//! plays in its place (`own_video.rs`; in a browser, the page plays it),
//! and the game's own if that one can't be played.

use super::{Cx, Go, Screen};
use crate::movie::Movie;
use crate::ui::{Dlg, Event, Gfx, Ui};

const PAGES: [&str; 3] = ["INTRO1", "INTRO2", "INTRO3"];

/// How long each shows.
const SECONDS: f32 = 3.0;

pub struct Intro {
    page: usize,
    dlg: Dlg,
    since: Option<f32>,
    movie: Option<Movie>,
    own: Own,
}

/// The player's own intro video, once the logos are done.
enum Own {
    /// Not asked for, or not got to yet.
    None,
    /// Opening (its first picture decoding), since this time.
    #[cfg(not(target_arch = "wasm32"))]
    Opening(crate::own_video::OwnVideo, std::time::Instant),
    #[cfg(not(target_arch = "wasm32"))]
    Playing(crate::own_video::OwnVideo),
    /// The browser page is playing it over the game.
    #[cfg(target_arch = "wasm32")]
    Page,
}

/// The player's own intro video, if they picked one.
fn own_intro() -> Option<&'static str> {
    Some(crate::tweaks::more().intro_video.as_str()).filter(|p| !p.is_empty())
}

impl Intro {
    pub fn open(ui: &mut Ui) -> Option<Intro> {
        Some(Intro {
            page: 0,
            dlg: Dlg::open(ui, PAGES[0], (0.0, 0.0))?,
            since: None,
            movie: None,
            own: Own::None,
        })
    }
}

impl Intro {
    /// The game's own intro movie, or the main menu without one.
    fn game_movie(&mut self, cx: &mut Cx) -> Go {
        let path = cx.game().join("UI/INTRO.AVI");
        let Some(mut movie) = Movie::open(&path, false) else {
            return Go::Back;
        };
        movie.play(cx.ui.audio.as_ref(), cx.now);
        self.movie = Some(movie);
        Go::Stay
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for Intro {
    fn drop(&mut self) {
        // Skipped in the game (a key the page didn't see): the page's
        // video stops too.
        if matches!(self.own, Own::Page) {
            crate::web::intro_stop();
        }
    }
}

impl Screen for Intro {
    fn layers(&mut self) -> Vec<&mut Dlg> {
        vec![&mut self.dlg]
    }

    fn event(&mut self, _cx: &mut Cx, _e: Event) -> Go {
        Go::Stay
    }

    fn escape(&mut self, _cx: &mut Cx) -> Go {
        Go::Back
    }

    fn pressed(&mut self, _cx: &mut Cx) -> Go {
        Go::Back
    }

    fn key(&mut self, _cx: &mut Cx, _code: u32) -> Go {
        Go::Back
    }

    fn tick(&mut self, cx: &mut Cx) -> Go {
        if let Some(m) = &self.movie {
            return if m.done(cx.now) { Go::Back } else { Go::Stay };
        }
        match &mut self.own {
            Own::None => {}
            #[cfg(not(target_arch = "wasm32"))]
            Own::Opening(v, since) => {
                // Give a file that won't decode a few seconds, then the
                // game's own.
                if v.ready() || since.elapsed().as_secs_f32() > 5.0 {
                    if v.failed() || !v.ready() {
                        eprintln!("intro video: nothing to show, playing the game's own");
                        self.own = Own::None;
                        return self.game_movie(cx);
                    }
                    let Own::Opening(mut v, _) = std::mem::replace(&mut self.own, Own::None) else { unreachable!() };
                    v.play(cx.ui.audio.as_ref(), cx.now);
                    self.own = Own::Playing(v);
                }
                return Go::Stay;
            }
            #[cfg(not(target_arch = "wasm32"))]
            Own::Playing(v) => return if v.done(cx.now) { Go::Back } else { Go::Stay },
            #[cfg(target_arch = "wasm32")]
            Own::Page => {
                if !crate::web::intro_done() {
                    return Go::Stay;
                }
                // The browser couldn't play it: the game's own.
                if crate::web::intro_failed() {
                    self.own = Own::None;
                    return self.game_movie(cx);
                }
                return Go::Back;
            }
        }
        let since = *self.since.get_or_insert(cx.now);
        if cx.now - since < SECONDS {
            return Go::Stay;
        }
        self.page += 1;
        let Some(dlg) = PAGES
            .get(self.page)
            .and_then(|p| Dlg::open(cx.ui, p, (0.0, 0.0)))
        else {
            if let Some(path) = own_intro() {
                #[cfg(not(target_arch = "wasm32"))]
                match crate::own_video::OwnVideo::open(std::path::Path::new(path)) {
                    Ok(v) => {
                        self.own = Own::Opening(v, std::time::Instant::now());
                        return Go::Stay;
                    }
                    Err(e) => eprintln!("intro video {path}: {e}; playing the game's own"),
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = path;
                    if crate::web::intro_play() {
                        self.own = Own::Page;
                        return Go::Stay;
                    }
                }
            }
            return self.game_movie(cx);
        };
        self.dlg = dlg;
        self.since = Some(cx.now);
        Go::Stay
    }

    fn draw(&mut self, ui: &mut Ui, gfx: &mut Gfx, now: f32) {
        match (&mut self.movie, &mut self.own) {
            // The game shows it stretched to the screen (`0x4540d0`).
            (Some(m), _) => m.draw(gfx, now, [0.0, 0.0, 640.0, 480.0]),
            // The player's own, its shape kept, as big as the window
            // allows.
            #[cfg(not(target_arch = "wasm32"))]
            (None, Own::Playing(v)) => {
                let rect = gfx.sprites.fit_window(v.aspect());
                v.draw(gfx, now, rect);
            }
            // Black while it opens, and under the page's.
            #[cfg(not(target_arch = "wasm32"))]
            (None, Own::Opening(..)) => {}
            #[cfg(target_arch = "wasm32")]
            (None, Own::Page) => {}
            (None, Own::None) => self.dlg.draw(ui, gfx, now),
        }
    }
}
