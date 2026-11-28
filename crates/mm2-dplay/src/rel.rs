//! The DirectPlay 4 reliable protocol ([MC-DPL4R]): the envelope every
//! message between two machines travels in over UDP once a game is joined,
//! when the session asks for it (the original game always does on
//! Windows).
//!
//! A frame names the sending and receiving players by index (the id
//! without the session's mask), then a flags byte, the message's id, a
//! sequence number and a serial number, then the data. Big messages span
//! several frames, from the one marked start to the one marked end. The
//! receiver acknowledges a frame that asks for it and every message's last
//! frame; reliable messages are sent again until acknowledged.
//!
//! As DirectX 7's DirectPlay does it: each message's frames count their
//! sequence from 1, and the message ids go up by one per message,
//! wrapping from 255 to 0.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Frame flags (section 2.2.2).
pub const EXT: u8 = 0x80;
pub const CMD: u8 = 0x20;
pub const STA: u8 = 0x10;
pub const EOM: u8 = 0x08;
pub const SAK: u8 = 0x04;
pub const ACK: u8 = 0x02;
pub const RLY: u8 = 0x01;

/// The most data one frame carries, keeping a frame inside one Ethernet
/// packet.
pub const FRAME_DATA: usize = 1200;
/// How long before an unacknowledged reliable message goes again.
const RETRY: Duration = Duration::from_millis(400);
/// How many times a reliable message goes before the link gives up on it.
const TRIES: u32 = 30;

/// Writes a player index: 7 bits a byte, low first, the top bit saying
/// another byte follows (section 2.2.1).
pub fn write_index(out: &mut Vec<u8>, index: u16) {
    let v = index as u32;
    if v < 0x80 {
        out.push(v as u8);
    } else if v < 0x4000 {
        out.push((v & 0x7f) as u8 | 0x80);
        out.push((v >> 7) as u8);
    } else {
        out.push((v & 0x7f) as u8 | 0x80);
        out.push(((v >> 7) & 0x7f) as u8 | 0x80);
        out.push((v >> 14) as u8 & 0x03);
    }
}

/// Reads a player index; the bytes it took.
pub fn read_index(b: &[u8]) -> Option<(u16, usize)> {
    let mut v = 0u32;
    for (i, &byte) in b.iter().enumerate().take(3) {
        v |= ((byte & 0x7f) as u32) << (7 * i);
        if byte & 0x80 == 0 || i == 2 {
            return Some((v as u16, i + 1));
        }
    }
    None
}

/// A frame taken apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<'a> {
    pub from: u16,
    pub to: u16,
    pub flags: u8,
    pub message: u8,
    pub sequence: u8,
    pub serial: u8,
    /// A data frame's data; empty for an ACK or NACK.
    pub data: &'a [u8],
}

impl Frame<'_> {
    pub fn is_ack(&self) -> bool {
        self.flags & ACK != 0
    }
}

/// Reads a frame. `None` for something that is not one.
pub fn parse(b: &[u8]) -> Option<Frame<'_>> {
    let (from, a) = read_index(b)?;
    let (to, c) = read_index(&b[a..])?;
    let mut at = a + c;
    let flags = *b.get(at)?;
    at += 1;
    if flags & ACK != 0 {
        // An ACK: id, sequence, serial (a NACK puts its extended flags
        // first, and has no serial).
        let mut nnack = 0;
        if flags & EXT != 0 {
            nnack = ((*b.get(at)? >> 1) & 3) as usize;
            at += 1;
        }
        let message = *b.get(at)?;
        let sequence = *b.get(at + 1)?;
        let serial = if flags & EXT != 0 { 0 } else { *b.get(at + 2)? };
        let _ = nnack; // the mask itself isn't needed: lost frames are resent on time
        return Some(Frame { from, to, flags, message, sequence, serial, data: &[] });
    }
    if flags & CMD == 0 {
        return None;
    }
    let message = *b.get(at)?;
    let sequence = *b.get(at + 1)?;
    let serial = *b.get(at + 2)?;
    at += 3;
    if flags & EXT != 0 {
        let ext = *b.get(at)?;
        at += 1 + ((ext >> 1) & 3) as usize;
    }
    Some(Frame { from, to, flags, message, sequence, serial, data: b.get(at..)? })
}

fn header(from: u16, to: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_DATA + 12);
    write_index(&mut out, from);
    write_index(&mut out, to);
    out
}

/// A data frame.
pub fn data_frame(from: u16, to: u16, flags: u8, message: u8, sequence: u8, serial: u8, data: &[u8]) -> Vec<u8> {
    let mut out = header(from, to);
    out.extend_from_slice(&[flags | CMD, message, sequence, serial]);
    out.extend_from_slice(data);
    out
}

/// An ACK frame, from the receiver back to the sender.
pub fn ack_frame(from: u16, to: u16, message: u8, sequence: u8, serial: u8, bytes: u32, tick: u32) -> Vec<u8> {
    let mut out = header(from, to);
    out.extend_from_slice(&[ACK, message, sequence, serial]);
    out.extend_from_slice(&bytes.to_le_bytes());
    out.extend_from_slice(&tick.to_le_bytes());
    out
}

struct Building {
    reliable: bool,
    start: Option<u8>,
    end: Option<u8>,
    parts: HashMap<u8, Vec<u8>>,
    at: Instant,
}

struct Unacked {
    message: u8,
    frames: Vec<Vec<u8>>,
    sent: Instant,
    tries: u32,
}

/// One machine's link to another: what has been received and what is
/// waiting for an ACK.
pub struct Link {
    next_message: u8,
    bytes_received: u32,
    building: HashMap<u8, Building>,
    /// Reliable messages already handed on, so a resend after a lost ACK
    /// isn't handed on twice.
    seen: VecDeque<u8>,
    unacked: Vec<Unacked>,
}

/// What a received frame gives: frames to send back, and a whole message
/// if this frame finished one.
#[derive(Default, Debug)]
pub struct Got {
    pub reply: Vec<Vec<u8>>,
    pub message: Option<Vec<u8>>,
}

impl Default for Link {
    fn default() -> Self {
        Link { next_message: 1, bytes_received: 0, building: HashMap::new(), seen: VecDeque::new(), unacked: Vec::new() }
    }
}

impl Link {
    /// The frames for one message from player index `from` to `to`.
    pub fn send(&mut self, from: u16, to: u16, data: &[u8], reliable: bool) -> Vec<Vec<u8>> {
        let message = self.next_message;
        self.next_message = self.next_message.wrapping_add(1);
        let chunks: Vec<&[u8]> = if data.is_empty() { vec![&[][..]] } else { data.chunks(FRAME_DATA).collect() };
        let n = chunks.len();
        let frames: Vec<Vec<u8>> = chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut flags = SAK;
                if i == 0 {
                    flags |= STA;
                }
                if i + 1 == n {
                    flags |= EOM;
                }
                if reliable {
                    flags |= RLY;
                }
                data_frame(from, to, flags, message, (i + 1) as u8, i as u8, c)
            })
            .collect();
        if reliable {
            self.unacked.push(Unacked { message, frames: frames.clone(), sent: Instant::now(), tries: 1 });
        }
        frames
    }

    /// Reliable messages due to go again, and whether any has run out of
    /// tries (the other machine has gone).
    pub fn resend(&mut self, now: Instant) -> (Vec<Vec<u8>>, bool) {
        let mut out = Vec::new();
        let mut dead = false;
        for u in &mut self.unacked {
            if now.duration_since(u.sent) >= RETRY {
                u.tries += 1;
                u.sent = now;
                dead |= u.tries > TRIES;
                for f in &u.frames {
                    // The serial counts the sends of the frame.
                    let mut f = f.clone();
                    if let Some(fr) = parse(&f) {
                        let at = f.len() - fr.data.len() - 1;
                        f[at] = f[at].wrapping_add(1);
                    }
                    out.push(f);
                }
            }
        }
        (out, dead)
    }

    /// Takes in one frame from the other machine. `tick` is this machine's
    /// millisecond count, for the ACK.
    pub fn receive(&mut self, raw: &[u8], tick: u32) -> Got {
        let mut got = Got::default();
        let Some(f) = parse(raw) else { return got };
        if f.is_ack() {
            // An ACK covers its message and any before it.
            let m = f.message;
            self.unacked.retain(|u| m.wrapping_sub(u.message) >= 128);
            return got;
        }
        let indexes = read_index(raw).map_or(0, |(_, a)| a + read_index(&raw[a..]).map_or(0, |(_, b)| b));
        self.bytes_received = self.bytes_received.wrapping_add((raw.len() - indexes) as u32);
        let reliable = f.flags & RLY != 0;
        if reliable && self.seen.contains(&f.message) {
            got.reply.push(ack_frame(f.to, f.from, f.message, f.sequence, f.serial, self.bytes_received, tick));
            return got;
        }
        let b = self.building.entry(f.message).or_insert_with(|| Building {
            reliable,
            start: None,
            end: None,
            parts: HashMap::new(),
            at: Instant::now(),
        });
        if f.flags & STA != 0 {
            b.start = Some(f.sequence);
        }
        if f.flags & EOM != 0 {
            b.end = Some(f.sequence);
        }
        b.parts.insert(f.sequence, f.data.to_vec());
        let mut done = None;
        if let (Some(s), Some(e)) = (b.start, b.end) {
            let n = e.wrapping_sub(s) as usize + 1;
            if (0..n).all(|i| b.parts.contains_key(&s.wrapping_add(i as u8))) {
                let mut whole = Vec::new();
                for i in 0..n {
                    whole.extend_from_slice(&b.parts[&s.wrapping_add(i as u8)]);
                }
                done = Some(whole);
            }
        }
        if f.flags & SAK != 0 || done.is_some() {
            got.reply.push(ack_frame(f.to, f.from, f.message, f.sequence, f.serial, self.bytes_received, tick));
        }
        if let Some(whole) = done {
            let reliable = self.building.remove(&f.message).map(|b| b.reliable).unwrap_or(reliable);
            if reliable {
                self.seen.push_back(f.message);
                if self.seen.len() > 64 {
                    self.seen.pop_front();
                }
            }
            got.message = Some(whole);
        }
        // Half-built messages that will never finish.
        let now = Instant::now();
        self.building.retain(|_, b| now.duration_since(b.at) < Duration::from_secs(15));
        got
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_round_trip() {
        for v in [0u16, 1, 0x7f, 0x80, 0x3fff, 0x4000, 0xfffe] {
            let mut b = Vec::new();
            write_index(&mut b, v);
            assert_eq!(read_index(&b), Some((v, b.len())));
        }
        let mut b = Vec::new();
        write_index(&mut b, 0xfffe);
        assert_eq!(b, [0xfe, 0xff, 0x03]);
    }

    #[test]
    fn reads_the_games_frames() {
        // The host's lobby settings to the joining copy, and its ACK.
        let data = [0x01, 0x03, 0x3c, 0x01, 0x01, 0x00, 0xfe, 0xf2, 0x10, 0x00];
        let f = parse(&data).unwrap();
        assert_eq!((f.from, f.to, f.flags, f.message, f.sequence, f.serial), (1, 3, 0x3c, 1, 1, 0));
        assert_eq!(f.data, &[0xfe, 0xf2, 0x10, 0x00]);
        let ack = [0x03, 0x01, 0x02, 0x01, 0x01, 0x00, 0xc0, 0, 0, 0, 0x90, 0x3f, 0x13, 0x00];
        let a = parse(&ack).unwrap();
        assert!(a.is_ack());
        assert_eq!((a.from, a.to, a.message, a.sequence), (3, 1, 1, 1));
        assert_eq!(ack_frame(3, 1, 1, 1, 0, 0xc0, 0x133f90), ack);
    }

    #[test]
    fn big_reliable_messages_arrive_whole_and_once() {
        let mut a = Link::default();
        let mut b = Link::default();
        let msg: Vec<u8> = (0..3000u32).map(|i| i as u8).collect();
        let frames = a.send(1, 3, &msg, true);
        assert_eq!(frames.len(), 3);
        let mut whole = None;
        for f in frames.iter().rev() {
            let got = b.receive(f, 0);
            for r in &got.reply {
                a.receive(r, 0);
            }
            whole = whole.or(got.message);
        }
        assert_eq!(whole.unwrap(), msg);
        assert!(a.unacked.is_empty());
        // A resend after a lost ACK is acknowledged but not handed on again.
        let again = b.receive(&frames[2], 0);
        assert!(again.message.is_none());
        assert_eq!(again.reply.len(), 1);
    }
}
