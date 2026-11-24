//! The wire format: one datagram is one [`Packet`], prefixed by [`MAGIC`].
//!
//! bincode over serde, because the packet is small and fixed in shape and
//! bincode's varints keep a sequence number to one or two bytes. Application
//! messages ride inside [`Packet::Payload`] as opaque bytes, so this file
//! never changes when the game protocol does.

use serde::{Deserialize, Serialize};

/// The first four bytes of every datagram. A packet without it is not ours (a
/// port scanner, a stale process of another program) and is dropped before
/// any decoding runs. The last byte is the wire version: bumping it makes old
/// and new builds ignore each other instead of misreading each other.
pub const MAGIC: [u8; 4] = *b"RZN\x02";

/// The most a datagram we build may hold. Below every real path MTU once IP
/// and UDP headers are added, so nothing is fragmented by the network itself,
/// where one lost IP fragment loses the whole datagram.
pub const MAX_PACKET_BYTES: usize = 1200;

/// The largest piece of a reliable message. A bigger one (a late joiner's
/// spawn of an entity with many components) is split into pieces this size
/// and reassembled in order on the far side.
pub const FRAGMENT_BYTES: usize = 1000;

/// The largest unreliable message. Unreliable messages are never split: a
/// split one is lost when any piece is, so it is worse than useless. The
/// replication layer chunks its snapshots under this.
pub const MAX_UNRELIABLE_BYTES: usize = 1100;

/// Why a server refused a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DenyReason {
    /// Every slot is taken.
    ServerFull,
    /// A different game, or a different build of this one.
    ProtocolMismatch,
}

impl std::fmt::Display for DenyReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFull => write!(f, "the server is full"),
            Self::ProtocolMismatch => write!(f, "the server runs a different game or build"),
        }
    }
}

/// One message inside a payload packet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WireMessage {
    /// Delivered at most once, in no particular order.
    Unreliable(Vec<u8>),
    /// Delivered exactly once, in send order. `more` is set on every piece of
    /// a split message but the last.
    Reliable { id: u16, more: bool, data: Vec<u8> },
}

impl WireMessage {
    /// Upper bound on what this adds to a packet, for packing.
    pub fn size_hint(&self) -> usize {
        match self {
            Self::Unreliable(d) => d.len() + 4,
            Self::Reliable { data, .. } => data.len() + 8,
        }
    }
}

/// A datagram.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Packet {
    /// Client: let me in. Repeated until answered.
    ConnectRequest { protocol: u64, nonce: u64 },
    /// Server: prove you receive at the address you claim. The token is a
    /// keyed hash of the address and nonce, so the server keeps no state for a
    /// client until the client echoes it. Without this step anyone could fill
    /// every slot by sending requests with forged source addresses.
    Challenge { nonce: u64, token: u64 },
    /// Client: the echo.
    ChallengeResponse { nonce: u64, token: u64 },
    /// Server: you are in, as `client_id`.
    Accepted { client_id: u64, tick_rate: u16 },
    /// Server: you are not.
    Denied { reason: DenyReason },
    /// Messages, plus acknowledgement of the last 33 packets received: `ack`
    /// and each set bit `i` of `ack_bits` for packet `ack - 1 - i`.
    Payload { seq: u16, ack: u16, ack_bits: u32, messages: Vec<WireMessage> },
    /// Either side: goodbye. Sent a few times, since it is never acknowledged.
    Disconnect,
}

pub fn encode(packet: &Packet) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    match bincode::serde::encode_to_vec(packet, bincode::config::standard()) {
        Ok(body) => out.extend_from_slice(&body),
        Err(_) => return Vec::new(),
    }
    out
}

pub fn decode(bytes: &[u8]) -> Option<Packet> {
    let body = bytes.strip_prefix(&MAGIC)?;
    // A decoder bound: a forged length prefix must not make us allocate
    // gigabytes. Nothing legitimate is larger than one datagram.
    let config = bincode::config::standard().with_limit::<{ MAX_PACKET_BYTES * 2 }>();
    bincode::serde::decode_from_slice(body, config).ok().map(|(p, _)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let p = Packet::Payload {
            seq: 65535,
            ack: 3,
            ack_bits: 0xdead_beef,
            messages: vec![
                WireMessage::Unreliable(vec![1, 2, 3]),
                WireMessage::Reliable { id: 9, more: true, data: vec![0; 40] },
            ],
        };
        assert_eq!(decode(&encode(&p)), Some(p));
    }

    #[test]
    fn foreign_datagrams_are_rejected() {
        assert_eq!(decode(b"hello"), None);
        assert_eq!(decode(&[]), None);
        let mut wrong_version = encode(&Packet::Disconnect);
        wrong_version[3] = 0x01;
        assert_eq!(decode(&wrong_version), None);
    }

    #[test]
    fn a_full_packet_fits_a_datagram() {
        let p = Packet::Payload {
            seq: 1,
            ack: 1,
            ack_bits: u32::MAX,
            messages: vec![WireMessage::Reliable { id: 1, more: true, data: vec![255; FRAGMENT_BYTES] }],
        };
        assert!(encode(&p).len() <= MAX_PACKET_BYTES);
    }
}
