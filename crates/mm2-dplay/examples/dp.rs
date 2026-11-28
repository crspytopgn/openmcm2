//! A DirectPlay test tool: `dp host NAME` hosts a Motocross Madness 2
//! session and shows what arrives; `dp find [IP]` lists sessions;
//! `dp join [IP]` joins the first one found.

use mm2_dplay::{Event, Session};
use std::net::Ipv4Addr;
use std::time::Duration;

/// The original game's DirectPlay application id.
const MM2: [u8; 16] = [0xc2, 0xbb, 0x4d, 0x18, 0x39, 0xff, 0xcf, 0x11, 0xb1, 0x12, 0x00, 0xa0, 0xc9, 0x0a, 0xea, 0xb6];

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn show(s: &Session, secs: u64) {
    let end = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < end {
        if let Some(e) = s.wait(Duration::from_millis(200)) {
            match e {
                Event::Data { from, to, data } => println!("data {from:08x}->{to:08x} type {:02x} len {} {}", data[0], data.len(), hex(&data[..data.len().min(48)])),
                e => println!("{e:?}"),
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ip = |i: usize| args.get(i).and_then(|a| a.parse::<Ipv4Addr>().ok());
    match args.get(1).map(String::as_str) {
        Some("host") => {
            let s = Session::host(MM2, args.get(2).map(String::as_str).unwrap_or("OpenMCM2"), 8, 0xa044, Ipv4Addr::UNSPECIFIED).unwrap();
            let me = s.create_player("OpenHost").unwrap();
            println!("hosting, ports {:?}, player {me:08x}", s.ports());
            show(&s, 600);
        }
        Some("find") => {
            for f in Session::find(MM2, ip(2), Duration::from_secs(2)).unwrap() {
                println!("{} at {} flags {:x} players {}/{}", f.name, f.host, f.desc.flags, f.desc.current_players, f.desc.max_players);
            }
        }
        Some("join") => {
            let found = Session::find(MM2, ip(2), Duration::from_secs(2)).unwrap();
            let f = found.first().expect("no session");
            let s = Session::join(f).unwrap();
            let me = s.create_player("OpenJoin").unwrap();
            println!("joined {} as {me:08x}: {:?}", f.name, s.players());
            show(&s, 600);
        }
        _ => eprintln!("dp host NAME | dp find [IP] | dp join [IP]"),
    }
}
