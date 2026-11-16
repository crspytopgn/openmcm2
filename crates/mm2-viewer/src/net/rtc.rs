//! Network play with browsers and without port forwarding: WebRTC data
//! channels, introduced by the free public PeerJS server the browser build
//! also uses (`web/net.js`), so a desktop and a browser player can race
//! together. A host is reached by its race code; its STUN and TURN servers
//! help through home routers. The game's messages go whole, one per data
//! channel message, as in the browser (PeerJS's "raw" serialization).

use super::{In, Link, Msg};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

/// The start of a host's id on the matchmaking server, then its code.
const PREFIX: &str = "openmcm2-";
/// No 0/O or 1/I/L, so a code read out loud comes through.
const LETTERS: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
/// The PeerJS version the browser loads, which the server is told.
const PEERJS: &str = "1.5.4";

/// Whether `s` is a race code (six of [`LETTERS`], any case).
pub fn is_code(s: &str) -> bool {
    s.len() == 6 && s.bytes().all(|b| LETTERS.contains(&b.to_ascii_uppercase()))
}

/// The matchmaking server: the public one, or `MM2_PEER_SERVER`'s (the
/// tests', such as `ws://127.0.0.1:9000`).
fn server() -> String {
    std::env::var("MM2_PEER_SERVER").unwrap_or_else(|_| "wss://0.peerjs.com:443".into())
}

/// What the matchmaking server and the connections say, with
/// `MM2_NET_DEBUG` set.
fn debug(what: impl std::fmt::Display) {
    static ON: OnceLock<bool> = OnceLock::new();
    if *ON.get_or_init(|| std::env::var_os("MM2_NET_DEBUG").is_some()) {
        eprintln!("network: {what}");
    }
}

fn random() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.finish()
}

fn new_code() -> String {
    let mut r = random();
    (0..6)
        .map(|_| {
            let c = LETTERS[(r % LETTERS.len() as u64) as usize] as char;
            r /= LETTERS.len() as u64;
            c
        })
        .collect()
}

/// The network's own threads, started the first time they're needed.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("mm2-rtc")
            .enable_all()
            .build()
            .expect("the network's threads")
    })
}

/// The browser's STUN and TURN servers (PeerJS's own).
fn config() -> RTCConfiguration {
    RTCConfiguration {
        ice_servers: vec![
            RTCIceServer { urls: vec!["stun:stun.l.google.com:19302".into()], ..Default::default() },
            RTCIceServer {
                urls: vec!["turn:eu-0.turn.peerjs.com:3478".into(), "turn:us-0.turn.peerjs.com:3478".into()],
                username: "peerjs".into(),
                credential: "peerjsp".into(),
            },
        ],
        ..Default::default()
    }
}

async fn new_peer() -> Result<Arc<RTCPeerConnection>, webrtc::Error> {
    Ok(Arc::new(APIBuilder::new().build().new_peer_connection(config()).await?))
}

/// A connection to the matchmaking server as `id`: what to send it, and
/// what it says (until it goes). It's told we're here every 5 seconds.
async fn signal(id: &str) -> Result<(UnboundedSender<Value>, UnboundedReceiver<Value>), String> {
    let url = format!("{}/peerjs?key=peerjs&id={id}&token={:x}&version={PEERJS}", server(), random());
    let (ws, _) = tokio::time::timeout(Duration::from_secs(10), tokio_tungstenite::connect_async(url))
        .await
        .map_err(|_| "the matchmaking server didn't answer".to_string())?
        .map_err(|e| e.to_string())?;
    let (mut write, mut read) = ws.split();
    let (out, mut to_send) = unbounded_channel::<Value>();
    let (heard, rx) = unbounded_channel();
    tokio::spawn(async move {
        let mut beat = tokio::time::interval(Duration::from_secs(5));
        loop {
            let m = tokio::select! {
                m = to_send.recv() => match m {
                    Some(m) => m,
                    None => break,
                },
                _ = beat.tick() => json!({ "type": "HEARTBEAT" }),
            };
            if write.send(Message::text(m.to_string())).await.is_err() {
                break;
            }
        }
        let _ = write.close().await;
    });
    tokio::spawn(async move {
        while let Some(Ok(m)) = read.next().await {
            if let Message::Text(t) = m {
                debug(format_args!("heard {}", &t[..t.len().min(160)]));
                if let Ok(v) = serde_json::from_str::<Value>(&t) {
                    if heard.send(v).is_err() {
                        break;
                    }
                }
            }
        }
    });
    Ok((out, rx))
}

/// Sends a connection's ICE candidates to the other side through the
/// matchmaking server, once the offer or answer has gone ahead of them.
#[derive(Clone)]
struct Trickle {
    out: UnboundedSender<Value>,
    dst: String,
    connection: String,
    /// Those found before the offer or answer went.
    held: Arc<Mutex<Option<Vec<Value>>>>,
}

impl Trickle {
    fn new(out: UnboundedSender<Value>, dst: &str, connection: &str) -> Trickle {
        Trickle { out, dst: dst.into(), connection: connection.into(), held: Arc::new(Mutex::new(Some(Vec::new()))) }
    }

    fn found(&self, c: RTCIceCandidate) {
        let Ok(c) = c.to_json() else { return };
        let m = json!({
            "type": "CANDIDATE",
            "dst": self.dst,
            "payload": {
                "type": "data",
                "connectionId": self.connection,
                "candidate": { "candidate": c.candidate, "sdpMid": c.sdp_mid, "sdpMLineIndex": c.sdp_mline_index },
            },
        });
        match self.held.lock().unwrap().as_mut() {
            Some(held) => held.push(m),
            None => {
                let _ = self.out.send(m);
            }
        }
    }

    /// Sends the offer or answer, then the candidates held for it.
    fn go(&self, m: Value) {
        let _ = self.out.send(m);
        for c in self.held.lock().unwrap().take().unwrap_or_default() {
            let _ = self.out.send(c);
        }
    }

    fn watch(&self, pc: &RTCPeerConnection) {
        let t = self.clone();
        pc.on_ice_candidate(Box::new(move |c| {
            if let Some(c) = c {
                t.found(c);
            }
            Box::pin(async {})
        }));
    }
}

/// A candidate the other side found, as PeerJS sends it.
async fn add_candidate(pc: &RTCPeerConnection, payload: &Value) {
    let c = &payload["candidate"];
    let Some(candidate) = c["candidate"].as_str().filter(|s| !s.is_empty()) else { return };
    let init = RTCIceCandidateInit {
        candidate: candidate.into(),
        sdp_mid: c["sdpMid"].as_str().map(str::to_string),
        sdp_mline_index: c["sdpMLineIndex"].as_u64().map(|n| n as u16),
        username_fragment: c["usernameFragment"].as_str().map(str::to_string),
    };
    if let Err(e) = pc.add_ice_candidate(init).await {
        eprintln!("network: a candidate: {e}");
    }
}

/// Our end of a data channel, for the session: what's sent waits until
/// it's open. Dropping it closes the connection.
pub struct Channel {
    out: UnboundedSender<Vec<u8>>,
    closed: Arc<AtomicBool>,
}

impl Channel {
    pub fn send(&mut self, body: Vec<u8>) -> io::Result<()> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.out.send(body).map_err(|_| io::ErrorKind::BrokenPipe.into())
    }
}

/// A channel and the queue its writer sends from.
fn channel() -> (Channel, UnboundedReceiver<Vec<u8>>, Arc<AtomicBool>) {
    let (out, rx) = unbounded_channel();
    let closed = Arc::new(AtomicBool::new(false));
    (Channel { out, closed: closed.clone() }, rx, closed)
}

/// Sends the queue's messages once the channel is open, until the
/// session lets go of it; then closes the connection.
async fn write(mut rx: UnboundedReceiver<Vec<u8>>, dc: Arc<RTCDataChannel>, pc: Arc<RTCPeerConnection>, open: Arc<Notify>) {
    open.notified().await;
    while let Some(m) = rx.recv().await {
        if dc.send(&m.into()).await.is_err() {
            break;
        }
    }
    let _ = pc.close().await;
}

/// Tells the session once that a connection has gone.
fn on_gone(tx: Sender<In>, id: u8) -> impl Fn() + Send + Sync + Clone {
    let once = Arc::new(AtomicBool::new(false));
    move || {
        if !once.swap(true, Ordering::Relaxed) {
            let _ = tx.send(In::Gone(id));
        }
    }
}

/// Hands a channel's messages to the session as player `id`'s, and tells
/// it when the connection goes.
fn hear(dc: &RTCDataChannel, pc: &RTCPeerConnection, id: u8, tx: &Sender<In>, closed: Arc<AtomicBool>) {
    let gone = on_gone(tx.clone(), id);
    {
        let tx = tx.clone();
        dc.on_message(Box::new(move |m: DataChannelMessage| {
            if let Ok(m) = Msg::decode(&m.data) {
                let _ = tx.send(In::Got(id, m));
            }
            Box::pin(async {})
        }));
    }
    {
        let (gone, closed) = (gone.clone(), closed.clone());
        dc.on_close(Box::new(move || {
            closed.store(true, Ordering::Relaxed);
            gone();
            Box::pin(async {})
        }));
    }
    pc.on_peer_connection_state_change(Box::new(move |s| {
        debug(format_args!("player {id}'s connection: {s}"));
        if matches!(s, RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed) {
            closed.store(true, Ordering::Relaxed);
            gone();
        }
        Box::pin(async {})
    }));
}

/// What a host shows: its race code once the matchmaking server has it.
#[derive(Default)]
pub struct Hosting {
    pub code: Mutex<String>,
}

/// Hosts `name` on the matchmaking server under a new race code (another
/// if it's taken), answering browsers' and desktops' offers: each data
/// channel that opens is a player numbered from `next`. Finders' probes get
/// the name. Until `stop`; a lost server is tried again.
pub fn host(name: String, next: Arc<AtomicU8>, tx: Sender<In>, stop: Arc<AtomicBool>) -> Arc<Hosting> {
    let hosting = Arc::new(Hosting::default());
    let shown = Arc::downgrade(&hosting);
    runtime().spawn(async move {
        let mut code = new_code();
        while !stop.load(Ordering::Relaxed) && shown.strong_count() > 0 {
            let id = format!("{PREFIX}{}", code.to_lowercase());
            match signal(&id).await {
                Ok((out, inc)) => {
                    if !serve(&code, &name, out, inc, &next, &tx, &stop, &shown).await {
                        code = new_code();
                        continue;
                    }
                }
                Err(e) => eprintln!("network: {e}"),
            }
            if let Some(h) = shown.upgrade() {
                h.code.lock().unwrap().clear();
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
    hosting
}

/// One connection to the matchmaking server, as host: false if the code
/// was taken.
#[allow(clippy::too_many_arguments)]
async fn serve(
    code: &str,
    name: &str,
    out: UnboundedSender<Value>,
    mut inc: UnboundedReceiver<Value>,
    next: &AtomicU8,
    tx: &Sender<In>,
    stop: &AtomicBool,
    shown: &Weak<Hosting>,
) -> bool {
    // The connections being made and in use, until they close.
    let mut pcs: HashMap<String, Arc<RTCPeerConnection>> = HashMap::new();
    let mut look = tokio::time::interval(Duration::from_millis(500));
    loop {
        let m = tokio::select! {
            m = inc.recv() => match m {
                Some(m) => m,
                None => return true,
            },
            _ = look.tick() => {
                if stop.load(Ordering::Relaxed) || shown.strong_count() == 0 {
                    return true;
                }
                pcs.retain(|_, pc| !matches!(pc.connection_state(), RTCPeerConnectionState::Closed | RTCPeerConnectionState::Failed));
                continue;
            }
        };
        let src = m["src"].as_str().unwrap_or_default().to_string();
        let payload = &m["payload"];
        let connection = payload["connectionId"].as_str().unwrap_or_default().to_string();
        match m["type"].as_str().unwrap_or_default() {
            "OPEN" => {
                if let Some(h) = shown.upgrade() {
                    *h.code.lock().unwrap() = code.to_string();
                }
            }
            "ID-TAKEN" => return false,
            "ERROR" => {
                eprintln!("network: the matchmaking server: {}", payload["msg"]);
                return true;
            }
            "OFFER" => {
                let probe = payload["metadata"]["probe"].as_bool() == Some(true);
                let id = if probe { 0 } else { next.fetch_add(1, Ordering::Relaxed) };
                match answer(&src, &connection, payload, probe, id, name, &out, tx).await {
                    Ok(pc) => {
                        pcs.insert(connection, pc);
                    }
                    Err(e) => eprintln!("network: answering a player: {e}"),
                }
            }
            "CANDIDATE" => {
                if let Some(pc) = pcs.get(&connection) {
                    add_candidate(&pc, payload).await;
                }
            }
            _ => {}
        }
    }
}

/// Answers an offer. The connection lives while its channel does (or for
/// a probe, until it's had the name).
#[allow(clippy::too_many_arguments)]
async fn answer(
    src: &str,
    connection: &str,
    payload: &Value,
    probe: bool,
    id: u8,
    name: &str,
    out: &UnboundedSender<Value>,
    tx: &Sender<In>,
) -> Result<Arc<RTCPeerConnection>, webrtc::Error> {
    let pc = new_peer().await?;
    let trickle = Trickle::new(out.clone(), src, connection);
    trickle.watch(&pc);
    let (weak, tx, name) = (Arc::downgrade(&pc), tx.clone(), name.to_string());
    pc.on_data_channel(Box::new(move |dc: Arc<RTCDataChannel>| {
        let Some(pc) = weak.upgrade() else { return Box::pin(async {}) };
        if probe {
            // A finder asking who we are: our session's name, then goodbye.
            let d = dc.clone();
            let name = name.clone();
            dc.on_open(Box::new(move || {
                Box::pin(async move {
                    let _ = d.send(&name.into_bytes().into()).await;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let _ = pc.close().await;
                })
            }));
            return Box::pin(async {});
        }
        let (ch, rx, closed) = channel();
        let open = Arc::new(Notify::new());
        hear(&dc, &pc, id, &tx, closed);
        let (tx, o) = (tx.clone(), open.clone());
        let ch = Mutex::new(Some(ch));
        debug(format_args!("player {id}'s channel came"));
        dc.on_open(Box::new(move || {
            debug(format_args!("player {id}'s channel is open"));
            if let Some(ch) = ch.lock().unwrap().take() {
                let _ = tx.send(In::Joined(id, Link::Rtc(ch)));
            }
            o.notify_one();
            Box::pin(async {})
        }));
        tokio::spawn(write(rx, dc, pc, open));
        Box::pin(async {})
    }));
    let offer = payload["sdp"]["sdp"].as_str().unwrap_or_default().to_string();
    pc.set_remote_description(RTCSessionDescription::offer(offer)?).await?;
    let a = pc.create_answer(None).await?;
    pc.set_local_description(a.clone()).await?;
    trickle.go(json!({
        "type": "ANSWER",
        "dst": src,
        "payload": { "type": "data", "connectionId": connection, "browser": "openmcm2", "sdp": { "type": "answer", "sdp": a.sdp } },
    }));
    // One that never opens is given up.
    let keep = Arc::downgrade(&pc);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        if let Some(pc) = keep.upgrade() {
            if probe || pc.connection_state() != RTCPeerConnectionState::Connected {
                let _ = pc.close().await;
            }
        }
    });
    Ok(pc)
}

/// Connects to the host with race `code`. The channel is the host (player
/// 0) for the session; what's sent waits until it opens, and the session
/// hears `Gone(0)` if it never does.
pub fn join(code: &str, tx: Sender<In>) -> Channel {
    let (ch, rx, closed) = channel();
    let host = format!("{PREFIX}{}", code.trim().to_lowercase());
    runtime().spawn(async move {
        let gone = on_gone(tx.clone(), 0);
        if let Err(e) = connect(&host, rx, closed.clone(), &tx).await {
            eprintln!("network: joining {host}: {e}");
            closed.store(true, Ordering::Relaxed);
            gone();
        }
    });
    ch
}

async fn connect(host: &str, rx: UnboundedReceiver<Vec<u8>>, closed: Arc<AtomicBool>, tx: &Sender<In>) -> Result<(), String> {
    let me = format!("{PREFIX}p{:016x}", random());
    let (out, mut inc) = signal(&me).await?;
    // The server says hello first.
    loop {
        let m = tokio::time::timeout(Duration::from_secs(10), inc.recv())
            .await
            .map_err(|_| "the matchmaking server didn't say hello")?
            .ok_or("the matchmaking server went")?;
        match m["type"].as_str() {
            Some("OPEN") => break,
            Some("ID-TAKEN") | Some("ERROR") => return Err(format!("the matchmaking server said {m}")),
            _ => {}
        }
    }
    let pc = new_peer().await.map_err(|e| e.to_string())?;
    let connection = format!("dc_{:012x}", random() & 0xffff_ffff_ffff);
    let dc = pc.create_data_channel(&connection, None).await.map_err(|e| e.to_string())?;
    let open = Arc::new(Notify::new());
    let opened = Arc::new(AtomicBool::new(false));
    hear(&dc, &pc, 0, tx, closed.clone());
    {
        let (open, opened) = (open.clone(), opened.clone());
        dc.on_open(Box::new(move || {
            debug("the host's channel is open");
            opened.store(true, Ordering::Relaxed);
            open.notify_one();
            Box::pin(async {})
        }));
    }
    let trickle = Trickle::new(out.clone(), host, &connection);
    trickle.watch(&pc);
    let offer = pc.create_offer(None).await.map_err(|e| e.to_string())?;
    pc.set_local_description(offer.clone()).await.map_err(|e| e.to_string())?;
    trickle.go(json!({
        "type": "OFFER",
        "dst": host,
        "payload": {
            "type": "data",
            "connectionId": connection,
            "browser": "openmcm2",
            "label": connection,
            "reliable": true,
            "serialization": "raw",
            "metadata": null,
            "sdp": { "type": "offer", "sdp": offer.sdp },
        },
    }));
    tokio::spawn(write(rx, dc, pc.clone(), open));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(25);
    // Until the channel opens: the answer and the host's candidates.
    while !opened.load(Ordering::Relaxed) {
        if closed.load(Ordering::Relaxed) {
            return Ok(());
        }
        if tokio::time::Instant::now() > deadline {
            return Err("the host didn't answer".into());
        }
        let m = match tokio::time::timeout(Duration::from_millis(250), inc.recv()).await {
            Err(_) => continue,
            Ok(None) => return Err("the matchmaking server went".into()),
            Ok(Some(m)) => m,
        };
        let payload = &m["payload"];
        if payload["connectionId"].as_str().is_some_and(|c| c != connection) {
            continue;
        }
        match m["type"].as_str().unwrap_or_default() {
            "ANSWER" => {
                let sdp = payload["sdp"]["sdp"].as_str().unwrap_or_default().to_string();
                let d = RTCSessionDescription::answer(sdp).map_err(|e| e.to_string())?;
                pc.set_remote_description(d).await.map_err(|e| e.to_string())?;
            }
            "CANDIDATE" => add_candidate(&pc, payload).await,
            "EXPIRE" => return Err("no race with that code".into()),
            "ERROR" => return Err(format!("the matchmaking server said {m}")),
            _ => {}
        }
    }
    // Open: the matchmaking server isn't needed any more.
    Ok(())
}
