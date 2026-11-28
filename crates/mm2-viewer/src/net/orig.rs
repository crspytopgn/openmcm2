//! Racing with the original Motocross Madness 2 from 2000, on the local
//! network: its players join an OpenMCM2 host, and OpenMCM2 players join
//! a game the original hosts.
//!
//! The original talks DirectPlay ([`mm2_dplay`]) and its own messages
//! inside it ([`mm2_dplay::game`]); OpenMCM2 talks [`Msg`]. This bridge
//! stands between them. Hosting, each original player is one more
//! connection of the host's session ([`Link::Orig`]), and each OpenMCM2
//! player is a DirectPlay player the originals see, so the session's own
//! code is unchanged. Joining, the original host is the session's
//! connection to "the host", and the bridge says what an OpenMCM2 host
//! would: who's in, the event, the start, everyone's bikes.
//!
//! The original's players send their bikes to everyone themselves, so
//! the host never passes one original's bike on to another.

use super::{take_id, GameInfo, In, Link, Msg, Player, RideInfo, EMPTY, MAX_PLAYERS};
use crate::frontend::EventKind;
use mm2_dplay::game::{self, kind, Computer, Grid, PlayerState, Settings, Slot, Start};
use mm2_dplay::{Event, Session as Dp};
use mm2_formats::vcr::{self, Delta, State};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The original game's DirectPlay application id.
pub const APP: [u8; 16] = [0xc2, 0xbb, 0x4d, 0x18, 0x39, 0xff, 0xcf, 0x11, 0xb1, 0x12, 0x00, 0xa0, 0xc9, 0x0a, 0xea, 0xb6];

/// The session flags the original hosts with: keep alive, DirectPlay's
/// reliable protocol, the host can move, quick over sure.
const FLAGS: u32 = 0xa044;

/// A bike and rider every copy of the original has, demo too: for a
/// player whose own isn't the original's (a Free Pack bike, say), as the
/// original players see them.
const STOCK: (&str, &str, &str, u32) = ("suzuki.mcf", "Kazuyuki250_08.tga", "Rainbow01_08.tga", 250);

/// How often the lobby's states and the host's settings go, as the
/// original sends them.
const STATE_EVERY: Duration = Duration::from_millis(1000);
const SETTINGS_EVERY: Duration = Duration::from_millis(2000);
/// A player heard of but silent this long gets in with the stock ride.
const HELLO_WAIT: Duration = Duration::from_millis(1500);
/// After the first racer finishes, how long until the race is over for
/// everyone (the original's 120 seconds, 30 with fast finishes).
const END_AFTER: Duration = Duration::from_secs(120);

/// An original player's number before a far host has given one.
const NOT_YET: u8 = 255;

static GAME: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Whether OpenMCM2 hosts for the original's players too, and looks for
/// the original's games (Extras; on unless switched off).
pub fn on() -> bool {
    crate::tweaks::more().original_lan
}

/// The game folder, for the tracks' files (the original checks the
/// track's size before a player can press Ready).
pub fn set_game_dir(dir: &Path) {
    if let Ok(mut g) = GAME.lock() {
        *g = Some(dir.to_path_buf());
    }
}

/// One original player as a connection of the host's session, or the
/// original host as a joined session's.
pub struct Peer {
    id: u8,
    tx: Sender<Cmd>,
}

impl Peer {
    pub fn send(&mut self, m: &Msg) -> io::Result<()> {
        self.tx
            .send(Cmd::Port(self.id, m.clone()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the original's game has gone"))
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Dropped(self.id));
    }
}

enum Cmd {
    /// What the session said to this connection.
    Port(u8, Msg),
    /// The session let this connection go.
    Dropped(u8),
}

/// An original player in the session.
struct Orig {
    dp: u32,
    /// The session's number for this player ([`NOT_YET`] until a far
    /// host welcomes them), and the connection their news goes on.
    id: u8,
    link: u8,
    name: String,
    state: Option<PlayerState>,
    /// Hosting: whether the session has had their hello.
    hello: bool,
    since: Instant,
}

/// An OpenMCM2 player as a DirectPlay player on this machine.
struct Mine {
    dp: u32,
    id: u8,
}

/// A race under way: which DirectPlay player and bike each racer is.
struct Race {
    /// By OpenMCM2's racer number: the player sending the bike and the
    /// original's number for it (0 a player's own, more a computer rider).
    racers: Vec<Option<(u32, u8)>>,
    /// The last whole state of each original bike, for its deltas, and
    /// the first clock heard from each player.
    last: HashMap<(u32, u8), State>,
    clock0: HashMap<u32, u32>,
    /// Stunt scores: the originals' totals heard, ours sent.
    heard_score: HashMap<(u32, u8), f32>,
    sent_score: HashMap<usize, f32>,
    /// Our racers' last states sent, for the results.
    ours: HashMap<usize, State>,
    finished: HashMap<usize, bool>,
    first_finish: Option<Instant>,
    results_sent: bool,
    fast_finish: bool,
}

impl Race {
    fn new(racers: Vec<Option<(u32, u8)>>, fast_finish: bool) -> Race {
        Race {
            racers,
            last: HashMap::new(),
            clock0: HashMap::new(),
            heard_score: HashMap::new(),
            sent_score: HashMap::new(),
            ours: HashMap::new(),
            finished: HashMap::new(),
            first_finish: None,
            results_sent: false,
            fast_finish,
        }
    }

    /// The racer a bike from `from` is: a player's own by who sent it, a
    /// computer rider by its number (the same on every machine).
    fn racer_of(&self, from: u32, index: u8) -> Option<usize> {
        self.racers.iter().position(|r| match r {
            Some((dp, 0)) => index == 0 && *dp == from,
            Some((_, i)) => *i == index && index != 0,
            None => false,
        })
    }

    fn finish(&mut self, racer: usize, done: bool) {
        if done && self.first_finish.is_none() {
            self.first_finish = Some(Instant::now());
        }
        self.finished.insert(racer, done);
    }
}

/// A guaranteed message waiting for its answer.
struct Pending {
    seq: u16,
    from: u32,
    to: u32,
    data: Vec<u8>,
    at: Instant,
    tries: u32,
}

/// How often an unanswered guaranteed message goes again (the game's
/// four seconds), and how many times.
const RESEND: Duration = Duration::from_secs(4);
const RESEND_TRIES: u32 = 8;

struct Bridge {
    dp: Dp,
    pending: Vec<Pending>,
    /// Hosting (else joined to an original host).
    host: bool,
    tx: Sender<In>,
    cmd_tx: Sender<Cmd>,
    cmds: Receiver<Cmd>,
    stop: Arc<AtomicBool>,
    /// Hosting for a far host's session (relaying): its numbers for the
    /// originals come in its welcomes.
    relay: bool,
    /// Hosting: the session's player numbers. Joined: the next number.
    next: Option<Arc<AtomicU8>>,
    next_id: u8,
    orig: Vec<Orig>,
    mine: Vec<Mine>,
    /// Hosting: the session's players, as it last said.
    players: Vec<Player>,
    /// Joined: this player.
    me: Player,
    game: Option<GameInfo>,
    settings: Option<Settings>,
    race: Option<Race>,
    /// Chat lines that came from the originals, so they don't go back.
    heard_chat: VecDeque<String>,
    seq: u16,
    epoch: Instant,
    states_at: Instant,
    settings_at: Instant,
    closed: bool,
    /// Set when the bridge ends: the router's ports go then.
    done: Arc<AtomicBool>,
}

/// Hosts the session for the original's players too: they find it, join
/// it and come in as `next`'s numbers, through `tx`. `me` is the host's
/// own player (none for a server that doesn't ride).
pub(super) fn host(name: &str, me: Option<&Player>, tx: Sender<In>, next: Arc<AtomicU8>, stop: Arc<AtomicBool>) {
    if !on() {
        return;
    }
    let dp = match Dp::host(APP, name, MAX_PLAYERS as u32, FLAGS, Ipv4Addr::UNSPECIFIED) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("original MM2 players can't join: {e}");
            return;
        }
    };
    let mut b = Bridge::new(dp, true, tx, stop, Some(next));
    b.open_router();
    if let Some(me) = me {
        b.players = vec![me.clone()];
        b.add_mine(me.id, &me.name);
    }
    std::thread::spawn(move || b.run());
}

/// Lets the original's players on this network into the OpenMCM2 race
/// at `to` (a race code, or an address), anywhere: OpenMCM2 hosts a game
/// here called `name` that they join as on a local network, and each of
/// them joins the far race through it as a player of its own. Runs until
/// `stop`; says what happens through `note`.
pub fn relay(to: super::Addr, name: &str, stop: Arc<AtomicBool>, note: impl Fn(String) + Send + 'static) -> io::Result<()> {
    let dp = Dp::host(APP, name, MAX_PLAYERS as u32, FLAGS, Ipv4Addr::UNSPECIFIED)?;
    let (tx, rx) = channel::<In>();
    let mut b = Bridge::new(dp, true, tx, stop.clone(), Some(Arc::new(AtomicU8::new(1))));
    b.relay = true;
    b.open_router();
    std::thread::spawn(move || b.run());
    std::thread::spawn(move || {
        // Each original player's connection to the far race.
        let mut ups: HashMap<u8, Link> = HashMap::new();
        while !stop.load(Ordering::Relaxed) {
            let Ok(i) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
            match i {
                In::Joined(link, mut peer) => {
                    let (utx, urx) = channel::<In>();
                    match super::connect(&to, utx) {
                        Ok((up, _)) => {
                            ups.insert(link, up);
                            note(format!("An original player is joining {to}."));
                            // What the far host says to them goes to the
                            // bridge, as a host here would say it.
                            std::thread::spawn(move || {
                                for m in urx {
                                    match m {
                                        In::Got(_, m) => {
                                            if super::send(&mut peer, &m).is_err() {
                                                return;
                                            }
                                        }
                                        In::Gone(_) => return,
                                        In::Joined(..) | In::Note(_) => {}
                                    }
                                }
                            });
                        }
                        Err(e) => note(format!("Can't reach {to}: {e}")),
                    }
                }
                In::Got(link, m) => {
                    if let Some(up) = ups.get_mut(&link) {
                        if super::send(up, &m).is_err() {
                            ups.remove(&link);
                        }
                    }
                }
                In::Note(_) => {}
                In::Gone(link) => {
                    if let Some(mut up) = ups.remove(&link) {
                        let _ = super::send(&mut up, &Msg::Leave);
                        note("An original player has left.".into());
                    }
                }
            }
        }
    });
    Ok(())
}

/// Joins the original's game hosted at `at` as `me`: the session's
/// connection to its host.
pub(super) fn join(at: SocketAddrV4, me: &Player, tx: Sender<In>) -> io::Result<Link> {
    let found = Dp::find(APP, Some(*at.ip()), Duration::from_millis(1500))?;
    let f = found
        .iter()
        .find(|f| f.host == at)
        .or(found.first())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no MM2 game there"))?;
    let dp = Dp::join(f)?;
    let mut b = Bridge::new(dp, false, tx, Arc::new(AtomicBool::new(false)), None);
    b.open_router();
    b.me = me.clone();
    let link = Link::Orig(Peer { id: 0, tx: b.cmd_tx.clone() });
    let id = b.take();
    b.me.id = id;
    let dp_id = b.dp.create_player(&me.name)?;
    b.mine.push(Mine { dp: dp_id, id });
    let _ = b.tx.send(In::Got(0, Msg::Welcome { id }));
    b.sync_players();
    b.send_states();
    std::thread::spawn(move || b.run());
    Ok(link)
}

/// Asks the router (UPnP) to let the original's players elsewhere reach
/// this machine's DirectPlay ports, `tcp` and `udp`, renewing each half
/// hour, and lets them go when `done`. A router without UPnP, or with it
/// off, is left alone: then the ports must be opened by hand.
fn forward(tcp: Vec<u16>, udp: Vec<u16>, done: Arc<AtomicBool>, tell: Option<Sender<In>>) {
    use igd_next::PortMappingProtocol::{TCP, UDP};
    std::thread::spawn(move || {
        let opts = igd_next::SearchOptions { timeout: Some(Duration::from_secs(5)), ..Default::default() };
        let Ok(gw) = igd_next::search_gateway(opts) else { return };
        // Our address on the router's side.
        let Some(ip) = std::net::UdpSocket::bind("0.0.0.0:0").and_then(|s| s.connect(gw.addr).and(s.local_addr())).ok().map(|a| a.ip()) else { return };
        let ports: Vec<_> = tcp.iter().map(|p| (TCP, *p)).chain(udp.iter().map(|p| (UDP, *p))).collect();
        let open = || {
            for (proto, port) in &ports {
                let at = std::net::SocketAddr::new(ip, *port);
                if let Err(e) = gw.add_port(*proto, *port, at, 3600, "OpenMCM2 original MM2 games") {
                    eprintln!("original MM2: the router won't open {proto} {port}: {e}");
                }
            }
        };
        open();
        if let (Some(tx), Ok(at)) = (tell, gw.get_external_ip()) {
            let _ = tx.send(In::Note(format!(
                "Original MM2 players elsewhere can join: in their Multiplayer, TCP/IP, they type {at} (their own router needs ports 2300 to 2400 open)."
            )));
        }
        let mut renewed = Instant::now();
        while !done.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(500));
            if renewed.elapsed() > Duration::from_secs(1800) {
                renewed = Instant::now();
                open();
            }
        }
        for (proto, port) in &ports {
            let _ = gw.remove_port(*proto, *port);
        }
    });
}

/// Looks for the original's games on the local network (and at the
/// addresses asked), every couple of seconds, while it lives.
pub struct Looker {
    pub found: Arc<Mutex<Vec<(String, SocketAddrV4)>>>,
    hosts: Arc<Mutex<Vec<Ipv4Addr>>>,
    stop: Arc<AtomicBool>,
}

impl Looker {
    pub fn new(hosts: Vec<Ipv4Addr>) -> Option<Looker> {
        if !on() {
            return None;
        }
        let found = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let hosts = Arc::new(Mutex::new(hosts));
        let (f, s, h) = (found.clone(), stop.clone(), hosts.clone());
        std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                let mut now = Vec::new();
                let asked = h.lock().map(|h| h.clone()).unwrap_or_default();
                let at = std::iter::once(None).chain(asked.into_iter().map(Some));
                for a in at {
                    for g in Dp::find(APP, a, Duration::from_millis(1200)).unwrap_or_default() {
                        // A game that is racing, or full, can't be joined.
                        let full = g.desc.current_players >= g.desc.max_players;
                        if g.desc.flags & mm2_dplay::wire::sflag::JOIN_DISABLED == 0 && !full && !now.iter().any(|(_, h)| *h == g.host) {
                            now.push((g.name, g.host));
                        }
                    }
                }
                if let Ok(mut f) = f.lock() {
                    *f = now;
                }
                std::thread::sleep(Duration::from_millis(800));
            }
        });
        Some(Looker { found, hosts, stop })
    }

    /// Looks at `ip` too (an address typed in).
    pub fn ask(&self, ip: Ipv4Addr) {
        if let Ok(mut h) = self.hosts.lock() {
            if !h.contains(&ip) {
                h.push(ip);
            }
        }
    }
}

impl Drop for Looker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// A ride the original's players can see: the player's own if it is one
/// of the original's bikes and outfits, else the stock one.
fn stock_ride(r: &RideInfo) -> (String, String, String, i32) {
    let tga = |s: &str| s.to_ascii_lowercase().ends_with(".tga") && !s.contains(['/', '\\']);
    let model = r.model.to_ascii_lowercase();
    if model.ends_with(".mcf") && !model.starts_with("fp_") && tga(&r.texture) && tga(&r.outfit) && r.cc > 0 {
        (r.model.clone(), r.texture.clone(), r.outfit.clone(), r.cc as i32)
    } else {
        (STOCK.0.into(), STOCK.1.into(), STOCK.2.into(), STOCK.3 as i32)
    }
}

fn ride_of(s: &PlayerState) -> RideInfo {
    RideInfo { model: s.model.clone(), texture: s.bike.clone(), outfit: s.rider.clone(), cc: s.cc.max(0) as u32, plate: s.plate.max(0) as u32 }
}

/// Where the track's file is: its folder's event (the tag event's tracks
/// are in three) and the file.
fn track_file(event: u8, stem: &str) -> Option<(u8, PathBuf)> {
    let game = GAME.lock().ok()?.clone()?;
    let ev = EventKind::from_number(event as u32)?;
    let folders: Vec<EventKind> = if ev == EventKind::Tag { vec![EventKind::Tag, EventKind::Baja, EventKind::Enduro] } else { vec![ev] };
    for f in folders {
        let Ok(rd) = crate::fs::read_dir(game.join(f.folder())) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let is_env = p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("env"));
            let named = p.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case(stem));
            if is_env && named {
                return Some((f as u8, p));
            }
        }
    }
    None
}

/// The event's settings as the original's lobby has them.
fn settings_of(g: &GameInfo, players: usize, slots: [Slot; 8]) -> Settings {
    let timed = matches!(EventKind::from_number(g.event as u32), Some(EventKind::Stunt | EventKind::Tag));
    let (series, file) = track_file(g.event, &g.track).map_or((g.event, None), |(s, p)| (s, Some(p)));
    let meta = file.and_then(|p| std::fs::metadata(p).ok());
    let file_time = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as u32);
    Settings {
        minutes: if timed { g.laps.clamp(1, 31) as u8 } else { 2 },
        event: g.event,
        mode: g.mode,
        laps: if timed { 1 } else { g.laps.clamp(1, 31) as u8 },
        opponents: g.computers.min(7) as u8,
        players: players.min(8) as u8,
        races: 2,
        series,
        trees: true,
        riders: g.collisions,
        fast_finish: false,
        bike_class: 4,
        tag_ball: g.tag_ball,
        stunts: g.tag_stunts,
        difficulty: g.level.min(3),
        course: g.course,
        file_time,
        file_size: meta.map_or(0, |m| m.len() as u32),
        track: g.track.clone(),
        slots,
    }
}

/// The original's settings as OpenMCM2's game.
fn game_of(s: &Settings) -> GameInfo {
    let timed = matches!(EventKind::from_number(s.event as u32), Some(EventKind::Stunt | EventKind::Tag));
    GameInfo {
        event: s.event,
        mode: s.mode,
        laps: if timed { s.minutes as u32 } else { s.laps as u32 },
        computers: s.opponents as u32,
        track: s.track.clone(),
        course: s.course,
        level: s.difficulty,
        tag_ball: s.tag_ball,
        tag_stunts: s.stunts,
        collisions: s.riders,
    }
}

/// The start's settings as the lobby's.
fn settings_of_start(st: &Start) -> Settings {
    let b = &st.settings;
    let int = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    use game::rs;
    Settings {
        minutes: st.minutes() as u8,
        event: st.event() as u8,
        mode: st.mode() as u8,
        laps: st.laps() as u8,
        opponents: st.computers() as u8,
        players: st.players(),
        series: st.series() as u8,
        riders: int(rs::RIDERS) != 0,
        trees: int(rs::TREES) != 0,
        fast_finish: int(rs::FAST_FINISH) != 0,
        tag_ball: int(rs::TAG_BALL) != 0,
        stunts: int(rs::STUNTS) != 0,
        difficulty: st.difficulty.clamp(0, 3) as u8,
        course: st.course(),
        track: st.track(),
        ..Default::default()
    }
}

/// OpenMCM2's stunt packet (type 10 as it sends it): one jump's score,
/// a whole number.
fn stunt_packet(racer: usize, score: f32) -> Vec<u8> {
    let mut d = vec![0u8; 8];
    d[0] = kind::SCORE;
    d[1] = racer as u8;
    d[4..8].copy_from_slice(&(score.round().max(0.0) as u32).to_le_bytes());
    d
}

impl Bridge {
    fn new(dp: Dp, host: bool, tx: Sender<In>, stop: Arc<AtomicBool>, next: Option<Arc<AtomicU8>>) -> Bridge {
        let (cmd_tx, cmds) = channel();
        let now = Instant::now();
        Bridge {
            dp,
            pending: Vec::new(),
            host,
            relay: false,
            tx,
            cmd_tx,
            cmds,
            stop,
            next,
            next_id: 1,
            orig: Vec::new(),
            mine: Vec::new(),
            players: Vec::new(),
            me: Player::default(),
            game: None,
            settings: None,
            race: None,
            heard_chat: VecDeque::new(),
            seq: 1,
            epoch: now,
            states_at: now,
            settings_at: now,
            closed: false,
            done: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Players elsewhere can reach this machine, if the router lets us.
    fn open_router(&self) {
        let (tcp, udp) = self.dp.ports();
        let udp = if self.host { vec![udp, mm2_dplay::session::ENUM_PORT] } else { vec![udp] };
        let tell = (self.host && !self.relay).then(|| self.tx.clone());
        forward(vec![tcp], udp, self.done.clone(), tell);
    }

    fn run(mut self) {
        let mut synced = Instant::now();
        while !self.closed && !self.stop.load(Ordering::Relaxed) {
            while let Ok(c) = self.cmds.try_recv() {
                self.command(c);
            }
            while let Some(e) = self.dp.poll() {
                self.event(e);
            }
            if synced.elapsed() > Duration::from_millis(200) {
                synced = Instant::now();
                self.sync_players();
            }
            self.timers();
            if let Some(e) = self.dp.wait(Duration::from_millis(5)) {
                self.event(e);
            }
        }
        self.done.store(true, Ordering::Relaxed);
        self.dp.close();
    }

    /// The next of a joined session's player numbers.
    fn take(&mut self) -> u8 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    fn clock(&self) -> u32 {
        self.epoch.elapsed().as_millis() as u32
    }

    /// Sends from one of our players. A guaranteed one goes stamped and
    /// again every few seconds until its receiver answers, as the game
    /// sends it (never with DirectPlay's own guarantee, which the
    /// original's DirectPlay holds back), to each player in turn.
    fn send(&mut self, from: u32, to: u32, mut data: Vec<u8>) {
        let k = data[0];
        if !game::guaranteed(k) {
            self.dp.send(from, to, &data, false);
            return;
        }
        let targets: Vec<u32> = if to == 0 { self.orig.iter().map(|o| o.dp).collect() } else { vec![to] };
        for t in targets {
            game::stamp(&mut data, k, self.seq);
            self.dp.send(from, t, &data, false);
            self.pending.push(Pending { seq: self.seq, from, to: t, data: data.clone(), at: Instant::now(), tries: 1 });
            self.seq = self.seq.wrapping_add(1);
        }
    }

    /// Guaranteed messages not answered yet go again.
    fn resend(&mut self) {
        let now = Instant::now();
        for p in &mut self.pending {
            if now - p.at >= RESEND {
                p.at = now;
                p.tries += 1;
                self.dp.send(p.from, p.to, &p.data, false);
            }
        }
        let dead: Vec<u32> = self.orig.iter().map(|o| o.dp).collect();
        self.pending.retain(|p| p.tries <= RESEND_TRIES && dead.contains(&p.to));
    }

    /// The player that speaks for this machine (the host's own player,
    /// or ours).
    fn speaker(&self) -> Option<u32> {
        self.mine.iter().find(|m| m.id == 0 || !self.host).or(self.mine.first()).map(|m| m.dp)
    }

    fn dp_of(&self, id: u8) -> Option<u32> {
        self.mine.iter().find(|m| m.id == id).map(|m| m.dp).or_else(|| self.orig.iter().find(|o| o.id == id).map(|o| o.dp))
    }

    fn is_mine(&self, dp: u32) -> bool {
        self.mine.iter().any(|m| m.dp == dp)
    }

    fn name_of(&self, dp: u32) -> String {
        if let Some(o) = self.orig.iter().find(|o| o.dp == dp) {
            return o.name.clone();
        }
        self.dp.players().into_iter().find(|p| p.0 == dp).map(|p| p.1).unwrap_or_default()
    }

    fn add_mine(&mut self, id: u8, name: &str) {
        if self.mine.iter().any(|m| m.id == id) {
            return;
        }
        match self.dp.create_player(name) {
            Ok(dp) => self.mine.push(Mine { dp, id }),
            Err(e) => eprintln!("original MM2: can't add {name}: {e}"),
        }
    }

    /// The connection that answers for everyone: the session says the
    /// same to each original player's, and it goes to DirectPlay once.
    fn primary(&self) -> Option<u8> {
        self.orig.iter().filter(|o| o.hello || !self.host).map(|o| o.link).min()
    }

    fn command(&mut self, c: Cmd) {
        match c {
            Cmd::Dropped(id) => {
                if !self.host {
                    self.closed = true;
                } else if let Some(o) = self.orig.iter().position(|o| o.link == id) {
                    // Turned away or sent off: off the original's list too.
                    let dp = self.orig[o].dp;
                    self.orig.remove(o);
                    if let Some(me) = self.speaker() {
                        self.send(me, 0, game::about(kind::REMOVE, dp));
                    }
                }
            }
            Cmd::Port(peer, m) if self.host => self.from_host(peer, m),
            Cmd::Port(_, m) => self.from_guest(m),
        }
    }

    /// What the OpenMCM2 host's session says to the original players.
    fn from_host(&mut self, peer: u8, m: Msg) {
        if let Msg::Leave = m {
            // The host sends this player off (or the session ends).
            if let Some(o) = self.orig.iter().position(|o| o.link == peer) {
                let dp = self.orig[o].dp;
                self.orig.remove(o);
                if let Some(me) = self.speaker() {
                    self.send(me, 0, game::about(kind::REMOVE, dp));
                }
            }
            return;
        }
        if let Msg::Welcome { id } = m {
            // A far host's number for this player (relaying).
            if let Some(o) = self.orig.iter_mut().find(|o| o.link == peer) {
                o.id = id;
            }
            return;
        }
        let first = self.primary() == Some(peer);
        match m {
            Msg::Game(g) => {
                let changed = self.game.as_ref() != Some(&g);
                self.game = Some(g);
                if changed {
                    self.send_settings();
                }
            }
            _ if !first => {}
            Msg::Players(ps) => {
                self.players = ps;
                let ids: Vec<(u8, String)> = self
                    .players
                    .iter()
                    .filter(|p| !self.orig.iter().any(|o| o.id == p.id))
                    .map(|p| (p.id, p.name.clone()))
                    .collect();
                for (id, name) in &ids {
                    self.add_mine(*id, name);
                }
                let gone: Vec<u32> = self.mine.iter().filter(|m| !ids.iter().any(|(id, _)| *id == m.id)).map(|m| m.dp).collect();
                for dp in gone {
                    self.dp.destroy_player(dp);
                    self.mine.retain(|m| m.dp != dp);
                }
                self.send_states();
                self.send_settings();
            }
            Msg::Chat(line) => self.chat_out(&line),
            Msg::Start { game, players } => self.host_start(&game, &players),
            Msg::Bike { racer, kind, data, .. } => self.bike_out(racer as usize, kind, &data),
            Msg::Pos { racer, data, .. } => {
                if let Some(st) = State::small(&data) {
                    self.bike_out(racer as usize, vcr::FULL, &st.write_full());
                }
            }
            _ => {}
        }
    }

    /// What this OpenMCM2 player says to the original host's game.
    fn from_guest(&mut self, m: Msg) {
        match m {
            Msg::Hello { ride, .. } | Msg::Ride(ride) => {
                self.me.ride = ride;
                self.send_states();
                self.tell_players();
            }
            Msg::Ready(r) => {
                self.me.ready = r;
                self.send_states();
                self.tell_players();
            }
            Msg::Chat(line) => self.chat_out(&line),
            Msg::Bike { racer, kind, data, .. } => self.bike_out(racer as usize, kind, &data),
            Msg::Leave => self.closed = true,
            _ => {}
        }
    }

    /// A chat line from OpenMCM2 ("name: text"): from that player's
    /// DirectPlay player, so the original shows their name, else as the
    /// game's.
    fn chat_out(&mut self, line: &str) {
        if let Some(i) = self.heard_chat.iter().position(|l| l == line) {
            self.heard_chat.remove(i);
            return;
        }
        let who = if self.host {
            self.players.iter().filter(|p| self.is_mine_id(p.id)).find(|p| line.starts_with(&format!("{}: ", p.name))).map(|p| (p.id, p.name.clone()))
        } else {
            line.starts_with(&format!("{}: ", self.me.name)).then(|| (self.me.id, self.me.name.clone()))
        };
        match who.and_then(|(id, name)| Some((self.dp_of(id)?, name))) {
            Some((dp, name)) => self.send(dp, 0, game::chat(kind::CHAT, &line[name.len() + 2..])),
            None => {
                if let Some(me) = self.speaker() {
                    self.send(me, 0, game::chat(kind::NOTICE, line));
                }
            }
        }
    }

    fn is_mine_id(&self, id: u8) -> bool {
        self.mine.iter().any(|m| m.id == id)
    }

    /// A chat line from an original player, for the session.
    fn chat_in(&mut self, from: u32, line: String) {
        let line = mm2_settings::clean::mask(&line);
        self.heard_chat.push_back(line.clone());
        while self.heard_chat.len() > 16 {
            self.heard_chat.pop_front();
        }
        let peer = self.peer_of(from);
        let _ = self.tx.send(In::Got(peer, Msg::Chat(line)));
    }

    /// The session's connection a player's news comes in on.
    fn peer_of(&self, dp: u32) -> u8 {
        if self.host {
            self.orig.iter().find(|o| o.dp == dp).map_or(0, |o| o.link)
        } else {
            0
        }
    }

    /// Our players' lobby states, to everyone.
    fn send_states(&mut self) {
        self.states_at = Instant::now();
        let clock = self.clock();
        let states: Vec<(u32, PlayerState)> = self
            .mine
            .iter()
            .filter_map(|m| {
                let p = if self.host { self.players.iter().find(|p| p.id == m.id)?.clone() } else { self.me.clone() };
                let (model, bike, rider, cc) = stock_ride(&p.ride);
                Some((
                    m.dp,
                    PlayerState {
                        ready: p.ready,
                        bike,
                        model,
                        rider,
                        time: clock,
                        cc,
                        plate: p.ride.plate as i32,
                        ..Default::default()
                    },
                ))
            })
            .collect();
        for (dp, s) in states {
            self.send(dp, 0, s.write());
        }
    }

    /// The host's event, to everyone.
    fn send_settings(&mut self) {
        self.settings_at = Instant::now();
        let (Some(g), Some(me)) = (self.game.clone(), self.speaker()) else { return };
        if !self.host {
            return;
        }
        let mut slots = [Slot::default(); 8];
        for (s, p) in slots.iter_mut().zip(&self.players) {
            if let Some(dp) = self.dp_of(p.id) {
                *s = Slot { player: dp, requested: 0, granted: 0 };
            }
        }
        if let Some(s) = slots.iter_mut().find(|s| s.player == me) {
            s.requested = g.computers as i32;
            s.granted = g.computers as i32;
        }
        let s = settings_of(&g, self.players.len(), slots);
        self.settings = Some(s.clone());
        self.send(me, 0, s.write());
    }

    fn timers(&mut self) {
        self.resend();
        if self.states_at.elapsed() >= STATE_EVERY {
            self.send_states();
            if self.race.is_some() {
                let mine: Vec<u32> = self.mine.iter().map(|m| m.dp).collect();
                for dp in mine {
                    self.send(dp, 0, game::about(kind::KEEPALIVE, dp));
                }
            }
        }
        if self.host && self.settings_at.elapsed() >= SETTINGS_EVERY {
            self.send_settings();
        }
        // Hosting: an original player who hasn't said what they ride yet
        // comes in with the stock ride.
        if self.host {
            let late: Vec<usize> = (0..self.orig.len()).filter(|&i| !self.orig[i].hello && self.orig[i].since.elapsed() > HELLO_WAIT).collect();
            for i in late {
                self.hello(i);
            }
        }
        // The race is over for everyone once all have finished, or some
        // time after the first.
        let over = self.race.as_ref().is_some_and(|r| {
            let limit = if r.fast_finish { Duration::from_secs(30) } else { END_AFTER };
            !r.results_sent
                && (r.first_finish.is_some_and(|t| t.elapsed() > limit)
                    || (!r.finished.is_empty() && r.racers.iter().enumerate().all(|(i, x)| x.is_none() || r.finished.get(&i) == Some(&true))))
        });
        if over {
            self.send_results();
        }
    }

    /// The session hears of an original player (hosting).
    fn hello(&mut self, i: usize) {
        let o = &mut self.orig[i];
        o.hello = true;
        let ride = o.state.as_ref().map(ride_of).unwrap_or_else(|| RideInfo {
            model: STOCK.0.into(),
            texture: STOCK.1.into(),
            outfit: STOCK.2.into(),
            cc: STOCK.3,
            plate: 1,
        });
        let ready = o.state.as_ref().is_some_and(|s| s.ready);
        let (id, name) = (o.link, o.name.clone());
        let _ = self.tx.send(In::Got(id, Msg::Hello { name, ride }));
        if ready {
            let _ = self.tx.send(In::Got(id, Msg::Ready(true)));
        }
    }

    /// Who is in the DirectPlay session now: newcomers in, leavers out.
    fn sync_players(&mut self) {
        let now: Vec<(u32, String)> = self.dp.players().into_iter().filter(|p| !p.2).map(|p| (p.0, p.1)).collect();
        let mut changed = false;
        for (dp, name) in &now {
            if self.orig.iter().any(|o| o.dp == *dp) {
                continue;
            }
            let id = if self.host {
                let Some(next) = &self.next else { continue };
                take_id(next)
            } else if self.dp.on_host(*dp) && !self.orig.iter().any(|o| o.id == 0) {
                0
            } else {
                self.take()
            };
            if self.host {
                let link = Link::Orig(Peer { id, tx: self.cmd_tx.clone() });
                if self.tx.send(In::Joined(id, link)).is_err() {
                    self.closed = true;
                }
            }
            let link = id;
            let id = if self.relay { NOT_YET } else { id };
            self.orig.push(Orig { dp: *dp, id, link, name: name.clone(), state: None, hello: false, since: Instant::now() });
            changed = true;
        }
        let gone: Vec<(u32, u8, u8)> = self.orig.iter().filter(|o| !now.iter().any(|(dp, _)| *dp == o.dp)).map(|o| (o.dp, o.link, o.id)).collect();
        for (dp, link, id) in gone {
            self.orig.retain(|o| o.dp != dp);
            changed = true;
            if self.host {
                let _ = self.tx.send(In::Gone(link));
            } else if id == 0 {
                // The original host has gone: the session is over.
                let _ = self.tx.send(In::Gone(0));
                self.closed = true;
            }
        }
        if changed && !self.host {
            self.tell_players();
        }
    }

    /// Joined: everyone in the session, as an OpenMCM2 host would say.
    fn tell_players(&mut self) {
        if self.host {
            return;
        }
        let _ = self.tx.send(In::Got(0, Msg::Players(self.player_list())));
    }

    fn player_list(&self) -> Vec<Player> {
        let mut ps: Vec<Player> = self
            .orig
            .iter()
            .map(|o| Player {
                id: o.id,
                name: o.name.clone(),
                ready: o.state.as_ref().is_some_and(|s| s.ready),
                ride: o.state.as_ref().map(ride_of).unwrap_or_default(),
            })
            .collect();
        ps.push(self.me.clone());
        ps.sort_by_key(|p| p.id);
        ps
    }

    fn event(&mut self, e: Event) {
        match e {
            Event::Data { from, to, data } if !data.is_empty() => {
                let k = data[0];
                if game::guaranteed(k) {
                    if let Some(seq) = game::sequence(&data) {
                        let me = if self.is_mine(to) { Some(to) } else { self.speaker() };
                        if let Some(me) = me {
                            self.dp.send(me, from, &game::ack(seq), false);
                        }
                    }
                }
                self.data(from, k, &data);
            }
            Event::Closed => {
                if !self.host {
                    let _ = self.tx.send(In::Gone(0));
                }
                self.closed = true;
            }
            _ => {}
        }
    }

    fn data(&mut self, from: u32, k: u8, data: &[u8]) {
        match k {
            // An original player has loaded the race and waits for ours:
            // ours went with the start, maybe before it could hear.
            kind::LOADED if self.race.is_some() => {
                let clock = self.clock();
                let mine: Vec<u32> = self.mine.iter().map(|m| m.dp).collect();
                for dp in mine {
                    self.send(dp, from, game::loaded(clock));
                }
            }
            kind::ACK => {
                if let Some(seq) = game::sequence(data) {
                    self.pending.retain(|p| !(p.seq == seq && p.to == from));
                }
            }
            kind::PLAYER => {
                let Some(s) = PlayerState::read(data) else { return };
                let Some(i) = self.orig.iter().position(|o| o.dp == from) else { return };
                let old = self.orig[i].state.replace(s.clone());
                if self.host {
                    if !self.orig[i].hello {
                        self.hello(i);
                    } else {
                        let id = self.orig[i].link;
                        if old.as_ref().map(|o| o.ready) != Some(s.ready) {
                            let _ = self.tx.send(In::Got(id, Msg::Ready(s.ready)));
                        }
                        if old.as_ref().map(ride_of) != Some(ride_of(&s)) {
                            let _ = self.tx.send(In::Got(id, Msg::Ride(ride_of(&s))));
                        }
                    }
                } else if old.as_ref().map(|o| (o.ready, ride_of(o))) != Some((s.ready, ride_of(&s))) {
                    self.tell_players();
                }
            }
            kind::SETTINGS if !self.host => {
                let Some(s) = Settings::read(data) else { return };
                if self.settings.as_ref() != Some(&s) {
                    let g = game_of(&s);
                    self.settings = Some(s);
                    if self.game.as_ref() != Some(&g) {
                        self.game = Some(g.clone());
                        let _ = self.tx.send(In::Got(0, Msg::Game(g)));
                    }
                }
            }
            kind::START if !self.host => {
                if let Some(st) = Start::read(data) {
                    self.guest_start(&st);
                }
            }
            kind::CHAT => {
                if let Some(text) = game::read_chat(data) {
                    let line = format!("{}: {}", self.name_of(from), text);
                    self.chat_in(from, line);
                }
            }
            kind::NOTICE => {
                if let Some(text) = game::read_chat(data) {
                    self.chat_in(from, text);
                }
            }
            kind::BIKE | kind::DELTA | kind::SCORE => self.bike_in(from, data),
            kind::RESULT => {
                if let Some(r) = game::Result::read(data) {
                    if let Some(race) = &mut self.race {
                        if let Some(i) = race.racer_of(from, r.racer) {
                            race.finish(i, true);
                        }
                    }
                }
                // The race is over for the sender, so for everyone.
                self.send_results();
            }
            kind::REMOVE if !self.host => {
                if game::read_about(data).is_some_and(|p| self.is_mine(p)) {
                    let _ = self.tx.send(In::Gone(0));
                    self.closed = true;
                }
            }
            _ => {}
        }
    }

    /// The OpenMCM2 host starts: the start, to each original player.
    fn host_start(&mut self, g: &GameInfo, players: &[Player]) {
        let Some(me) = self.speaker() else { return };
        self.game = Some(g.clone());
        let humans: Vec<&Player> = players.iter().filter(|p| p.id != EMPTY).collect();
        let n = humans.len();
        let mut racers: Vec<Option<(u32, u8)>> = players.iter().map(|p| self.dp_of(p.id).filter(|_| p.id != EMPTY).map(|dp| (dp, 0))).collect();
        let mut grid: Vec<Grid> = humans.iter().filter_map(|p| Some(Grid { player: self.dp_of(p.id)?, points: 0, racer: 0 })).collect();
        let mut list: Vec<Computer> =
            humans.iter().map(|p| Computer { plate: p.ride.plate as i32, name: p.name.clone(), ..Default::default() }).collect();
        let computers = (g.computers as usize).min(MAX_PLAYERS.saturating_sub(n));
        for k in 0..computers {
            let index = (n + k) as u8;
            racers.push(Some((me, index)));
            grid.push(Grid { player: me, points: 0, racer: index });
            list.push(Computer {
                plate: 2 + (n + k) as i32,
                name: format!("Rider {}", n + k + 1),
                model: STOCK.0.into(),
                bike: STOCK.1.into(),
                rider: STOCK.2.into(),
                cc: STOCK.3 as i32,
                four_stroke: false,
            });
        }
        let mut slots = [Slot::default(); 8];
        for (s, p) in slots.iter_mut().zip(&humans) {
            if let Some(dp) = self.dp_of(p.id) {
                *s = Slot { player: dp, requested: 0, granted: 0 };
            }
        }
        if let Some(s) = slots.iter_mut().find(|s| s.player == me) {
            s.requested = computers as i32;
            s.granted = computers as i32;
        }
        let mut settings = settings_of(g, n, slots);
        settings.opponents = computers as u8;
        let fast = settings.fast_finish;
        let start = Start { start: true, settings: Start::settings_from(&settings, &grid), difficulty: g.level as i32, racers: list };
        let data = start.write();
        let to: Vec<u32> = self.orig.iter().map(|o| o.dp).collect();
        for dp in to {
            self.send(me, dp, data.clone());
        }
        self.race = Some(Race::new(racers, fast));
        self.loaded();
    }

    /// The original host starts: the race, as an OpenMCM2 host would
    /// start it.
    fn guest_start(&mut self, st: &Start) {
        if !st.start {
            return;
        }
        let s = settings_of_start(st);
        let game = game_of(&s);
        let players = self.player_list();
        let n = st.players() as usize;
        let mut racers: Vec<Option<(u32, u8)>> = players.iter().map(|p| self.dp_of(p.id).map(|dp| (dp, 0))).collect();
        racers.resize(n, None);
        // A computer rider's number is its place, the same everywhere.
        let computers = st.computers().max(0) as usize;
        racers.extend((n..n + computers).map(|i| Some((0, i as u8))));
        self.race = Some(Race::new(racers, s.fast_finish));
        let game = GameInfo { computers: st.computers().max(0) as u32, ..game };
        let _ = self.tx.send(In::Got(0, Msg::Start { game, players }));
        self.loaded();
    }

    /// Tells the originals our players have loaded the race: they wait for
    /// everyone's before the start.
    fn loaded(&mut self) {
        let clock = self.clock();
        let mine: Vec<u32> = self.mine.iter().map(|m| m.dp).collect();
        for dp in mine {
            self.send(dp, 0, game::loaded(clock));
        }
    }

    /// One of our bikes, to the originals.
    fn bike_out(&mut self, racer: usize, k: i32, data: &[u8]) {
        let Some(race) = &self.race else { return };
        let Some(Some((dp, index))) = race.racers.get(racer).copied() else { return };
        if !self.is_mine(dp) {
            return;
        }
        let out = match k {
            vcr::FULL => {
                let Some(mut st) = State::full(data) else { return };
                st.index = index;
                let done = st.flags[1] & 0x20 != 0;
                let race = self.race.as_mut().unwrap();
                race.ours.insert(racer, st);
                race.finish(racer, done);
                let mut d = st.write_full();
                d[0] = kind::BIKE;
                d
            }
            vcr::STUNT => {
                let Some(score) = data.get(4..8).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as f32) else { return };
                let race = self.race.as_mut().unwrap();
                let total = race.sent_score.entry(racer).or_insert(0.0);
                *total += score;
                game::score(index, *total)
            }
            _ => return,
        };
        self.dp.send(dp, 0, &out, false);
    }

    /// An original's bike, for the session.
    fn bike_in(&mut self, from: u32, data: &[u8]) {
        let peer = self.peer_of(from);
        let Some(race) = &mut self.race else { return };
        let msg = match data[0] {
            kind::SCORE => {
                let Some((index, total)) = game::read_score(data) else { return };
                let Some(racer) = race.racer_of(from, index) else { return };
                let before = race.heard_score.insert((from, index), total).unwrap_or(0.0);
                if total <= before {
                    return;
                }
                Msg::Bike { racer: racer as u8, time: 0.0, kind: vcr::STUNT, data: stunt_packet(racer, total - before) }
            }
            k => {
                let Some(index) = game::bike_racer(data) else { return };
                let Some(racer) = race.racer_of(from, index) else { return };
                let st = if k == kind::BIKE {
                    State::full(data)
                } else {
                    let (Some(d), Some(mut s)) = (Delta::read(data), race.last.get(&(from, index)).copied()) else { return };
                    d.apply(&mut s);
                    Some(s)
                };
                let Some(mut st) = st else { return };
                race.last.insert((from, index), st);
                race.finish(racer, st.flags[1] & 0x20 != 0);
                let base = *race.clock0.entry(from).or_insert(st.clock);
                let time = st.clock.wrapping_sub(base) as f32 / 1000.0;
                st.index = racer as u8;
                Msg::Bike { racer: racer as u8, time, kind: vcr::FULL, data: st.write_full() }
            }
        };
        let _ = self.tx.send(In::Got(peer, msg));
    }

    /// Our racers' results, once: the race is over.
    fn send_results(&mut self) {
        let Some(race) = &mut self.race else { return };
        if race.results_sent {
            return;
        }
        race.results_sent = true;
        let mut out = Vec::new();
        for (i, r) in race.racers.iter().enumerate() {
            let Some((dp, index)) = *r else { continue };
            if !self.mine.iter().any(|m| m.dp == dp) {
                continue;
            }
            let st = race.ours.get(&i);
            let finished = race.finished.get(&i) == Some(&true);
            out.push((
                dp,
                game::Result {
                    racer: index,
                    finished,
                    score: race.sent_score.get(&i).copied().unwrap_or(0.0),
                    time: if finished { st.map_or(f32::MAX, |s| s.total) } else { f32::MAX },
                    best_lap: st.map_or(0.0, |s| s.best_lap),
                    ..Default::default()
                },
            ));
        }
        for (dp, r) in out {
            self.send(dp, 0, r.write());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_pack_rides_show_as_stock() {
        let r = RideInfo { model: "fp_bike_mx2.mcf".into(), texture: "fp_livery_blue".into(), outfit: "fp_suit".into(), cc: 250, plate: 3 };
        assert_eq!(stock_ride(&r).0, "suzuki.mcf");
        let r = RideInfo { model: "honda.mcf".into(), texture: "Honda250_01.tga".into(), outfit: "Rainbow01_08.tga".into(), cc: 250, plate: 3 };
        assert_eq!(stock_ride(&r).0, "honda.mcf");
    }

    #[test]
    fn settings_go_both_ways() {
        let g = GameInfo { event: 3, mode: 1, laps: 4, computers: 2, track: "SX01".into(), course: 0, level: 2, tag_ball: false, tag_stunts: false, collisions: true };
        let s = settings_of(&g, 2, [Slot::default(); 8]);
        assert_eq!((s.event, s.laps, s.opponents, s.players, s.series), (3, 4, 2, 2, 3));
        assert_eq!(game_of(&Settings::read(&s.write()).unwrap()), g);
        // The stunt event's laps are minutes.
        let g = GameInfo { event: 0, laps: 5, ..g };
        let s = settings_of(&g, 2, [Slot::default(); 8]);
        assert_eq!((s.minutes, s.laps), (5, 1));
        assert_eq!(game_of(&s).laps, 5);
    }

    #[test]
    fn computer_riders_match_by_number() {
        let r = Race::new(vec![Some((10, 0)), Some((20, 0)), Some((10, 2)), Some((10, 3))], false);
        assert_eq!(r.racer_of(20, 0), Some(1));
        assert_eq!(r.racer_of(10, 0), Some(0));
        assert_eq!(r.racer_of(99, 3), Some(3));
        assert_eq!(r.racer_of(99, 0), None);
    }
}
