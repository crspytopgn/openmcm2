//! Motocross Madness 2's own messages, the ones its players send each
//! other inside DirectPlay: the lobby's (who is ready on what bike, the
//! host's event, the start), the chat, and the race's few that aren't a
//! bike's state (a bike's state is the same packet as on the game's
//! replay tapes).
//!
//! Every message starts with its type in the first byte. A type with the
//! top bit set is one the game makes sure of itself: its sequence number
//! is in bytes 2 and 3 and the receiver answers with [`ACK`] carrying the
//! same number. A type with bit `0x20` set carries the player it is
//! really for in four bytes at its end (the client and server mode, which
//! LAN games don't use).
//!
//! The layouts are the game's own structures, little endian, as they go
//! over the network.

/// The types.
pub mod kind {
    /// A bike's whole state (88 bytes).
    pub const BIKE: u8 = 1;
    /// A lobby player's state: ready, bike and rider (212 bytes).
    pub const PLAYER: u8 = 2;
    /// A bike's score in the stunt events, sent every two seconds.
    pub const SCORE: u8 = 10;
    /// A bike's state as small changes since the last (23 bytes).
    pub const DELTA: u8 = 13;
    /// The host's event and options (180 bytes).
    pub const SETTINGS: u8 = 0x0f;
    /// The receiver's answer to a message the game makes sure of.
    pub const ACK: u8 = 0x40;
    /// "Still here", every second or so in a race.
    pub const KEEPALIVE: u8 = 0x4b;
    /// The host's start (and its track changes) with the computer riders.
    pub const START: u8 = 0x83;
    /// A player has loaded the race (the time it says is when).
    pub const LOADED: u8 = 0x84;
    /// A line of chat.
    pub const CHAT: u8 = 0x85;
    /// A racer's result, when the race is over for its machine.
    pub const RESULT: u8 = 0x86;
    /// Moto-Tag: who has the tag.
    pub const TAG: u8 = 0x87;
    /// A player left the race for the menus.
    pub const QUIT: u8 = 0x89;
    /// The host has removed a player.
    pub const REMOVE: u8 = 0x8e;
    /// A line of chat in the system's name ("Orig is ready").
    pub const NOTICE: u8 = 0x92;
    /// A player has gone (it timed out, or couldn't load the race).
    pub const GONE: u8 = 0xcc;
}

/// The players a session holds.
pub const MAX_PLAYERS: usize = 8;

/// Whether the game makes sure of a message of this type.
pub fn guaranteed(kind: u8) -> bool {
    kind & 0x80 != 0
}

/// A guaranteed message's sequence number.
pub fn sequence(data: &[u8]) -> Option<u16> {
    Some(u16::from_le_bytes([*data.get(2)?, *data.get(3)?]))
}

/// Stamps `data` with its type and, for a guaranteed one, `sequence`.
pub fn stamp(data: &mut [u8], kind: u8, sequence: u16) {
    data[0] = kind;
    if guaranteed(kind) && data.len() >= 4 {
        data[2..4].copy_from_slice(&sequence.to_le_bytes());
    }
}

/// The answer to guaranteed message `sequence`.
pub fn ack(sequence: u16) -> Vec<u8> {
    let mut d = vec![kind::ACK, 0, 0, 0];
    d[2..4].copy_from_slice(&sequence.to_le_bytes());
    d
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4).map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    u32_at(b, o) as i32
}

fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_bits(u32_at(b, o))
}

fn put_u32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// A C string in `len` bytes.
fn text_at(b: &[u8], o: usize, len: usize) -> String {
    let s = b.get(o..(o + len).min(b.len())).unwrap_or_default();
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    s[..end].iter().map(|&c| c as char).collect()
}

/// Writes `s` as a C string in `len` bytes, cut to fit, its letters
/// Latin-1 (others become `?`).
fn put_text(b: &mut [u8], o: usize, len: usize, s: &str) {
    let room = &mut b[o..o + len];
    room.fill(0);
    for (slot, c) in room.iter_mut().take(len - 1).zip(s.chars()) {
        *slot = if (c as u32) < 0x100 { c as u8 } else { b'?' };
    }
}

/// A lobby player's state (`PlayerStateMessage`, type 2): sent about
/// once a second by each player, and when it changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerState {
    pub ready: bool,
    /// The bike's paint (`Kazuyuki250_08.tga`), its model (`suzuki.mcf`)
    /// and the rider's outfit (`Rainbow01_08.tga`).
    pub bike: String,
    pub model: String,
    pub rider: String,
    pub bike_index: u8,
    /// The computer riders this player asks to run.
    pub requested: u8,
    /// The sender's clock (milliseconds) when it went.
    pub time: u32,
    pub cc: i32,
    pub four_stroke: bool,
    pub custom_skin: u8,
    pub plate: i32,
}

impl PlayerState {
    pub const LEN: usize = 0xd4;

    pub fn read(b: &[u8]) -> Option<PlayerState> {
        if b.len() < Self::LEN || b[0] != kind::PLAYER {
            return None;
        }
        Some(PlayerState {
            ready: b[1] != 0,
            bike: text_at(b, 0x02, 0x40),
            model: text_at(b, 0x42, 0x40),
            rider: text_at(b, 0x82, 0x40),
            bike_index: b[0xc2],
            requested: b[0xc3],
            time: u32_at(b, 0xc4),
            cc: i32_at(b, 0xc8),
            four_stroke: b[0xcc] != 0,
            custom_skin: b[0xcd],
            plate: i32_at(b, 0xd0),
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut b = vec![0u8; Self::LEN];
        b[0] = kind::PLAYER;
        b[1] = self.ready as u8;
        put_text(&mut b, 0x02, 0x40, &self.bike);
        put_text(&mut b, 0x42, 0x40, &self.model);
        put_text(&mut b, 0x82, 0x40, &self.rider);
        b[0xc2] = self.bike_index;
        b[0xc3] = self.requested;
        put_u32(&mut b, 0xc4, self.time);
        put_u32(&mut b, 0xc8, self.cc as u32);
        b[0xcc] = self.four_stroke as u8;
        b[0xcd] = self.custom_skin;
        put_u32(&mut b, 0xd0, self.plate as u32);
        b
    }
}

/// One player's place in the host's list of who runs how many computer
/// riders (`LobbySlot`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Slot {
    pub player: u32,
    pub requested: i32,
    pub granted: i32,
}

fn read_slots(b: &[u8], o: usize) -> [Slot; 8] {
    let mut s = [Slot::default(); 8];
    for (i, slot) in s.iter_mut().enumerate() {
        let at = o + i * 12;
        *slot = Slot { player: u32_at(b, at), requested: i32_at(b, at + 4), granted: i32_at(b, at + 8) };
    }
    s
}

fn put_slots(b: &mut [u8], o: usize, slots: &[Slot; 8]) {
    for (i, s) in slots.iter().enumerate() {
        let at = o + i * 12;
        put_u32(b, at, s.player);
        put_u32(b, at + 4, s.requested as u32);
        put_u32(b, at + 8, s.granted as u32);
    }
}

/// The host's event and options (`LobbySettingsMessage`, type 0x0f):
/// sent every two seconds and when they change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    /// The stunt and tag events' time limit.
    pub minutes: u8,
    /// The event: 0 stunt quarry, 1 baja, 2 nationals, 3 supercross,
    /// 4 tag, 5 enduro.
    pub event: u8,
    /// 0 practice, 1 single race, 2 championship.
    pub mode: u8,
    pub laps: u8,
    /// Computer riders.
    pub opponents: u8,
    pub players: u8,
    pub races: u8,
    /// Which folder the track is in, by event number (the tag event's
    /// tracks come from three).
    pub series: u8,
    pub trees: bool,
    pub riders: bool,
    pub fast_finish: bool,
    pub bike_class: u8,
    pub tag_ball: bool,
    pub stunts: bool,
    pub difficulty: u8,
    /// The course on the track (baja and enduro tracks hold several).
    pub course: u8,
    /// The track's file's time and size: a player whose file is another
    /// size can't press Ready.
    pub file_time: u32,
    pub file_size: u32,
    /// The track's file name without `.env` (`Quarry01`).
    pub track: String,
    pub slots: [Slot; 8],
}

impl Settings {
    pub const LEN: usize = 0xb4;

    pub fn read(b: &[u8]) -> Option<Settings> {
        if b.len() < Self::LEN || b[0] != kind::SETTINGS {
            return None;
        }
        Some(Settings {
            minutes: b[4] & 0x1f,
            event: b[4] >> 5,
            mode: b[5] & 7,
            laps: b[5] >> 3,
            opponents: b[6] & 0xf,
            players: b[6] >> 4,
            races: b[7] & 7,
            series: (b[7] >> 3) & 7,
            trees: b[7] & 0x40 != 0,
            riders: b[7] & 0x80 != 0,
            fast_finish: b[8] & 1 != 0,
            bike_class: (b[8] >> 1) & 7,
            tag_ball: b[8] & 0x10 != 0,
            stunts: b[8] & 0x20 != 0,
            difficulty: b[8] >> 6,
            course: b[9],
            file_time: u32_at(b, 0x0c),
            file_size: u32_at(b, 0x10),
            track: text_at(b, 0x14, 0x40),
            slots: read_slots(b, 0x54),
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut b = vec![0u8; Self::LEN];
        b[0] = kind::SETTINGS;
        b[4] = (self.minutes & 0x1f) | (self.event & 7) << 5;
        b[5] = (self.mode & 7) | (self.laps & 0x1f) << 3;
        b[6] = (self.opponents & 0xf) | (self.players & 0xf) << 4;
        b[7] = (self.races & 7) | (self.series & 7) << 3 | (self.trees as u8) << 6 | (self.riders as u8) << 7;
        b[8] = self.fast_finish as u8
            | (self.bike_class & 7) << 1
            | (self.tag_ball as u8) << 4
            | (self.stunts as u8) << 5
            | (self.difficulty & 3) << 6;
        b[9] = self.course;
        put_u32(&mut b, 0x0c, self.file_time);
        put_u32(&mut b, 0x10, self.file_size);
        put_text(&mut b, 0x14, 0x40, &self.track);
        put_slots(&mut b, 0x54, &self.slots);
        b
    }
}

/// A start grid place: whose racer, and which of theirs (0 is the player
/// themself, more a computer rider their machine runs).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Grid {
    pub player: u32,
    pub points: u8,
    pub racer: u8,
}

/// A computer rider in the start message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Computer {
    pub plate: i32,
    pub name: String,
    pub model: String,
    pub bike: String,
    pub rider: String,
    pub cc: i32,
    pub four_stroke: bool,
}

/// The host's start (`LobbyStartMessage`, type 0x83): with `start`, the
/// race begins; without it the host only changed the track. It carries
/// the game's whole race settings block (`RaceSettings`, the replay
/// tapes' too).
#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    pub start: bool,
    /// The race settings block as the game keeps it (0x1ec bytes); the
    /// fields below read and write it.
    pub settings: Vec<u8>,
    pub difficulty: i32,
    /// The racers in order: the players' first (only their plates and
    /// names count), then the computer riders.
    pub racers: Vec<Computer>,
}

/// Offsets in the race settings block.
pub mod rs {
    pub const LEN: usize = 0x1ec;
    pub const MODE: usize = 0x00;
    pub const EVENT: usize = 0x04;
    pub const SERIES: usize = 0x08;
    pub const RACES: usize = 0x0c;
    pub const TREES: usize = 0x10;
    pub const RIDERS: usize = 0x14;
    pub const FAST_FINISH: usize = 0x18;
    pub const BIKE_CLASS: usize = 0x1c;
    pub const LAPS: usize = 0x20;
    pub const COMPUTERS: usize = 0x28;
    pub const RANDOM_GATES: usize = 0x2c;
    pub const GATE_COUNT: usize = 0x2d;
    pub const GATE_SEED: usize = 0x30;
    pub const COURSE: usize = 0x34;
    pub const PLAYERS: usize = 0x35;
    pub const TRACK: usize = 0x36;
    pub const TRACK_LEN: usize = 0x20;
    pub const FILE_TIME: usize = 0x138;
    pub const FILE_SIZE: usize = 0x13c;
    pub const MINUTES: usize = 0x140;
    pub const TAG_BALL: usize = 0x144;
    pub const STUNTS: usize = 0x148;
    pub const GRID: usize = 0x14c;
    pub const SLOTS: usize = 0x18c;
}

impl Start {
    pub const LEN: usize = 0x8d8;
    const SETTINGS: usize = 0x08;

    pub fn read(b: &[u8]) -> Option<Start> {
        if b.len() < Self::LEN || b[0] != kind::START {
            return None;
        }
        let settings = b[Self::SETTINGS..Self::SETTINGS + rs::LEN].to_vec();
        let racers = (0..MAX_PLAYERS)
            .map(|i| Computer {
                plate: i32_at(b, 0x1f8 + i * 4),
                name: text_at(b, 0x218 + i * 0x10, 0x10),
                model: text_at(b, 0x298 + i * 0x40, 0x40),
                bike: text_at(b, 0x498 + i * 0x40, 0x40),
                rider: text_at(b, 0x698 + i * 0x40, 0x40),
                cc: i32_at(b, 0x898 + i * 4),
                four_stroke: u32_at(b, 0x8b8 + i * 4) != 0,
            })
            .collect();
        Some(Start { start: b[4] != 0 || b[5] != 0, settings, difficulty: i32_at(b, 0x1f4), racers })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut b = vec![0u8; Self::LEN];
        b[0] = kind::START;
        b[4] = self.start as u8;
        let n = self.settings.len().min(rs::LEN);
        b[Self::SETTINGS..Self::SETTINGS + n].copy_from_slice(&self.settings[..n]);
        put_u32(&mut b, 0x1f4, self.difficulty as u32);
        for (i, r) in self.racers.iter().take(MAX_PLAYERS).enumerate() {
            put_u32(&mut b, 0x1f8 + i * 4, r.plate as u32);
            put_text(&mut b, 0x218 + i * 0x10, 0x10, &r.name);
            put_text(&mut b, 0x298 + i * 0x40, 0x40, &r.model);
            put_text(&mut b, 0x498 + i * 0x40, 0x40, &r.bike);
            put_text(&mut b, 0x698 + i * 0x40, 0x40, &r.rider);
            put_u32(&mut b, 0x898 + i * 4, r.cc as u32);
            put_u32(&mut b, 0x8b8 + i * 4, r.four_stroke as u32);
        }
        b
    }

    fn int(&self, o: usize) -> i32 {
        i32_at(&self.settings, o)
    }

    pub fn mode(&self) -> i32 {
        self.int(rs::MODE)
    }
    pub fn event(&self) -> i32 {
        self.int(rs::EVENT)
    }
    pub fn series(&self) -> i32 {
        self.int(rs::SERIES)
    }
    pub fn laps(&self) -> i32 {
        self.int(rs::LAPS)
    }
    pub fn computers(&self) -> i32 {
        self.int(rs::COMPUTERS)
    }
    pub fn players(&self) -> u8 {
        self.settings[rs::PLAYERS]
    }
    pub fn course(&self) -> u8 {
        self.settings[rs::COURSE]
    }
    pub fn track(&self) -> String {
        text_at(&self.settings, rs::TRACK, rs::TRACK_LEN)
    }
    pub fn minutes(&self) -> f32 {
        f32_at(&self.settings, rs::MINUTES)
    }
    pub fn grid(&self) -> Vec<Grid> {
        (0..MAX_PLAYERS)
            .map(|i| {
                let at = rs::GRID + i * 8;
                Grid { player: u32_at(&self.settings, at), points: self.settings[at + 4], racer: self.settings[at + 5] }
            })
            .collect()
    }
    pub fn slots(&self) -> [Slot; 8] {
        read_slots(&self.settings, rs::SLOTS)
    }

    /// A race settings block for `s`, the lobby's settings, with the
    /// start grid.
    pub fn settings_from(s: &Settings, grid: &[Grid]) -> Vec<u8> {
        let mut b = vec![0u8; rs::LEN];
        put_u32(&mut b, rs::MODE, s.mode as u32);
        put_u32(&mut b, rs::EVENT, s.event as u32);
        put_u32(&mut b, rs::SERIES, s.series as u32);
        put_u32(&mut b, rs::RACES, s.races as u32);
        put_u32(&mut b, rs::TREES, s.trees as u32);
        put_u32(&mut b, rs::RIDERS, s.riders as u32);
        put_u32(&mut b, rs::FAST_FINISH, s.fast_finish as u32);
        put_u32(&mut b, rs::BIKE_CLASS, s.bike_class as u32);
        put_u32(&mut b, rs::LAPS, s.laps as u32);
        put_u32(&mut b, rs::COMPUTERS, s.opponents as u32);
        b[rs::GATE_COUNT] = 30;
        put_u32(&mut b, rs::GATE_SEED, 1);
        b[rs::COURSE] = s.course;
        b[rs::PLAYERS] = s.players;
        put_text(&mut b, rs::TRACK, rs::TRACK_LEN, &s.track);
        put_u32(&mut b, rs::FILE_TIME, s.file_time);
        put_u32(&mut b, rs::FILE_SIZE, s.file_size);
        put_u32(&mut b, rs::MINUTES, (s.minutes as f32).to_bits());
        put_u32(&mut b, rs::TAG_BALL, s.tag_ball as u32);
        put_u32(&mut b, rs::STUNTS, s.stunts as u32);
        for (i, g) in grid.iter().take(MAX_PLAYERS).enumerate() {
            let at = rs::GRID + i * 8;
            put_u32(&mut b, at, g.player);
            b[at + 4] = g.points;
            b[at + 5] = g.racer;
        }
        put_slots(&mut b, rs::SLOTS, &s.slots);
        b
    }
}

/// A line of chat (`ChatMessage`, types 0x85 and 0x92): the text after
/// the header, up to 73 letters.
pub fn chat(kind: u8, text: &str) -> Vec<u8> {
    let mut b = vec![0u8; 0x4e];
    b[0] = kind;
    put_text(&mut b, 4, 0x4a, text);
    b
}

pub fn read_chat(b: &[u8]) -> Option<String> {
    (b.len() > 4).then(|| text_at(b, 4, 0x4a))
}

/// A message about one player (types 0x4b, 0xcc and 0x8e).
pub fn about(kind: u8, player: u32) -> Vec<u8> {
    let mut b = vec![0u8; 8];
    b[0] = kind;
    put_u32(&mut b, 4, player);
    b
}

pub fn read_about(b: &[u8]) -> Option<u32> {
    (b.len() >= 8).then(|| u32_at(b, 4))
}

/// "Loaded" (type 0x84) with the sender's clock.
pub fn loaded(clock: u32) -> Vec<u8> {
    about(kind::LOADED, clock)
}

/// A racer's result (`RacerResultsMessage`, type 0x86).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Result {
    pub racer: u8,
    pub finished: bool,
    pub score: f32,
    /// The racer's whole time, seconds (`f32::MAX` when it didn't finish).
    pub time: f32,
    pub best_lap: f32,
    pub best_stunt: i32,
    pub trick_time: f32,
    pub best_tag: f32,
    pub total_tag: f32,
}

impl Result {
    pub const LEN: usize = 0x24;

    pub fn read(b: &[u8]) -> Option<Result> {
        if b.len() < Self::LEN || b[0] != kind::RESULT {
            return None;
        }
        Some(Result {
            racer: b[4],
            finished: b[5] != 0,
            score: f32_at(b, 0x08),
            time: f32_at(b, 0x0c),
            best_lap: f32_at(b, 0x10),
            best_stunt: i32_at(b, 0x14),
            trick_time: f32_at(b, 0x18),
            best_tag: f32_at(b, 0x1c),
            total_tag: f32_at(b, 0x20),
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut b = vec![0u8; Self::LEN];
        b[0] = kind::RESULT;
        b[4] = self.racer;
        b[5] = self.finished as u8;
        put_u32(&mut b, 0x08, self.score.to_bits());
        put_u32(&mut b, 0x0c, self.time.to_bits());
        put_u32(&mut b, 0x10, self.best_lap.to_bits());
        put_u32(&mut b, 0x14, self.best_stunt as u32);
        put_u32(&mut b, 0x18, self.trick_time.to_bits());
        put_u32(&mut b, 0x1c, self.best_tag.to_bits());
        put_u32(&mut b, 0x20, self.total_tag.to_bits());
        b
    }
}

/// A stunt event's score (type 10): the bike (0 the player's own, more a
/// computer rider) and its total so far.
pub fn score(racer: u8, total: f32) -> Vec<u8> {
    let mut b = vec![0u8; 8];
    b[0] = kind::SCORE;
    b[1] = racer;
    put_u32(&mut b, 4, total.to_bits());
    b
}

pub fn read_score(b: &[u8]) -> Option<(u8, f32)> {
    (b.len() >= 8 && b[0] == kind::SCORE).then(|| (b[1], f32_at(b, 4)))
}

/// "Left the race" (type 0x89) with the player's name.
pub fn quit(name: &str) -> Vec<u8> {
    let mut b = vec![0u8; 0x14];
    b[0] = kind::QUIT;
    put_text(&mut b, 4, 0x10, name);
    b
}

/// Which of a sender's bikes a bike packet is about (0 its player, more
/// a computer rider): byte 0x54 of a whole one, 0x16 of a delta.
pub fn bike_racer(b: &[u8]) -> Option<u8> {
    match *b.first()? {
        kind::BIKE => b.get(0x54).copied(),
        kind::DELTA => b.get(0x16).copied(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// The demo host's settings, as captured: Quarry01, the stunt event,
    /// two minutes, two players.
    const SETTINGS: &str = "0f000000022820c288001000e1fc0a6bb152b00051756172727930310074726f6c2c003858f0cc00e4f378000a00000000000000427574526479506c";

    #[test]
    fn settings() {
        let mut b = hex(SETTINGS);
        b.resize(Settings::LEN, 0);
        let s = Settings::read(&b).unwrap();
        assert_eq!((s.minutes, s.event, s.mode, s.laps), (2, 0, 0, 5));
        assert_eq!((s.opponents, s.players, s.races, s.series), (0, 2, 2, 0));
        assert!(s.trees && s.riders && !s.fast_finish);
        assert_eq!((s.bike_class, s.difficulty, s.course), (4, 2, 0));
        assert_eq!((s.file_size, s.track.as_str()), (11_555_505, "Quarry01"));
        // The bits come back the same; the junk after the name doesn't.
        let w = s.write();
        assert_eq!(&w[..0x0a], &b[..0x0a]);
        assert_eq!(&w[0x0c..0x1d], &b[0x0c..0x1d]);
        assert_eq!(Settings::read(&w).unwrap(), s);
    }

    #[test]
    fn player_state() {
        let p = PlayerState {
            ready: true,
            bike: "Kazuyuki250_08.tga".into(),
            model: "suzuki.mcf".into(),
            rider: "Rainbow01_08.tga".into(),
            requested: 2,
            time: 1286487,
            cc: 250,
            plate: 101,
            ..Default::default()
        };
        let b = p.write();
        assert_eq!(b.len(), 212);
        assert_eq!(&b[..22], &hex("02014b617a7579756b693235305f30382e746761000000")[..22]);
        assert_eq!(PlayerState::read(&b).unwrap(), p);
    }

    #[test]
    fn start_round_trip() {
        let s = Settings { event: 3, mode: 1, laps: 3, players: 2, opponents: 1, track: "SX01".into(), file_size: 1234, ..Default::default() };
        let grid = [Grid { player: 0x10f2fe, points: 0, racer: 0 }, Grid { player: 0x10f2fb, points: 0, racer: 0 }];
        let st = Start {
            start: true,
            settings: Start::settings_from(&s, &grid),
            difficulty: 2,
            racers: vec![Computer::default(), Computer::default(), Computer { name: "Bob".into(), model: "honda.mcf".into(), cc: 250, plate: 7, ..Default::default() }],
        };
        let b = st.write();
        assert_eq!(b.len(), 0x8d8);
        let r = Start::read(&b).unwrap();
        assert!(r.start);
        assert_eq!((r.event(), r.mode(), r.laps(), r.computers(), r.players()), (3, 1, 3, 1, 2));
        assert_eq!(r.track(), "SX01");
        assert_eq!(r.grid()[1].player, 0x10f2fb);
        assert_eq!(r.racers[2].name, "Bob");
        assert_eq!(r.racers[2].cc, 250);
    }

    #[test]
    fn captured_start() {
        // The first 64 bytes of the demo host's start: a stunt race on
        // Quarry01 for two players.
        let mut b = hex("8302010001000000000000000000000000000000020000000100000001000000\
0000000004000000050000000000000000000000001e00000100000000025175");
        b.resize(Start::LEN, 0);
        b[0x40..0x48].copy_from_slice(b"arry01\0\0");
        let s = Start::read(&b).unwrap();
        assert!(s.start);
        assert_eq!((s.event(), s.laps(), s.players(), s.course()), (0, 5, 2, 0));
        assert_eq!(s.track(), "Quarry01");
        assert_eq!(sequence(&b), Some(1));
    }

    #[test]
    fn chat_and_small() {
        let c = chat(kind::CHAT, "hello from joiner");
        assert_eq!(c.len(), 0x4e);
        assert_eq!(read_chat(&c).unwrap(), "hello from joiner");
        assert_eq!(ack(0x1fd), hex("4000fd01"));
        assert_eq!(read_score(&score(0, 150.0)), Some((0, 150.0)));
        let r = Result { racer: 3, finished: true, time: 61.5, ..Default::default() };
        assert_eq!(Result::read(&r.write()), Some(r));
        let mut m = chat(kind::CHAT, "x");
        stamp(&mut m, kind::CHAT, 7);
        assert_eq!(sequence(&m), Some(7));
    }
}
