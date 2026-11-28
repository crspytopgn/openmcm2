//! The bytes of DirectPlay 4's messages, as Microsoft's open
//! specification "DirectPlay 4 Protocol: Core and Service Providers"
//! ([MC-DPL4CS]) lays them out, checked against what the original game
//! and DirectX 7's own DirectPlay send to each other.
//!
//! A message is a header, the word `play`, a command number and a
//! version, then the command's own fields. Sent on its own (over TCP, or
//! as a broadcast), the header starts with the message's size and the
//! sender's address; inside the reliable protocol's frames, it doesn't.

use std::net::{Ipv4Addr, SocketAddrV4};

/// The word every DirectPlay system message carries.
pub const SIGNATURE: &[u8; 4] = b"play";
/// The protocol version DirectX 7's DirectPlay gives (DX61AVERSION), so
/// the original game sees what it is used to.
pub const VERSION: u16 = 11;
/// The size of the header before `play`: size and token, then an address.
pub const PREFIX: usize = 20;
/// The token in the top 12 bits of a message's size: sent by a remote
/// DirectPlay machine.
pub const TOKEN_REMOTE: u32 = 0xfab;

/// The commands this crate sends or reads (section 2.2.6).
pub mod cmd {
    pub const ENUMSESSIONSREPLY: u16 = 0x01;
    pub const ENUMSESSIONS: u16 = 0x02;
    pub const REQUESTPLAYERID: u16 = 0x05;
    pub const REQUESTPLAYERREPLY: u16 = 0x07;
    pub const CREATEPLAYER: u16 = 0x08;
    pub const DELETEPLAYER: u16 = 0x0b;
    pub const ADDFORWARDREQUEST: u16 = 0x13;
    pub const PACKET: u16 = 0x15;
    pub const PING: u16 = 0x16;
    pub const PINGREPLY: u16 = 0x17;
    pub const YOUAREDEAD: u16 = 0x18;
    pub const SESSIONDESCCHANGED: u16 = 0x1a;
    pub const ADDFORWARDREPLY: u16 = 0x24;
    pub const SUPERENUMPLAYERSREPLY: u16 = 0x29;
    pub const ADDFORWARD: u16 = 0x2e;
    pub const ADDFORWARDACK: u16 = 0x2f;
    pub const PACKET2_DATA: u16 = 0x30;
    pub const PACKET2_ACK: u16 = 0x31;
    pub const CREATEPLAYERVERIFY: u16 = 0x38;
}

/// Player flags (section 2.2.2).
pub mod pflag {
    /// The machine's own system player.
    pub const SYSTEM: u32 = 0x1;
    /// The host's system player.
    pub const NAMESERVER: u32 = 0x2;
    /// In a group (every system player is).
    pub const IN_GROUP: u32 = 0x4;
    /// On the sending machine.
    pub const LOCAL: u32 = 0x8;
    /// The session's system group, which holds every player.
    pub const SYSTEM_GROUP: u32 = 0x40;
}

/// Session flags (section 2.2.5).
pub mod sflag {
    pub const NEW_PLAYERS_DISABLED: u32 = 0x1;
    pub const MIGRATE_HOST: u32 = 0x4;
    pub const NO_MESSAGE_ID: u32 = 0x8;
    pub const JOIN_DISABLED: u32 = 0x20;
    pub const KEEP_ALIVE: u32 = 0x40;
    pub const CLIENT_SERVER: u32 = 0x1000;
    /// The reliable protocol carries every message once players are in.
    pub const RELIABLE_PROTOCOL: u32 = 0x2000;
}

/// DPERR_NONEWPLAYERS: a full or closed session's answer.
pub const ERR_NO_NEW_PLAYERS: u32 = 0x8877_014a;

pub type Guid = [u8; 16];

/// Why bytes could not be read as a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Short;

/// A little-endian reader that runs out politely.
#[derive(Clone)]
pub struct Rd<'a> {
    pub b: &'a [u8],
    pub at: usize,
}

impl<'a> Rd<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Rd { b, at: 0 }
    }
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], Short> {
        let s = self.b.get(self.at..self.at + n).ok_or(Short)?;
        self.at += n;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8, Short> {
        Ok(self.bytes(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16, Short> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32, Short> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    /// A length in 1, 2 or 4 bytes, as a superpacked player's mask says.
    fn sized(&mut self, code: u32) -> Result<u32, Short> {
        match code {
            0 => Ok(0),
            1 => self.u8().map(u32::from),
            2 => self.u16().map(u32::from),
            _ => self.u32(),
        }
    }
    pub fn guid(&mut self) -> Result<Guid, Short> {
        Ok(self.bytes(16)?.try_into().unwrap())
    }
    /// A null-terminated UTF-16 string.
    pub fn wstr(&mut self) -> Result<String, Short> {
        let mut units = Vec::new();
        loop {
            let u = self.u16()?;
            if u == 0 {
                break;
            }
            units.push(u);
        }
        Ok(String::from_utf16_lossy(&units))
    }
    pub fn rest(&self) -> &'a [u8] {
        &self.b[self.at.min(self.b.len())..]
    }
}

/// Little-endian writing.
pub trait Wr {
    fn u16(&mut self, v: u16);
    fn u32(&mut self, v: u32);
    fn wstr(&mut self, s: &str);
}

impl Wr for Vec<u8> {
    fn u16(&mut self, v: u16) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.extend_from_slice(&v.to_le_bytes());
    }
    fn wstr(&mut self, s: &str) {
        for u in s.encode_utf16() {
            self.u16(u);
        }
        self.u16(0);
    }
}

/// The bytes of `s` as a null-terminated UTF-16 string.
pub fn wstr_len(s: &str) -> usize {
    (s.encode_utf16().count() + 1) * 2
}

/// A SOCKADDR_IN: the family, the port (in network order) and the
/// address, padded to 16 bytes (section 2.2.1). 0.0.0.0 stands for "the
/// address this came from".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Addr {
    pub ip: Ipv4Addr,
    pub port: u16,
}

impl Addr {
    pub fn new(ip: Ipv4Addr, port: u16) -> Self {
        Addr { ip, port }
    }
    pub fn read(r: &mut Rd) -> Result<Self, Short> {
        let b = r.bytes(16)?;
        Ok(Addr { port: u16::from_be_bytes([b[2], b[3]]), ip: Ipv4Addr::new(b[4], b[5], b[6], b[7]) })
    }
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&[2, 0]);
        out.extend_from_slice(&self.port.to_be_bytes());
        out.extend_from_slice(&self.ip.octets());
        out.extend_from_slice(&[0; 8]);
    }
    /// This address, with 0.0.0.0 taken as `from`.
    pub fn or_from(&self, from: Ipv4Addr) -> SocketAddrV4 {
        let ip = if self.ip.is_unspecified() { from } else { self.ip };
        SocketAddrV4::new(ip, self.port)
    }
}

/// A message with its header taken apart.
#[derive(Debug, Clone)]
pub struct Msg<'a> {
    /// The sender's address from a full header: the port it listens on.
    pub from: Option<Addr>,
    pub command: u16,
    pub version: u16,
    /// The command's fields, after the version.
    pub body: &'a [u8],
    /// The bytes from `play` on, which the offsets in some messages count
    /// from.
    pub whole: &'a [u8],
}

/// Reads a message with a full header (over TCP or a broadcast).
pub fn parse_full(b: &[u8]) -> Result<Msg<'_>, Short> {
    if b.len() < PREFIX + 8 || &b[PREFIX..PREFIX + 4] != SIGNATURE {
        return Err(Short);
    }
    let mut r = Rd::new(&b[4..]);
    let from = Addr::read(&mut r)?;
    let mut m = parse_bare(&b[PREFIX..])?;
    m.from = Some(from);
    Ok(m)
}

/// Reads a message whose header is `play` onwards (inside a reliable
/// frame or a packet).
pub fn parse_bare(b: &[u8]) -> Result<Msg<'_>, Short> {
    if b.len() < 8 || &b[..4] != SIGNATURE {
        return Err(Short);
    }
    Ok(Msg {
        from: None,
        command: u16::from_le_bytes([b[4], b[5]]),
        version: u16::from_le_bytes([b[6], b[7]]),
        body: &b[8..],
        whole: b,
    })
}

/// The total size a full header gives, so a TCP reader knows how much to
/// read: the low 20 bits of the first word.
pub fn full_size(first: [u8; 4]) -> usize {
    (u32::from_le_bytes(first) & 0x000f_ffff) as usize
}

/// A message with a full header: size and token, the port the sender
/// listens on (its address as 0.0.0.0), then `play`.
pub fn full(command: u16, body: &[u8], listen_port: u16) -> Vec<u8> {
    let size = PREFIX + 8 + body.len();
    let mut out = Vec::with_capacity(size);
    out.u32((size as u32 & 0x000f_ffff) | (TOKEN_REMOTE << 20));
    Addr::new(Ipv4Addr::UNSPECIFIED, listen_port).write(&mut out);
    out.extend_from_slice(&bare(command, body));
    out
}

/// A message whose header is just `play`, the command and the version.
pub fn bare(command: u16, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(SIGNATURE);
    out.u16(command);
    out.u16(VERSION);
    out.extend_from_slice(body);
    out
}

/// DPSESSIONDESC2 (section 2.2.5), 80 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionDesc {
    pub flags: u32,
    pub instance: Guid,
    pub application: Guid,
    pub max_players: u32,
    pub current_players: u32,
    /// The mask player ids are XORed with.
    pub id_key: u32,
    pub reserved2: u32,
    pub user: [u32; 4],
}

impl SessionDesc {
    pub const SIZE: usize = 80;
    pub fn read(r: &mut Rd) -> Result<Self, Short> {
        let size = r.u32()? as usize;
        let start = r.at - 4;
        let flags = r.u32()?;
        let instance = r.guid()?;
        let application = r.guid()?;
        let max_players = r.u32()?;
        let current_players = r.u32()?;
        r.u32()?; // name pointer
        r.u32()?; // password pointer
        let id_key = r.u32()?;
        let reserved2 = r.u32()?;
        let user = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
        r.at = start + size.max(Self::SIZE);
        Ok(SessionDesc { flags, instance, application, max_players, current_players, id_key, reserved2, user })
    }
    pub fn write(&self, out: &mut Vec<u8>) {
        out.u32(Self::SIZE as u32);
        out.u32(self.flags);
        out.extend_from_slice(&self.instance);
        out.extend_from_slice(&self.application);
        out.u32(self.max_players);
        out.u32(self.current_players);
        out.u32(0);
        out.u32(0);
        out.u32(self.id_key);
        out.u32(self.reserved2);
        for u in self.user {
            out.u32(u);
        }
    }
}

/// What DirectPlay knows of a player or group, in either of the two
/// packed forms the protocol sends.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Player {
    pub id: u32,
    pub flags: u32,
    pub name: Option<String>,
    pub long_name: Option<String>,
    /// A system player's protocol version.
    pub version: u32,
    /// The system player of the machine the player is on (a system
    /// player's own id).
    pub system: u32,
    pub data: Vec<u8>,
    /// Where to reach the player over TCP and UDP.
    pub stream: Option<Addr>,
    pub dgram: Option<Addr>,
    /// A group's members.
    pub members: Vec<u32>,
    pub parent: u32,
}

impl Player {
    pub fn is_system(&self) -> bool {
        self.flags & pflag::SYSTEM != 0
    }
    pub fn is_group(&self) -> bool {
        self.flags & pflag::SYSTEM_GROUP != 0
    }

    fn sp_data(&self) -> Vec<u8> {
        let mut sp = Vec::new();
        if let (Some(s), Some(d)) = (self.stream, self.dgram) {
            s.write(&mut sp);
            d.write(&mut sp);
        }
        sp
    }

    fn read_sp(&mut self, sp: &[u8]) {
        if sp.len() >= 32 {
            let mut r = Rd::new(sp);
            self.stream = Addr::read(&mut r).ok();
            self.dgram = Addr::read(&mut r).ok();
        }
    }

    /// DPLAYI_PACKEDPLAYER (section 2.2.2), as CREATEPLAYER and
    /// ADDFORWARDREQUEST carry a player.
    pub fn write_packed(&self, out: &mut Vec<u8>) {
        let short = self.name.as_deref().map(wstr_len).unwrap_or(0);
        let long = self.long_name.as_deref().map(wstr_len).unwrap_or(0);
        let sp = self.sp_data();
        let size = 48 + short + long + sp.len() + self.data.len() + 4 * self.members.len();
        out.u32(size as u32);
        out.u32(self.flags);
        out.u32(self.id);
        out.u32(short as u32);
        out.u32(long as u32);
        out.u32(sp.len() as u32);
        out.u32(self.data.len() as u32);
        out.u32(self.members.len() as u32);
        out.u32(self.system);
        out.u32(48);
        out.u32(self.version);
        out.u32(self.parent);
        if let Some(n) = &self.name {
            out.wstr(n);
        }
        if let Some(n) = &self.long_name {
            out.wstr(n);
        }
        out.extend_from_slice(&sp);
        out.extend_from_slice(&self.data);
        for m in &self.members {
            out.u32(*m);
        }
    }

    pub fn read_packed(r: &mut Rd) -> Result<Self, Short> {
        let start = r.at;
        let size = r.u32()? as usize;
        let flags = r.u32()?;
        let id = r.u32()?;
        let short = r.u32()? as usize;
        let long = r.u32()? as usize;
        let sp = r.u32()? as usize;
        let data = r.u32()? as usize;
        let count = r.u32()? as usize;
        let system = r.u32()?;
        let fixed = r.u32()? as usize;
        let version = r.u32()?;
        let parent = r.u32()?;
        r.at = start + fixed.max(48);
        let mut p = Player { id, flags, system, version, parent, ..Default::default() };
        if short > 0 {
            p.name = Some(Rd::new(r.bytes(short)?).wstr().unwrap_or_default());
        }
        if long > 0 {
            p.long_name = Some(Rd::new(r.bytes(long)?).wstr().unwrap_or_default());
        }
        let spd = r.bytes(sp)?;
        p.read_sp(spd);
        p.data = r.bytes(data)?.to_vec();
        for _ in 0..count {
            p.members.push(r.u32()?);
        }
        r.at = r.at.max(start + size);
        Ok(p)
    }

    /// DPLAYI_SUPERPACKEDPLAYER (section 2.2.3), as SUPERENUMPLAYERSREPLY
    /// carries each player.
    pub fn write_super(&self, out: &mut Vec<u8>) {
        let sp = self.sp_data();
        let mut mask = 0u32;
        if self.name.is_some() {
            mask |= 0x1;
        }
        if self.long_name.is_some() {
            mask |= 0x2;
        }
        if !sp.is_empty() {
            mask |= size_code(sp.len()) << 2;
        }
        if !self.data.is_empty() {
            mask |= size_code(self.data.len()) << 4;
        }
        if !self.members.is_empty() {
            mask |= size_code(self.members.len()) << 6;
        }
        if self.parent != 0 {
            mask |= 0x100;
        }
        out.u32(16);
        out.u32(self.flags);
        out.u32(self.id);
        out.u32(mask);
        out.u32(if self.is_system() { self.version } else { self.system });
        if let Some(n) = &self.name {
            out.wstr(n);
        }
        if let Some(n) = &self.long_name {
            out.wstr(n);
        }
        if !self.data.is_empty() {
            write_sized(out, self.data.len(), (mask >> 4) & 3);
            out.extend_from_slice(&self.data);
        }
        if !sp.is_empty() {
            write_sized(out, sp.len(), (mask >> 2) & 3);
            out.extend_from_slice(&sp);
        }
        if !self.members.is_empty() {
            write_sized(out, self.members.len(), (mask >> 6) & 3);
            for m in &self.members {
                out.u32(*m);
            }
        }
        if self.parent != 0 {
            out.u32(self.parent);
        }
    }

    pub fn read_super(r: &mut Rd) -> Result<Self, Short> {
        let start = r.at;
        let size = r.u32()? as usize;
        let flags = r.u32()?;
        let id = r.u32()?;
        let mask = r.u32()?;
        let vs = r.u32()?;
        r.at = start + size.max(16) + 4;
        let mut p = Player { id, flags, ..Default::default() };
        if p.is_system() {
            p.version = vs;
            p.system = id;
        } else {
            p.system = vs;
        }
        if mask & 0x1 != 0 {
            p.name = Some(r.wstr()?);
        }
        if mask & 0x2 != 0 {
            p.long_name = Some(r.wstr()?);
        }
        let pd = r.sized((mask >> 4) & 3)? as usize;
        p.data = r.bytes(pd)?.to_vec();
        let sp = r.sized((mask >> 2) & 3)? as usize;
        let spd = r.bytes(sp)?;
        p.read_sp(spd);
        let count = r.sized((mask >> 6) & 3)?;
        for _ in 0..count {
            p.members.push(r.u32()?);
        }
        if mask & 0x100 != 0 {
            p.parent = r.u32()?;
        }
        let shortcuts = r.sized((mask >> 9) & 3)?;
        for _ in 0..shortcuts {
            r.u32()?;
        }
        Ok(p)
    }
}

fn size_code(n: usize) -> u32 {
    if n < 0x100 {
        1
    } else if n < 0x10000 {
        2
    } else {
        3
    }
}

fn write_sized(out: &mut Vec<u8>, n: usize, code: u32) {
    match code {
        1 => out.push(n as u8),
        2 => out.u16(n as u16),
        _ => out.u32(n as u32),
    }
}

// ---- The messages, as this crate builds and reads them. ----

/// ENUMSESSIONS: who has a game of this application?
pub fn enum_sessions(app: &Guid, flags: u32) -> Vec<u8> {
    let mut b = app.to_vec();
    b.u32(0);
    b.u32(flags);
    b
}

/// An ENUMSESSIONS request's application and flags.
pub fn read_enum_sessions(body: &[u8]) -> Result<(Guid, u32), Short> {
    let mut r = Rd::new(body);
    let app = r.guid()?;
    r.u32()?;
    Ok((app, r.u32()?))
}

/// ENUMSESSIONSREPLY: a session and its name.
pub fn enum_reply(desc: &SessionDesc, name: &str) -> Vec<u8> {
    let mut b = Vec::new();
    desc.write(&mut b);
    b.u32(8 + SessionDesc::SIZE as u32 + 4);
    b.wstr(name);
    b
}

/// An ENUMSESSIONSREPLY's session and name.
pub fn read_enum_reply(m: &Msg) -> Result<(SessionDesc, String), Short> {
    let mut r = Rd::new(m.body);
    let desc = SessionDesc::read(&mut r)?;
    let off = r.u32()? as usize;
    let name = if off >= 8 && off < m.whole.len() { Rd::new(&m.whole[off..]).wstr()? } else { String::new() };
    Ok((desc, name))
}

/// REQUESTPLAYERREPLY: the new id, or an error.
pub fn player_reply(id: u32, result: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.u32(id);
    b.extend_from_slice(&[0; 24]);
    b.u32(0);
    b.u32(0);
    b.u32(result);
    b
}

/// A REQUESTPLAYERREPLY's id and result.
pub fn read_player_reply(body: &[u8]) -> Result<(u32, u32), Short> {
    let mut r = Rd::new(body);
    let id = r.u32()?;
    r.bytes(24 + 8)?;
    Ok((id, r.u32()?))
}

/// The fields CREATEPLAYER, DELETEPLAYER, ADDFORWARD and
/// ADDFORWARDREQUEST start with, then the player when there is one.
#[derive(Debug, Clone, Default)]
pub struct PlayerMsg {
    pub to: u32,
    pub player_id: u32,
    pub group: u32,
    pub player: Option<Player>,
}

/// CREATEPLAYER (and ADDFORWARD, which is laid out the same): a player
/// to add.
pub fn create_player(to: u32, p: &Player) -> Vec<u8> {
    let mut b = Vec::new();
    b.u32(to);
    b.u32(p.id);
    b.u32(0);
    b.u32(28);
    b.u32(0);
    p.write_packed(&mut b);
    b.u16(0);
    b.u32(0);
    b
}

/// ADDFORWARDREQUEST: a joining machine's system player, sent to the host.
/// The last field is the session's id mask (see [`SessionDesc::id_key`]),
/// which is what DirectX's DirectPlay checks there.
pub fn add_forward_request(p: &Player, tick: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.u32(0);
    b.u32(p.id);
    b.u32(0);
    b.u32(28);
    let mut packed = Vec::new();
    p.write_packed(&mut packed);
    b.u32(28 + packed.len() as u32);
    b.extend_from_slice(&packed);
    b.u16(0);
    b.u32(tick);
    b
}

/// DELETEPLAYER: a player that has gone.
pub fn delete_player(id: u32) -> Vec<u8> {
    let mut b = Vec::new();
    for v in [0, id, 0, 0, 0] {
        b.u32(v);
    }
    b
}

/// Reads the start of any of the player messages, and the packed player
/// after it when the create offset points at one.
pub fn read_player_msg(m: &Msg) -> Result<PlayerMsg, Short> {
    let mut r = Rd::new(m.body);
    let to = r.u32()?;
    let player_id = r.u32()?;
    let group = r.u32()?;
    let create = r.u32()? as usize;
    r.u32()?;
    let player = if create >= 8 && create < m.whole.len() {
        Some(Player::read_packed(&mut Rd::new(&m.whole[create..]))?)
    } else {
        None
    };
    Ok(PlayerMsg { to, player_id, group, player })
}

/// SUPERENUMPLAYERSREPLY: the session and everyone in it.
pub fn super_enum_reply(desc: &SessionDesc, name: &str, players: &[Player], groups: &[Player]) -> Vec<u8> {
    let head = 8 + 28;
    let desc_off = head;
    let name_off = desc_off + SessionDesc::SIZE;
    let packed_off = name_off + wstr_len(name);
    let mut b = Vec::new();
    b.u32(players.len() as u32);
    b.u32(groups.len() as u32);
    b.u32(packed_off as u32);
    b.u32(0);
    b.u32(desc_off as u32);
    b.u32(name_off as u32);
    b.u32(0);
    desc.write(&mut b);
    b.wstr(name);
    for p in players.iter().chain(groups) {
        p.write_super(&mut b);
    }
    b
}

/// A SUPERENUMPLAYERSREPLY's session, name, players and groups.
pub fn read_super_enum_reply(m: &Msg) -> Result<(SessionDesc, String, Vec<Player>), Short> {
    let mut r = Rd::new(m.body);
    let players = r.u32()? as usize;
    let groups = r.u32()? as usize;
    let packed = r.u32()? as usize;
    let shortcuts = r.u32()? as usize;
    let desc_off = r.u32()? as usize;
    let name_off = r.u32()? as usize;
    let desc = SessionDesc::read(&mut Rd::new(m.whole.get(desc_off..).ok_or(Short)?))?;
    let name = if name_off >= 8 { Rd::new(m.whole.get(name_off..).ok_or(Short)?).wstr()? } else { String::new() };
    let mut r = Rd::new(m.whole.get(packed..).ok_or(Short)?);
    let mut all = Vec::new();
    for _ in 0..players + groups + shortcuts {
        all.push(Player::read_super(&mut r)?);
    }
    Ok((desc, name, all))
}

/// SESSIONDESCCHANGED: the session as it is now.
pub fn session_changed(desc: &SessionDesc, name: &str) -> Vec<u8> {
    let mut b = Vec::new();
    b.u32(0);
    b.u32(8 + 12 + SessionDesc::SIZE as u32);
    b.u32(0);
    desc.write(&mut b);
    b.wstr(name);
    b
}

/// A SESSIONDESCCHANGED's session.
pub fn read_session_changed(body: &[u8]) -> Result<SessionDesc, Short> {
    let mut r = Rd::new(body);
    r.bytes(12)?;
    SessionDesc::read(&mut r)
}

/// PING and PINGREPLY: who, and the tick count to echo.
pub fn ping(from: u32, tick: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.u32(from);
    b.u32(tick);
    b
}

/// The message a PACKET or PACKET2_DATA carries a piece of: its id, this
/// piece's index and place, and how many pieces and bytes there are.
#[derive(Debug, Clone)]
pub struct Piece<'a> {
    pub message: Guid,
    pub index: u32,
    pub offset: u32,
    pub total: u32,
    pub size: u32,
    pub data: &'a [u8],
}

pub fn read_piece<'a>(body: &'a [u8]) -> Result<Piece<'a>, Short> {
    let mut r = Rd::new(body);
    let message = r.guid()?;
    let index = r.u32()?;
    let len = r.u32()? as usize;
    let offset = r.u32()?;
    let total = r.u32()?;
    let size = r.u32()?;
    r.u32()?;
    Ok(Piece { message, index, offset, total, size, data: r.bytes(len)? })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// The original game's ENUMSESSIONSREPLY, from two copies of the demo
    /// playing each other on DirectX 7's DirectPlay.
    const ENUM_REPLY: &str = "8200b0fa02000934000000000000000000000000706c617901000b005000000044a00000e12cf16340ef564f8fa4111f90f39df1c2bb4d1839ffcf11b11200a0c90aeab608000000010000000000000000000000fff2110000000000000000000000000000000000000000005c0000004f0072006900670052006100630065000000";

    #[test]
    fn reads_and_rebuilds_the_games_enum_reply() {
        let b = hex(ENUM_REPLY);
        assert_eq!(full_size(b[..4].try_into().unwrap()), b.len());
        let m = parse_full(&b).unwrap();
        assert_eq!(m.from.unwrap().port, 2356);
        assert_eq!(m.command, cmd::ENUMSESSIONSREPLY);
        let (desc, name) = read_enum_reply(&m).unwrap();
        assert_eq!(name, "OrigRace");
        assert_eq!(desc.flags, 0xa044);
        assert_eq!(desc.max_players, 8);
        assert_eq!(desc.id_key, 0x0011_f2ff);
        assert_eq!(full(cmd::ENUMSESSIONSREPLY, &enum_reply(&desc, &name), 2356), b);
    }

    /// The host's SUPERENUMPLAYERSREPLY to the joining copy.
    const SUPER: &str = "5c01b0fa02000934000000000000000000000000706c617929000b00030000000100000086000000000000002400000074000000000000005000000044a00000e12cf16340ef564f8fa4111f90f39df1c2bb4d1839ffcf11b11200a0c90aeab608000000010000000000000000000000fff2110000000000000000000000000000000000000000004f00720069006700520061006300650000001000000005000000fcf21000040000000b00000020020008fec0000202000000000000000002000901c00002020000000000000000100000000f000000fff21000040000000b000000200200093400000000000000000000000002000949000000000000000000000000100000000c000000fef2100005000000fff210004f0072006900670000002002000934000000000000000000000000020009490000000000000000000000001000000048000000fdf2100040000000fff2100001fef21000";

    #[test]
    fn reads_and_rebuilds_the_games_player_list() {
        let b = hex(SUPER);
        let m = parse_full(&b).unwrap();
        let (desc, name, all) = read_super_enum_reply(&m).unwrap();
        assert_eq!(name, "OrigRace");
        assert_eq!(all.len(), 4);
        assert_eq!(all[0].id, 0x0010_f2fc);
        assert!(all[0].is_system());
        assert_eq!(all[0].stream.unwrap().port, 2302);
        assert_eq!(all[0].dgram.unwrap().port, 2305);
        assert_eq!(all[1].flags, 0xf);
        assert_eq!(all[1].dgram.unwrap().port, 2377);
        assert_eq!(all[2].name.as_deref(), Some("Orig"));
        assert_eq!(all[2].system, 0x0010_f2ff);
        assert!(all[3].is_group());
        assert_eq!(all[3].members, vec![0x0010_f2fe]);
        let rebuilt = full(cmd::SUPERENUMPLAYERSREPLY, &super_enum_reply(&desc, &name, &all[..3], &all[3..]), 2356);
        assert_eq!(rebuilt, b);
    }

    /// The joining copy's CREATEPLAYER for its rider, as it sent it (inside
    /// a PACKET).
    const CREATE: &str = "c000b0fa020008fe000000000000000000000000706c617915000b0097a59638a0d50c4cb5914fcb07679ba4000000007c00000000000000010000007c00000000000000706c617908000b0000000000fbf21000000000001c000000000000005a00000008000000fbf210000a00000000000000200000000000000000000000fcf21000300000000b000000000000004f007200690067000000020008fe00000000000000000000000002000901000000000000000000000000000000000000";

    #[test]
    fn reads_a_packet_with_a_create_player_in_it() {
        let b = hex(CREATE);
        let m = parse_full(&b).unwrap();
        assert_eq!(m.command, cmd::PACKET);
        let piece = read_piece(m.body).unwrap();
        assert_eq!(piece.total, 1);
        let inner = parse_bare(piece.data).unwrap();
        assert_eq!(inner.command, cmd::CREATEPLAYER);
        let pm = read_player_msg(&inner).unwrap();
        let p = pm.player.unwrap();
        assert_eq!(p.name.as_deref(), Some("Orig"));
        assert_eq!(p.id, 0x0010_f2fb);
        assert_eq!(p.system, 0x0010_f2fc);
        assert_eq!(p.stream.unwrap(), Addr::new(Ipv4Addr::UNSPECIFIED, 2302));
        assert_eq!(bare(cmd::CREATEPLAYER, &create_player(0, &p)), piece.data);
    }
}
