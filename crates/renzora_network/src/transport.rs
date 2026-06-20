//! From-scratch UDP transport: a connection handshake plus a reliable,
//! de-duplicated message channel over `std::net::UdpSocket`.
//!
//! This is intentionally synchronous and Bevy-agnostic — the client/server
//! systems poll it once per frame. It replaces the previous lightyear stack:
//! the engine only needs reliable `GameEvent` (RPC) delivery + connection
//! lifecycle, so this does transport + lightweight netcode, **not** full state
//! replication (which was always TODO). Encryption is deliberately omitted for
//! now — add it before internet-facing production.

use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::messages::GameEvent;

/// Resend an unacked reliable packet after this long without an ack.
const RESEND_AFTER: Duration = Duration::from_millis(150);
/// Drop a peer we haven't heard from in this long.
pub const PEER_TIMEOUT: Duration = Duration::from_secs(10);
/// Send a keep-alive if we haven't sent anything for this long.
const KEEPALIVE_EVERY: Duration = Duration::from_secs(1);
/// Max UDP datagram we read into. RPC events are small; oversized payloads are
/// simply dropped by the socket (no fragmentation in v1).
pub(crate) const MAX_DATAGRAM: usize = 4096;

/// On-wire packet, bincode-encoded.
#[derive(Serialize, Deserialize)]
pub(crate) enum Packet {
    /// Client → server: request to join under a self-chosen id.
    ConnectRequest { client_id: u64 },
    /// Server → client: connection accepted.
    ConnectAccept { client_id: u64 },
    /// Either direction: graceful close.
    Disconnect,
    /// A reliable, de-duplicated application event.
    Reliable { seq: u32, event: GameEvent },
    /// Acknowledgement of a reliable packet.
    Ack { seq: u32 },
    /// Liveness probe (resets the peer timeout).
    KeepAlive,
}

pub(crate) fn encode(p: &Packet) -> Vec<u8> {
    bincode::serde::encode_to_vec(p, bincode::config::standard()).unwrap_or_default()
}

pub(crate) fn decode(bytes: &[u8]) -> Option<Packet> {
    bincode::serde::decode_from_slice(bytes, bincode::config::standard())
        .ok()
        .map(|(p, _)| p)
}

/// Per-peer reliability + liveness state. Used by both the client (one peer =
/// the server) and the server (one per connected client).
pub(crate) struct Peer {
    pub addr: SocketAddr,
    pub client_id: u64,
    next_seq: u32,
    /// Reliable packets sent but not yet acked: seq → (event, last_sent).
    unacked: HashMap<u32, (GameEvent, Instant)>,
    /// Reliable seqs we've already delivered, for dedup. Bounded by the RPC
    /// volume of a session (low); a ring buffer is a future refinement.
    seen: HashSet<u32>,
    pub last_recv: Instant,
    last_sent: Instant,
}

impl Peer {
    pub fn new(addr: SocketAddr, client_id: u64) -> Self {
        let now = Instant::now();
        Self {
            addr,
            client_id,
            next_seq: 0,
            unacked: HashMap::new(),
            seen: HashSet::new(),
            last_recv: now,
            last_sent: now,
        }
    }

    fn raw_send(&mut self, socket: &UdpSocket, p: &Packet) {
        let _ = socket.send_to(&encode(p), self.addr);
        self.last_sent = Instant::now();
    }

    /// Queue + send a reliable event to this peer.
    pub fn send_reliable(&mut self, socket: &UdpSocket, event: GameEvent) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.raw_send(socket, &Packet::Reliable { seq, event: event.clone() });
        self.unacked.insert(seq, (event, Instant::now()));
    }

    /// An incoming reliable packet arrived: always ack it, and return `true`
    /// the first time we see a given seq (i.e. deliver it once).
    pub fn on_reliable(&mut self, socket: &UdpSocket, seq: u32) -> bool {
        self.raw_send(socket, &Packet::Ack { seq });
        self.seen.insert(seq)
    }

    /// Drop a reliable packet from the resend queue once acked.
    pub fn on_ack(&mut self, seq: u32) {
        self.unacked.remove(&seq);
    }

    /// Per-frame upkeep: resend timed-out reliable packets + keep-alive.
    pub fn tick(&mut self, socket: &UdpSocket) {
        let now = Instant::now();
        let addr = self.addr;
        for (seq, (event, sent)) in self.unacked.iter_mut() {
            if now.duration_since(*sent) >= RESEND_AFTER {
                let _ = socket.send_to(&encode(&Packet::Reliable { seq: *seq, event: event.clone() }), addr);
                *sent = now;
            }
        }
        if now.duration_since(self.last_sent) >= KEEPALIVE_EVERY {
            self.raw_send(socket, &Packet::KeepAlive);
        }
    }

    pub fn timed_out(&self) -> bool {
        self.last_recv.elapsed() >= PEER_TIMEOUT
    }
}
