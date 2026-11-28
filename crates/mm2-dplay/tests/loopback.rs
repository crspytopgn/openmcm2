//! Two sessions on this machine: one hosts, one finds and joins it, both
//! make a player, and messages go both ways.

use mm2_dplay::{Event, Session};
use std::net::Ipv4Addr;
use std::time::Duration;

const APP: [u8; 16] = *b"OpenMCM2-loopbck";

fn wait_for(s: &Session, want: impl Fn(&Event) -> bool) -> Event {
    for _ in 0..50 {
        if let Some(e) = s.wait(Duration::from_millis(100)) {
            if want(&e) {
                return e;
            }
        }
    }
    panic!("nothing came");
}

#[test]
fn host_join_and_talk() {
    let host = Session::host(APP, "Loop race", 8, 0xa040, Ipv4Addr::UNSPECIFIED).unwrap();
    let hp = host.create_player("Hoster").unwrap();
    let found = Session::find(APP, Some(Ipv4Addr::LOCALHOST), Duration::from_millis(800)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Loop race");
    let guest = Session::join(&found[0]).unwrap();
    match wait_for(&guest, |e| matches!(e, Event::PlayerJoined { .. })) {
        Event::PlayerJoined { id, name } => assert_eq!((id, name.as_str()), (hp, "Hoster")),
        _ => unreachable!(),
    }
    let gp = guest.create_player("Guest").unwrap();
    match wait_for(&host, |e| matches!(e, Event::PlayerJoined { .. })) {
        Event::PlayerJoined { id, name } => assert_eq!((id, name.as_str()), (gp, "Guest")),
        _ => unreachable!(),
    }
    host.send(hp, 0, b"\x0fhello", false);
    assert_eq!(
        wait_for(&guest, |e| matches!(e, Event::Data { .. })),
        Event::Data { from: hp, to: 0, data: b"\x0fhello".to_vec() }
    );
    let big: Vec<u8> = (0..5000u32).map(|i| i as u8).collect();
    guest.send(gp, hp, &big, true);
    assert_eq!(wait_for(&host, |e| matches!(e, Event::Data { .. })), Event::Data { from: gp, to: hp, data: big });
    assert_eq!(host.players().len(), 2);
    guest.close();
    assert_eq!(wait_for(&host, |e| matches!(e, Event::PlayerLeft { .. })), Event::PlayerLeft { id: gp });
}
