//! An always-open free roam server (`--serve`): a session host with no
//! window, no game files and no rider of its own, for a computer that
//! stays on. It keeps one free roam session going on one track for good:
//! players find it in the server browser's FREE ROAM list (from the game
//! or a browser, by its race code), drop in by pressing Ready and leave
//! when they like. It only passes the players' bikes and chat between
//! them, as any host does, so it needs very little.
//!
//! `mm2-viewer --serve` takes, all optional:
//!
//! * `--name TEXT`: the session's name in the list ("OpenMCM2 Free Roam").
//! * `--event N` and `--track STEM`: the event's number (2 Nationals, 3
//!   Supercross, 1 Baja, 5 Enduro) and the track's file stem; the Free
//!   Pack's Copper Ridge (`FPNAT01`) by default, so players without the
//!   original's files can ride too.
//! * `--title TEXT`: the track's name as the list shows it.
//! * `--files DIR`: a game or Free Pack folder to give the track from to a
//!   player who hasn't it (the lobby downloads extra).
//! * `--welcome TEXT`: what each player is told on coming.

use crate::frontend::EventKind;
use crate::net::{self, GameInfo, Heard, Msg, Player};
use crate::share::Share;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How often the server polls its players: as often as their bikes come.
const TICK: Duration = Duration::from_millis(10);
/// How often it says how it's doing, in its log.
const REPORT: Duration = Duration::from_secs(600);

pub struct Options {
    pub name: String,
    pub game: GameInfo,
    pub title: String,
    pub files: Option<PathBuf>,
    pub welcome: String,
}

impl Options {
    /// The options from the command line's flags.
    pub fn from_args(flag: impl Fn(&str) -> Option<String>) -> Result<Options, String> {
        let event: u8 = match flag("--event") {
            Some(e) => e.parse().map_err(|_| format!("--event {e}: a number, 1 to 5"))?,
            None => EventKind::Nationals as u8,
        };
        let kind = EventKind::from_number(event as u32).filter(|k| *k != EventKind::Tag && *k != EventKind::Stunt);
        if kind.is_none() {
            return Err(format!("--event {event}: free roam is for 1 Baja, 2 Nationals, 3 Supercross or 5 Enduro"));
        }
        let track = flag("--track").unwrap_or_else(|| "FPNAT01".into());
        let title = flag("--title").unwrap_or_else(|| if track.eq_ignore_ascii_case("FPNAT01") { "Copper Ridge".into() } else { track.clone() });
        let game = GameInfo { event, mode: net::ROAM_MODE, laps: 99, track, ..Default::default() };
        let welcome = flag("--welcome").unwrap_or_else(|| {
            format!("Server: Welcome to free roam at {title}! Press Ready to ride, and leave the race any time to come back here.")
        });
        Ok(Options {
            name: flag("--name").unwrap_or_else(|| "OpenMCM2 Free Roam".into()),
            game,
            title,
            files: flag("--files").map(PathBuf::from),
            welcome,
        })
    }
}

/// Runs the server until it is stopped (Ctrl+C, or its service manager).
pub fn run(o: Options) -> Result<(), crate::BoxError> {
    let mut session = net::Session::serve(&o.name)?;
    session.title = format!("Free roam: {}", o.title);
    session.say(Msg::Game(o.game.clone()));
    // The free roam race starts now, with every place empty; players drop
    // in as they ready.
    session.say(Msg::Start { game: o.game.clone(), players: vec![Player { id: net::EMPTY, ..Default::default() }; net::MAX_PLAYERS] });
    eprintln!(
        "serving free roam \"{}\" on {} (event {}, track {}), TCP port {}",
        o.name, o.title, o.game.event, o.game.track, session.port
    );
    net::set(Some(session));
    let event = EventKind::from_number(o.game.event as u32);
    let track = event.map(|e| (e, o.game.track.as_str()));
    let mut share = Share::default();
    let mut known: HashSet<u8> = HashSet::new();
    let mut code = String::new();
    let mut report = Instant::now();
    let mut riders_seen = 0usize;
    loop {
        let heard = net::with(|s| s.poll()).unwrap_or_default();
        let (players, now_code, notes) = net::with(|s| (s.players.clone(), s.code(), std::mem::take(&mut s.notes))).unwrap_or_default();
        for n in notes {
            eprintln!("{n}");
        }
        if now_code != code && !now_code.is_empty() {
            eprintln!("race code {now_code}");
        }
        code = now_code;
        for h in heard {
            match h {
                Heard::Players => {
                    let ids: HashSet<u8> = players.iter().map(|p| p.id).collect();
                    for p in players.iter().filter(|p| !known.contains(&p.id)) {
                        eprintln!("{} came ({} here)", p.name, players.len());
                        riders_seen += 1;
                        net::with(|s| s.tell(p.id, &Msg::Chat(o.welcome.clone())));
                    }
                    for id in known.difference(&ids) {
                        eprintln!("player {id} went ({} here)", players.len());
                    }
                    known = ids;
                }
                Heard::Chat(line) => eprintln!("chat: {line}"),
                Heard::File(m) => match &o.files {
                    Some(dir) => {
                        let _ = share.heard(dir, m, track, ("", ""));
                    }
                    None => {
                        if let Msg::FileAsk(n) = m {
                            let back = net::FileNote { to: n.from, from: 0, kind: n.kind, name: n.name };
                            net::with(|s| s.send_file(Msg::FileNone(back)));
                        }
                    }
                },
                _ => {}
            }
        }
        share.pump();
        if report.elapsed() > REPORT {
            report = Instant::now();
            let riding = net::with(|s| s.roam.as_ref().map_or(0, |r| r.places.iter().flatten().count())).unwrap_or(0);
            eprintln!("{} here, {riding} riding, {riders_seen} came since it started, race code {code}", players.len());
        }
        std::thread::sleep(TICK);
    }
}
