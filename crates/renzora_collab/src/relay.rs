//! The relay transport: a session through renzora.com instead of a direct port.
//!
//! Direct TCP needs the host to be reachable, which on two ordinary home
//! connections nobody is — both ends are behind NAT and neither can accept an
//! inbound connection. Here both editors connect *outward* to renzora.com over
//! WebSocket, which every network already allows, and the server forwards bytes
//! between them. It is the difference between "collaborate with someone on your
//! LAN" and "collaborate with a friend".
//!
//! ## What is different, and what deliberately is not
//!
//! Everything above [`crate::link`] is unchanged. A relayed guest holds exactly
//! the same [`Link`] a direct guest holds, and the session cannot tell them
//! apart. That was the point of putting `Link` between the session and the
//! socket in the first place.
//!
//! The host is where the shapes differ. Direct hosting gives one socket per
//! guest, so a guest arriving *is* a socket arriving. Relayed hosting has a
//! single socket carrying everyone, so:
//!
//! - Each binary message is wrapped as `[peer: u32 LE][frame]` — see
//!   [`envelope`] — and this module multiplexes it back into per-peer links.
//! - Guests arriving and leaving are announced by the *server*, as JSON text
//!   frames, because there is no per-guest socket whose open and close could say
//!   it instead.
//!
//! ## A known inefficiency, left in on purpose
//!
//! The server understands a broadcast target, and this client never uses it: a
//! message for every guest is sent once per guest link, so a host with two
//! guests uploads a scene snapshot twice. Using the broadcast target would
//! upload it once, which on a home connection's upstream is the scarce
//! direction. It is not done yet because it means the session sending *past*
//! the per-peer links rather than through them, and getting the plain path
//! correct came first. Worth doing when sessions routinely have more than one
//! guest.
//!
//! ## Why a thread per guest on the outbound side
//!
//! Each `Link` owns a channel of messages waiting to go out, and the multiplexer
//! has to take from all of them and write to one socket. Rather than teach
//! `Link` about tagging — which would push relay concerns into the type the
//! whole point was to keep transport-agnostic — each guest link gets a small
//! forwarder thread that stamps its peer id on and pushes into the shared write
//! queue. Sessions have a handful of participants, not thousands, so a thread
//! each is the cheap option as well as the tidy one.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{unbounded, Sender};

use crate::link::{plumbing, Acceptor, Link, LinkEvent, LinkPlumbing};
use crate::protocol::CollabMsg;

/// The peer id the host answers to. Mirrors the server's constant of the same
/// name — the two are one wire format and must move together.
const HOST_PEER: u32 = 0;

/// How long the socket read parks before the worker re-checks for shutdown.
///
/// A timeout is safe here where it is not on the raw TCP reader: a WebSocket
/// library hands over whole messages, so a read that times out has either
/// produced a message or produced nothing. There is no half-consumed frame to
/// lose, which is exactly what made the same trick unsafe over `read_exact`.
const READ_TICK: Duration = Duration::from_millis(250);

// ── Envelope ────────────────────────────────────────────────────────────────

/// Wrap a frame for the relay.
fn envelope(peer: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 4);
    out.extend_from_slice(&peer.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

/// Split a relayed message into `(peer, payload)`.
fn unwrap_envelope(data: &[u8]) -> Option<(u32, &[u8])> {
    if data.len() < 4 {
        return None;
    }
    let peer = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    Some((peer, &data[4..]))
}

/// Encode a message the way a link would put it on a socket, so the relay
/// carries byte-identical frames to the direct transport.
fn encode(msg: &CollabMsg) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    crate::protocol::write_frame(&mut buffer, msg).ok()?;
    Some(buffer)
}

// ── Guest ───────────────────────────────────────────────────────────────────

/// Join a relayed session. The returned link behaves exactly like a direct one.
pub fn join(url: String, token: String) -> Link {
    let (mut link, plumb) = plumbing(url.clone());
    let LinkPlumbing { out_rx, inbox_tx, shutdown } = plumb;

    let stop = shutdown.clone();
    link.on_hangup(move || stop.store(true, Ordering::Relaxed));

    let spawned = std::thread::Builder::new()
        .name("collab-relay-guest".into())
        .spawn(move || {
            let mut socket = match open(&url, &token) {
                Ok(socket) => socket,
                Err(e) => {
                    let _ = inbox_tx.send(LinkEvent::Closed(e));
                    return;
                }
            };
            let _ = inbox_tx.send(LinkEvent::Connected);

            // A guest's frames all go to the host, so the peer field is a
            // constant. The server ignores it and stamps the guest's real id on
            // before handing it over — a guest cannot address anyone else, by
            // construction rather than by rule.
            let mut buffer = Vec::new();
            loop {
                if shutdown.load(Ordering::Relaxed) {
                    let _ = socket.close(None);
                    return;
                }
                while let Ok(msg) = out_rx.try_recv() {
                    let Some(bytes) = encode(&msg) else { continue };
                    buffer.clear();
                    buffer.extend_from_slice(&envelope(HOST_PEER, &bytes));
                    if socket.send(tungstenite::Message::Binary(buffer.clone())).is_err() {
                        let _ = inbox_tx.send(LinkEvent::Closed("relay send failed".into()));
                        return;
                    }
                }
                match read(&mut socket) {
                    Ok(Some(Frame::Data(data))) => {
                        let Some((_, payload)) = unwrap_envelope(&data) else { continue };
                        for msg in decode_all(payload) {
                            if inbox_tx.send(LinkEvent::Message(msg)).is_err() {
                                return;
                            }
                        }
                    }
                    Ok(Some(Frame::Control(json))) => {
                        if json.get("event").and_then(|e| e.as_str()) == Some("host_gone") {
                            let reason = json
                                .get("reason")
                                .and_then(|r| r.as_str())
                                .unwrap_or("the session ended");
                            let _ = inbox_tx.send(LinkEvent::Closed(reason.to_string()));
                            return;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let _ = inbox_tx.send(LinkEvent::Closed(e));
                        return;
                    }
                }
            }
        });

    if let Err(e) = spawned {
        log::error!("[collab] could not spawn relay thread: {e}");
    }
    link
}

// ── Host ────────────────────────────────────────────────────────────────────

/// Host a relayed session. Guests arriving on the single socket surface as
/// links on the returned acceptor, exactly as accepted sockets do.
pub fn host(url: String, token: String, code: String) -> Acceptor {
    let (incoming_tx, incoming) = unbounded();
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();

    let spawned = std::thread::Builder::new()
        .name("collab-relay-host".into())
        .spawn(move || host_loop(url, token, incoming_tx, flag));
    if let Err(e) = spawned {
        log::error!("[collab] could not spawn relay host thread: {e}");
    }

    let stop = shutdown.clone();
    Acceptor::new(incoming, format!("session {code}"), move || {
        stop.store(true, Ordering::Relaxed)
    })
}

/// One guest, from the host's side of the relay.
struct Guest {
    /// Where this guest's inbound frames are delivered.
    inbox: Sender<LinkEvent>,
    /// Set when the guest leaves, so its forwarder thread retires.
    gone: Arc<AtomicBool>,
}

fn host_loop(url: String, token: String, incoming: Sender<Link>, shutdown: Arc<AtomicBool>) {
    let mut socket = match open(&url, &token) {
        Ok(socket) => socket,
        Err(e) => {
            log::error!("[collab] relay host could not connect: {e}");
            return;
        }
    };
    log::info!("[collab] relay host connected");

    // Everything bound for the socket funnels through one queue, so the guest
    // forwarder threads never write to it directly and never race each other.
    let (write_tx, write_rx) = unbounded::<Vec<u8>>();
    let guests: Arc<Mutex<HashMap<u32, Guest>>> = Arc::new(Mutex::new(HashMap::new()));

    loop {
        if shutdown.load(Ordering::Relaxed) {
            let _ = socket.close(None);
            break;
        }

        while let Ok(bytes) = write_rx.try_recv() {
            if socket.send(tungstenite::Message::Binary(bytes)).is_err() {
                log::warn!("[collab] relay send failed");
                break;
            }
        }

        match read(&mut socket) {
            Ok(Some(Frame::Data(data))) => {
                let Some((peer, payload)) = unwrap_envelope(&data) else { continue };
                let messages = decode_all(payload);
                if let Ok(map) = guests.lock() {
                    if let Some(guest) = map.get(&peer) {
                        for msg in messages {
                            let _ = guest.inbox.send(LinkEvent::Message(msg));
                        }
                    }
                }
            }
            Ok(Some(Frame::Control(json))) => {
                handle_control(&json, &guests, &write_tx, &incoming);
            }
            Ok(None) => {}
            Err(e) => {
                log::warn!("[collab] relay host link ended: {e}");
                break;
            }
        }
    }

    // Tell every guest link the session is over; without this they would sit
    // waiting on a socket that no longer exists.
    //
    // Taken out of the map first rather than iterated in place: a lock guard
    // held in the tail position of a function outlives the map it borrows, which
    // the borrow checker rejects outright.
    let departing: Vec<Guest> = match guests.lock() {
        Ok(mut map) => map.drain().map(|(_, guest)| guest).collect(),
        Err(_) => Vec::new(),
    };
    for guest in departing {
        guest.gone.store(true, Ordering::Relaxed);
        let _ = guest.inbox.send(LinkEvent::Closed("the relay connection ended".into()));
    }
}

/// Act on a relay control frame.
fn handle_control(
    json: &serde_json::Value,
    guests: &Arc<Mutex<HashMap<u32, Guest>>>,
    write_tx: &Sender<Vec<u8>>,
    incoming: &Sender<Link>,
) {
    match json.get("event").and_then(|e| e.as_str()) {
        Some("peer_joined") => {
            let Some(peer) = json.get("peer").and_then(|p| p.as_u64()) else { return };
            let peer = peer as u32;
            let name = json
                .get("username")
                .and_then(|u| u.as_str())
                .unwrap_or("guest")
                .to_string();

            let (link, plumb) = plumbing(name.clone());
            let LinkPlumbing { out_rx, inbox_tx, shutdown } = plumb;
            let gone = Arc::new(AtomicBool::new(false));

            // This guest's outbound queue, stamped and merged into the socket's.
            let forwarder_gone = gone.clone();
            let forwarder_tx = write_tx.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("collab-relay-peer-{peer}"))
                .spawn(move || {
                    while !forwarder_gone.load(Ordering::Relaxed)
                        && !shutdown.load(Ordering::Relaxed)
                    {
                        match out_rx.recv_timeout(READ_TICK) {
                            Ok(msg) => {
                                let Some(bytes) = encode(&msg) else { continue };
                                if forwarder_tx.send(envelope(peer, &bytes)).is_err() {
                                    return;
                                }
                            }
                            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                            Err(_) => return,
                        }
                    }
                });
            if spawned.is_err() {
                log::error!("[collab] could not spawn forwarder for peer {peer}");
                return;
            }

            let _ = inbox_tx.send(LinkEvent::Connected);
            if let Ok(mut map) = guests.lock() {
                map.insert(peer, Guest { inbox: inbox_tx, gone });
            }
            log::info!("[collab] {name} joined the relayed session as peer {peer}");
            let _ = incoming.send(link);
        }
        Some("peer_left") => {
            let Some(peer) = json.get("peer").and_then(|p| p.as_u64()) else { return };
            if let Ok(mut map) = guests.lock() {
                if let Some(guest) = map.remove(&(peer as u32)) {
                    guest.gone.store(true, Ordering::Relaxed);
                    let _ = guest.inbox.send(LinkEvent::Closed("peer disconnected".into()));
                }
            }
        }
        Some("ready") => {
            log::info!("[collab] relay accepted this editor as the host");
        }
        _ => {}
    }
}

// ── Socket plumbing ─────────────────────────────────────────────────────────

type Socket = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

enum Frame {
    Data(Vec<u8>),
    Control(serde_json::Value),
}

/// Open the relay socket.
///
/// The `User-Agent` is not optional: renzora.com sits behind Cloudflare, whose
/// managed rules block a request that arrives without one, and the handshake
/// then fails with a 403 HTML page instead of reaching the route. `tungstenite`
/// sends none by default. This is the same trap `renzora_social`'s live socket
/// documents, and it costs an afternoon every time it is rediscovered.
fn open(url: &str, token: &str) -> Result<Socket, String> {
    use tungstenite::client::IntoClientRequest;

    ensure_crypto_provider();

    let separator = if url.contains('?') { '&' } else { '?' };
    let full = format!("{url}{separator}token={token}");
    let mut request = full.into_client_request().map_err(|e| format!("bad relay URL: {e}"))?;
    request.headers_mut().insert(
        tungstenite::http::header::USER_AGENT,
        tungstenite::http::HeaderValue::from_static("renzora-editor"),
    );

    let (socket, _response) = tungstenite::connect(request).map_err(|e| match e {
        tungstenite::Error::Http(response) if response.status() == 401 => {
            "the relay rejected this session's sign-in — try signing in again".to_string()
        }
        tungstenite::Error::Http(response) if response.status() == 404 => {
            "no session with that code — it may have ended".to_string()
        }
        tungstenite::Error::Http(response) if response.status() == 409 => {
            "that session already has a host connected".to_string()
        }
        other => format!("could not reach the relay: {other}"),
    })?;

    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_ref() {
        let _ = stream.set_read_timeout(Some(READ_TICK));
    } else if let tungstenite::stream::MaybeTlsStream::Rustls(stream) = socket.get_ref() {
        let _ = stream.get_ref().set_read_timeout(Some(READ_TICK));
    }
    Ok(socket)
}

/// Install ring as the process-level rustls provider exactly once.
///
/// A no-op if something else got there first — `renzora_social`'s live socket
/// installs the same one, and whichever runs first wins.
fn ensure_crypto_provider() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Read one message, treating a timeout as "nothing yet".
fn read(socket: &mut Socket) -> Result<Option<Frame>, String> {
    match socket.read() {
        Ok(tungstenite::Message::Binary(data)) => Ok(Some(Frame::Data(data))),
        Ok(tungstenite::Message::Text(text)) => match serde_json::from_str(&text) {
            Ok(json) => Ok(Some(Frame::Control(json))),
            Err(_) => Ok(None),
        },
        Ok(tungstenite::Message::Ping(payload)) => {
            let _ = socket.send(tungstenite::Message::Pong(payload));
            Ok(None)
        }
        Ok(tungstenite::Message::Close(_)) => Err("the relay closed the connection".into()),
        Ok(_) => Ok(None),
        Err(tungstenite::Error::Io(e))
            if matches!(
                e.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Decode every frame in a relayed payload.
///
/// Usually exactly one — the sender writes a frame per message and the relay
/// forwards them whole. It is written as a loop anyway because nothing in the
/// path *guarantees* one frame per message, and a payload carrying two would
/// otherwise lose the second silently.
fn decode_all(mut payload: &[u8]) -> Vec<CollabMsg> {
    let mut out = Vec::new();
    while !payload.is_empty() {
        let before = payload.len();
        match crate::protocol::read_frame(&mut payload) {
            Ok(msg) => out.push(msg),
            Err(e) => {
                log::warn!("[collab] dropping a malformed relayed frame: {e}");
                break;
            }
        }
        if payload.len() == before {
            break; // no progress; refuse to spin
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Golden vectors ──────────────────────────────────────────────────────
    //
    // The envelope is the one place where this crate and the *server* must
    // agree byte for byte, and they are separate codebases in separate repos
    // that are deployed independently. So both assert the same literal bytes:
    // the server's copy is in `crates/api/src/collab.rs` in the website repo
    // (`mod tests`, same constant names), and the layout is written up in
    // `docs/r1-alpha7/platform-api/collab.md`.
    //
    // Literal bytes rather than `3u32.to_le_bytes()`, because a test phrased in
    // terms of the implementation's own helper passes just as happily when the
    // helper is wrong. The point is to pin the wire, so the wire is spelled out.

    /// Peer 3, payload `b"hi"`.
    const PEER_3_HI: &[u8] = &[0x03, 0x00, 0x00, 0x00, b'h', b'i'];
    /// The host (peer 0), payload `b"hi"`.
    const HOST_HI: &[u8] = &[0x00, 0x00, 0x00, 0x00, b'h', b'i'];
    /// The broadcast target, payload `b"hi"`. Understood by the server; this
    /// client does not send it yet (see the module docs).
    const BROADCAST_HI: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF, b'h', b'i'];

    #[test]
    fn envelope_is_little_endian() {
        assert_eq!(envelope(3, b"hi"), PEER_3_HI);
        assert_eq!(envelope(HOST_PEER, b"hi"), HOST_HI);
        assert_eq!(envelope(u32::MAX, b"hi"), BROADCAST_HI);

        assert_eq!(unwrap_envelope(PEER_3_HI), Some((3, &b"hi"[..])));
        assert_eq!(unwrap_envelope(HOST_HI), Some((HOST_PEER, &b"hi"[..])));
        assert_eq!(unwrap_envelope(BROADCAST_HI), Some((u32::MAX, &b"hi"[..])));
    }

    /// The failure this pins down is silent: read the header big-endian and a
    /// message for peer 3 is addressed to peer 50331648, which matches nobody
    /// and is dropped without an error anywhere.
    #[test]
    fn big_endian_would_be_a_different_peer() {
        assert_eq!(u32::from_be_bytes([0x03, 0x00, 0x00, 0x00]), 50_331_648);
        assert_eq!(unwrap_envelope(PEER_3_HI).map(|(peer, _)| peer), Some(3));
    }

    /// A guest always addresses the host as peer 0, and that is the byte the
    /// server expects to see in front of a guest's traffic.
    #[test]
    fn the_host_is_peer_zero() {
        assert_eq!(HOST_PEER, 0);
        assert_eq!(&envelope(HOST_PEER, b"")[..], &[0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn envelope_round_trips() {
        let wrapped = envelope(7, b"payload");
        let (peer, payload) = unwrap_envelope(&wrapped).expect("well-formed");
        assert_eq!(peer, 7);
        assert_eq!(payload, b"payload");
    }

    #[test]
    fn short_envelopes_are_refused() {
        assert!(unwrap_envelope(&[]).is_none());
        assert!(unwrap_envelope(&[1, 2, 3]).is_none());
        // Exactly four bytes is a valid envelope with an empty payload.
        let (peer, payload) = unwrap_envelope(&[0, 0, 0, 0]).expect("header only");
        assert_eq!(peer, 0);
        assert!(payload.is_empty());
    }

    /// A relayed payload is decoded with the same framing the direct transport
    /// uses, so the two carry identical bytes and a session cannot tell which
    /// one it is on.
    #[test]
    fn payloads_decode_as_ordinary_frames() {
        let encoded = encode(&CollabMsg::Ping).expect("encode");
        let decoded = decode_all(&encoded);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].label(), "ping");

        // Two frames in one payload — not how the relay sends them today, but
        // nothing guarantees one per message, and losing the second silently
        // would be the worst possible failure mode.
        let mut both = encode(&CollabMsg::Ping).expect("encode");
        both.extend_from_slice(&encode(&CollabMsg::Pong).expect("encode"));
        let decoded = decode_all(&both);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[1].label(), "pong");
    }

    /// Garbage must stop the decode rather than spin.
    #[test]
    fn malformed_payloads_do_not_loop() {
        assert!(decode_all(b"not a frame at all").is_empty());
    }

    /// What the server does to a guest's message, restated here.
    ///
    /// Not a mock of the relay — a written-down assumption about it. If the
    /// server ever stops rewriting the tag, or starts touching the payload, the
    /// engine's expectation is at least stated somewhere a reader can check
    /// against `crates/api/src/collab.rs` rather than having to infer it.
    fn as_the_relay_would(from_guest: u32, sent: &[u8]) -> Vec<u8> {
        let (_ignored_target, payload) = unwrap_envelope(sent).expect("well-formed");
        envelope(from_guest, payload)
    }

    /// A message survives the whole path: encoded here, wrapped, passed through
    /// the relay's rewrite, and decoded on the other side.
    ///
    /// This is the end the golden vectors cannot reach on their own. They prove
    /// the header is the right four bytes; this proves the payload behind it is
    /// still a frame this crate can read after the server has handled it.
    #[test]
    fn a_message_survives_the_relay_round_trip() {
        let original = CollabMsg::EntityDespawn { ids: vec![11, 22, 33] };

        // Guest side: encode, address the host.
        let sent = envelope(HOST_PEER, &encode(&original).expect("encode"));
        assert_eq!(&sent[..4], &[0x00, 0x00, 0x00, 0x00]);

        // Server side: retag as coming from guest 4, payload untouched.
        let relayed = as_the_relay_would(4, &sent);
        assert_eq!(&relayed[..4], &[0x04, 0x00, 0x00, 0x00]);

        // Host side: unwrap, and route by the peer the relay named.
        let (peer, payload) = unwrap_envelope(&relayed).expect("well-formed");
        assert_eq!(peer, 4);
        let decoded = decode_all(payload);
        assert_eq!(decoded.len(), 1);
        match &decoded[0] {
            CollabMsg::EntityDespawn { ids } => assert_eq!(ids, &vec![11, 22, 33]),
            other => panic!("expected EntityDespawn, got {}", other.label()),
        }
    }

    /// The payload is opaque to the relay, so a frame large enough to be split
    /// across reads must come back byte-identical.
    #[test]
    fn a_large_payload_survives_the_round_trip() {
        let original = CollabMsg::SceneReset {
            bsn: "x".repeat(200_000),
            ids: vec![(1, 2); 1000],
        };
        let encoded = encode(&original).expect("encode");
        let relayed = as_the_relay_would(1, &envelope(HOST_PEER, &encoded));
        let (_, payload) = unwrap_envelope(&relayed).expect("well-formed");
        assert_eq!(payload, &encoded[..], "the relay must not touch the payload");

        match &decode_all(payload)[0] {
            CollabMsg::SceneReset { bsn, ids } => {
                assert_eq!(bsn.len(), 200_000);
                assert_eq!(ids.len(), 1000);
            }
            other => panic!("expected SceneReset, got {}", other.label()),
        }
    }
}
