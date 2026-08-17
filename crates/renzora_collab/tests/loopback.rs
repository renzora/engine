//! A real listener and a real client, over a real socket.
//!
//! Every framing bug this transport has had lived in the gap between "the
//! framing functions round-trip against a `Vec<u8>`" (which they always did) and
//! "the framing functions round-trip against an *accepted socket*" (which they
//! did not). The accepted socket is the part with the sharp edge: on Windows it
//! inherits the listener's non-blocking flag, and a non-blocking `read_exact`
//! returns `WouldBlock` having already eaten part of a frame.
//!
//! So these tests drive the actual `listen`/`connect` path rather than the
//! codec, and the large-payload one exists specifically because the corruption
//! only showed up once a message was big enough to span several reads.

use std::time::{Duration, Instant};

use renzora_collab::link::{Link, LinkEvent};
use renzora_collab::protocol::CollabMsg;
use renzora_collab::tcp;

/// Poll until `f` produces a value, or fail after `secs`.
fn wait_for<T>(secs: u64, mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(value) = f() {
            return value;
        }
        if Instant::now() > deadline {
            panic!("timed out after {secs}s");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A link plus the messages drained from it but not yet consumed.
///
/// `Link::drain` empties the link — deliberately, since presence is state and a
/// stale one should not be processed when a newer is already queued. A test that
/// reads one message at a time must therefore keep the rest, or it silently
/// discards everything that arrived in the same batch and then waits forever for
/// messages it already threw away. (Which is exactly what the first version of
/// this file did, and it looked like a transport hang.)
struct Peer {
    link: Link,
    pending: std::collections::VecDeque<CollabMsg>,
}

impl Peer {
    fn new(link: Link) -> Self {
        Self { link, pending: std::collections::VecDeque::new() }
    }

    /// The next message, waiting for it if necessary. Fails on close.
    fn next(&mut self, secs: u64) -> CollabMsg {
        if let Some(msg) = self.pending.pop_front() {
            return msg;
        }
        let link = &mut self.link;
        let pending = &mut self.pending;
        wait_for(secs, || {
            for event in link.drain() {
                match event {
                    LinkEvent::Message(msg) => pending.push_back(msg),
                    LinkEvent::Closed(reason) => panic!("link closed: {reason}"),
                    LinkEvent::Connected => {}
                }
            }
            pending.pop_front()
        })
    }
}

/// Bring up a connected pair on the loopback interface.
fn pair() -> (Peer, Link) {
    // Port 0 asks the OS for a free one, so the test never collides with a real
    // session or another test run.
    let listener = tcp::listen(0).expect("listen");
    let port = listener.local_port.expect("a TCP acceptor binds a port");
    let client = tcp::connect(format!("127.0.0.1:{port}"));
    let server = wait_for(5, || listener.incoming.try_recv().ok());
    // The listener is dropped here on purpose: an established link must outlive
    // the accept loop that produced it.
    (Peer::new(server), client)
}

/// The case that was broken in the first live session: a large message followed
/// by small ones, over an accepted socket.
///
/// The scene is the big one in practice, and the failure only appeared after
/// "sent the scene". A payload that spans several reads is what turns a
/// partially-consuming read error into a permanently misaligned stream, so the
/// payload here is deliberately megabytes rather than bytes.
#[test]
fn large_frame_then_small_frames_survive_the_accept_path() {
    let (mut server, client) = pair();

    let big = "x".repeat(4 * 1024 * 1024);
    client.send(CollabMsg::SceneReset { bsn: big.clone(), ids: vec![(1, 2); 5000] });
    for i in 0..20u64 {
        client.send(CollabMsg::EntityDespawn { ids: vec![i] });
    }

    match server.next(20) {
        CollabMsg::SceneReset { bsn, ids } => {
            assert_eq!(bsn.len(), big.len(), "the large payload came back the wrong size");
            assert_eq!(bsn, big, "the large payload came back corrupted");
            assert_eq!(ids.len(), 5000);
        }
        other => panic!("expected SceneReset, got {}", other.label()),
    }

    // Everything queued behind the big one must still be intact and in order —
    // this is what a desynchronised stream destroys.
    for expected in 0..20u64 {
        match server.next(20) {
            CollabMsg::EntityDespawn { ids } => assert_eq!(ids, vec![expected]),
            other => panic!("expected EntityDespawn({expected}), got {}", other.label()),
        }
    }
}

/// Both directions at once, with nothing lost.
#[test]
fn messages_flow_both_ways() {
    let (mut server, client) = pair();
    let mut client = Peer::new(client);

    client.link.send(CollabMsg::Hello {
        protocol: renzora_collab::protocol::PROTOCOL_VERSION,
        display_name: "guest".into(),
        project: "demo".into(),
    });
    match server.next(10) {
        CollabMsg::Hello { display_name, .. } => assert_eq!(display_name, "guest"),
        other => panic!("expected Hello, got {}", other.label()),
    }

    server.link.send(CollabMsg::Welcome {
        protocol: renzora_collab::protocol::PROTOCOL_VERSION,
        peer_id: 1,
        host_name: "host".into(),
        project: "demo".into(),
    });
    match client.next(10) {
        CollabMsg::Welcome { host_name, peer_id, .. } => {
            assert_eq!(host_name, "host");
            assert_eq!(peer_id, 1);
        }
        other => panic!("expected Welcome, got {}", other.label()),
    }
}

/// A long quiet spell must not disturb the stream.
///
/// The reader blocks indefinitely by design. This is the regression guard for
/// re-introducing a read timeout "so the thread can poll something" — with one,
/// the idle gap below is where a partially-consumed read would land.
#[test]
fn an_idle_link_stays_usable() {
    let (mut server, client) = pair();

    client.send(CollabMsg::Ping);
    assert!(matches!(server.next(10), CollabMsg::Ping));

    // Comfortably longer than any timeout anyone would reach for.
    std::thread::sleep(Duration::from_millis(1500));

    client.send(CollabMsg::EntityDespawn { ids: vec![99] });
    match server.next(10) {
        CollabMsg::EntityDespawn { ids } => assert_eq!(ids, vec![99]),
        other => panic!("expected EntityDespawn after idle, got {}", other.label()),
    }
}

/// Dropping a link hangs up, and the other end notices rather than hanging.
#[test]
fn dropping_a_link_closes_the_peer() {
    let (mut server, client) = pair();
    client.send(CollabMsg::Ping);
    assert!(matches!(server.next(10), CollabMsg::Ping));

    drop(client);

    let closed = wait_for(10, || {
        server.link.drain().into_iter().find_map(|e| match e {
            LinkEvent::Closed(reason) => Some(reason),
            _ => None,
        })
    });
    assert!(!closed.is_empty());
    assert!(server.link.is_closed());
}

/// A message queued immediately before the link is dropped still goes out.
///
/// This is how a rejected peer learns *why* it was rejected: the host queues
/// `Rejected` and drops the link in the same breath. If the hangup closed the
/// write half, the explanation would be lost and the guest would see a bare
/// disconnect — which is the failure `Rejected` exists to prevent.
#[test]
fn a_message_queued_before_hangup_still_arrives() {
    let (server, mut client) = pair();

    server.link.send(CollabMsg::Rejected { reason: "protocol mismatch".into() });
    drop(server);

    let reason = wait_for(10, || {
        client.drain().into_iter().find_map(|e| match e {
            LinkEvent::Message(CollabMsg::Rejected { reason }) => Some(reason),
            _ => None,
        })
    });
    assert_eq!(reason, "protocol mismatch");
}
