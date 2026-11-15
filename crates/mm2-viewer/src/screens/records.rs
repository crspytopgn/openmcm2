//! Track records and the High Scores screen (`UI/HISCORES.DAT`,
//! `0x524c50`).
//!
//! The game keeps ten records per track for each of three tables: the
//! fastest lap and the best five and ten lap races (for the Stunt Quarry
//! the best single stunt and five and ten minute sessions, for Moto-Tag
//! the time spent "it"), each a name and a time or score
//! (`TrackRecords`, `0x524630`: ten 20 byte entries of a 16 letter name
//! and a float). Here each track's tables are a small text file in the
//! user's data folder, `record/<event>/<track>.txt`, which [`add`]
//! updates after a race.

use super::{Cx, Go, Screen};
use crate::ui::{Dlg, Event};
use std::path::{Path, PathBuf};

/// Records per table (`0x524630`).
pub const KEEP: usize = 10;

/// The event tabs, as the game numbers the events, with their text
/// (`0x524c50` open, strings 5153 to 5158).
const TABS: [(&str, u32, u32); 6] = [
    ("TabQuarry", 0, 5155),
    ("TabBaja", 1, 5153),
    ("TabNationals", 2, 5157),
    ("TabSupercross", 3, 5156),
    ("TabTag", 4, 5158),
    ("TabEnduro", 5, 5154),
];

const TABLES: [&str; 3] = ["TabLeft", "TabMiddle", "TabRight"];

/// One record: who, and their time in seconds or their score.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub name: String,
    pub value: f32,
}

/// One track's records.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackRecords {
    /// The track's name as the lists show it.
    pub title: String,
    pub tables: [Vec<Record>; 3],
}

impl TrackRecords {
    pub fn parse(text: &str) -> TrackRecords {
        let mut r = TrackRecords::default();
        let mut table = None;
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if let Some(t) = line.strip_prefix("title=") {
                r.title = t.to_string();
            } else if let Some(n) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                table = n.parse::<usize>().ok().filter(|&n| n < 3);
            } else if let (Some(t), Some((name, value))) = (table, line.rsplit_once('\t')) {
                if let Ok(value) = value.trim().parse() {
                    r.tables[t].push(Record {
                        name: name.to_string(),
                        value,
                    });
                }
            }
        }
        r
    }

    pub fn text(&self) -> String {
        let mut out = format!("title={}\n", self.title);
        for (i, t) in self.tables.iter().enumerate() {
            out += &format!("[{i}]\n");
            for r in t {
                out += &format!("{}\t{}\n", r.name, r.value);
            }
        }
        out
    }
}

/// Whether a bigger value is the better one: scores, and Moto-Tag's
/// time spent "it".
pub fn higher_wins(event: u32) -> bool {
    matches!(event, 0 | 4)
}

fn folder(user: &Path, event: u32) -> PathBuf {
    user.join("record").join(event.to_string())
}

/// A file name for a track (and course, for Baja and Enduro).
fn key(track: &Path, course: usize) -> String {
    let stem = track
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("{stem}_{course}.txt")
}

/// Every track of an event that has records, in name order.
pub fn tracks(user: &Path, event: u32) -> Vec<TrackRecords> {
    let mut v: Vec<TrackRecords> = crate::fs::read_dir(folder(user, event))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter_map(|e| crate::fs::read_to_string(e.path()).ok())
                .map(|t| TrackRecords::parse(&t))
                .filter(|r| r.tables.iter().any(|t| !t.is_empty()))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|r| r.title.to_ascii_lowercase());
    v
}

/// Enters a result in a table if it makes the top ten; its place from 0
/// if it did. `table` is 0 for a lap or single stunt, 1 and 2 for the
/// five and ten lap (or minute) events.
pub fn add(
    user: &Path,
    event: u32,
    track: &Path,
    course: usize,
    title: &str,
    table: usize,
    record: Record,
) -> std::io::Result<Option<usize>> {
    let dir = folder(user, event);
    let path = dir.join(key(track, course));
    let mut r = crate::fs::read_to_string(&path)
        .map(|t| TrackRecords::parse(&t))
        .unwrap_or_default();
    r.title = title.to_string();
    let t = &mut r.tables[table.min(2)];
    let better = |a: f32, b: f32| if higher_wins(event) { a > b } else { a < b };
    let at = t
        .iter()
        .position(|o| better(record.value, o.value))
        .unwrap_or(t.len());
    if at >= KEEP {
        return Ok(None);
    }
    t.insert(at, record);
    t.truncate(KEEP);
    crate::fs::create_dir_all(&dir)?;
    crate::fs::write(path, r.text())?;
    Ok(Some(at))
}

/// The race a result goes in the records for: its event (as the game
/// numbers them), track and course, the name the lists show, and the laps
/// it ran (minutes, for the Stunt Quarry).
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub event: u32,
    pub track: PathBuf,
    pub course: usize,
    pub title: String,
    pub length: u32,
}

/// The player's result: their best lap (best single stunt), and the whole
/// race's time (session's score) if they ran it to the end.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Outcome {
    pub best: Option<f32>,
    pub whole: Option<f32>,
}

/// Enters `name`'s result for the race `entry`: the best lap (or stunt)
/// in the first table, and the whole race in the second or third when it
/// ran five or ten laps (minutes). Each table's place from 0, where it
/// made the top ten.
pub fn enter(user: &Path, entry: &Entry, name: &str, result: Outcome) -> std::io::Result<[Option<usize>; 3]> {
    let mut places = [None; 3];
    let whole_table = match entry.length {
        5 => Some(1),
        10 => Some(2),
        _ => None,
    };
    let results = [(Some(0), result.best), (whole_table, result.whole)];
    for (table, value) in results {
        let (Some(table), Some(value)) = (table, value.filter(|v| *v > 0.0)) else {
            continue;
        };
        let record = Record {
            name: name.to_string(),
            value,
        };
        places[table] = add(user, entry.event, &entry.track, entry.course, &entry.title, table, record)?;
    }
    Ok(places)
}

/// A time as the records show it: minutes and hundredths
/// (`0x51dd10`, "%2d:%05.2f").
pub fn time(t: f32) -> String {
    let cs = (t.max(0.0) * 100.0).round() as u32;
    format!("{}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
}

pub struct HighScores {
    dlg: Dlg,
    event: u32,
    table: usize,
    tracks: Vec<TrackRecords>,
    /// The leaderboards' Online button (ours): everyone's best laps from
    /// the leaderboard server instead of this computer's records.
    online: bool,
    boards: Vec<crate::online::BoardInfo>,
    asked_boards: Option<crate::online::Reply>,
    asked_board: Option<crate::online::Reply>,
}

/// The Online button's name and text.
const ONLINE: &str = "TabOnline";
const ONLINE_TEXT: &str = "Online";

/// The events with lap leaderboards.
fn has_boards(event: u32) -> bool {
    matches!(event, 1 | 2 | 3 | 5)
}

impl HighScores {
    pub fn open(cx: &mut Cx) -> Option<HighScores> {
        let mut dlg = Dlg::open(cx.ui, "HISCORES", (0.0, 0.0))?;
        dlg.set_text("TxtTitle", cx.ui.lang(2392));
        for (tab, _, id) in TABS {
            dlg.set_text(tab, cx.ui.lang(id));
        }
        dlg.set_selectable("LstTrack", true);
        // The Online button, a copy of the last event's under it.
        if crate::online::address().is_some() {
            if let (Some(a), Some(b)) = (dlg.index("TabBaja"), dlg.index("TabEnduro")) {
                let (above, last) = (dlg.dialog.controls[a].rect, dlg.dialog.controls[b].rect);
                let step = last[1] - above[1];
                dlg.add_copy("TabEnduro", ONLINE, [last[0], last[1] + step + step / 3, last[2], last[3]]);
                dlg.set_text(ONLINE, ONLINE_TEXT);
            }
        }
        let mut h = HighScores {
            dlg,
            event: 0,
            table: 0,
            tracks: Vec::new(),
            online: false,
            boards: Vec::new(),
            asked_boards: None,
            asked_board: None,
        };
        h.show(cx);
        Some(h)
    }

    /// The event's tracks and its tables' names (`0x5259e0`, `0x525290`).
    fn show(&mut self, cx: &mut Cx) {
        let d = &mut self.dlg;
        for (tab, event, _) in TABS {
            d.set_on(tab, event == self.event);
        }
        d.set_on(ONLINE, self.online);
        d.set_text("TxtTitle", if self.online { "Online Leaderboard".to_string() } else { cx.ui.lang(2392) });
        for tab in &TABLES[1..] {
            d.show(tab, !self.online);
        }
        if self.online {
            return self.show_online(cx);
        }
        let points = matches!(self.event, 0 | 4);
        let names = if points {
            let left = if self.event == 4 { 5163 } else { 5162 };
            [cx.ui.lang(left), cx.ui.lang(5168), cx.ui.lang(5168)]
        } else {
            [cx.ui.lang(5159), cx.ui.lang(5167), cx.ui.lang(5167)]
        };
        for (i, (tab, name)) in TABLES.iter().zip(names).enumerate() {
            let n = [0, 5, 10][i];
            d.set_text(tab, name.replace("%d", &n.to_string()));
            d.set_on(tab, i == self.table);
        }
        d.set_text("TxtTrack", cx.ui.lang(3014));
        d.set_text("TxtStats1", "");
        d.set_text("TxtStats2", "");
        d.set_items("LstStats1", Vec::new(), 0);
        d.set_items("LstStats2", Vec::new(), 0);
        self.tracks = tracks(&cx.user.store.dir, self.event);
        if self.tracks.is_empty() {
            d.set_items("LstTrack", vec![cx.ui.lang(5166)], 0);
            d.enable("LstTrack", false);
            d.mark("LstTrack", None);
        } else {
            let titles = self.tracks.iter().map(|t| t.title.clone()).collect();
            d.set_items("LstTrack", titles, 0);
            d.enable("LstTrack", true);
            d.mark("LstTrack", None);
        }
    }

    /// The Online view: the event's tracks with laps on the server, then a
    /// track's best laps in every class.
    fn show_online(&mut self, cx: &mut Cx) {
        let d = &mut self.dlg;
        d.set_text(TABLES[0], cx.ui.lang(5159));
        d.set_on(TABLES[0], true);
        d.set_text("TxtTrack", cx.ui.lang(3014));
        d.set_text("TxtStats1", "");
        d.set_text("TxtStats2", "");
        d.set_items("LstStats1", Vec::new(), 0);
        d.set_items("LstStats2", Vec::new(), 0);
        self.boards.clear();
        self.asked_board = None;
        let url = crate::online::address();
        match (has_boards(self.event), url) {
            (true, Some(url)) => {
                d.set_items("LstTrack", vec!["Fetching...".into()], 0);
                d.enable("LstTrack", false);
                self.asked_boards = Some(crate::online::get(format!("{url}/boards?event={}", self.event)));
            }
            _ => {
                d.set_items("LstTrack", vec![cx.ui.lang(5166)], 0);
                d.enable("LstTrack", false);
                self.asked_boards = None;
            }
        }
        d.mark("LstTrack", None);
    }

    /// A board's name in the track list.
    fn board_line(&self, b: &crate::online::BoardInfo) -> String {
        let mut name = if b.title.is_empty() { b.track.to_ascii_uppercase() } else { b.title.clone() };
        if matches!(self.event, 1 | 5) && b.course > 0 {
            name += &format!(" ({})", b.course + 1);
        }
        format!("{name} [{}]", b.riders)
    }

    /// The answers from the server, as they come.
    fn poll(&mut self, cx: &mut Cx) {
        if let Some(r) = self.asked_boards.as_ref().and_then(crate::online::take) {
            self.asked_boards = None;
            let d = &mut self.dlg;
            match r {
                Ok(text) => {
                    self.boards = crate::online::parse_boards(&text);
                    if self.boards.is_empty() {
                        d.set_items("LstTrack", vec![cx.ui.lang(5166)], 0);
                    } else {
                        let lines = self.boards.iter().map(|b| self.board_line(b)).collect();
                        let d = &mut self.dlg;
                        d.set_items("LstTrack", lines, 0);
                        d.enable("LstTrack", true);
                    }
                }
                Err(e) => {
                    eprintln!("the leaderboard server: {e}");
                    d.set_items("LstTrack", vec!["Can't reach the leaderboard".into()], 0);
                }
            }
            self.dlg.mark("LstTrack", None);
        }
        if let Some(r) = self.asked_board.as_ref().and_then(crate::online::take) {
            self.asked_board = None;
            let rows = r.map(|t| crate::online::parse_board(&t)).unwrap_or_default();
            let names = rows.iter().enumerate().map(|(i, (rider, _, class))| format!("{}. {rider} ({class}cc)", i + 1)).collect();
            let times = rows.iter().map(|(_, ms, _)| time(*ms as f32 / 1000.0)).collect();
            self.dlg.set_items("LstStats1", names, 0);
            self.dlg.set_items("LstStats2", times, 0);
        }
    }

    /// A track's best laps on the server.
    fn show_board(&mut self, cx: &mut Cx, line: usize) {
        let (Some(b), Some(url)) = (self.boards.get(line), crate::online::address()) else {
            return;
        };
        let d = &mut self.dlg;
        d.set_text("TxtStats1", cx.ui.lang(3012));
        d.set_text("TxtStats2", cx.ui.lang(3013));
        d.set_items("LstStats1", vec!["Fetching...".into()], 0);
        d.set_items("LstStats2", Vec::new(), 0);
        let q = format!(
            "{url}/board?track={}&event={}&course={}",
            crate::online::escape(&b.track),
            self.event,
            b.course
        );
        self.asked_board = Some(crate::online::get(q));
    }

    /// A track's table (`0x525b30`): names, and times or scores.
    fn show_track(&mut self, cx: &mut Cx, line: usize) {
        if self.online {
            return self.show_board(cx, line);
        }
        let Some(t) = self.tracks.get(line) else {
            return;
        };
        let d = &mut self.dlg;
        d.set_text("TxtStats1", cx.ui.lang(3012));
        let second = if self.event == 0 { 2360 } else { 3013 };
        d.set_text("TxtStats2", cx.ui.lang(second));
        let rows = &t.tables[self.table];
        let names = rows.iter().map(|r| r.name.clone()).collect();
        let values = rows
            .iter()
            .map(|r| {
                if self.event == 0 {
                    format!("{}", r.value as i64)
                } else {
                    time(r.value)
                }
            })
            .collect();
        d.set_items("LstStats1", names, 0);
        d.set_items("LstStats2", values, 0);
    }
}

impl Screen for HighScores {
    fn layers(&mut self) -> Vec<&mut Dlg> {
        vec![&mut self.dlg]
    }

    fn tick(&mut self, cx: &mut Cx) -> Go {
        self.poll(cx);
        Go::Stay
    }

    fn event(&mut self, cx: &mut Cx, e: Event) -> Go {
        match e {
            Event::Clicked(n) => {
                if n.eq_ignore_ascii_case("Back") {
                    return Go::Back;
                }
                if n.eq_ignore_ascii_case(ONLINE) {
                    self.online = !self.online;
                    self.table = 0;
                    // Only the lap races have leaderboards.
                    if self.online && !has_boards(self.event) {
                        self.event = 2;
                    }
                    self.show(cx);
                } else if let Some((_, event, _)) = TABS.iter().find(|t| t.0.eq_ignore_ascii_case(&n)) {
                    self.event = *event;
                    self.show(cx);
                } else if self.online {
                    self.dlg.set_on(TABLES[0], true);
                } else if let Some(i) = TABLES.iter().position(|t| t.eq_ignore_ascii_case(&n)) {
                    let line = self.dlg.marked("LstTrack");
                    self.table = i;
                    self.show(cx);
                    if let Some(l) = line {
                        self.dlg.mark("LstTrack", Some(l));
                        self.show_track(cx, l);
                    }
                }
            }
            Event::Selected(n, line) if n.eq_ignore_ascii_case("LstTrack") => {
                self.show_track(cx, line)
            }
            _ => {}
        }
        Go::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_race_goes_in_its_tables() {
        let dir = std::env::temp_dir().join(format!("mm2-records-enter-{}", std::process::id()));
        let entry = |length| Entry {
            event: 2,
            track: PathBuf::from("TERAFORM/NATIONAL/CHANOS.ENV"),
            course: 0,
            title: "Chanos".into(),
            length,
        };
        let done = Outcome {
            best: Some(52.5),
            whole: Some(270.0),
        };
        // Five laps: the best lap and the race.
        assert_eq!(enter(&dir, &entry(5), "Daniel", done).unwrap(), [Some(0), Some(0), None]);
        // Three laps: only the best lap has a table.
        assert_eq!(enter(&dir, &entry(3), "Daniel", done).unwrap(), [Some(1), None, None]);
        // Not finished: only the best lap.
        let crashed = Outcome { best: Some(50.0), whole: None };
        assert_eq!(enter(&dir, &entry(10), "Daniel", crashed).unwrap(), [Some(0), None, None]);
        // No lap done: nothing.
        assert_eq!(enter(&dir, &entry(10), "Daniel", Outcome::default()).unwrap(), [None; 3]);
        let saved = tracks(&dir, 2);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].title, "Chanos");
        let times = |t: usize| saved[0].tables[t].iter().map(|r| r.value).collect::<Vec<_>>();
        assert_eq!(times(0), [50.0, 52.5, 52.5]);
        assert_eq!(times(1), [270.0]);
        assert!(times(2).is_empty());
        crate::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tables() {
        let dir = std::env::temp_dir().join(format!("mm2-records-{}", std::process::id()));
        let track = Path::new("TERAFORM/SX/SX01.env");
        let rec = |n: &str, v| Record {
            name: n.into(),
            value: v,
        };
        assert_eq!(
            add(&dir, 3, track, 0, "Arena", 0, rec("A", 40.0)).unwrap(),
            Some(0)
        );
        assert_eq!(
            add(&dir, 3, track, 0, "Arena", 0, rec("B", 35.0)).unwrap(),
            Some(0)
        );
        assert_eq!(
            add(&dir, 3, track, 0, "Arena", 0, rec("C", 50.0)).unwrap(),
            Some(2)
        );
        for i in 0..8 {
            add(
                &dir,
                3,
                track,
                0,
                "Arena",
                0,
                rec("D", 45.0 + i as f32 * 0.1),
            )
            .unwrap();
        }
        assert_eq!(
            add(&dir, 3, track, 0, "Arena", 0, rec("E", 60.0)).unwrap(),
            None
        );
        let t = tracks(&dir, 3);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].tables[0].len(), KEEP);
        assert_eq!(t[0].tables[0][0].name, "B");
        // Scores count the other way.
        add(&dir, 0, track, 0, "Quarry", 0, rec("A", 100.0)).unwrap();
        assert_eq!(
            add(&dir, 0, track, 0, "Quarry", 0, rec("B", 900.0)).unwrap(),
            Some(0)
        );
        crate::fs::remove_dir_all(&dir).ok();
        assert_eq!(time(83.456), "1:23.46");
    }
}
