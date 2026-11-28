//! A DirectPlay 4 session over TCP/IP: hosting one, finding them, joining
//! one, and the players and messages in it. It behaves as Microsoft's
//! DirectPlay does on the wire, so the original game, on Windows or under
//! Wine, takes it for another copy of itself.
//!
//! How DirectPlay 4 uses the network ([MC-DPL4CS] section 2.1, and what
//! DirectX 7 was seen doing): a host listens for UDP broadcasts on port
//! 47624. Every machine listens on one TCP and one UDP port between 2300
//! and 2400. Joining, and news of players coming and going, travel as one
//! message per TCP connection, made by the sender, answered by a new
//! connection back to the port named in the message's header. The game's
//! own messages go straight between machines over UDP, inside the reliable
//! protocol's frames when the session asks for it (see [`crate::rel`]).

use crate::rel;
use crate::wire::{self, cmd, pflag, sflag, Addr, Guid, Player, SessionDesc};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// The UDP port a host hears session searches on.
pub const ENUM_PORT: u16 = 47624;
/// The ports every machine listens on.
pub const PORTS: std::ops::RangeInclusive<u16> = 2300..=2400;
/// How long to wait for the host's answers while joining.
const JOIN_WAIT: Duration = Duration::from_secs(8);

/// Something that happened in the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A player on another machine came in.
    PlayerJoined { id: u32, name: String },
    /// A player left (or their machine went quiet for good).
    PlayerLeft { id: u32 },
    /// A message from a player: `to` is 0 when it went to everyone.
    Data { from: u32, to: u32, data: Vec<u8> },
    /// The host changed the session (for one, closed it to new players).
    Changed(SessionDesc),
    /// The session is over for this machine.
    Closed,
}

/// A session a search found.
#[derive(Debug, Clone)]
pub struct Found {
    pub name: String,
    pub desc: SessionDesc,
    /// The host's TCP port and address.
    pub host: SocketAddrV4,
}

/// Another machine in the session.
struct Machine {
    tcp: SocketAddrV4,
    udp: SocketAddrV4,
    link: rel::Link,
}

struct State {
    host: bool,
    desc: SessionDesc,
    name: String,
    /// Every player and group, ours too.
    players: Vec<Player>,
    /// Our system player.
    me: u32,
    machines: HashMap<u32, Machine>,
    /// The host's id allocation: the next index to try, and the count in
    /// the top half that keeps reused indexes distinct.
    next_index: u16,
    uniq: u16,
    /// Messages arriving in pieces, by message id.
    pieces: HashMap<Guid, Vec<Option<Vec<u8>>>>,
}

struct Shared {
    st: Mutex<State>,
    udp: UdpSocket,
    tcp_port: u16,
    udp_port: u16,
    events: Sender<Event>,
    /// Answers a joining or creating call is waiting for.
    waiter: Mutex<Option<Sender<(u16, Vec<u8>, Ipv4Addr)>>>,
    quit: AtomicBool,
    start: Instant,
    tick_base: u32,
}

/// One machine's place in a DirectPlay session.
pub struct Session {
    sh: Arc<Shared>,
    events: Receiver<Event>,
}

fn bind_ports(ip: Ipv4Addr) -> io::Result<(TcpListener, UdpSocket)> {
    let tcp = PORTS
        .clone()
        .find_map(|p| TcpListener::bind((ip, p)).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::AddrInUse, "no free TCP port in 2300-2400"))?;
    let tcp_port = tcp.local_addr()?.port();
    // DirectPlay puts its UDP port just above its TCP one; any in range does.
    let udp = (tcp_port + 1..=*PORTS.end())
        .chain(*PORTS.start()..tcp_port)
        .find_map(|p| UdpSocket::bind((ip, p)).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::AddrInUse, "no free UDP port in 2300-2400"))?;
    Ok((tcp, udp))
}

fn new_guid() -> Guid {
    let mut g = [0u8; 16];
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let mut x = t.as_nanos() as u64 ^ (std::process::id() as u64) << 32;
    for b in g.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
    g[7] = (g[7] & 0x0f) | 0x40;
    g
}

/// Sends one message on its own TCP connection, as DirectPlay does,
/// without holding up the caller.
fn tcp_send(to: SocketAddrV4, msg: Vec<u8>) {
    thread::spawn(move || {
        if let Ok(mut s) = TcpStream::connect_timeout(&SocketAddr::V4(to), Duration::from_secs(4)) {
            let _ = s.write_all(&msg);
            let _ = s.flush();
            let _ = s.shutdown(std::net::Shutdown::Write);
            let mut sink = [0u8; 64];
            let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = s.read(&mut sink);
        }
    });
}

impl Session {
    /// Hosts a session of `app` called `name`, with session `flags` (the
    /// original game uses `0xa040` or `0xa044`). Listens on `bind` (0.0.0.0
    /// for every network).
    pub fn host(app: Guid, name: &str, max_players: u32, flags: u32, bind: Ipv4Addr) -> io::Result<Session> {
        let enum_sock = UdpSocket::bind((bind, ENUM_PORT))?;
        let (tcp, udp) = bind_ports(bind)?;
        let key = u32::from_le_bytes(new_guid()[..4].try_into().unwrap());
        let desc = SessionDesc {
            flags,
            instance: new_guid(),
            application: app,
            max_players,
            current_players: 0,
            id_key: key,
            ..Default::default()
        };
        let s = Self::start(tcp, udp, desc, name, true)?;
        {
            let mut st = s.sh.st.lock().unwrap();
            let me = st.new_id();
            st.me = me;
            let (tp, up) = (s.sh.tcp_port, s.sh.udp_port);
            st.players.push(Player {
                id: me,
                flags: pflag::SYSTEM | pflag::NAMESERVER | pflag::IN_GROUP | pflag::LOCAL,
                version: wire::VERSION as u32,
                system: me,
                stream: Some(Addr::new(Ipv4Addr::UNSPECIFIED, tp)),
                dgram: Some(Addr::new(Ipv4Addr::UNSPECIFIED, up)),
                ..Default::default()
            });
            // The group every player is in, as DirectPlay keeps one.
            let group = st.new_id();
            st.players.push(Player {
                id: group,
                flags: pflag::SYSTEM_GROUP | pflag::LOCAL,
                system: me,
                ..Default::default()
            });
        }
        let sh = s.sh.clone();
        thread::spawn(move || enum_loop(sh, enum_sock));
        Ok(s)
    }

    /// Looks for sessions of `app`: on the local network when `at` is
    /// `None`, else at that address. Waits `wait` for answers.
    pub fn find(app: Guid, at: Option<Ipv4Addr>, wait: Duration) -> io::Result<Vec<Found>> {
        let (tcp, _udp) = bind_ports(Ipv4Addr::UNSPECIFIED)?;
        let port = tcp.local_addr()?.port();
        let ask = wire::full(cmd::ENUMSESSIONS, &wire::enum_sessions(&app, 1), port);
        let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        sock.set_broadcast(true)?;
        let target = at.unwrap_or(Ipv4Addr::BROADCAST);
        let _ = sock.send_to(&ask, (target, ENUM_PORT));
        tcp.set_nonblocking(true)?;
        let end = Instant::now() + wait;
        let mut found: Vec<Found> = Vec::new();
        let mut resent = false;
        while Instant::now() < end {
            match tcp.accept() {
                Ok((mut c, SocketAddr::V4(from))) => {
                    let _ = c.set_nonblocking(false);
                    let _ = c.set_read_timeout(Some(Duration::from_secs(2)));
                    for raw in read_messages(&mut c) {
                        let Ok(m) = wire::parse_full(&raw) else { continue };
                        if m.command != cmd::ENUMSESSIONSREPLY {
                            continue;
                        }
                        let Ok((desc, name)) = wire::read_enum_reply(&m) else { continue };
                        let host = SocketAddrV4::new(*from.ip(), m.from.map(|a| a.port).unwrap_or(0));
                        if desc.application == app && !found.iter().any(|f| f.desc.instance == desc.instance) {
                            found.push(Found { name, desc, host });
                        }
                    }
                }
                _ => thread::sleep(Duration::from_millis(20)),
            }
            if !resent && Instant::now() + wait / 2 > end {
                resent = true;
                let _ = sock.send_to(&ask, (target, ENUM_PORT));
            }
        }
        Ok(found)
    }

    /// Joins a session a search found.
    pub fn join(found: &Found) -> io::Result<Session> {
        let (tcp, udp) = bind_ports(Ipv4Addr::UNSPECIFIED)?;
        let s = Self::start(tcp, udp, found.desc.clone(), &found.name, false)?;
        let host = found.host;
        // A system player for this machine.
        let me = s.request_id(host, pflag::SYSTEM | pflag::LOCAL)?;
        let (tp, up) = (s.sh.tcp_port, s.sh.udp_port);
        let sys = Player {
            id: me,
            flags: pflag::SYSTEM | pflag::LOCAL,
            version: wire::VERSION as u32,
            system: me,
            stream: Some(Addr::new(Ipv4Addr::UNSPECIFIED, tp)),
            dgram: Some(Addr::new(Ipv4Addr::UNSPECIFIED, up)),
            ..Default::default()
        };
        let rx = s.wait_for();
        // DirectX's DirectPlay puts the session's id mask where the
        // specification has a tick count, and turns the request away
        // without it.
        tcp_send(host, wire::full(cmd::ADDFORWARDREQUEST, &wire::add_forward_request(&sys, found.desc.id_key), tp));
        let end = Instant::now() + JOIN_WAIT;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok((cmd::SUPERENUMPLAYERSREPLY, raw, ip)) => {
                    let m = wire::parse_bare(&raw).map_err(|_| bad("bad player list"))?;
                    let (desc, name, all) = wire::read_super_enum_reply(&m).map_err(|_| bad("bad player list"))?;
                    let mut st = s.sh.st.lock().unwrap();
                    st.me = me;
                    st.desc = desc;
                    st.name = name;
                    for mut p in all {
                        if p.id == me {
                            p.flags |= pflag::LOCAL;
                        } else {
                            p.flags &= !pflag::LOCAL;
                            // 0.0.0.0 is the host's own address.
                            fill_addr(&mut p, ip);
                        }
                        st.players.retain(|q| q.id != p.id);
                        st.players.push(p);
                    }
                    st.refresh_machines();
                    let joined: Vec<Event> = st
                        .players
                        .iter()
                        .filter(|p| !p.is_system() && !p.is_group() && p.system != me)
                        .map(|p| Event::PlayerJoined { id: p.id, name: p.name.clone().unwrap_or_default() })
                        .collect();
                    drop(st);
                    for e in joined {
                        let _ = s.sh.events.send(e);
                    }
                    *s.sh.waiter.lock().unwrap() = None;
                    return Ok(s);
                }
                Ok((cmd::ADDFORWARDREPLY, _, _)) => return Err(bad("the host turned us away")),
                Ok(_) => {}
                Err(_) => return Err(io::Error::new(io::ErrorKind::TimedOut, "the host didn't answer")),
            }
        }
    }

    fn start(tcp: TcpListener, udp: UdpSocket, desc: SessionDesc, name: &str, host: bool) -> io::Result<Session> {
        let (tx, rx) = channel();
        let tcp_port = tcp.local_addr()?.port();
        let udp_port = udp.local_addr()?.port();
        let sh = Arc::new(Shared {
            st: Mutex::new(State {
                host,
                desc,
                name: name.to_string(),
                players: Vec::new(),
                me: 0,
                machines: HashMap::new(),
                next_index: 0,
                uniq: 1,
                pieces: HashMap::new(),
            }),
            udp,
            tcp_port,
            udp_port,
            events: tx,
            waiter: Mutex::new(None),
            quit: AtomicBool::new(false),
            start: Instant::now(),
            tick_base: u32::from_le_bytes(new_guid()[..4].try_into().unwrap()) & 0x00ff_ffff,
        });
        let s1 = sh.clone();
        thread::spawn(move || tcp_loop(s1, tcp));
        let s2 = sh.clone();
        thread::spawn(move || udp_loop(s2));
        let s3 = sh.clone();
        thread::spawn(move || timer_loop(s3));
        Ok(Session { sh, events: rx })
    }

    fn wait_for(&self) -> Receiver<(u16, Vec<u8>, Ipv4Addr)> {
        let (tx, rx) = channel();
        *self.sh.waiter.lock().unwrap() = Some(tx);
        rx
    }

    /// Asks the host for a new player id.
    fn request_id(&self, host: SocketAddrV4, flags: u32) -> io::Result<u32> {
        let rx = self.wait_for();
        let mut body = Vec::new();
        wire::Wr::u32(&mut body, flags);
        tcp_send(host, wire::full(cmd::REQUESTPLAYERID, &body, self.sh.tcp_port));
        let end = Instant::now() + JOIN_WAIT;
        loop {
            match rx.recv_timeout(end.saturating_duration_since(Instant::now())) {
                Ok((cmd::REQUESTPLAYERREPLY, raw, _)) => {
                    let m = wire::parse_bare(&raw).map_err(|_| bad("bad id"))?;
                    let (id, result) = wire::read_player_reply(m.body).map_err(|_| bad("bad id"))?;
                    if result != 0 {
                        return Err(bad("the session is full or closed"));
                    }
                    return Ok(id);
                }
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "the host didn't answer"))
                }
            }
        }
    }

    /// Makes a player on this machine and tells everyone. Its id.
    pub fn create_player(&self, name: &str) -> io::Result<u32> {
        let (host, me) = {
            let st = self.sh.st.lock().unwrap();
            (st.host, st.me)
        };
        let id = if host {
            self.sh.st.lock().unwrap().new_id()
        } else {
            let addr = self.host_addr().ok_or_else(|| bad("no host"))?;
            self.request_id(addr, pflag::LOCAL)?
        };
        let (tp, up) = (self.sh.tcp_port, self.sh.udp_port);
        let p = Player {
            id,
            flags: pflag::LOCAL,
            name: Some(name.to_string()),
            version: wire::VERSION as u32,
            system: me,
            stream: Some(Addr::new(Ipv4Addr::UNSPECIFIED, tp)),
            dgram: Some(Addr::new(Ipv4Addr::UNSPECIFIED, up)),
            ..Default::default()
        };
        let mut st = self.sh.st.lock().unwrap();
        st.players.push(p.clone());
        st.add_to_group(id);
        st.desc.current_players += 1;
        let msg = wire::full(cmd::CREATEPLAYER, &wire::create_player(0, &p), tp);
        for m in st.machines.values() {
            tcp_send(m.tcp, msg.clone());
        }
        Ok(id)
    }

    /// Removes a player of this machine and tells everyone.
    pub fn destroy_player(&self, id: u32) {
        let mut st = self.sh.st.lock().unwrap();
        if !st.players.iter().any(|p| p.id == id && p.flags & pflag::LOCAL != 0) {
            return;
        }
        st.remove(id);
        let msg = wire::full(cmd::DELETEPLAYER, &wire::delete_player(id), self.sh.tcp_port);
        for m in st.machines.values() {
            tcp_send(m.tcp, msg.clone());
        }
    }

    /// Sends a game message from player `from` (on this machine) to player
    /// `to`, or to everyone when `to` is 0. `reliable` asks the reliable
    /// protocol to see it there.
    pub fn send(&self, from: u32, to: u32, data: &[u8], reliable: bool) {
        let mut st = self.sh.st.lock().unwrap();
        let key = st.desc.id_key;
        let rp = st.desc.flags & sflag::RELIABLE_PROTOCOL != 0;
        let mut payload = Vec::with_capacity(8 + data.len());
        if st.desc.flags & sflag::NO_MESSAGE_ID == 0 {
            wire::Wr::u32(&mut payload, from);
            wire::Wr::u32(&mut payload, to);
        }
        payload.extend_from_slice(data);
        // Each machine gets it once: addressed to its system player when it
        // is for everyone.
        let targets: Vec<(u32, u32)> = if to == 0 {
            st.machines.keys().map(|&m| (m, m)).collect()
        } else {
            st.players.iter().filter(|p| p.id == to && st.machines.contains_key(&p.system)).map(|p| (p.system, to)).collect()
        };
        for (machine, dest) in targets {
            let Some(m) = st.machines.get_mut(&machine) else { continue };
            if rp {
                for f in m.link.send(index(from, key), index(dest, key), &payload, reliable) {
                    let _ = self.sh.udp.send_to(&f, m.udp);
                }
            } else {
                let _ = self.sh.udp.send_to(&raw_player_message(&payload, self.sh.tcp_port), m.udp);
            }
        }
    }

    /// Opens or closes the session to newcomers (the host's call), and
    /// tells everyone.
    pub fn set_joinable(&self, open: bool) {
        let mut st = self.sh.st.lock().unwrap();
        if !st.host {
            return;
        }
        if open {
            st.desc.flags &= !sflag::JOIN_DISABLED;
        } else {
            st.desc.flags |= sflag::JOIN_DISABLED;
        }
        let msg = wire::full(cmd::SESSIONDESCCHANGED, &wire::session_changed(&st.desc, &st.name), self.sh.tcp_port);
        for m in st.machines.values() {
            tcp_send(m.tcp, msg.clone());
        }
    }

    /// The next thing that happened, if anything has.
    pub fn poll(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// Waits up to `wait` for the next thing that happens.
    pub fn wait(&self, wait: Duration) -> Option<Event> {
        self.events.recv_timeout(wait).ok()
    }

    /// The players (not system players or groups), and their names.
    pub fn players(&self) -> Vec<(u32, String, bool)> {
        let st = self.sh.st.lock().unwrap();
        st.players
            .iter()
            .filter(|p| !p.is_system() && !p.is_group())
            .map(|p| (p.id, p.name.clone().unwrap_or_default(), p.flags & pflag::LOCAL != 0))
            .collect()
    }

    /// Whether player `id` is on the host's machine.
    pub fn on_host(&self, id: u32) -> bool {
        let st = self.sh.st.lock().unwrap();
        let Some(ns) = st.players.iter().find(|p| p.is_system() && p.flags & pflag::NAMESERVER != 0).map(|p| p.id) else {
            return false;
        };
        st.players.iter().any(|p| p.id == id && p.system == ns)
    }

    pub fn is_host(&self) -> bool {
        self.sh.st.lock().unwrap().host
    }

    pub fn desc(&self) -> SessionDesc {
        self.sh.st.lock().unwrap().desc.clone()
    }

    pub fn name(&self) -> String {
        self.sh.st.lock().unwrap().name.clone()
    }

    /// The ports this machine listens on: TCP, UDP.
    pub fn ports(&self) -> (u16, u16) {
        (self.sh.tcp_port, self.sh.udp_port)
    }

    fn host_addr(&self) -> Option<SocketAddrV4> {
        let st = self.sh.st.lock().unwrap();
        let host = st.players.iter().find(|p| p.flags & pflag::NAMESERVER != 0)?;
        st.machines.get(&host.id).map(|m| m.tcp)
    }

    /// Leaves: tells everyone this machine's players have gone.
    pub fn close(&self) {
        if self.sh.quit.swap(true, Ordering::SeqCst) {
            return;
        }
        let st = self.sh.st.lock().unwrap();
        let mine: Vec<u32> = st
            .players
            .iter()
            .filter(|p| p.flags & pflag::LOCAL != 0 && !p.is_group())
            .map(|p| p.id)
            .collect();
        for m in st.machines.values() {
            // Game players first, the system player last.
            for id in mine.iter().filter(|&&id| id != st.me).chain(std::iter::once(&st.me)) {
                tcp_send(m.tcp, wire::full(cmd::DELETEPLAYER, &wire::delete_player(*id), self.sh.tcp_port));
            }
        }
        // Wake the listening threads so they see the quit.
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, self.sh.tcp_port)),
            Duration::from_millis(200),
        );
        let _ = self.sh.udp.send_to(&[0], (Ipv4Addr::LOCALHOST, self.sh.udp_port));
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

/// Whether to say what arrives (`MM2_DPLAY_DEBUG=1`).
fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("MM2_DPLAY_DEBUG").is_some())
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_string())
}

/// A player's index: its id without the session's mask, as the reliable
/// protocol names players.
fn index(id: u32, key: u32) -> u16 {
    ((id ^ key) & 0xffff) as u16
}

/// A player message without the reliable protocol: full header, no `play`.
fn raw_player_message(payload: &[u8], port: u16) -> Vec<u8> {
    let size = wire::PREFIX + payload.len();
    let mut out = Vec::with_capacity(size);
    wire::Wr::u32(&mut out, (size as u32 & 0x000f_ffff) | (wire::TOKEN_REMOTE << 20));
    Addr::new(Ipv4Addr::UNSPECIFIED, port).write(&mut out);
    out.extend_from_slice(payload);
    out
}

/// Puts `ip` in a player's addresses where they say 0.0.0.0.
fn fill_addr(p: &mut Player, ip: Ipv4Addr) {
    for a in [&mut p.stream, &mut p.dgram].into_iter().flatten() {
        if a.ip.is_unspecified() {
            a.ip = ip;
        }
    }
}

impl State {
    fn new_id(&mut self) -> u32 {
        let used: Vec<u16> = self.players.iter().map(|p| index(p.id, self.desc.id_key)).collect();
        while used.contains(&self.next_index) {
            self.next_index = self.next_index.wrapping_add(1);
        }
        let i = self.next_index;
        self.next_index = self.next_index.wrapping_add(1);
        let id = ((self.uniq as u32) << 16 | i as u32) ^ self.desc.id_key;
        id
    }

    fn add_to_group(&mut self, id: u32) {
        if let Some(g) = self.players.iter_mut().find(|p| p.is_group()) {
            if !g.members.contains(&id) {
                g.members.push(id);
            }
        }
    }

    fn remove(&mut self, id: u32) -> bool {
        let before = self.players.len();
        let was_player = self.players.iter().any(|p| p.id == id && !p.is_system() && !p.is_group());
        self.players.retain(|p| p.id != id);
        for g in self.players.iter_mut() {
            g.members.retain(|m| *m != id);
        }
        if was_player {
            self.desc.current_players = self.desc.current_players.saturating_sub(1);
        }
        self.machines.remove(&id);
        before != self.players.len()
    }

    /// Makes sure every other machine's system player has a link.
    fn refresh_machines(&mut self) {
        let me = self.me;
        for p in self.players.iter().filter(|p| p.is_system() && p.id != me) {
            if let (Some(t), Some(u)) = (p.stream, p.dgram) {
                if t.ip.is_unspecified() {
                    continue;
                }
                self.machines.entry(p.id).or_insert_with(|| Machine {
                    tcp: SocketAddrV4::new(t.ip, t.port),
                    udp: SocketAddrV4::new(u.ip, u.port),
                    link: rel::Link::default(),
                });
            }
        }
    }

    fn machine_of_index(&self, i: u16) -> Option<u32> {
        let key = self.desc.id_key;
        self.players.iter().find(|p| index(p.id, key) == i).map(|p| p.system)
    }
}

impl Shared {
    fn tick(&self) -> u32 {
        self.tick_base.wrapping_add(self.start.elapsed().as_millis() as u32)
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    /// Handles one system message. `from` is the sender's IP, `reply` its
    /// TCP port for answers.
    fn handle(&self, raw: &[u8], from: Ipv4Addr, reply: Option<u16>) {
        let Ok(m) = wire::parse_bare(raw) else { return };
        if debug() {
            eprintln!("dplay: got command {:#x} ({} bytes) from {from} reply port {reply:?}", m.command, raw.len());
        }
        let reply_to = reply.map(|p| SocketAddrV4::new(from, p));
        match m.command {
            cmd::PACKET | cmd::PACKET2_DATA => {
                let Ok(piece) = wire::read_piece(m.body) else { return };
                if m.command == cmd::PACKET2_DATA {
                    if let Some(to) = reply_to {
                        let mut ack = piece.message.to_vec();
                        wire::Wr::u32(&mut ack, piece.index);
                        tcp_send(to, wire::full(cmd::PACKET2_ACK, &ack, self.tcp_port));
                    }
                }
                let whole = {
                    let mut st = self.st.lock().unwrap();
                    let total = piece.total.clamp(1, 4096) as usize;
                    let parts = st.pieces.entry(piece.message).or_insert_with(|| vec![None; total]);
                    if (piece.index as usize) < parts.len() {
                        parts[piece.index as usize] = Some(piece.data.to_vec());
                    }
                    if parts.iter().all(|p| p.is_some()) {
                        let w: Vec<u8> = parts.iter().flatten().flatten().copied().collect();
                        st.pieces.remove(&piece.message);
                        Some(w)
                    } else {
                        None
                    }
                };
                if let Some(w) = whole {
                    self.handle(&w, from, reply);
                }
            }
            cmd::REQUESTPLAYERREPLY | cmd::SUPERENUMPLAYERSREPLY | cmd::ADDFORWARDREPLY => {
                if let Some(w) = self.waiter.lock().unwrap().as_ref() {
                    let _ = w.send((m.command, raw.to_vec(), from));
                }
            }
            cmd::REQUESTPLAYERID => {
                let Some(to) = reply_to else { return };
                let flags = wire::Rd::new(m.body).u32().unwrap_or(0);
                let mut st = self.st.lock().unwrap();
                if !st.host {
                    return;
                }
                let joining = flags & pflag::SYSTEM != 0;
                let full = st.desc.current_players >= st.desc.max_players;
                let closed = if joining {
                    st.desc.flags & sflag::JOIN_DISABLED != 0
                } else {
                    st.desc.flags & sflag::NEW_PLAYERS_DISABLED != 0
                };
                let (id, result) = if full || closed { (0, wire::ERR_NO_NEW_PLAYERS) } else { (st.new_id(), 0) };
                if id != 0 {
                    // Hold the id until the player turns up.
                    st.players.push(Player { id, flags: pflag::SYSTEM * joining as u32, system: id, ..Default::default() });
                }
                drop(st);
                tcp_send(to, wire::full(cmd::REQUESTPLAYERREPLY, &wire::player_reply(id, result), self.tcp_port));
            }
            cmd::ADDFORWARDREQUEST => {
                let Some(to) = reply_to else { return };
                let Ok(pm) = wire::read_player_msg(&m) else { return };
                let Some(mut p) = pm.player else { return };
                let mut st = self.st.lock().unwrap();
                if !st.host {
                    return;
                }
                p.flags &= !pflag::LOCAL;
                p.flags |= pflag::IN_GROUP;
                fill_addr(&mut p, from);
                st.players.retain(|q| q.id != p.id);
                // Tell the machines already in about the new one.
                let fwd = wire::full(cmd::ADDFORWARD, &wire::create_player(0, &p), self.tcp_port);
                for mc in st.machines.values() {
                    tcp_send(mc.tcp, fwd.clone());
                }
                st.players.push(p);
                st.refresh_machines();
                let players: Vec<Player> = st.players.iter().filter(|q| !q.is_group()).cloned().collect();
                let groups: Vec<Player> = st.players.iter().filter(|q| q.is_group()).cloned().collect();
                let msg = wire::full(
                    cmd::SUPERENUMPLAYERSREPLY,
                    &wire::super_enum_reply(&st.desc, &st.name, &players, &groups),
                    self.tcp_port,
                );
                drop(st);
                tcp_send(to, msg);
            }
            cmd::ADDFORWARD => {
                // The host's news of a machine that has joined.
                let Ok(pm) = wire::read_player_msg(&m) else { return };
                let Some(mut p) = pm.player else { return };
                fill_addr(&mut p, from);
                p.flags &= !pflag::LOCAL;
                let mut st = self.st.lock().unwrap();
                st.players.retain(|q| q.id != p.id);
                let id = p.id;
                st.players.push(p);
                st.refresh_machines();
                drop(st);
                if let Some(to) = reply_to {
                    let mut body = Vec::new();
                    wire::Wr::u32(&mut body, id);
                    tcp_send(to, wire::full(cmd::ADDFORWARDACK, &body, self.tcp_port));
                }
            }
            cmd::CREATEPLAYER | cmd::CREATEPLAYERVERIFY => {
                let Ok(pm) = wire::read_player_msg(&m) else { return };
                let Some(mut p) = pm.player else { return };
                p.flags &= !pflag::LOCAL;
                fill_addr(&mut p, from);
                let mut st = self.st.lock().unwrap();
                let new = !st.players.iter().any(|q| q.id == p.id && q.name.is_some());
                st.players.retain(|q| q.id != p.id);
                let (id, name, system) = (p.id, p.name.clone().unwrap_or_default(), p.is_system());
                st.players.push(p);
                if !system {
                    st.add_to_group(id);
                    if new {
                        st.desc.current_players += 1;
                    }
                }
                st.refresh_machines();
                drop(st);
                if new && !system {
                    self.emit(Event::PlayerJoined { id, name });
                }
            }
            cmd::DELETEPLAYER => {
                let Ok(pm) = wire::read_player_msg(&m) else { return };
                self.forget(pm.player_id);
            }
            cmd::SESSIONDESCCHANGED => {
                if let Ok(d) = wire::read_session_changed(m.body) {
                    self.st.lock().unwrap().desc = d.clone();
                    self.emit(Event::Changed(d));
                }
            }
            cmd::YOUAREDEAD => self.emit(Event::Closed),
            _ => {}
        }
    }

    /// A player or machine gone: forget it, and its players with it.
    fn forget(&self, id: u32) {
        let mut st = self.st.lock().unwrap();
        let mut gone = vec![id];
        if st.players.iter().any(|p| p.id == id && p.is_system()) {
            gone.extend(st.players.iter().filter(|p| p.system == id && p.id != id && !p.is_group()).map(|p| p.id));
        }
        let mut left = Vec::new();
        for g in gone {
            let was_player = st.players.iter().any(|p| p.id == g && !p.is_system() && !p.is_group());
            if st.remove(g) && was_player {
                left.push(g);
            }
        }
        let host_gone = st.players.iter().all(|p| p.flags & pflag::NAMESERVER == 0) && !st.host;
        drop(st);
        for g in left {
            self.emit(Event::PlayerLeft { id: g });
        }
        if host_gone && id != 0 {
            // The host left without handing on: nothing more will come.
        }
    }

    /// A message the reliable protocol (or a raw datagram) delivered from
    /// machine `machine`.
    fn deliver(&self, machine: u32, data: &[u8], from_ip: Ipv4Addr) {
        if data.len() >= 8 && &data[..4] == wire::SIGNATURE {
            let Ok(m) = wire::parse_bare(data) else { return };
            match m.command {
                cmd::PING => {
                    let mut r = wire::Rd::new(m.body);
                    let (who, tick) = (r.u32().unwrap_or(0), r.u32().unwrap_or(0));
                    let reply = wire::bare(cmd::PINGREPLY, &wire::ping(who, tick));
                    let mut st = self.st.lock().unwrap();
                    let key = st.desc.id_key;
                    let me = st.me;
                    if let Some(mc) = st.machines.get_mut(&machine) {
                        for f in mc.link.send(index(me, key), index(machine, key), &reply, false) {
                            let _ = self.udp.send_to(&f, mc.udp);
                        }
                    }
                }
                cmd::PINGREPLY => {}
                _ => self.handle(data, from_ip, None),
            }
            return;
        }
        let nm = self.st.lock().unwrap().desc.flags & sflag::NO_MESSAGE_ID != 0;
        if nm {
            self.emit(Event::Data { from: 0, to: 0, data: data.to_vec() });
        } else if data.len() >= 8 {
            let mut r = wire::Rd::new(data);
            let (from, to) = (r.u32().unwrap(), r.u32().unwrap());
            self.emit(Event::Data { from, to, data: data[8..].to_vec() });
        }
    }
}

/// Reads the next message on a TCP connection; `None` when the sender has
/// closed it (or sent something that isn't DirectPlay).
fn read_message(c: &mut TcpStream) -> Option<Vec<u8>> {
    let mut first = [0u8; 4];
    c.read_exact(&mut first).ok()?;
    let size = wire::full_size(first);
    if !(wire::PREFIX..=0x10_0000).contains(&size) {
        return None;
    }
    let mut msg = first.to_vec();
    msg.resize(size, 0);
    c.read_exact(&mut msg[4..]).ok()?;
    Some(msg)
}

/// Reads every message on a TCP connection until the sender closes it.
fn read_messages(c: &mut TcpStream) -> Vec<Vec<u8>> {
    std::iter::from_fn(|| read_message(c)).collect()
}

fn tcp_loop(sh: Arc<Shared>, tcp: TcpListener) {
    for c in tcp.incoming() {
        if sh.quit.load(Ordering::SeqCst) {
            break;
        }
        let Ok(mut c) = c else { continue };
        let sh = sh.clone();
        thread::spawn(move || {
            // DirectPlay may keep a connection open and send more on it
            // after our answer, so each message is handled as it comes.
            let _ = c.set_read_timeout(Some(Duration::from_secs(120)));
            let ip = match c.peer_addr() {
                Ok(SocketAddr::V4(a)) => *a.ip(),
                _ => return,
            };
            while let Some(raw) = read_message(&mut c) {
                if let Ok(m) = wire::parse_full(&raw) {
                    sh.handle(&raw[wire::PREFIX..], ip, m.from.map(|a| a.port));
                }
                if sh.quit.load(Ordering::SeqCst) {
                    break;
                }
            }
        });
    }
}

fn udp_loop(sh: Arc<Shared>) {
    let mut buf = vec![0u8; 65536];
    loop {
        let Ok((n, SocketAddr::V4(src))) = sh.udp.recv_from(&mut buf) else {
            if sh.quit.load(Ordering::SeqCst) {
                break;
            }
            continue;
        };
        if sh.quit.load(Ordering::SeqCst) {
            break;
        }
        let b = &buf[..n];
        // A full header (size and a DirectPlay token) is a message sent
        // without the reliable protocol; anything else is a frame.
        if n >= wire::PREFIX + 8 && matches!(u32::from_le_bytes(b[..4].try_into().unwrap()) >> 20, 0xfab | 0xcab | 0xbab) {
            if &b[wire::PREFIX..wire::PREFIX + 4] == wire::SIGNATURE {
                let port = wire::parse_full(b).ok().and_then(|m| m.from).map(|a| a.port);
                sh.handle(&b[wire::PREFIX..], *src.ip(), port);
            } else if n >= wire::PREFIX + 8 {
                let mut r = wire::Rd::new(&b[wire::PREFIX..]);
                let from = r.u32().unwrap_or(0);
                let machine = sh.st.lock().unwrap().players.iter().find(|p| p.id == from).map(|p| p.system).unwrap_or(0);
                sh.deliver(machine, &b[wire::PREFIX..], *src.ip());
            }
            continue;
        }
        let Some(f) = rel::parse(b) else { continue };
        let tick = sh.tick();
        let (got, machine) = {
            let mut st = sh.st.lock().unwrap();
            let Some(machine) = st.machine_of_index(f.from) else { continue };
            let Some(mc) = st.machines.get_mut(&machine) else { continue };
            let got = mc.link.receive(b, tick);
            for r in &got.reply {
                let _ = sh.udp.send_to(r, mc.udp);
            }
            (got, machine)
        };
        if let Some(msg) = got.message {
            sh.deliver(machine, &msg, *src.ip());
        }
    }
}

fn enum_loop(sh: Arc<Shared>, sock: UdpSocket) {
    let mut buf = vec![0u8; 2048];
    loop {
        let Ok((n, SocketAddr::V4(src))) = sock.recv_from(&mut buf) else { continue };
        if sh.quit.load(Ordering::SeqCst) {
            break;
        }
        let Ok(m) = wire::parse_full(&buf[..n]) else { continue };
        if m.command != cmd::ENUMSESSIONS {
            continue;
        }
        let Ok((app, _flags)) = wire::read_enum_sessions(m.body) else { continue };
        let st = sh.st.lock().unwrap();
        let open = st.desc.flags & sflag::JOIN_DISABLED == 0 && st.desc.current_players < st.desc.max_players;
        if app != st.desc.application || !open {
            continue;
        }
        let Some(port) = m.from.map(|a| a.port) else { continue };
        let reply = wire::full(cmd::ENUMSESSIONSREPLY, &wire::enum_reply(&st.desc, &st.name), sh.tcp_port);
        drop(st);
        tcp_send(SocketAddrV4::new(*src.ip(), port), reply);
    }
}

fn timer_loop(sh: Arc<Shared>) {
    while !sh.quit.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
        let now = Instant::now();
        let mut dead = Vec::new();
        {
            let mut st = sh.st.lock().unwrap();
            for (id, mc) in st.machines.iter_mut() {
                let (frames, gone) = mc.link.resend(now);
                for f in frames {
                    let _ = sh.udp.send_to(&f, mc.udp);
                }
                if gone {
                    dead.push(*id);
                }
            }
        }
        for id in dead {
            sh.forget(id);
        }
    }
}
