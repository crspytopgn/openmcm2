//! The saved replays and ghost races (`UI/GRBASE.DAT`, `0x454c10`): two
//! tabs, Saved Ghost Race Files (`GhostFil`, `0x455030`) and Saved Replay
//! Files (`ReplyFil`, `0x455a50`), each listing the game's files in
//! `Record\` (`*.gho`, `*.vcr`) by track, rider, laps, length and
//! description. Start plays the file picked (a ghost as a ghost race on
//! its track), and shows only when there is one.
//!
//! Description... renames the file's line in the list: the description
//! is kept in the tape's header (31 letters), asked for in a message box
//! (lang 5183).
//!
//! The files are kept in `Record` in the user's folder (the game kept
//! them in its own).
//!
//! With the online ghosts (an extra, off by default), the ghost tab lists
//! the leaderboard server's ghosts under the player's own: the fastest
//! lap of each track the player has. Start fetches the ghost into
//! `Record/Online` and races it as a saved one.

use super::{popup, Cx, Go, Screen};
use crate::ui::{Dlg, Event};
use mm2_formats::vcr::{self, Tape};
use std::path::PathBuf;

/// Where the tabs' pages go (`0x454e90`).
const PAGE: (f32, f32) = (20.0, 35.0);

/// The tabs, their pages, and their names (strings 5178, 5177).
const TABS: [(&str, &str, u32); 2] = [
    ("TabLeft", "GHOSTFIL", 5178),
    ("TabRight", "REPLYFIL", 5177),
];

const LISTS: [&str; 5] = ["LstTrack", "LstRider", "LstLaps", "LstLength", "LstDesc"];

pub struct Replays {
    base: Dlg,
    page: Dlg,
    tab: usize,
    /// The files listed, in the lists' order.
    files: Vec<PathBuf>,
    picked: Option<usize>,
    /// The description box, while it is up.
    describe: Option<Dlg>,
    /// The server's ghosts for the player's tracks, under the files.
    online: Vec<crate::online::GhostInfo>,
    /// The server's list of ghosts on its way.
    asked: Option<crate::online::Reply>,
    /// A ghost being fetched to race, and where it goes.
    fetching: Option<(crate::online::Reply, PathBuf)>,
}

/// Whether the ghost tab lists the server's ghosts (the extra).
fn online_ghosts() -> Option<String> {
    crate::tweaks::more().online_ghosts.then(crate::online::address).flatten()
}

/// The server's ghosts on a track the player has, as a ghost race finds
/// it (`Frontend::ghost_setup`).
fn on_my_tracks(cx: &Cx, ghosts: Vec<crate::online::GhostInfo>) -> Vec<crate::online::GhostInfo> {
    use crate::frontend::{list_tracks, EventKind};
    let mut lists = std::collections::HashMap::new();
    ghosts
        .into_iter()
        .filter(|g| {
            let Some(event) = EventKind::from_number(g.event) else { return false };
            let tracks = lists.entry(g.event).or_insert_with(|| list_tracks(&cx.game().join(event.folder()), event));
            tracks.iter().any(|e| {
                e.path.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case(&g.track))
                    && (e.course == g.course as usize || !event.courses())
            })
        })
        .collect()
}

/// The tapes of a tab in `Record`: ghosts (`*.gho`) or replays (`*.vcr`),
/// by name, each with its header.
fn tapes(cx: &Cx, tab: usize) -> Vec<(PathBuf, Tape)> {
    let ext = if tab == 0 { "gho" } else { "vcr" };
    let dir = crate::replay::record_dir(&cx.user.store.dir);
    let mut paths: Vec<PathBuf> = crate::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)))
                .collect()
        })
        .unwrap_or_default();
    paths.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_uppercase()));
    paths
        .into_iter()
        .filter_map(|p| {
            let tape = Tape::parse(&crate::fs::read(&p).ok()?).ok()?;
            Some((p, tape))
        })
        .collect()
}

impl Replays {
    /// Opens on the replays (`0x454c10` open).
    pub fn open(cx: &mut Cx) -> Option<Replays> {
        let mut base = Dlg::open(cx.ui, "GRBASE", (0.0, 0.0))?;
        for (tab, _, id) in TABS {
            base.set_text(tab, cx.ui.lang(id));
        }
        let page = Dlg::open(cx.ui, TABS[1].1, PAGE)?;
        let mut r = Replays {
            base,
            page,
            tab: 1,
            files: Vec::new(),
            picked: None,
            describe: None,
            online: Vec::new(),
            asked: None,
            fetching: None,
        };
        r.show(cx, 1);
        Some(r)
    }

    fn show(&mut self, cx: &mut Cx, tab: usize) {
        let Some(mut page) = Dlg::open(cx.ui, TABS[tab].1, PAGE) else {
            return;
        };
        for (i, (name, _, _)) in TABS.iter().enumerate() {
            self.base.set_on(name, i == tab);
        }
        let tapes = tapes(cx, tab);
        // The server's ghosts go under the player's own, on the ghost tab.
        let online: &[crate::online::GhostInfo] = if tab == 0 { &self.online } else { &[] };
        let column = |f: &dyn Fn(&Tape) -> String, g: &dyn Fn(&crate::online::GhostInfo) -> String| {
            tapes.iter().map(|(_, t)| f(t)).chain(online.iter().map(g)).collect()
        };
        page.set_items(
            "LstTrack",
            column(&|t| t.track(), &|g| if g.title.is_empty() { g.track.clone() } else { g.title.clone() }),
            0,
        );
        page.set_items(
            "LstRider",
            column(&|t| t.riders.first().map(|r| r.name.clone()).unwrap_or_default(), &|g| g.rider.clone()),
            0,
        );
        page.set_items(
            "LstLaps",
            column(&|t| t.setting(vcr::setting::LAPS).to_string(), &|_| String::new()),
            0,
        );
        page.set_items(
            "LstLength",
            column(
                &|t| crate::hud::clock_text(t.length).trim().to_string(),
                &|g| crate::hud::clock_text(g.ms as f32 / 1000.0).trim().to_string(),
            ),
            0,
        );
        page.set_items(
            "LstDesc",
            column(&|t| t.description.clone(), &|g| format!("Online fastest lap, {}cc", g.class)),
            0,
        );
        for list in LISTS {
            page.set_selectable(list, true);
            // The original shows the picked tape in pink, on no box.
            page.set_select_color(list, 0xff89ff);
            page.mark(list, None);
        }
        self.files = tapes.into_iter().map(|(p, _)| p).collect();
        self.picked = None;
        self.tab = tab;
        // Ghost laps have no lap count (`0x455030`).
        page.show("LstLaps", tab == 1);
        page.show("TxtLaps", tab == 1);
        page.enable("ButDescription", false);
        page.enable("ButDelete", false);
        self.base.show("Start", false);
        self.page = page;
        if tab == 0 && self.asked.is_none() && self.online.is_empty() {
            if let Some(url) = online_ghosts() {
                self.asked = Some(crate::online::get(format!("{url}/ghosts")));
            }
        }
    }

    /// Whether the line picked is one of the server's ghosts.
    fn online_line(&self) -> Option<&crate::online::GhostInfo> {
        let line = self.picked?.checked_sub(self.files.len())?;
        (self.tab == 0).then(|| self.online.get(line)).flatten()
    }

    /// Fetches the server's ghost picked to race, into `Record/Online`.
    fn fetch(&mut self, cx: &Cx) {
        let (Some(g), Some(url)) = (self.online_line(), online_ghosts()) else { return };
        let keep = |s: &str| -> String { s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect() };
        let name = format!("{}_{}_{}_{}.gho", keep(&g.track), g.event, g.course, keep(&g.rider));
        let path = crate::replay::record_dir(&cx.user.store.dir).join("Online").join(name);
        self.fetching = Some((crate::online::get(g.address(&url)), path));
        self.base.show("Start", false);
    }
}

impl Replays {
    /// The description box for the file picked, empty.
    fn describe_box(cx: &mut Cx) -> Option<Dlg> {
        let mut d = popup(cx.ui, "MESSBOX1")?;
        d.set_text("TitleText", "");
        d.set_text("OKButton", cx.ui.lang(5097));
        d.set_text("CancelButton", cx.ui.lang(5084));
        d.set_text("TxtPrompt", cx.ui.lang(5183));
        if let Some(c) = d.ctl("EditBox") {
            c.max_len = 31;
            c.text = Some(String::new());
        }
        d.edit(Some("EditBox"));
        Some(d)
    }

    /// Writes the description into the picked file's header.
    fn redescribe(&mut self, text: &str) {
        let Some(p) = self.picked.and_then(|i| self.files.get(i)) else {
            return;
        };
        let written = crate::fs::read(p)
            .ok()
            .and_then(|b| Tape::parse(&b).ok())
            .map(|mut t| {
                t.description = text.chars().take(31).collect();
                crate::fs::write(p, t.write())
            });
        if let Some(Err(e)) = written {
            eprintln!("{}: {e}", p.display());
        }
    }

    fn describe_event(&mut self, cx: &mut Cx, e: Event) -> Go {
        let Some(d) = &mut self.describe else {
            return Go::Stay;
        };
        let clicked = |n: &str| matches!(&e, Event::Clicked(c) if c.eq_ignore_ascii_case(n));
        let done = clicked("OKButton")
            || matches!(&e, Event::Released(n) if n.eq_ignore_ascii_case("EditBox"));
        if done {
            let text = d
                .get("EditBox")
                .and_then(|c| c.text.clone())
                .unwrap_or_default();
            self.describe = None;
            let line = self.picked;
            self.redescribe(text.trim());
            self.show(cx, self.tab);
            if let Some(l) = line.filter(|&l| l < self.files.len()) {
                for list in LISTS {
                    self.page.mark(list, Some(l));
                }
                self.picked = Some(l);
                self.page.enable("ButDescription", true);
                self.page.enable("ButDelete", true);
                self.base.show("Start", true);
            }
        } else if clicked("CancelButton") {
            self.describe = None;
        } else if clicked("EditBox") {
            d.edit(Some("EditBox"));
        }
        Go::Stay
    }
}

impl Screen for Replays {
    fn tick(&mut self, cx: &mut Cx) -> Go {
        if let Some(r) = self.asked.as_ref().and_then(crate::online::take) {
            match r {
                Ok(text) => {
                    self.online = on_my_tracks(cx, crate::online::parse_ghosts(&text));
                    if self.tab == 0 && !self.online.is_empty() && self.describe.is_none() {
                        self.show(cx, 0);
                    }
                }
                Err(e) => eprintln!("the online ghosts: {e}"),
            }
        }
        if let Some(r) = self.fetching.as_ref().and_then(|f| crate::online::take(&f.0)) {
            let (_, path) = self.fetching.take().unwrap();
            let tape = r.and_then(|t| crate::online::parse_ghost(&t).ok_or_else(|| "not a ghost".to_string()));
            let written = tape.and_then(|t| {
                path.parent().map_or(Ok(()), crate::fs::create_dir_all).and_then(|_| crate::fs::write(&path, t.write())).map_err(|e| e.to_string())
            });
            self.base.show("Start", self.picked.is_some());
            match written {
                Ok(()) => return Go::Ghost(path),
                Err(e) => eprintln!("the online ghosts: {e}"),
            }
        }
        Go::Stay
    }

    fn layers(&mut self) -> Vec<&mut Dlg> {
        match &mut self.describe {
            Some(d) => vec![&mut self.base, &mut self.page, d],
            None => vec![&mut self.base, &mut self.page],
        }
    }

    fn modal(&self) -> bool {
        self.describe.is_some()
    }

    fn escape(&mut self, _cx: &mut Cx) -> Go {
        if self.describe.take().is_some() {
            return Go::Stay;
        }
        Go::Back
    }

    fn event(&mut self, cx: &mut Cx, e: Event) -> Go {
        if self.describe.is_some() {
            return self.describe_event(cx, e);
        }
        if let Event::Selected(n, line) = &e {
            // A line picked in one list picks it in them all.
            let online = if self.tab == 0 { self.online.len() } else { 0 };
            if LISTS.iter().any(|l| l.eq_ignore_ascii_case(n)) && *line < self.files.len() + online {
                for list in LISTS {
                    self.page.mark(list, Some(*line));
                }
                self.picked = Some(*line);
                // A server's ghost is only to race.
                let file = *line < self.files.len();
                self.page.enable("ButDelete", file);
                self.page.enable("ButDescription", file);
                self.base.show("Start", self.fetching.is_none());
            }
            return Go::Stay;
        }
        let Event::Clicked(n) = e else {
            return Go::Stay;
        };
        if n.eq_ignore_ascii_case("Back") {
            return Go::Back;
        }
        let picked = self.picked.and_then(|i| self.files.get(i)).cloned();
        if n.eq_ignore_ascii_case("Start") {
            if self.online_line().is_some() {
                if self.fetching.is_none() {
                    self.fetch(cx);
                }
                return Go::Stay;
            }
            if let Some(p) = picked {
                return if self.tab == 0 {
                    Go::Ghost(p)
                } else {
                    Go::Replay(p)
                };
            }
            return Go::Stay;
        }
        if n.eq_ignore_ascii_case("ButDescription") {
            self.describe = Self::describe_box(cx);
            return Go::Stay;
        }
        if n.eq_ignore_ascii_case("ButDelete") {
            if let Some(p) = picked {
                if let Err(err) = crate::fs::remove_file(&p) {
                    eprintln!("{}: {err}", p.display());
                }
                self.show(cx, self.tab);
            }
            return Go::Stay;
        }
        if let Some(tab) = TABS.iter().position(|t| t.0.eq_ignore_ascii_case(&n)) {
            self.show(cx, tab);
        }
        Go::Stay
    }
}
