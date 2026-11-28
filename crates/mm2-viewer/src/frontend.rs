//! The game's front end as the original lays it out: the main menu with its
//! pull-downs, the single player screen (event, bike and race tabs), and
//! the loading screen while a track loads.
//!
//! The layouts, art and sounds are the game's own (`UI/*.DAT`); what the
//! buttons do follows `Mcm2.exe` (main menu `0x44c1d0`, single player tabs
//! `0x44e600`, event page `0x44e830`, `0x44ee90` and `0x44f2c0`).

use crate::audio::Audio;
use crate::movie::Movie;
use crate::roster::{Ride, Roster};
use crate::screens::{self, profiles, Cx, Go};
use crate::ui::{color, Dlg, Event, Gfx, Ui};
use mm2_formats::{rs2, scn, tga};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The kinds of event, numbered as the game numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    Stunt = 0,
    Baja = 1,
    Nationals = 2,
    Supercross = 3,
    /// Moto-Tag, multiplayer only.
    Tag = 4,
    Enduro = 5,
}

impl EventKind {
    /// The order of the event page's drop-down, with each line's text in
    /// `LANG.DLL`.
    const LIST: [(EventKind, u32); 5] = [
        (EventKind::Baja, 5101),
        (EventKind::Stunt, 5102),
        (EventKind::Enduro, 5104),
        (EventKind::Nationals, 5103),
        (EventKind::Supercross, 5106),
    ];

    /// The event the game numbers `n`.
    pub fn from_number(n: u32) -> Option<EventKind> {
        Some(match n {
            0 => EventKind::Stunt,
            1 => EventKind::Baja,
            2 => EventKind::Nationals,
            3 => EventKind::Supercross,
            4 => EventKind::Tag,
            5 => EventKind::Enduro,
            _ => return None,
        })
    }

    /// Where the event's tracks are, under the game folder.
    pub fn folder(self) -> &'static str {
        match self {
            EventKind::Stunt => "TERAFORM/QUARRIES",
            EventKind::Baja => "TERAFORM/BAJA",
            EventKind::Nationals => "TERAFORM/NATIONAL",
            EventKind::Supercross => "TERAFORM/SX",
            EventKind::Enduro => "TERAFORM/ENDURO",
            EventKind::Tag => "TERAFORM/TAG",
        }
    }

    /// The loading screen's dialog.
    pub fn loading(self) -> &'static str {
        match self {
            EventKind::Stunt => "LOADQUAR",
            EventKind::Baja => "LOADBAJA",
            EventKind::Nationals => "LOADNAT",
            EventKind::Supercross => "LOADSX",
            EventKind::Enduro => "LOADEND",
            EventKind::Tag => "LOADTAG",
        }
    }

    /// Baja and enduro tracks hold several courses, each its own race.
    pub fn courses(self) -> bool {
        matches!(self, EventKind::Baja | EventKind::Enduro)
    }

    /// Laps start at 2 for the stadium and outdoor races, as in the
    /// original, or at 1 with the one-lap extra (`--one-lap`).
    fn fewest_laps(self) -> u32 {
        match self {
            EventKind::Nationals | EventKind::Supercross if !crate::tweaks::get().one_lap => 2,
            _ => 1,
        }
    }
}

/// The race modes, as the game numbers them, with their text.
const MODES: [(u32, u32); 4] = [(0, 5107), (1, 5108), (2, 5109), (4, 5110)];
pub const GHOST: u32 = 4;
pub const PRACTICE: u32 = 0;

/// What the player chose to race.
#[derive(Debug, Clone)]
pub struct Setup {
    pub event: EventKind,
    pub track: PathBuf,
    /// Which course of the track (Baja and enduro have several).
    pub course: usize,
    /// Laps, or minutes for the stunt event.
    pub laps: u32,
    pub opponents: u32,
    pub mode: u32,
    /// The track's name as the list shows it.
    pub name: String,
    /// The player's bike, outfit and number.
    pub ride: Option<Ride>,
    /// The computer riders' level, 0 to 2.
    pub level: usize,
    /// Each racer's name, the player's first; any left out take the
    /// game's list.
    pub names: Vec<String>,
    /// The computer riders' rides by racer (the player's is `ride`); any
    /// left out take the default grid.
    pub rides: Vec<Option<Ride>>,
    /// A ghost race's saved ghost to race (`Record\*.gho`); none records
    /// a new one.
    pub ghost: Option<PathBuf>,
    /// A network race: which racer is this player's, and which are other
    /// players' (the rest are the host's computer riders).
    pub net: Option<NetRace>,
    /// A Baja race on random gates (the event page's Random Gate box).
    pub random_gates: Option<crate::gates::RandomGates>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetRace {
    pub me: usize,
    pub humans: usize,
    pub host: bool,
    /// Moto-Tag with the ball, and in stunt mode.
    pub tag_ball: bool,
    pub tag_stunts: bool,
    /// A free roam session (multiplayer Practice): riders drop in and
    /// out, and places nobody rides in are empty.
    pub roam: bool,
}

/// One rider's line of the Race Info page: name, place, best lap and
/// total time.
pub type ResultLine = [String; 4];

/// The last race's lines for the Race Info page, and whether it was the
/// Stunt Quarry, which shows names and scores only (`0x4521b0`).
#[derive(Debug, Clone, Default)]
pub struct RaceResults {
    pub lines: Vec<ResultLine>,
    pub stunt: bool,
    /// Each racer's place and what the race ran up, by racer (the player
    /// first), for the Pro Circuit's money (`0x45d670`).
    pub placings: Vec<Placing>,
    /// The race's tape, for the View Replay button.
    pub replay: Option<PathBuf>,
}

/// One racer's end of a race.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Placing {
    /// From 1.
    pub place: usize,
    /// Repair, medical and stunt money units (bike +0x7ac, +0x7b0, +0x7b4).
    pub repair: f32,
    pub medical: f32,
    pub stunt: f32,
}

/// One line of the track list.
#[derive(Debug, Clone)]
pub struct TrackEntry {
    pub path: PathBuf,
    pub course: usize,
    pub name: String,
    /// The preview picture's name inside the track's archive.
    pub preview: String,
}

/// What the app should do after an input.
#[allow(clippy::large_enum_variant)]
pub enum Next {
    Stay,
    Quit,
    Race(Setup),
    /// A tape to play.
    Replay(PathBuf),
}

// There is only ever one screen, so its size does not matter.
#[allow(clippy::large_enum_variant)]
enum Screen {
    Main(Dlg),
    Single {
        base: Dlg,
        page: Dlg,
    },
    Loading(Dlg),
    /// One of the screens in `screens`: options, high scores, credits.
    Extra(Box<dyn screens::Screen>),
}

/// The event page's difficulty boxes, Easy to Hard: the computer riders'
/// level, 0 to 2.
const LEVELS: [&str; 3] = ["RadLODEasy", "RadLODMedium", "RadLODHard"];

/// Race mode 2: a series of races on one track for points.
pub const SERIES: u32 = 2;

/// A series' points for 1st to 10th (`0x45d670`); 11th scores none.
const SERIES_POINTS: [u32; 10] = [20, 17, 15, 12, 10, 8, 6, 5, 3, 1];

/// A series of races (race mode 2): the Races box's choice, the races
/// run and each racer's points. Under way from its first race until the
/// last one's results, leaving the mode or the main menu (`0x45d670`).
#[derive(Debug, Clone, Default)]
struct Series {
    /// The Races box's data: 2, 4 or 6 for 3, 5 or 7 races (+0x2d7c).
    races: u32,
    /// Races run so far (championship +0x48).
    run: u32,
    /// Each racer's points, the player first.
    points: Vec<u32>,
    /// Under way (+0x1664).
    on: bool,
}

impl Series {
    /// The racers best first: most points, then by name (`0x45d540`).
    fn ranked(points: &[u32], names: &[String]) -> Vec<usize> {
        let mut order: Vec<usize> = (0..points.len()).collect();
        order.sort_by(|&a, &b| points[b].cmp(&points[a]).then_with(|| names.get(a).cmp(&names.get(b))));
        order
    }

    /// The points after a race whose finishing order is `standings`.
    fn with_race(&self, standings: &[usize]) -> Vec<u32> {
        let mut points = self.points.clone();
        for (place, &racer) in standings.iter().enumerate() {
            if racer >= points.len() {
                points.resize(racer + 1, 0);
            }
            points[racer] += SERIES_POINTS.get(place).copied().unwrap_or(0);
        }
        points
    }
}

pub struct Frontend {
    ui: Ui,
    screen: Screen,
    event: EventKind,
    mode: u32,
    laps: u32,
    opponents: u32,
    /// The Easy, Medium or Hard box on the event page, 0 to 2.
    level: usize,
    track: usize,
    tracks: HashMap<EventKind, Vec<TrackEntry>>,
    /// Which tracks the event page lists with the track filter extra:
    /// 0 all, 1 the game's own, 2 custom ones.
    track_source: usize,
    /// The last race's results, for the Race Info page.
    results: RaceResults,
    /// The series under way (race mode 2), and the Race Info page's
    /// title for its last race.
    series: Series,
    series_title: Option<String>,
    /// The bikes and outfits on offer, and the player's picks.
    roster: Roster,
    bike: usize,
    /// The player has picked a bike on the Bike & Rider page.
    bike_picked: bool,
    outfit: usize,
    /// The Kit & Paint page shows in the Bike & Rider page's place.
    paint_page: bool,
    /// The Kit & Paint page's colours, as places in freepack::COLOURS, in
    /// PAINT_ROWS' order (kit, kit trim, bike, bike trim, helmet); used
    /// while `paint_on`.
    paint: [usize; 5],
    paint_on: bool,
    plate: String,
    /// The plate number box has the keyboard.
    typing: bool,
    /// The event page's Random Gate box (Baja): ticked, how many gates
    /// and the race number (0x44f5a0).
    random_gates: bool,
    gate_count: u32,
    seed: u32,
    /// The player's profile and the boxes the main menu's profile
    /// buttons open.
    user: screens::User,
    boxes: profiles::Profiles,
    /// After a ghost race whose best lap beat the ghost: "Do you want to
    /// save the current ghost lap?" over the Race Info page, and the lap.
    /// A box over the screen: keep a ghost race's lap (with the lap), or
    /// a notice with one button (none).
    ghost_ask: Option<(Dlg, Option<mm2_formats::vcr::Tape>)>,
    /// The screen a race was started from (the Pro Circuit), to go back
    /// to with its results.
    parked: Option<Box<dyn screens::Screen>>,
    /// The single player screen while its Options or Joystick button's
    /// options are up, to go back to.
    beneath: Option<Screen>,
    /// The main menu's little screen of event clips (`UI/EVENTS.AVI`),
    /// and how long the menu has waited with no pull-down open.
    events: Option<Movie>,
    events_wait: f32,
    now: f32,
    /// The ghost race list's files (after its first line, "Record a new
    /// ghost race"), and the line picked.
    ghosts: Vec<PathBuf>,
    ghost: usize,
    /// What a screen asked between inputs (a network race starting).
    pending: Option<Next>,
    /// Where the update note was drawn last, and its ×, for clicks.
    note_at: Option<([f32; 4], [f32; 4])>,
}

/// The Bike & Rider page's engine sizes (`0x44fe50`), for the bikes made
/// from the player's own paint (`Bikes/*.tga`); the makers' bikes come
/// with theirs and leave the list disabled.
const ENGINE_SIZES: [(u32, &str); 5] = [
    (125, "125cc 2-stroke"),
    (250, "250cc 2-stroke"),
    (400, "400cc 4-stroke"),
    (500, "500cc 2-stroke"),
    (600, "600cc 4-stroke"),
];

/// Where the single player screen's tab pages go (`0x44e600`).
const PAGE: (f32, f32) = (16.0, 41.0);

/// How far up the event page's preview, track and difficulty move to
/// make room for the time and weather row (the fork's), px.
const SKY_ROOM: i32 = 26;

/// Adds the time of day and weather row (the fork's, for the enhanced
/// graphics) to the event page, under the difficulty: the preview a row
/// shorter and what was under it moved up, then a drop-down list for each,
/// like the Event Type's.
fn sky_row(page: &mut Dlg) {
    for c in &mut page.dialog.controls {
        if c.anchor.is_some() {
            continue;
        }
        match c.name.as_str() {
            "PictureBox" => c.rect[3] -= SKY_ROOM,
            "CornerLL" | "CornerLR" | "DDLTextBox" | "TrackLeft" | "TrackRight" | "GhostBox" | "rbwLabel2" | "RadLODEasy" | "RadLODMedium" | "RadLODHard" => {
                c.rect[1] -= SKY_ROOM
            }
            _ => {}
        }
    }
    // The Event Type box's own look: a box with its arrow, a list under it.
    let y = 272;
    page.add_copy("rbwLabel2", "TimeLabel", [24, y + 1, 100, 18]);
    page.add_copy("EventTypeDropDown", "TimeList", [126, y - 3, 129, 27]);
    page.add_copy("rbwLabel2", "WeatherLabel", [262, y + 1, 66, 18]);
    page.add_copy("EventTypeDropDown", "WeatherList", [330, y - 3, 129, 27]);
    page.set_text("TimeLabel", "Time of Day:");
    page.set_text("WeatherLabel", "Weather:");
}

/// The Kit & Paint page's rows: list, label, and top in the page.
const PAINT_ROWS: [(&str, &str, i32); 5] = [
    ("PaintKit", "Kit", 4),
    ("PaintKitTrim", "Kit trim", 27),
    ("PaintBike", "Bike", 274),
    ("PaintBikeTrim", "Bike trim", 297),
    ("PaintHelmet", "Helmet", 320),
];

fn title(name: &str) -> String {
    let mut c = name.chars();
    c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
}

/// The track filter's choices (the extra's).
const TRACK_SOURCES: [&str; 3] = ["All tracks", "Base game", "Custom"];

/// The track filter (an extra): the track list a little narrower, and a
/// drop-down list beside it for all tracks, the game's own or custom ones.
fn filter_row(page: &mut Dlg) {
    let Some(i) = page.index("DDLTextBox") else {
        return;
    };
    let at = page.dialog.controls[i].rect;
    let width = 128;
    page.dialog.controls[i].rect[2] = at[2] - width - 4;
    page.add_copy("EventTypeDropDown", "TrackSource", [at[0] + at[2] - width, at[1] - 3, width, 27]);
}

/// The tracks that came with the game, by event (the rest are custom).
pub fn is_stock(event: EventKind, path: &Path) -> bool {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_uppercase();
    let numbered = |prefix: &str, most: u32| {
        stem.strip_prefix(prefix)
            .filter(|n| n.len() == 2)
            .and_then(|n| n.parse::<u32>().ok())
            .is_some_and(|n| (1..=most).contains(&n))
    };
    match event {
        EventKind::Stunt => numbered("QUARRY", 5),
        EventKind::Baja => numbered("BAJA", 5),
        EventKind::Nationals => numbered("NAT", 12),
        EventKind::Supercross => numbered("SX", 17),
        EventKind::Tag => numbered("TAG", 3),
        EventKind::Enduro => ["AIRFIELD", "FARM", "OPENPIT", "SKILODGE", "TRAILER"].contains(&stem.as_str()),
    }
}

/// The time of day and weather now, as places in mm2_settings::TIMES and
/// WEATHERS.
fn sky_now() -> (usize, usize) {
    let Some(e) = crate::graphics::enhanced() else {
        return (0, 0);
    };
    let time = mm2_settings::TIMES
        .iter()
        .position(|t| e.time.is_some_and(|h| crate::graphics::hour(t.0) == Some(h)))
        .unwrap_or(0);
    let weather = mm2_settings::WEATHERS.iter().position(|w| crate::graphics::weather(w.0) == Some(e.weather)).unwrap_or(0);
    (time, weather)
}

/// Picks the time of day and weather (mm2_settings::TIMES and WEATHERS'
/// keys) from the next race on, and saves them for the launcher and the
/// next start, if they changed.
fn set_sky(time: &str, weather: &str) {
    let hour = crate::graphics::hour(time);
    let w = crate::graphics::weather(weather).unwrap_or(crate::graphics::Weather::Clear);
    if crate::graphics::enhanced().is_none_or(|e| e.time == hour && e.weather == w) {
        return;
    }
    crate::graphics::set_sky(hour, w);
    let mut s = mm2_settings::Settings::load();
    s.time = time.to_string();
    s.weather = weather.to_string();
    if let Err(e) = s.save() {
        eprintln!("couldn't save the time and weather to {}: {e}", mm2_settings::Settings::path().display());
    }
}

/// The menus' rows of tabs, for the pad's shoulder buttons (the fork's):
/// single player and the lobby, the options, the career, the records'
/// events, and the ghosts and replays.
const TAB_ROWS: [&[&str]; 5] = [
    &["EventTab", "BikeRiderTab", "RaceInfoTab", "OptionsTab"],
    &["GameSettings", "Graphics", "Sound", "Controls", "Messages", "Garage"],
    &["TabNext", "TabBikeRider", "TabStandings"],
    &["TabQuarry", "TabBaja", "TabNationals", "TabSupercross", "TabTag", "TabEnduro"],
    &["TabLeft", "TabMiddle", "TabRight"],
];

impl Frontend {
    pub fn new(game: &Path) -> Option<Frontend> {
        let mut ui = Ui::new(game);
        let main = Dlg::open(&mut ui, "MAINMENU", (0.0, 0.0))?;
        let user = screens::User::load(&ui);
        ui.volume = user.profile.sound.ui_gain();
        let mut f = Frontend {
            ui,
            screen: Screen::Main(main),
            event: EventKind::Supercross,
            mode: 1,
            laps: 3,
            opponents: 3,
            level: 1,
            track: 0,
            tracks: HashMap::new(),
            track_source: 0,
            results: RaceResults::default(),
            series: Series { races: 2, ..Series::default() },
            series_title: None,
            roster: Roster::load(game),
            bike: 0,
            bike_picked: false,
            outfit: 0,
            paint_page: false,
            paint: [1, 10, 1, 10, 1],
            paint_on: false,
            plate: "107".into(),
            typing: false,
            random_gates: false,
            gate_count: crate::gates::RandomGates::FEWEST,
            seed: 0,
            user,
            boxes: profiles::Profiles::default(),
            ghost_ask: None,
            parked: None,
            beneath: None,
            events: None,
            events_wait: 0.0,
            now: 0.0,
            ghosts: Vec::new(),
            ghost: 0,
            pending: None,
            note_at: None,
        };
        f.fill_main();
        Some(f)
    }

    /// The main menu's profile list; the name box first if there is no
    /// profile yet (`0x44c1d0` open).
    fn fill_main(&mut self) {
        let Screen::Main(d) = &mut self.screen else {
            return;
        };
        let mut cx = Cx {
            ui: &mut self.ui,
            audio: None,
            user: &mut self.user,
            now: self.now,
        };
        profiles::fill(&mut cx, d);
        // The connections DirectPlay offered (`0x44c1d0`): the port has
        // its own TCP/IP play, and the Gaming Zone's place (long closed)
        // opens the online races: the server browser.
        d.set_tip("ButZone", "Online races: join races and free roam servers from everywhere, or host your own");
        d.set_tip("ButTCP", "Races on your home network or online, or join a friend by race code");
        if crate::demo::active() {
            for (name, tip) in crate::demo::event_tips() {
                d.set_tip(name, tip);
            }
        }
        for (name, on) in [
            ("ButTCP", true),
            ("ButIPX", false),
            ("ButZone", true),
            ("ButCable", false),
            ("ButModem", false),
        ] {
            d.enable(name, on);
        }
        self.boxes.start(&mut cx);
    }

    /// The logos before the main menu, unless the player's profile
    /// turned the intro off.
    pub fn intro(&mut self) {
        if !self.user.profile.game.intro_movie && !self.user.profile.name.is_empty() {
            return;
        }
        if let Some(i) = screens::intro::Intro::open(&mut self.ui) {
            self.screen = Screen::Extra(Box::new(i));
        }
    }

    /// The main menu's clips play on the screen of the Single Player
    /// pull-down: four seconds after it is all the way down they start,
    /// and go round until it goes up again (`0x44c1d0`, `0x44d6f0`,
    /// `0x44c190`).
    fn events_tick(&mut self, dt: f32) {
        let Screen::Main(d) = &self.screen else {
            self.events = None;
            self.events_wait = 0.0;
            return;
        };
        let down = d.is_down("MenuSingleAnimation");
        let audio = self.ui.audio.as_ref();
        let events = self.events.get_or_insert_with(|| {
            Movie::open(&self.ui.game().join("UI/EVENTS.AVI"), true).unwrap_or_else(Movie::none)
        });
        if !down {
            events.stop();
            self.events_wait = 0.0;
        } else if events.playing() {
            events.tick(audio, self.now);
        } else {
            self.events_wait += dt;
            if self.events_wait >= 4.0 {
                events.play(audio, self.now);
            }
        }
    }

    /// The mixer, for the movies.
    pub fn set_audio(&mut self, audio: &Audio) {
        self.ui.audio = Some(audio.share());
    }

    /// The player's profile.
    /// The menus' pictures and texts, for drawing over them.
    pub fn ui_mut(&mut self) -> &mut Ui {
        &mut self.ui
    }

    pub fn user(&self) -> &screens::User {
        &self.user
    }

    pub fn user_mut(&mut self) -> &mut screens::User {
        &mut self.user
    }

    /// Back to the main menu (after a race), or to the screen the race
    /// was started from.
    pub fn main_menu(&mut self) {
        self.beneath = None;
        self.series.on = false;
        if self.unpark(None) {
            return;
        }
        if let Some(main) = Dlg::open(&mut self.ui, "MAINMENU", (0.0, 0.0)) {
            self.screen = Screen::Main(main);
            self.fill_main();
        }
    }

    fn dialogs(&mut self) -> Vec<&mut Dlg> {
        if let Some((d, _)) = &mut self.ghost_ask {
            return vec![d];
        }
        match &mut self.screen {
            Screen::Main(d) => match self.boxes.dialog() {
                Some(b) => vec![b],
                None => vec![d],
            },
            Screen::Loading(d) => vec![d],
            Screen::Single { base, page } => vec![base, page],
            Screen::Extra(s) => screens::inputs(s.as_mut()),
        }
    }

    /// What the box that has the keyboard holds (a profile's name, the
    /// race number, the plate), or None when no box is being typed in:
    /// a phone shows its keyboard for it.
    pub fn typing_text(&mut self) -> Option<String> {
        if self.typing {
            return Some(self.plate.clone());
        }
        self.dialogs().iter().find_map(|d| d.editing_text())
    }

    pub fn mouse_move(&mut self, audio: &Audio, x: f32, y: f32) {
        let mut events = Vec::new();
        for d in self.dialogs() {
            events.extend(d.mouse_move(x, y));
        }
        self.sounds(audio, &events);
        // A slider being dragged.
        for e in events {
            if matches!(e, Event::Changed(_)) {
                self.extra(audio, e);
            }
        }
    }

    /// An event for the screen in `screens` that is up, or the main
    /// menu's profile boxes.
    fn extra(&mut self, audio: &Audio, e: Event) -> Next {
        let mut cx = Cx {
            ui: &mut self.ui,
            audio: Some(audio),
            user: &mut self.user,
            now: self.now,
        };
        match &mut self.screen {
            Screen::Extra(s) => {
                let go = s.event(&mut cx, e);
                self.go(go)
            }
            Screen::Main(d) => {
                self.boxes.event(&mut cx, d, e);
                Next::Stay
            }
            _ => Next::Stay,
        }
    }

    fn go(&mut self, go: Go) -> Next {
        match go {
            Go::Stay => Next::Stay,
            Go::Quit => Next::Quit,
            Go::Back => {
                match self.beneath.take() {
                    Some(s) => self.screen = s,
                    None => self.main_menu(),
                }
                Next::Stay
            }
            Go::Open(s) => {
                self.screen = Screen::Extra(s);
                Next::Stay
            }
            Go::Race(setup) => {
                if let Some(d) = Dlg::open(&mut self.ui, setup.event.loading(), (0.0, 0.0)) {
                    if let Screen::Extra(s) =
                        std::mem::replace(&mut self.screen, Screen::Loading(d))
                    {
                        self.parked = Some(s);
                    }
                }
                Next::Race(*setup)
            }
            Go::Replay(path) => Next::Replay(path),
            Go::Ghost(path) => match self.ghost_setup(&path) {
                Some(setup) => self.go(Go::Race(Box::new(setup))),
                None => Next::Stay,
            },
        }
    }

    /// A ghost race against the ghost in `path`: its track and course, the
    /// player on their bike.
    fn ghost_setup(&mut self, path: &Path) -> Option<Setup> {
        let tape = mm2_formats::vcr::Tape::parse(&crate::fs::read(path).ok()?).ok()?;
        let event = EventKind::from_number(tape.setting(mm2_formats::vcr::setting::EVENT))?;
        let (stem, course) = (tape.track(), tape.course());
        let entry = self
            .tracks(event)
            .iter()
            .filter(|e| {
                e.path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case(&stem))
            })
            .find(|e| e.course == course || !event.courses())?
            .clone();
        Some(Setup {
            event,
            track: entry.path,
            course: entry.course,
            laps: 99,
            opponents: 0,
            mode: GHOST,
            name: entry.name,
            ride: self.ride(),
            level: 1,
            names: vec![self.user.profile.name.clone()],
            rides: Vec::new(),
            ghost: Some(path.to_path_buf()),
            net: None,
            random_gates: None,
        })
    }

    /// What a screen asked since the last input.
    pub fn take_next(&mut self) -> Option<Next> {
        self.pending.take()
    }

    /// The loading screen of `event`'s tracks, while a tape's track loads.
    pub fn loading(&mut self, event: EventKind) {
        if let Some(d) = Dlg::open(&mut self.ui, event.loading(), (0.0, 0.0)) {
            if let Screen::Extra(s) = std::mem::replace(&mut self.screen, Screen::Loading(d)) {
                self.parked = Some(s);
            }
        }
    }

    /// Back from a replay: to the screen it was started from, else the
    /// Race Info page it was viewed from.
    pub fn replay_over(&mut self) {
        if self.unpark(None) {
            return;
        }
        self.open_single();
        self.show_tab("raceinfotab");
    }

    /// Back to the screen a race was started from, with its results (none
    /// if the race was left).
    fn unpark(&mut self, results: Option<&RaceResults>) -> bool {
        let Some(mut s) = self.parked.take() else {
            return false;
        };
        let mut cx = Cx {
            ui: &mut self.ui,
            audio: None,
            user: &mut self.user,
            now: self.now,
        };
        s.race_over(&mut cx, results);
        self.screen = Screen::Extra(s);
        true
    }

    /// A key pressed, for the controls page.
    pub fn key(&mut self, audio: &Audio, key: winit::keyboard::KeyCode) -> Next {
        let Some(code) = screens::keys::scan_code(key) else {
            return Next::Stay;
        };
        let Screen::Extra(s) = &mut self.screen else {
            return Next::Stay;
        };
        let mut cx = Cx {
            ui: &mut self.ui,
            audio: Some(audio),
            user: &mut self.user,
            now: self.now,
        };
        let go = s.key(&mut cx, code);
        self.go(go)
    }

    /// Whether the screen up waits for a joystick input to bind.
    #[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
    pub fn wants_pad(&self) -> bool {
        matches!(&self.screen, Screen::Extra(s) if s.wants_pad())
    }

    /// A joystick button or axis, for the controls page.
    #[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
    pub fn pad_input(&mut self, audio: &Audio, input: (i32, i32)) -> Next {
        let Screen::Extra(s) = &mut self.screen else {
            return Next::Stay;
        };
        let mut cx = Cx {
            ui: &mut self.ui,
            audio: Some(audio),
            user: &mut self.user,
            now: self.now,
        };
        let go = s.pad(&mut cx, input);
        self.go(go)
    }

    /// Whether the screen up is one of `screens`, or a box is open over
    /// the main menu.
    fn extra_up(&self) -> bool {
        matches!(self.screen, Screen::Extra(_))
            || matches!(self.screen, Screen::Main(_)) && self.boxes.is_open()
    }

    pub fn mouse_down(&mut self, x: f32, y: f32) {
        for d in self.dialogs() {
            d.mouse_down(x, y);
        }
    }

    pub fn wheel(&mut self, lines: i32) {
        for d in self.dialogs() {
            d.wheel(lines);
        }
    }

    pub fn mouse_up(&mut self, audio: &Audio, x: f32, y: f32, now: f32) -> Next {
        // The update note, over the main menu.
        if let Some((note, close)) = self.note_at {
            let inside = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
            if inside(close) {
                crate::update_note::close();
                self.note_at = None;
                return Next::Stay;
            }
            if inside(note) {
                return if crate::update_note::open() { Next::Quit } else { Next::Stay };
            }
        }
        // An open drop-down list takes the click, wherever it lands.
        let mut events = Vec::new();
        let dialogs = self.dialogs();
        let with_list = dialogs.iter().position(|d| d.list_open());
        for (i, d) in dialogs.into_iter().enumerate() {
            if with_list.is_none_or(|l| l == i) {
                events.extend(d.mouse_up(x, y));
            }
        }
        self.handle(audio, events, now)
    }

    /// Where the pad's D-pad can jump the pointer to (see
    /// [`Dlg::targets`]).
    #[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
    pub fn targets(&mut self) -> Vec<(f32, f32)> {
        self.dialogs().iter().flat_map(|d| d.targets()).collect()
    }

    /// The pad's shoulder buttons: the next tab along (`step` 1) or the
    /// one before (−1), as clicked.
    #[cfg_attr(not(feature = "gamepad"), allow(dead_code))]
    pub fn step_tab(&mut self, audio: &Audio, step: i32, now: f32) -> Next {
        if self.ghost_ask.is_some() || self.dialogs().iter().any(|d| d.list_open()) {
            return Next::Stay;
        }
        let events = self.dialogs().into_iter().find_map(|d| {
            TAB_ROWS.iter().find_map(|row| d.step_tab(row, step))
        });
        match events {
            Some(events) => self.handle(audio, events, now),
            None => Next::Stay,
        }
    }

    /// What a click's events do.
    fn handle(&mut self, audio: &Audio, events: Vec<Event>, now: f32) -> Next {
        self.sounds(audio, &events);
        if let Screen::Extra(s) = &mut self.screen {
            let mut cx = Cx {
                ui: &mut self.ui,
                audio: Some(audio),
                user: &mut self.user,
                now,
            };
            let go = s.pressed(&mut cx);
            if !matches!(go, Go::Stay) {
                return self.go(go);
            }
        }
        let mut next = Next::Stay;
        for e in events {
            // Every screen's Help button: the help page at that screen.
            if let Event::Clicked(name) = &e {
                if crate::help::is_help(name) && self.ghost_ask.is_none() {
                    crate::help::open(&self.help_pack());
                    continue;
                }
            }
            if self.ghost_ask.is_some() {
                if let Event::Clicked(name) = &e {
                    self.ghost_answer(name);
                }
                continue;
            }
            if self.extra_up() {
                next = self.extra(audio, e);
                if !matches!(next, Next::Stay) {
                    break;
                }
                continue;
            }
            next = match e {
                Event::Clicked(name) => self.clicked(audio, &name, now),
                Event::Selected(name, line) => {
                    self.selected(&name, line);
                    Next::Stay
                }
                Event::Sound(_) | Event::Changed(_) | Event::Released(_) => Next::Stay,
            };
            if !matches!(next, Next::Stay) {
                break;
            }
        }
        next
    }

    /// Esc: closes a list or a pull-down, or goes back a screen.
    pub fn escape(&mut self, now: f32) -> Next {
        match &mut self.screen {
            Screen::Main(_) if self.boxes.is_open() => self.boxes.escape(),
            Screen::Extra(s) => {
                let mut cx = Cx {
                    ui: &mut self.ui,
                    audio: None,
                    user: &mut self.user,
                    now,
                };
                let go = s.escape(&mut cx);
                return self.go(go);
            }
            Screen::Main(d) => {
                let open: Vec<_> = PULL_DOWNS.iter().filter(|(_, p)| d.is_open(p)).collect();
                if open.is_empty() {
                    return Next::Quit;
                }
                let animate = self.user.profile.game.ui_anims;
                for (_, p) in open {
                    d.pull(p, false, now, animate);
                }
            }
            _ if self.ghost_ask.is_some() => self.ghost_ask = None,
            Screen::Single { base, page } => {
                if page.list_open() || base.list_open() {
                    page.close_list();
                    base.close_list();
                } else {
                    self.main_menu();
                }
            }
            Screen::Loading(_) => {}
        }
        Next::Stay
    }

    /// The dialog Help was pressed on: the single player screen's page,
    /// else the screen's top dialog.
    fn help_pack(&mut self) -> String {
        match &mut self.screen {
            Screen::Main(d) => d.pack.clone(),
            Screen::Loading(d) => d.pack.clone(),
            Screen::Single { page, .. } => page.pack.clone(),
            Screen::Extra(s) => s.pack(),
        }
    }

    fn sounds(&mut self, audio: &Audio, events: &[Event]) {
        let pack = match &mut self.screen {
            Screen::Main(d) => match self.boxes.dialog() {
                Some(b) => b.pack.clone(),
                None => d.pack.clone(),
            },
            Screen::Loading(d) => d.pack.clone(),
            Screen::Single { base, .. } => base.pack.clone(),
            Screen::Extra(s) => s.pack(),
        };
        for e in events {
            if let Event::Sound(s) = e {
                self.ui.play(audio, &pack, s);
            }
        }
    }

    fn clicked(&mut self, audio: &Audio, name: &str, now: f32) -> Next {
        let name = name.to_ascii_lowercase();
        match &mut self.screen {
            Screen::Main(d) => {
                // A button the free pack adds.
                let action = crate::freepack::ui_action("MAINMENU", &name);
                if let Some(url) = action.and_then(|a| a.strip_prefix("open_url:")) {
                    crate::update_note::open_page(url);
                    return Next::Stay;
                }
                // The Course Builder's link (or one the free pack adds).
                if name.eq_ignore_ascii_case(crate::menu_link::NAME) || action == Some("course_builder") {
                    crate::builder::ask_open();
                    return Next::Stay;
                }
                if let Some((_, pull)) = PULL_DOWNS.iter().find(|(b, _)| *b == name) {
                    // One pull-down at a time.
                    let open = !d.is_open(pull);
                    let animate = self.user.profile.game.ui_anims;
                    let mut events = Vec::new();
                    for (_, other) in PULL_DOWNS.iter().filter(|(_, p)| p != pull) {
                        events.extend(d.pull(other, false, now, animate));
                    }
                    events.extend(d.pull(pull, open, now, animate));
                    self.sounds(audio, &events);
                    return Next::Stay;
                }
                let mut cx = Cx {
                    ui: &mut self.ui,
                    audio: Some(audio),
                    user: &mut self.user,
                    now,
                };
                if self.boxes.clicked(&mut cx, d, &name) {
                    return Next::Stay;
                }
                let event = match name.as_str() {
                    "menuexit" => return Next::Quit,
                    "menuhighscore" => {
                        if let Some(h) = screens::records::HighScores::open(&mut cx) {
                            self.screen = Screen::Extra(Box::new(h));
                        }
                        return Next::Stay;
                    }
                    "butzone" => {
                        if let Some(m) = screens::multi::Multi::open_online(&mut cx) {
                            self.screen = Screen::Extra(Box::new(m));
                        }
                        return Next::Stay;
                    }
                    "buttcp" => {
                        if let Some(m) = screens::multi::Multi::open(&mut cx) {
                            self.screen = Screen::Extra(Box::new(m));
                        }
                        return Next::Stay;
                    }
                    "menucredits" => {
                        if let Some(c) = screens::credits::Credits::open(&mut cx) {
                            self.screen = Screen::Extra(Box::new(c));
                        }
                        return Next::Stay;
                    }
                    "menureplay" => {
                        if let Some(r) = screens::replays::Replays::open(&mut cx) {
                            self.screen = Screen::Extra(Box::new(r));
                        }
                        return Next::Stay;
                    }
                    "menuoptions" => {
                        let o = screens::options::Options::open(&mut cx, 0);
                        if let Some(o) = o {
                            self.screen = Screen::Extra(Box::new(o));
                        }
                        return Next::Stay;
                    }
                    "butstuntquarry" => EventKind::Stunt,
                    "butbaja" => EventKind::Baja,
                    "butprocircuit" => {
                        if let Some(p) = screens::pro::Pro::open(&mut cx) {
                            self.screen = Screen::Extra(Box::new(p));
                        }
                        return Next::Stay;
                    }
                    "butnationals" => EventKind::Nationals,
                    "butsupercross" => EventKind::Supercross,
                    "butenduro" => EventKind::Enduro,
                    "butcancelprofile" => {
                        let events = d.pull("MenuUserAnimation", false, now, self.user.profile.game.ui_anims);
                        self.sounds(audio, &events);
                        return Next::Stay;
                    }
                    _ => return Next::Stay,
                };
                if self.event != event {
                    self.track = 0;
                }
                self.event = event;
                self.open_single();
            }
            Screen::Single { .. } => match name.as_str() {
                "back" => self.main_menu(),
                "start" => return self.start(),
                "butviewreplay" => {
                    if let Some(p) = self.results.replay.clone() {
                        return Next::Replay(p);
                    }
                }
                "eventtab" | "bikeridertab" | "raceinfotab" => self.show_tab(&name),
                // The options, at the controls page for Joystick or the
                // Garage for the Bike & Rider page's wrench (Tune), and back
                // here after.
                "options" | "joystick" | "butwrench" => {
                    let at = match name.as_str() {
                        "joystick" => 3,
                        "butwrench" => 5,
                        _ => 0,
                    };
                    let mut cx = Cx {
                        ui: &mut self.ui,
                        audio: Some(audio),
                        user: &mut self.user,
                        now,
                    };
                    if let Some(o) = screens::options::Options::open(&mut cx, at) {
                        let single = std::mem::replace(&mut self.screen, Screen::Extra(Box::new(o)));
                        self.beneath = Some(single);
                    }
                }
                _ => self.page_clicked(&name),
            },
            Screen::Loading(_) | Screen::Extra(_) => {}
        }
        Next::Stay
    }

    /// Back from a race: the single player screen, on the Race Info page
    /// with these results.
    /// Asks whether to keep a ghost race's new best lap (lang 5184, Yes
    /// and No).
    pub fn ask_ghost(&mut self, lap: mm2_formats::vcr::Tape) {
        let title = self.ui.lang(5320);
        let prompt = self.ui.lang(5184);
        if let Some(b) = screens::MsgBox::ask(&mut self.ui, &title, &prompt, [5182, 0, 5181]) {
            self.ghost_ask = Some((b.dlg, Some(lap)));
        }
    }

    fn ghost_answer(&mut self, button: &str) {
        let button = button.to_ascii_lowercase();
        // A notice: its one button closes it.
        if matches!(self.ghost_ask, Some((_, None))) {
            if button == "butmiddle" {
                self.ghost_ask = None;
            }
            return;
        }
        if button != "butleft" && button != "butright" {
            return;
        }
        if let (Some((_, Some(lap))), "butleft") = (self.ghost_ask.take(), button.as_str()) {
            match crate::replay::save_ghost(&lap, &self.user.store.dir) {
                Ok(p) => eprintln!("kept the ghost lap as {}", p.display()),
                Err(e) => eprintln!("keeping the ghost lap: {e}"),
            }
        }
    }

    pub fn race_over(&mut self, results: RaceResults) {
        if self.unpark(Some(&results)) {
            return;
        }
        let mut results = results;
        self.series_title = None;
        if self.mode == SERIES && self.series.on {
            self.series_race(&mut results);
        }
        self.results = results;
        self.open_single();
        self.show_tab("raceinfotab");
    }

    /// A series race's results (`0x45d670`, `0x4521b0`): each racer's
    /// points for the place, then the lines in the series' order, with
    /// that place and the points so far; the title counts the races, and
    /// after the last one the series is over.
    fn series_race(&mut self, results: &mut RaceResults) {
        let mut line_of = vec![usize::MAX; results.placings.len()];
        let mut standings: Vec<(usize, usize)> = Vec::new();
        for (racer, p) in results.placings.iter().enumerate() {
            if p.place >= 1 && p.place <= results.lines.len() {
                line_of[racer] = p.place - 1;
                standings.push((p.place, racer));
            }
        }
        standings.sort();
        let standings: Vec<usize> = standings.into_iter().map(|s| s.1).collect();
        let s = &mut self.series;
        s.points = s.with_race(&standings);
        s.run += 1;
        let names: Vec<String> = (0..s.points.len())
            .map(|r| line_of.get(r).and_then(|&l| results.lines.get(l)).map_or(String::new(), |l| l[0].clone()))
            .collect();
        let order = Series::ranked(&s.points, &names);
        let mut lines = Vec::new();
        for (rank, &racer) in order.iter().enumerate() {
            let Some(mut line) = line_of.get(racer).and_then(|&l| results.lines.get(l)).cloned() else {
                continue;
            };
            if !results.stunt {
                line[1] = (rank + 1).to_string();
            }
            line[3] = s.points[racer].to_string();
            lines.push(line);
            if let Some(p) = results.placings.get_mut(racer) {
                p.place = lines.len();
            }
        }
        results.lines = lines;
        let (run, races) = (s.run, s.races);
        self.series_title = Some(if races < run {
            self.series.on = false;
            self.ui.lang(2362)
        } else {
            format!("{} ({run}/{})", self.ui.lang(2363), races + 1)
        });
    }

    /// In a series past its first race, each racer's place on the start
    /// grid, from 0: the most points first, ties as they stand
    /// (`0x45d5a0`).
    pub fn series_grid(&self) -> Option<Vec<usize>> {
        let s = &self.series;
        if self.mode != SERIES || !s.on || s.run == 0 {
            return None;
        }
        // A stable sort, as the game's bubble sort.
        let mut order: Vec<usize> = (0..s.points.len()).collect();
        order.sort_by(|&a, &b| s.points[b].cmp(&s.points[a]));
        let mut slot = vec![0; order.len()];
        for (k, &racer) in order.iter().enumerate() {
            slot[racer] = k;
        }
        Some(slot)
    }

    /// Whether a series is under way, and, when the race just run whose
    /// finishing order is `standings` is its last, the series' order for
    /// the podium (`0x45f610`).
    pub fn series_end(&self, standings: &[usize], names: &[String]) -> Option<Option<Vec<usize>>> {
        let s = &self.series;
        if self.mode != SERIES || !s.on {
            return None;
        }
        Some((s.races < s.run + 1).then(|| Series::ranked(&s.with_race(standings), names)))
    }

    /// Switches the single player screen's page.
    fn show_tab(&mut self, name: &str) {
        let Screen::Single { base, .. } = &mut self.screen else {
            return;
        };
        for tab in ["EventTab", "BikeRiderTab", "RaceInfoTab"] {
            base.set_on(tab, tab.eq_ignore_ascii_case(name));
        }
        let pack = match name {
            "eventtab" => "SPEVENT",
            "bikeridertab" => "SPBIKER",
            _ => "SPRINFO",
        };
        if let Some(p) = Dlg::open(&mut self.ui, pack, PAGE) {
            if let Screen::Single { page, .. } = &mut self.screen {
                *page = p;
            }
            self.typing = false;
            match pack {
                "SPEVENT" => self.fill_event_page(),
                "SPRINFO" => self.fill_race_info(),
                _ => self.fill_bike_page(),
            }
        }
    }

    /// The game's folder.
    pub fn game(&self) -> Option<&Path> {
        Some(self.ui.game())
    }

    /// While the Bike & Rider page shows: the ride to turn in its window,
    /// and whether Auto Rotate is on.
    pub fn preview(&self) -> Option<(Ride, bool)> {
        match &self.screen {
            Screen::Single { page, .. } if page.index("DDLBikes").is_some() => {
                let turning = page.get("ChkAutoRotate").is_none_or(|c| c.on);
                Some((self.ride()?, turning))
            }
            Screen::Extra(s) => s.preview().map(|p| (p.0, p.1)),
            _ => None,
        }
    }

    /// The bike and rider were dragged round: Auto Rotate goes off
    /// (0x44fae0).
    pub fn stop_turning(&mut self) {
        match &mut self.screen {
            Screen::Single { page, .. } if page.index("DDLBikes").is_some() => page.set_on("ChkAutoRotate", false),
            Screen::Extra(s) => s.stop_turning(),
            _ => {}
        }
    }

    /// Where the turning bike goes on the screen.
    pub fn preview_window(&self) -> [f32; 4] {
        match &self.screen {
            Screen::Extra(s) => s.preview().map_or(crate::preview::VIEWPORT, |p| p.2),
            _ => crate::preview::VIEWPORT,
        }
    }

    /// A practice ride on `track` with the player's bike, alone: the
    /// Course Builder's Ride it. The loading screen shows while it loads.
    pub fn practice(&mut self, event: EventKind, track: PathBuf, name: String) -> Setup {
        // Baked tracks join the track lists.
        self.tracks.clear();
        self.loading(event);
        Setup {
            event,
            track,
            course: 0,
            laps: 99,
            opponents: 0,
            mode: PRACTICE,
            name,
            ride: self.ride(),
            level: 1,
            names: vec![self.user.profile.name.clone()],
            rides: Vec::new(),
            ghost: None,
            net: None,
            random_gates: None,
        }
    }

    /// Each track of `event` there is: its name, file and course.
    pub fn track_list(&mut self, event: EventKind) -> Vec<(String, PathBuf, usize)> {
        self.tracks(event)
            .iter()
            .filter(|t| !crate::demo::is_locked(&t.path))
            .map(|t| (t.name.clone(), t.path.clone(), t.course))
            .collect()
    }

    /// Forgets the track lists, so new tracks show in them.
    pub fn forget_tracks(&mut self) {
        self.tracks.clear();
    }

    /// The player's choices on the Bike & Rider page.
    fn ride(&self) -> Option<Ride> {
        let mut ride = self.roster.default_ride()?;
        if let Some(b) = self.roster.bikes.get(self.bike) {
            ride.bike = b.clone();
        }
        if let Some(o) = self.roster.outfits.get(self.outfit) {
            ride.outfit = o.file.clone();
        }
        if self.paint_on {
            let c = |i: usize| crate::freepack::COLOURS[self.paint[i].min(11)].0;
            if let Some(set) = crate::freepack::paint_set(&ride.outfit) {
                ride.outfit = crate::freepack::mix_name(&set, c(0), c(1), Some(c(4)));
            }
            if let Some(set) = crate::freepack::paint_set(&ride.bike.texture) {
                ride.bike.texture = crate::freepack::mix_name(&set, c(2), c(3), None);
            }
        }
        let mut plate: u32 = self.plate.parse().unwrap_or(0);
        if plate < 100 {
            plate += 100;
        }
        ride.plate = plate.clamp(101, 999);
        Some(ride)
    }

    /// The Bike & Rider page (`0x44fe50`, `0x450630`): the bike and outfit
    /// lists on the player's picks, the engine size, and the plate number.
    fn fill_bike_page(&mut self) {
        // The Free Pack's kits named OpenMX beside the game's own.
        let (bikes, outfits) = self.roster.shown();
        let cc = self.roster.bikes.get(self.bike).map_or(250, |b| b.cc);
        // The player's own paint takes any engine size, as the game's
        // page offers for them, and a Free Pack bike the sizes it comes in.
        let own = self.roster.bikes.get(self.bike).is_some_and(|b| b.own || b.sizes.len() > 1);
        let size = ENGINE_SIZES
            .iter()
            .position(|e| e.0 >= cc)
            .unwrap_or(ENGINE_SIZES.len() - 1);
        let Screen::Single { page, .. } = &mut self.screen else {
            return;
        };
        page.set_items("DDLBikes", bikes, self.bike);
        page.set_items("DDLRiders", outfits, self.outfit);
        let sizes = ENGINE_SIZES.iter().map(|e| e.1.to_string()).collect();
        page.set_items("DDLEngineSize", sizes, size);
        page.enable("DDLEngineSize", own);
        page.enable("TxtEngineSize", own);
        page.set_on("ChkAutoRotate", true);
        page.set_text("EditPlateNumber", self.plate.clone());
        self.fill_paint_rows();
    }

    /// Whether the picked kit or bike takes the Kit & Paint page's colours
    /// (the high-detail ones, whose paint has masks).
    fn paintable(&self) -> bool {
        let outfit = self.roster.outfits.get(self.outfit).is_some_and(|o| crate::freepack::paint_set(&o.file).is_some());
        let bike = self.roster.bikes.get(self.bike).is_some_and(|b| crate::freepack::paint_set(&b.texture).is_some());
        outfit || bike
    }

    /// The Kit & Paint page: the Bike & Rider page's own boxes and arrows,
    /// copied, one row for each colour around the turning bike and rider,
    /// and its box beside Auto Rotate to come and go.
    fn fill_paint_rows(&mut self) {
        let paintable = self.paintable();
        if !paintable {
            self.paint_page = false;
        }
        let (on, paint) = (self.paint_page, self.paint);
        let Screen::Single { page, .. } = &mut self.screen else {
            return;
        };
        if page.index("ChkPaint").is_none() {
            page.add_copy("ChkAutoRotate", "ChkPaint", [416, 4, 129, 21]);
            page.set_text("ChkPaint", "Kit & Paint");
            for (name, _, y) in PAINT_ROWS {
                page.add_copy("DDLRiders", name, [95, y, 285, 21]);
                page.add_copy("RiderLeft", &format!("{name}Left"), [73, y, 21, 21]);
                page.add_copy("RiderRight", &format!("{name}Right"), [381, y, 21, 21]);
            }
        }
        page.set_tip(
            "ChkPaint",
            if paintable {
                "Pick your own colours for the HD kit, helmet and bike"
            } else {
                "Pick an HD outfit or bike to choose its colours"
            },
        );
        page.enable("ChkPaint", paintable);
        page.set_on("ChkPaint", on);
        for name in ["DDLRiders", "RiderLeft", "RiderRight", "DDLBikes", "BikeLeft", "BikeRight", "DDLEngineSize", "TxtEngineSize", "rbwLabel1", "EditPlateNumber", "ButWrench"] {
            page.show(name, !on);
        }
        for (i, (name, label, _)) in PAINT_ROWS.iter().enumerate() {
            for n in [name.to_string(), format!("{name}Left"), format!("{name}Right")] {
                page.show(&n, on);
            }
            let items = crate::freepack::COLOURS.iter().map(|c| format!("{label}: {}", title(c.0))).collect();
            page.set_items(name, items, paint[i]);
        }
    }

    /// The Bike & Rider page's arrows and boxes.
    fn bike_clicked(&mut self, name: &str) {
        let step = |at: usize, count: usize, by: isize| {
            (at as isize + by).rem_euclid(count.max(1) as isize) as usize
        };
        let (bikes, outfits) = (self.roster.bikes.len(), self.roster.outfits.len());
        match name {
            "bikeleft" => {
                self.bike = step(self.bike, bikes, -1);
                self.bike_picked = true;
            }
            "bikeright" => {
                self.bike = step(self.bike, bikes, 1);
                self.bike_picked = true;
            }
            "riderleft" => {
                self.outfit = step(self.outfit, outfits, -1);
                self.paint_on = false;
            }
            "riderright" => {
                self.outfit = step(self.outfit, outfits, 1);
                self.paint_on = false;
            }
            "chkpaint" => {
                self.paint_page = !self.paint_page;
                if self.paint_page && !self.paint_on {
                    self.paint = self.paint_from_picks();
                    self.paint_on = true;
                }
            }
            "editplatenumber" => {
                self.typing = true;
                return;
            }
            other => {
                let (row, by) = match (other.strip_suffix("left"), other.strip_suffix("right")) {
                    (Some(row), _) => (row, -1),
                    (_, Some(row)) => (row, 1),
                    _ => return,
                };
                let Some(i) = PAINT_ROWS.iter().position(|r| r.0.eq_ignore_ascii_case(row)) else {
                    return;
                };
                self.paint[i] = step(self.paint[i], crate::freepack::COLOURS.len(), by);
                self.paint_on = true;
            }
        }
        if self.typing {
            self.commit_plate();
        }
        self.keep_ride();
        self.fill_bike_page();
    }

    /// Keys typed into the plate number box: up to three digits.
    pub fn type_char(&mut self, c: char) {
        if self.extra_up() {
            let mut cx = Cx {
                ui: &mut self.ui,
                audio: None,
                user: &mut self.user,
                now: self.now,
            };
            match &mut self.screen {
                Screen::Extra(s) => {
                    let go = s.type_char(&mut cx, c);
                    self.go(go);
                }
                Screen::Main(d) => {
                    let events = self
                        .boxes
                        .dialog()
                        .map(|b| b.type_char(c))
                        .unwrap_or_default();
                    for e in events {
                        self.boxes.event(&mut cx, d, e);
                    }
                }
                _ => {}
            }
            return;
        }
        // The race number box takes digits only.
        if let Screen::Single { page, .. } = &mut self.screen {
            if page.editing() == Some("EditSeed") {
                if c.is_ascii_digit() || matches!(c, '\u{8}' | '\r' | '\n' | '\t') {
                    page.type_char(c);
                    self.read_event_page();
                }
                return;
            }
        }
        if !self.typing {
            return;
        }
        match c {
            '0'..='9' if self.plate.len() < 3 => self.plate.push(c),
            '\u{8}' => {
                self.plate.pop();
            }
            '\r' | '\n' | '\t' => self.commit_plate(),
            _ => return,
        }
        let plate = self.plate.clone();
        if let Screen::Single { page, .. } = &mut self.screen {
            page.set_text("EditPlateNumber", plate);
        }
    }

    /// Leaving the plate box (`0x44fe50`, message 10): numbers run from 101
    /// to 999, and one under 100 gains 100.
    fn commit_plate(&mut self) {
        self.typing = false;
        let mut n: u32 = self.plate.parse().unwrap_or(0);
        if n < 100 {
            n += 100;
        }
        self.plate = n.clamp(101, 999).to_string();
        self.keep_ride();
    }

    /// The profile's bike, outfit and plate on the Bike & Rider page, by
    /// name (`0x44fe50`); a name no longer listed keeps the current pick.
    fn load_ride(&mut self) {
        let split = split_looks(&mut self.user.profile);
        if originals_first(&mut self.user.profile, &self.roster) || split {
            self.user.save();
        }
        let p = &self.user.profile;
        // The free graphics' picks, or the game's own (the demo's too).
        let (bike, outfit) = if crate::freepack::chosen() {
            (p.free_bike.as_deref().unwrap_or(&p.bike), p.free_outfit.as_deref().unwrap_or(&p.outfit))
        } else {
            (p.bike.as_str(), p.outfit.as_str())
        };
        if let Some(i) = self.roster.find_bike(&bike.to_string()) {
            self.bike = i;
            self.bike_picked = true;
        }
        if let Some(i) = self.roster.outfits.iter().position(|o| o.name == outfit) {
            self.outfit = i;
        }
        if (101..=999).contains(&p.plate) {
            self.plate = p.plate.to_string();
        }
        let named: Vec<usize> = p
            .paint
            .split(',')
            .filter_map(|n| crate::freepack::COLOURS.iter().position(|c| c.0 == n.trim()))
            .collect();
        if let Ok(paint) = <[usize; 5]>::try_from(named) {
            self.paint = paint;
            self.paint_on = true;
        }
    }

    /// The Kit & Paint page's colours to start from: the picked kit's and
    /// bike's own (their colourway's name), white trim.
    fn paint_from_picks(&self) -> [usize; 5] {
        let named = |file: &str| {
            let stem = file.rsplit_once('.').map_or(file, |s| s.0).to_ascii_lowercase();
            let last = stem.rsplit('_').next().unwrap_or("");
            crate::freepack::COLOURS.iter().position(|c| c.0 == last)
        };
        let kit = self.roster.outfits.get(self.outfit).and_then(|o| named(&o.file)).unwrap_or(self.paint[0]);
        let bike = self.roster.bikes.get(self.bike).and_then(|b| named(&b.texture)).unwrap_or(self.paint[2]);
        let trim = |c: usize| if matches!(crate::freepack::COLOURS[c].0, "white" | "yellow" | "lime") { 0 } else { 10 };
        [kit, trim(kit), bike, trim(bike), kit]
    }

    /// The page's picks into the profile, saved (`0x4513f0`), so the next
    /// time the player signs in they ride the same.
    fn keep_ride(&mut self) {
        let bike = self.roster.bikes.get(self.bike).filter(|_| self.bike_picked).map(|b| b.saved_name().to_string());
        let outfit = self.roster.outfits.get(self.outfit).map(|o| o.name.clone());
        let plate = self.plate.parse().ok().filter(|n| (101..=999).contains(n));
        let paint = if self.paint_on {
            self.paint.iter().map(|&i| crate::freepack::COLOURS[i.min(11)].0).collect::<Vec<_>>().join(",")
        } else {
            String::new()
        };
        let p = &mut self.user.profile;
        let before = p.clone();
        p.paint = paint;
        // Into the picks of the graphics being played with.
        let (to_bike, to_outfit) = if crate::freepack::chosen() {
            (p.free_bike.get_or_insert_with(String::new), p.free_outfit.get_or_insert_with(String::new))
        } else {
            (&mut p.bike, &mut p.outfit)
        };
        if let Some(b) = bike {
            *to_bike = b;
        }
        if let Some(o) = outfit {
            *to_outfit = o;
        }
        if let Some(n) = plate {
            p.plate = n;
        }
        if *p != before {
            self.user.save();
        }
    }

    /// The Race Info page (`0x4521b0`): for a race, each rider's name,
    /// place, and best lap with the total time after it ("incomplete" if
    /// they did not finish), in finishing order. The replay button waits
    /// for a replay to view.
    fn fill_race_info(&mut self) {
        let heads = if self.results.stunt {
            [self.ui.lang(2356), String::new(), self.ui.lang(2359)]
        } else {
            [
                self.ui.lang(2356),
                self.ui.lang(2357),
                format!("{} ({})", self.ui.lang(2358), self.ui.lang(5048)),
            ]
        };
        let Screen::Single { page, .. } = &mut self.screen else {
            return;
        };
        for (i, head) in heads.into_iter().enumerate() {
            page.set_text(&format!("ButStats{}", i + 1), head);
        }
        // A series' results: the race count in the title, and the points.
        if let Some(title) = &self.series_title {
            page.set_text("TitleText", title.clone());
            page.set_text("ButStats4", self.ui.lang(2360));
        }
        for column in 0..4 {
            let items = self
                .results
                .lines
                .iter()
                .map(|r| r[column].clone())
                .collect();
            page.set_items(&format!("LstStats{}", column + 1), items, 0);
        }
        page.enable("ButViewReplay", self.results.replay.is_some());
    }

    fn open_single(&mut self) {
        self.load_ride();
        let Some(mut base) = Dlg::open(&mut self.ui, "SPBASE", (0.0, 0.0)) else {
            return;
        };
        // The tabs' names come from the code (`0x44e660`).
        for (tab, id) in [
            ("EventTab", 5091),
            ("BikeRiderTab", 5092),
            ("RaceInfoTab", 5093),
        ] {
            base.set_text(tab, self.ui.lang(id));
            base.set_font(tab, 18.0);
        }
        base.set_on("EventTab", true);
        let Some(page) = Dlg::open(&mut self.ui, "SPEVENT", PAGE) else {
            return;
        };
        let mut page = page;
        if crate::graphics::enhanced().is_some() {
            sky_row(&mut page);
        }
        if crate::tweaks::more().track_filter {
            filter_row(&mut page);
        }
        self.screen = Screen::Single { base, page };
        self.fill_event_page();
    }

    /// The event page's arrows.
    fn page_clicked(&mut self, name: &str) {
        let Screen::Single { page, .. } = &mut self.screen else {
            return;
        };
        if page.index("DDLBikes").is_some() {
            return self.bike_clicked(name);
        }
        if page.index("LapsListBox").is_none() {
            return;
        }
        let step = |page: &mut Dlg, list: &str, by: i32| {
            let count = page.get(list).map_or(0, |c| c.items.len()) as i32;
            let at = (page.selected(list) as i32 + by).clamp(0, (count - 1).max(0));
            page.select(list, at as usize);
        };
        match name {
            "ghostleft" => step(page, "DDLGhostRaces", -1),
            "ghostright" => step(page, "DDLGhostRaces", 1),
            "lapsleftarrow" => step(page, "LapsListBox", -1),
            "lapsrightarrow" => step(page, "LapsListBox", 1),
            "opponentsleftarrow" => step(page, "OpponentsListBox", -1),
            "opponentsrightarrow" => step(page, "OpponentsListBox", 1),
            "racesleftarrow" => step(page, "RacesListBox", -1),
            "racesrightarrow" => step(page, "RacesListBox", 1),
            "randomgatesleftarrow" => step(page, "LstNumGates", -1),
            "randomgatesrightarrow" => step(page, "LstNumGates", 1),
            "chkrandomgates" => {
                let on = page.get("ChkRandomGates").is_some_and(|c| c.on);
                enable_seed_box(page, on);
            }
            // The race number's arrows go round 0 to 999 (0x44ec50).
            "seedleft" | "seedright" => {
                let last = crate::gates::RandomGates::LAST_SEED as i32;
                let n: i32 = page.get("EditSeed").and_then(|c| c.text.as_deref()).and_then(|t| t.trim().parse().ok()).unwrap_or(0);
                let n = if name == "seedleft" {
                    if n - 1 < 0 { last } else { n - 1 }
                } else if n + 1 > last {
                    0
                } else {
                    n + 1
                };
                page.edit(None);
                page.set_text("EditSeed", n.to_string());
            }
            "trackleft" | "trackright" => {
                let count = page.get("DDLTextBox").map_or(0, |c| c.items.len());
                if count > 0 {
                    let at = page.selected("DDLTextBox");
                    let at = if name == "trackleft" {
                        (at + count - 1) % count
                    } else {
                        (at + 1) % count
                    };
                    page.select("DDLTextBox", at);
                }
            }
            _ => {}
        }
        self.read_event_page();
        // A new track has its own ghosts.
        if self.mode == GHOST && name.starts_with("track") {
            self.fill_event_page();
        }
    }

    fn selected(&mut self, name: &str, line: usize) {
        match name.to_ascii_lowercase().as_str() {
            "ddlbikes" => {
                self.bike = line;
                self.bike_picked = true;
                self.keep_ride();
                self.fill_bike_page();
            }
            "ddlriders" => {
                self.outfit = line;
                self.paint_on = false;
                self.keep_ride();
            }
            n if PAINT_ROWS.iter().any(|r| r.0.eq_ignore_ascii_case(n)) => {
                let i = PAINT_ROWS.iter().position(|r| r.0.eq_ignore_ascii_case(n)).unwrap_or(0);
                self.paint[i] = line.min(crate::freepack::COLOURS.len() - 1);
                self.paint_on = true;
                self.keep_ride();
                self.fill_bike_page();
            }
            "tracksource" => {
                self.track_source = line.min(TRACK_SOURCES.len() - 1);
                self.track = 0;
                self.fill_event_page();
            }
            "ddlenginesize" => {
                if let (Some(b), Some(&(cc, name))) = (self.roster.bikes.get_mut(self.bike), ENGINE_SIZES.get(line)) {
                    if b.own {
                        b.cc = cc;
                        b.four_stroke = name.contains("4-stroke");
                    } else if b.set_size(cc) {
                        // A Free Pack bike's size is kept with it.
                        self.bike_picked = true;
                        self.keep_ride();
                    }
                }
                self.fill_bike_page();
            }
            "eventtypedropdown" => {
                let event = EventKind::LIST[line.min(EventKind::LIST.len() - 1)].0;
                if event != self.event {
                    self.event = event;
                    self.track = 0;
                }
                self.fill_event_page();
            }
            "racemodedropdown" => {
                self.read_event_page();
                self.fill_event_page();
            }
            "ddltextbox" => {
                self.read_event_page();
                if self.mode == GHOST {
                    self.fill_event_page();
                }
            }
            _ => self.read_event_page(),
        }
    }

    /// Takes the page's choices.
    fn read_event_page(&mut self) {
        let Screen::Single { page, .. } = &self.screen else {
            return;
        };
        if page.index("LapsListBox").is_none() {
            return;
        }
        let modes: Vec<u32> = MODES
            .iter()
            .map(|m| m.0)
            .filter(|&m| m != GHOST || self.event != EventKind::Stunt)
            .collect();
        self.mode = modes[page.selected("RaceModeDropDown").min(modes.len() - 1)];
        let item = |name: &str| {
            page.get(name)
                .and_then(|c| c.item())
                .and_then(|s| s.parse().ok())
        };
        if let Some(laps) = item("LapsListBox") {
            self.laps = laps;
        }
        if let Some(n) = item("OpponentsListBox") {
            self.opponents = n;
        }
        if let Some(l) = LEVELS.iter().position(|n| page.get(n).is_some_and(|c| c.on)) {
            self.level = l;
        }
        // The Races box is the series' count until it is under way; any
        // other mode ends it (0x44fb90).
        if self.mode != SERIES {
            self.series.on = false;
        }
        if !self.series.on && page.get("RacesListBox").is_some_and(|c| c.items.len() == 3) {
            self.series.races = page.selected("RacesListBox").min(2) as u32 * 2 + 2;
        }
        self.track = page.selected("DDLTextBox");
        self.ghost = page.selected("DDLGhostRaces");
        if page.index("ChkRandomGates").is_some() {
            self.random_gates = page.get("ChkRandomGates").is_some_and(|c| c.on);
            if let Some(n) = item("LstNumGates") {
                self.gate_count = n;
            }
            if let Some(n) = page.get("EditSeed").and_then(|c| c.text.as_deref()).and_then(|t| t.trim().parse::<u32>().ok()) {
                self.seed = n.min(crate::gates::RandomGates::LAST_SEED);
            }
        }
        if page.index("TimeList").is_some() {
            let time = mm2_settings::TIMES[page.selected("TimeList").min(mm2_settings::TIMES.len() - 1)].0;
            let weather = mm2_settings::WEATHERS[page.selected("WeatherList").min(mm2_settings::WEATHERS.len() - 1)].0;
            set_sky(time, weather);
        }
    }

    /// The ghost race list (`0x44fa20`): "Record a new ghost race", then
    /// each `Record\*.gho` made on this track and course (`0x44fb30`).
    fn ghost_list(&mut self, entry: Option<&TrackEntry>) -> Vec<String> {
        let mut lines = vec![self.ui.lang(5319)];
        self.ghosts.clear();
        let Some(entry) = entry else {
            return lines;
        };
        let stem = entry
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let dir = crate::replay::record_dir(&self.user.store.dir);
        let mut files: Vec<PathBuf> = crate::fs::read_dir(dir)
            .map(|d| {
                d.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gho")))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        for f in files {
            let Some(tape) = crate::fs::read(&f)
                .ok()
                .and_then(|b| mm2_formats::vcr::Tape::parse(&b).ok())
            else {
                continue;
            };
            if tape.track().eq_ignore_ascii_case(stem)
                && tape.length > 0.0
                && tape.course() == entry.course
            {
                lines.push(format!("{} - {}", self.ui.lang(5320), tape.description));
                self.ghosts.push(f);
            }
        }
        lines
    }

    /// Fills the event page for the chosen event (`0x44f2c0`), and shows
    /// the boxes its race mode uses (`0x44ee90`).
    fn fill_event_page(&mut self) {
        let event = self.event;
        let tracks = self.shown_tracks(event);
        let lang = |id| self.ui.lang(id);
        let events: Vec<String> = EventKind::LIST.iter().map(|e| lang(e.1)).collect();
        let modes: Vec<(u32, String)> = MODES
            .iter()
            .filter(|m| m.0 != GHOST || event != EventKind::Stunt)
            .map(|m| (m.0, lang(m.1)))
            .collect();
        let laps_label = lang(if event == EventKind::Stunt {
            2351
        } else {
            2345
        });
        let Screen::Single { page, .. } = &mut self.screen else {
            return;
        };
        if page.index("LapsListBox").is_none() {
            return;
        }
        let at = EventKind::LIST
            .iter()
            .position(|e| e.0 == event)
            .unwrap_or(0);
        page.set_items("EventTypeDropDown", events, at);
        let mode_at = modes.iter().position(|m| m.0 == self.mode).unwrap_or(1);
        self.mode = modes[mode_at].0;
        page.set_items(
            "RaceModeDropDown",
            modes.into_iter().map(|m| m.1).collect(),
            mode_at,
        );
        // The more riders extra: up to its count on the gate in a single
        // event or practice; a series keeps the game's 11, as its points do.
        let most = match crate::tweaks::more().riders {
            n if n > 11 && self.mode != 2 => n - 1,
            _ => 10,
        };
        page.set_items(
            "OpponentsListBox",
            (0..=most).map(|n| n.to_string()).collect(),
            self.opponents.min(most) as usize,
        );
        let (from, to) = if event == EventKind::Stunt {
            (2, 30)
        } else {
            (event.fewest_laps(), 20)
        };
        self.laps = self.laps.clamp(from, to);
        page.set_items(
            "LapsListBox",
            (from..=to).map(|n| n.to_string()).collect(),
            (self.laps - from) as usize,
        );
        page.set_text("LapsLabel", laps_label);
        // 3, 5 or 7 races (0x44eea0); while a series is under way, the race
        // it is on, with the opponents and the count held.
        let series = &self.series;
        let held = self.mode == SERIES && series.on;
        if held {
            page.set_items("RacesListBox", vec![format!("{}/{}", series.run + 1, series.races + 1)], 0);
        } else {
            page.set_items(
                "RacesListBox",
                [3, 5, 7].iter().map(|n| n.to_string()).collect(),
                (series.races / 2).saturating_sub(1).min(2) as usize,
            );
        }
        for arrow in ["RacesLeftArrow", "RacesRightArrow", "OpponentsLeftArrow", "OpponentsRightArrow"] {
            page.show(arrow, !held);
        }
        page.enable("OpponentsListBox", !held);
        page.set_text("RacesLabel", self.ui.lang(if held { 2347 } else { 2346 }));
        self.track = self.track.min(tracks.len().saturating_sub(1));
        page.set_items(
            "DDLTextBox",
            tracks.iter().map(shown_name).collect(),
            self.track,
        );
        if self.mode == GHOST {
            let lines = self.ghost_list(tracks.get(self.track));
            self.ghost = self.ghost.min(lines.len() - 1);
            if let Screen::Single { page, .. } = &mut self.screen {
                page.set_items("DDLGhostRaces", lines, self.ghost);
            }
        }
        let Screen::Single { page, .. } = &mut self.screen else {
            return;
        };
        // The boxes each race mode uses.
        let mode = self.mode;
        page.show("OpponentsControlBox", mode != GHOST);
        page.show("ChkRecordRace", mode != GHOST);
        page.show("GhostBox", mode == GHOST);
        page.show_group(1011, mode != GHOST);
        page.show("LapsControlBox", mode != 0 && mode != GHOST);
        page.show("racesControlBox", mode == 2);
        // Random gates are for Baja races (0x44f2c0): 10 to 30 of them,
        // the count, the race number and its arrows greyed while the box
        // is not ticked.
        page.show("WaypointBox", event == EventKind::Baja && mode != GHOST);
        use crate::gates::RandomGates;
        page.set_items(
            "LstNumGates",
            (RandomGates::FEWEST..=RandomGates::MOST).map(|n| n.to_string()).collect(),
            (self.gate_count.clamp(RandomGates::FEWEST, RandomGates::MOST) - RandomGates::FEWEST) as usize,
        );
        page.set_on("ChkRandomGates", self.random_gates);
        if page.editing() != Some("EditSeed") {
            page.set_text("EditSeed", self.seed.to_string());
        }
        enable_seed_box(page, self.random_gates);
        page.show("ChkTreeCollision", true);
        for (i, name) in LEVELS.iter().enumerate() {
            page.set_on(name, i == self.level);
        }
        if page.index("TrackSource").is_some() {
            page.set_items("TrackSource", TRACK_SOURCES.iter().map(|s| s.to_string()).collect(), self.track_source);
        }
        if page.index("TimeList").is_some() {
            let (time, weather) = sky_now();
            page.set_items("TimeList", mm2_settings::TIMES.iter().map(|t| t.1.to_string()).collect(), time);
            page.set_items("WeatherList", mm2_settings::WEATHERS.iter().map(|w| w.1.to_string()).collect(), weather);
        }
    }

    /// The tracks of an event, in the order the game lists them (by file
    /// name), each course of a Baja or enduro track its own line.
    /// The event's tracks the page lists: all of them, or with the track
    /// filter extra only the game's own or only custom ones.
    fn shown_tracks(&mut self, event: EventKind) -> Vec<TrackEntry> {
        let source = self.track_source;
        self.tracks(event)
            .iter()
            .filter(|t| match source {
                1 => is_stock(event, &t.path),
                2 => !is_stock(event, &t.path),
                _ => true,
            })
            .cloned()
            .collect()
    }

    fn tracks(&mut self, event: EventKind) -> &[TrackEntry] {
        let game = self.ui.game().to_path_buf();
        self.tracks
            .entry(event)
            .or_insert_with(|| {
                // With the demo, the full game's tracks it doesn't let you
                // ride follow, as its own lists had them.
                let mut list = picker_tracks(list_tracks(&game.join(event.folder()), event));
                list.extend(crate::demo::locked(&game, event));
                list
            })
    }

    fn start(&mut self) -> Next {
        self.read_event_page();
        // The garage's curve follows the bike ridden, picked or not: a
        // Free Pack 125 left as the page's first choice still rides with
        // a 125's power, not that of the class the garage last held.
        if let Some(b) = self.roster.bikes.get(self.bike) {
            let classes = crate::screens::options::engine_classes(self.ui.game());
            let mut garage = self.user.profile.garage.clone();
            crate::screens::options::ride_bike(&mut garage, &classes, b.cc);
            if garage != self.user.profile.garage {
                self.user.profile.garage = garage;
                if let Err(e) = self.user.store.save(&self.user.profile) {
                    eprintln!("could not save the profile: {e}");
                }
            }
        }
        let (event, track) = (self.event, self.track);
        let Some(entry) = self.shown_tracks(event).get(track).cloned() else {
            return Next::Stay;
        };
        // One of the full game's tracks the demo lists but can't ride: the
        // demo's box naming the tracks it has (and where to get the rest).
        if crate::demo::is_locked(&entry.path) {
            let (title, prompt) = crate::demo::locked_note();
            if let Some(b) = screens::MsgBox::ask(&mut self.ui, title, prompt, [0, 5097, 0]) {
                self.ghost_ask = Some((b.dlg, None));
            }
            return Next::Stay;
        }
        // A series' first race starts it with no points (0x45e490).
        if self.mode == SERIES && !self.series.on {
            let s = &mut self.series;
            s.on = true;
            s.run = 0;
            s.points = vec![0; self.opponents.min(10) as usize + 1];
        }
        if let Some(d) = Dlg::open(&mut self.ui, self.event.loading(), (0.0, 0.0)) {
            self.screen = Screen::Loading(d);
        }
        Next::Race(Setup {
            event: self.event,
            track: entry.path,
            course: entry.course,
            laps: if self.mode == 0 { 99 } else { self.laps },
            opponents: if self.mode == GHOST {
                0
            } else if self.mode == SERIES {
                self.opponents.min(10)
            } else {
                self.opponents
            },
            mode: self.mode,
            name: entry.name,
            ride: self.ride(),
            level: self.level,
            names: vec![self.user.profile.name.clone()],
            rides: Vec::new(),
            ghost: match self.mode {
                GHOST => self
                    .ghost
                    .checked_sub(1)
                    .and_then(|g| self.ghosts.get(g))
                    .cloned(),
                _ => None,
            },
            net: None,
            random_gates: (self.event == EventKind::Baja && self.mode != GHOST && self.random_gates)
                .then_some(crate::gates::RandomGates { count: self.gate_count, seed: self.seed }),
        })
    }

    pub fn draw(&mut self, gfx: &mut Gfx, now: f32, progress: f32) {
        let dt = (now - self.now).max(0.0);
        self.now = now;
        self.events_tick(dt);
        // Set again below while the main menu shows it.
        self.note_at = None;
        if let Screen::Extra(s) = &mut self.screen {
            let mut cx = Cx {
                ui: &mut self.ui,
                audio: None,
                user: &mut self.user,
                now,
            };
            let go = s.tick(&mut cx);
            let next = self.go(go);
            if !matches!(next, Next::Stay) {
                self.pending = Some(next);
            }
        }
        let event = self.event;
        let track = self.track;
        let preview = match &self.screen {
            Screen::Single { page, .. } if page.index("PictureBox").is_some() => {
                self.shown_tracks(event).get(track).cloned()
            }
            _ => None,
        };
        let ui = &mut self.ui;
        match &mut self.screen {
            Screen::Main(d) => {
                d.tick(now);
                // The clips go over the Single pull-down and under the
                // screen's frame (`0x44c1d0` puts the video control just
                // before ScreenOverCtl).
                d.draw_background(ui, gfx);
                let n = d.ctls_len();
                let over = d.index("ScreenOverCtl").unwrap_or(n);
                d.draw_some(ui, gfx, now, 0..over);
                if let Some(m) = &mut self.events {
                    // Under the frame (ScreenOverCtl, 355,113 144x104 in
                    // the game's own layout), wherever the pull-down's art
                    // has moved it: the new look slides the pull-down right.
                    let (x, y) = d.origin;
                    let at = d.rect_of("ScreenOverCtl").unwrap_or([x + 355.0, y + 113.0, 144.0, 104.0]);
                    m.draw(gfx, now, at);
                }
                d.draw_some(ui, gfx, now, over..n);
                d.draw_list(ui, gfx);
                if self.boxes.dims() {
                    // Half as bright: 0.75 of black blended on the linear
                    // target about halves the stored colours.
                    ui.fill(gfx, [0.0, 0.0, 640.0, 480.0], [0.0, 0.0, 0.0, 0.75]);
                }
                if let Some(b) = self.boxes.dialog() {
                    b.tick(now);
                    b.draw(ui, gfx, now);
                }
                // Only on the bare menu: not over a pull-down or a box.
                let bare = self.ghost_ask.is_none()
                    && !self.boxes.is_open()
                    && PULL_DOWNS.iter().all(|(_, anim)| !d.is_shown(anim));
                self.note_at = match crate::update_note::get() {
                    Some(n) if bare => draw_note(ui, gfx, d, &n.tag),
                    _ => None,
                };
            }
            Screen::Extra(s) => s.draw(ui, gfx, now),
            Screen::Single { base, page } => {
                base.tick(now);
                base.draw(ui, gfx, now);
                page.tick(now);
                page.draw_under(ui, gfx, now);
                if let (Some(t), Some(r)) = (preview, page.rect_of("PictureBox")) {
                    let key = format!("{}#{}", t.path.display(), t.preview);
                    if let Some(tex) = ui.picture(gfx, &key, || preview_image(&t)) {
                        // Its middle, as much as the box holds (shorter
                        // than the game's when the time and weather row
                        // is there).
                        let (w, h) = gfx.sprites.size(tex);
                        let (w, h) = (w as f32, h as f32);
                        let (cw, ch) = (w.min(r[2]), h.min(r[3]));
                        let (u, v) = ((w - cw) / 2.0 / w, (h - ch) / 2.0 / h);
                        gfx.sprites.quad(
                            tex,
                            [u, v, 1.0 - u, 1.0 - v],
                            [(r[0] + (r[2] - cw) / 2.0).round(), (r[1] + (r[3] - ch) / 2.0).round(), cw, ch],
                            [1.0; 4],
                        );
                    }
                }
                page.draw_list(ui, gfx);
            }
            Screen::Loading(d) => {
                d.draw(ui, gfx, now);
                // The bar fills as the track loads.
                if let Some(i) = d.index("StaBarRect") {
                    let r = d.rect(i);
                    if let Some(t) = ui.image(gfx, &d.pack, "BlueBar") {
                        let p = progress.clamp(0.0, 1.0);
                        gfx.sprites.quad(
                            t,
                            [0.0, 0.0, p, 1.0],
                            [r[0], r[1], r[2] * p, r[3]],
                            [1.0; 4],
                        );
                    }
                }
            }
        }
        // The ghost lap question, over whatever screen the race went back to.
        if let Some((d, _)) = &mut self.ghost_ask {
            d.tick(now);
            d.draw(ui, gfx, now);
        }
        // The pointer's tip, from the dialog on top, if the player keeps
        // the Game page's "Show Tool Tips" on.
        let mut layers: Vec<&mut Dlg> = match (&mut self.ghost_ask, &mut self.screen) {
            (Some((d, _)), _) => vec![d],
            (None, Screen::Main(d)) => match self.boxes.dialog() {
                Some(b) => vec![b],
                None => vec![d],
            },
            (None, Screen::Extra(s)) => s.layers(),
            (None, Screen::Single { base, page }) => vec![base, page],
            (None, Screen::Loading(_)) => Vec::new(),
        };
        let top = layers.iter().rposition(|d| d.has_tip());
        let on = self.user.profile.game.tooltips;
        for (k, d) in layers.iter_mut().enumerate() {
            d.draw_tip(ui, gfx, now, on && Some(k) == top);
        }
    }
}

/// The update note just above the Current Profile bar, drawn as that bar
/// is: a dark blue box with a grey rim and the dialogs' light blue text,
/// white under the pointer. Where it and its × are.
fn draw_note(ui: &mut Ui, gfx: &mut Gfx, d: &Dlg, tag: &str) -> Option<([f32; 4], [f32; 4])> {
    let bar = d.rect_of("TxtCurrentProfile").unwrap_or([216.0, 448.0, 254.0, 26.0]);
    let text = format!("OpenMCM2 {tag} is out. Click here to update.");
    let t = ui.text(gfx, &text, 12.0, false)?;
    let x_mark = ui.text(gfx, "×", 12.0, true)?;
    let (tw, th) = gfx.sprites.size(t);
    let (xw, xh) = gfx.sprites.size(x_mark);
    let (tw, th, xw, xh) = (tw as f32, th as f32, xw as f32, xh as f32);
    let h = 18.0;
    let w = (tw + xw + 26.0).round();
    let x = (bar[0] + (bar[2] - w) / 2.0).round();
    let y = bar[1] - h - 6.0;
    ui.fill(gfx, [x, y, w, h], color(0x392920));
    ui.fill(gfx, [x + 1.0, y + 1.0, w - 2.0, h - 2.0], color(0x392010));
    let (px, py) = d.pointer();
    let close = [x + w - xw - 10.0, y, xw + 10.0, h];
    let note = [x, y, w - close[2], h];
    let over = |r: [f32; 4]| px >= r[0] && px < r[0] + r[2] && py >= r[1] && py < r[1] + r[3];
    let tint = |on: bool| if on { [1.0; 4] } else { color(0xfeb97a) };
    gfx.sprites.quad(t, [0.0, 0.0, 1.0, 1.0], [x + 8.0, y + ((h - th) / 2.0).floor(), tw, th], tint(over(note)));
    gfx.sprites.quad(
        x_mark,
        [0.0, 0.0, 1.0, 1.0],
        [close[0] + 4.0, y + ((h - xh) / 2.0).floor(), xw, xh],
        tint(over(close)),
    );
    Some((note, close))
}

/// The main menu's buttons and the pull-downs they open.
const PULL_DOWNS: [(&str, &str); 3] = [
    ("menusingle", "MenuSingleAnimation"),
    ("menumulti", "MenuMultiAnimation"),
    ("menuuserprofile", "MenuUserAnimation"),
];

/// A Free Pack track's entries in an event's list (one per course for a
/// Baja or enduro track), built at `path` when first ridden.
pub fn free_entries(path: &Path, def: &mm2_course::free::Def, event: EventKind) -> Vec<TrackEntry> {
    let stem = def.file;
    let courses = crate::free_tracks::courses(def);
    if event.courses() && !courses.is_empty() {
        courses
            .into_iter()
            .enumerate()
            .map(|(c, course)| TrackEntry { path: path.to_path_buf(), course: c, name: course, preview: format!("{stem}{:02}.tga", c + 1) })
            .collect()
    } else {
        vec![TrackEntry { path: path.to_path_buf(), course: 0, name: def.name.to_string(), preview: format!("{stem}.tga") }]
    }
}

/// A track's line in the event page's list. With the demo, what isn't the
/// trial's own says so: our free tracks are "(open source)", as the free
/// bikes are, and the full game's ones it can't ride "(Full Game)".
pub fn shown_name(t: &TrackEntry) -> String {
    if !crate::demo::active() {
        return t.name.clone();
    }
    if crate::demo::is_locked(&t.path) {
        return format!("{} {}", t.name, crate::demo::FULL_GAME);
    }
    let ours = t.path.file_stem().and_then(|s| s.to_str()).and_then(mm2_course::free::find).is_some();
    if ours {
        format!("{} (open source)", t.name)
    } else {
        t.name.clone()
    }
}

/// An event's tracks as the single race and multiplayer pickers list them:
/// without our own free tracks (the Free Pack's) unless the free tracks
/// extra is on, or the event has nothing else. The Pro Circuit, replays
/// and joining a host's race use [`list_tracks`] as it is.
pub fn picker_tracks(list: Vec<TrackEntry>) -> Vec<TrackEntry> {
    if crate::tweaks::more().free_tracks {
        return list;
    }
    let ours = |t: &TrackEntry| t.path.file_stem().and_then(|s| s.to_str()).and_then(mm2_course::free::find).is_some();
    let others: Vec<TrackEntry> = list.iter().filter(|t| !ours(t)).cloned().collect();
    if others.is_empty() {
        list
    } else {
        others
    }
}

pub fn list_tracks(folder: &Path, event: EventKind) -> Vec<TrackEntry> {
    let mut files: Vec<PathBuf> = crate::fs::read_dir(folder)
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("env")))
                .collect()
        })
        .unwrap_or_default();
    files.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_uppercase()));
    let mut out = Vec::new();
    // The Free Pack's tracks, listed whether or not they are built yet
    // (each is built from its recipe the first time it is ridden).
    let (teraform, name) = (folder.parent(), folder.file_name().and_then(|n| n.to_str()));
    let free = teraform.zip(name).map(|(t, n)| crate::free_tracks::listed(t, n)).unwrap_or_default();
    files.retain(|p| !free.iter().any(|(f, _)| f.file_name().is_some_and(|n| p.file_name().is_some_and(|m| m.eq_ignore_ascii_case(n)))));
    // The demo's other tracks, rebuilt the first time they are ridden
    // (an extra); once built, still listed only as that.
    let rebuilt = crate::demo_tracks::listed(folder);
    files.retain(|p| !crate::fs::is_file(&p.with_extension("rebuilt")));
    let mut extra: Vec<TrackEntry> = rebuilt
        .iter()
        .flat_map(|t| {
            let stem = t.path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
            t.names.iter().enumerate().map(move |(c, name)| TrackEntry {
                path: t.path.clone(),
                course: c,
                name: name.clone(),
                preview: if event.courses() { format!("{stem}{:02}.tga", c + 1) } else { format!("{stem}.tga") },
            })
        })
        .collect();
    extra.extend(free.iter().flat_map(|(path, def)| free_entries(path, def, event)));
    for path in files {
        let Ok(bytes) = crate::read_archive(&path) else {
            continue;
        };
        let Ok(ar) = rs2::Archive::parse(&bytes) else {
            continue;
        };
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let ini = ar
            .entries()
            .iter()
            .find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))
            .map(|e| scn::Ini::parse(&String::from_utf8_lossy(ar.data(e))));
        let info = ini.as_ref().and_then(|i| i.section("SceneInfo"));
        let name = info
            .and_then(|s| s.get("SceneName"))
            .filter(|n| !n.is_empty())
            .unwrap_or(&stem)
            .to_string();
        if event.courses() {
            // Each course has its `.tdf`, or (many players' tracks) only
            // its gates' `.wpt`.
            let count = |ext: &str| ar.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(ext)).count();
            let courses = count(".tdf").max(count(".wpt"));
            for c in 0..courses {
                let course = info
                    .and_then(|s| s.get(&format!("WaypointRaceName{}", c + 1)))
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{name} {}", c + 1));
                out.push(TrackEntry {
                    path: path.clone(),
                    course: c,
                    name: course,
                    preview: format!("{stem}{:02}.tga", c + 1),
                });
            }
        } else {
            out.push(TrackEntry {
                path: path.clone(),
                course: 0,
                name,
                preview: format!("{stem}.tga"),
            });
        }
    }
    // After the game's own.
    out.extend(extra);
    out
}

pub fn preview_image(t: &TrackEntry) -> Option<tga::Image> {
    own_preview(t)
        .or_else(|| {
            // A rebuilt demo track not built yet: the demo's own picture.
            tga::parse(&crate::demo_tracks::preview(&t.path, &t.preview)?).ok()
        })
        .or_else(|| {
            // A free track not built yet: the picture the Free Pack carries.
            let path = crate::freepack::root()?.join("previews").join(&t.preview);
            tga::parse(&crate::fs::read(path).ok()?).ok()
        })
}

/// The picture a track's own files give (see [`preview_image`]).
fn own_preview(t: &TrackEntry) -> Option<tga::Image> {
    let bytes = crate::read_archive(&t.path).ok()?;
    let ar = rs2::Archive::parse(&bytes).ok()?;
    if let Some(entry) = ar.entries().iter().find(|e| e.name.eq_ignore_ascii_case(&t.preview)) {
        return tga::parse(ar.data(entry)).ok();
    }
    // A track with no picture of its own (made before the builder drew
    // them, or brought in without one): one drawn from its ground.
    let files: Vec<(String, Vec<u8>)> = ar
        .entries()
        .iter()
        .filter(|e| {
            let n = e.name.to_ascii_lowercase();
            n.ends_with(".trn") || n.ends_with(".tdf")
        })
        .map(|e| (e.name.clone(), ar.data(e).to_vec()))
        .collect();
    let stem = t.preview.rsplit_once('.').map_or(t.preview.as_str(), |(s, _)| s);
    let size = mm2_course::preview::LARGE;
    let rgb = mm2_course::preview::picture(stem, &files, size)?;
    let rgba = rgb.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
    Some(tga::Image { width: size.0 as u32, height: size.1 as u32, rgba })
}

/// The Random Gate box's count, race number and their arrows, greyed while
/// the box is not ticked (group 111, 0x46f9b0).
fn enable_seed_box(page: &mut Dlg, on: bool) {
    for name in ["LstNumGates", "RandomGatesLeftArrow", "RandomGatesRightArrow", "EditSeed", "SeedLeft", "SeedRight", "rbwLabel1"] {
        page.enable(name, on);
    }
    if !on && page.editing() == Some("EditSeed") {
        page.edit(None);
    }
}

/// With the game's graphics (the demo's too), a profile starts on the
/// game's own bike and rider. A Free Pack bike or kit saved in the game's
/// picks before the lists told the two apart (a tap left of the game's
/// first lands on the pack's last) goes to the free graphics' picks,
/// once (`OriginalFirst=1`); one picked after that stays.
/// Whether the profile changed.
fn originals_first(p: &mut crate::profile::Profile, roster: &crate::roster::Roster) -> bool {
    if p.original_first || crate::freepack::chosen() || roster.free_from == (0, 0) {
        return false;
    }
    p.original_first = true;
    let free_bike = roster.bikes.iter().position(|b| b.named(&p.bike)).is_some_and(|i| roster.free_bike(i));
    let free_outfit = roster.outfits.iter().position(|o| o.name == p.outfit).is_some_and(|i| roster.free_outfit(i));
    if free_bike {
        let b = std::mem::take(&mut p.bike);
        if p.free_bike.as_deref().is_none_or(str::is_empty) {
            p.free_bike = Some(b);
        }
    }
    if free_outfit {
        let o = std::mem::take(&mut p.outfit);
        if p.free_outfit.as_deref().is_none_or(str::is_empty) {
            p.free_outfit = Some(o);
        }
    }
    true
}

/// A profile from before the free graphics kept picks of their own
/// (`FreeBike=`/`FreeOutfit=`): a Free Pack bike or kit it saved was
/// picked riding with the free graphics, so it moves to those, and the
/// game's own (the demo's) rider and bike come back with its files.
/// Whether the profile changed.
fn split_looks(p: &mut crate::profile::Profile) -> bool {
    if p.free_bike.is_some() || p.free_outfit.is_some() || crate::freepack::outfits().is_empty() {
        return false;
    }
    let free_bike = p.bike.starts_with(&format!("{} ", crate::roster::FREE_MAKER));
    let free_outfit = crate::freepack::outfits().iter().any(|o| o.name == p.outfit);
    p.free_bike = Some(if free_bike { std::mem::take(&mut p.bike) } else { String::new() });
    p.free_outfit = Some(if free_outfit { std::mem::take(&mut p.outfit) } else { String::new() });
    true
}

#[cfg(test)]
mod series_tests {
    use super::Series;

    #[test]
    fn a_series_scores_the_top_ten_and_ranks_by_points_then_name() {
        let s = Series { races: 2, run: 0, points: vec![0; 12], on: true };
        // Racers 10 and 11 came 11th and 12th: no points.
        let standings = [3, 0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11];
        let p = s.with_race(&standings);
        assert_eq!(p[3], 20);
        assert_eq!(p[0], 17);
        assert_eq!(p[9], 1);
        assert_eq!(p[10], 0);
        assert_eq!(p[11], 0);
        let names: Vec<String> = ["b", "a"].iter().map(|n| n.to_string()).collect();
        assert_eq!(Series::ranked(&[5, 5], &names), vec![1, 0]);
    }
}
