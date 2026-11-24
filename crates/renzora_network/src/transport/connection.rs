//! Reliability over one UDP peer: packet acks, the reliable-ordered channel,
//! fragmentation, and the round-trip and loss figures the HUD shows.
//!
//! # How reliability works here
//!
//! Every payload packet carries a sequence number and acknowledges the last 33
//! packets it heard (`ack` + a 32-bit field), so acks ride on traffic that is
//! going out anyway and a lost ack is repaired by the next packet. A reliable
//! message is re-included in outgoing packets on a timer until a packet that
//! carried it is acknowledged; the receiver buffers out-of-order arrivals and
//! releases them strictly in id order.
//!
//! The previous transport acked each reliable message with a datagram of its
//! own and delivered in arrival order, so two RPCs sent in one frame could be
//! handled in either order, and a message bigger than 4 KiB was silently
//! dropped by the socket. Ordering and fragmentation are both what make it
//! possible to send a late joiner the whole world as a stream of spawns.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use super::packet::{Packet, WireMessage, FRAGMENT_BYTES, MAX_PACKET_BYTES, MAX_UNRELIABLE_BYTES};

/// Which delivery guarantee a message gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Reliable,
    Unreliable,
}

/// Send something, even if only acks, at least this often, so the peer's
/// view of what we received never goes stale and its timeout never fires on
/// a quiet but healthy link.
const KEEPALIVE: Duration = Duration::from_millis(100);
/// The floor on the reliable resend timer, so a LAN round trip of 1 ms does
/// not resend every frame.
const MIN_RESEND: Duration = Duration::from_millis(50);
/// A packet not acknowledged in this long is counted lost.
const PACKET_LOST_AFTER: Duration = Duration::from_secs(1);
/// How far ahead of the oldest unacknowledged reliable message we will send.
/// Message ids are 16-bit, so the receiver can only tell a new id from an old
/// one within half the id space; staying far inside it keeps that unambiguous.
const RELIABLE_WINDOW: u16 = 4096;
/// The most packets one flush builds, so a burst (a late joiner's world) is
/// spread over several frames instead of dropped by a full socket buffer.
const MAX_PACKETS_PER_FLUSH: usize = 48;

/// `a` is later than `b` in 16-bit wrapping order.
pub fn seq_newer(a: u16, b: u16) -> bool {
    a != b && a.wrapping_sub(b) < 0x8000
}

struct SentPacket {
    at: Instant,
    reliable_ids: Vec<u16>,
}

struct OutReliable {
    id: u16,
    more: bool,
    data: Vec<u8>,
    last_sent: Option<Instant>,
}

/// One remote peer's reliability state.
pub struct Connection {
    // Packets we send.
    local_seq: u16,
    sent: HashMap<u16, SentPacket>,
    // Packets we received: the newest seq and which of the 32 before it.
    remote_seq: u16,
    remote_bits: u32,
    received_any: bool,
    ack_owed: bool,

    // Reliable channel, outgoing.
    next_reliable_id: u16,
    reliable_out: VecDeque<OutReliable>,
    // Reliable channel, incoming.
    next_expected: u16,
    reliable_in: HashMap<u16, (bool, Vec<u8>)>,
    assembling: Vec<u8>,

    unreliable_out: Vec<Vec<u8>>,
    inbox: Vec<(Channel, Vec<u8>)>,

    pub last_recv: Instant,
    last_send: Instant,

    /// Smoothed round trip, seconds.
    rtt: f32,
    rtt_jitter: f32,
    rtt_sampled: bool,
    loss: f32,
    window_start: Instant,
    bytes_in: usize,
    bytes_out: usize,
    bytes_in_rate: f32,
    bytes_out_rate: f32,
}

/// What [`Connection::stats`] reports.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConnectionStats {
    pub rtt_ms: f32,
    pub jitter_ms: f32,
    pub packet_loss: f32,
    pub bytes_in_per_sec: f32,
    pub bytes_out_per_sec: f32,
}

impl Connection {
    pub fn new(now: Instant) -> Self {
        Self {
            local_seq: 0,
            sent: HashMap::new(),
            remote_seq: 0,
            remote_bits: 0,
            received_any: false,
            ack_owed: false,
            next_reliable_id: 0,
            reliable_out: VecDeque::new(),
            next_expected: 0,
            reliable_in: HashMap::new(),
            assembling: Vec::new(),
            unreliable_out: Vec::new(),
            inbox: Vec::new(),
            last_recv: now,
            last_send: now,
            rtt: 0.1,
            rtt_jitter: 0.0,
            rtt_sampled: false,
            loss: 0.0,
            window_start: now,
            bytes_in: 0,
            bytes_out: 0,
            bytes_in_rate: 0.0,
            bytes_out_rate: 0.0,
        }
    }

    /// Queue a message.
    pub fn send(&mut self, channel: Channel, data: Vec<u8>) {
        match channel {
            Channel::Unreliable if data.len() <= MAX_UNRELIABLE_BYTES => self.unreliable_out.push(data),
            // Too big to go unsplit, and a split unreliable message is lost
            // whenever any piece is. Better late than not at all.
            Channel::Unreliable => self.send_reliable(data),
            Channel::Reliable => self.send_reliable(data),
        }
    }

    fn send_reliable(&mut self, data: Vec<u8>) {
        if data.is_empty() {
            self.push_reliable(false, data);
            return;
        }
        let pieces: Vec<&[u8]> = data.chunks(FRAGMENT_BYTES).collect();
        let last = pieces.len() - 1;
        for (i, piece) in pieces.into_iter().enumerate() {
            self.push_reliable(i != last, piece.to_vec());
        }
    }

    fn push_reliable(&mut self, more: bool, data: Vec<u8>) {
        let id = self.next_reliable_id;
        self.next_reliable_id = self.next_reliable_id.wrapping_add(1);
        self.reliable_out.push_back(OutReliable { id, more, data, last_sent: None });
    }

    /// Everything delivered since the last call, in delivery order.
    pub fn take_received(&mut self) -> Vec<(Channel, Vec<u8>)> {
        std::mem::take(&mut self.inbox)
    }

    /// Reliable messages not yet acknowledged. A server watches this to stop
    /// queueing to a peer that has stopped acknowledging anything.
    pub fn reliable_backlog(&self) -> usize {
        self.reliable_out.len()
    }

    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            rtt_ms: self.rtt * 1000.0,
            jitter_ms: self.rtt_jitter * 1000.0,
            packet_loss: self.loss,
            bytes_in_per_sec: self.bytes_in_rate,
            bytes_out_per_sec: self.bytes_out_rate,
        }
    }

    pub fn rtt(&self) -> f32 {
        self.rtt
    }

    /// Note a datagram's size for the bandwidth figures.
    pub fn count_in(&mut self, bytes: usize) {
        self.bytes_in += bytes;
    }

    /// Take in a payload packet's contents.
    pub fn on_payload(&mut self, seq: u16, ack: u16, ack_bits: u32, messages: Vec<WireMessage>, now: Instant) {
        self.last_recv = now;
        if !self.note_received(seq) {
            // A duplicate the network made. Its acks are still good news; its
            // messages were already handled the first time.
            self.on_acks(ack, ack_bits, now);
            return;
        }
        self.ack_owed = true;
        self.on_acks(ack, ack_bits, now);

        for message in messages {
            match message {
                WireMessage::Unreliable(data) => self.inbox.push((Channel::Unreliable, data)),
                WireMessage::Reliable { id, more, data } => {
                    // Ahead of what we have delivered, and inside the window:
                    // keep it. Behind: a resend of something already
                    // delivered, whose ack was lost.
                    let ahead = id.wrapping_sub(self.next_expected);
                    if ahead < RELIABLE_WINDOW * 2 {
                        self.reliable_in.entry(id).or_insert((more, data));
                    }
                }
            }
        }

        // Release everything now contiguous with what came before.
        while let Some((more, data)) = self.reliable_in.remove(&self.next_expected) {
            self.next_expected = self.next_expected.wrapping_add(1);
            self.assembling.extend_from_slice(&data);
            if !more {
                self.inbox.push((Channel::Reliable, std::mem::take(&mut self.assembling)));
            }
        }
    }

    /// Record `seq` as received. False if it already was.
    fn note_received(&mut self, seq: u16) -> bool {
        if !self.received_any {
            self.received_any = true;
            self.remote_seq = seq;
            self.remote_bits = 0;
            return true;
        }
        if seq == self.remote_seq {
            return false;
        }
        if seq_newer(seq, self.remote_seq) {
            let shift = seq.wrapping_sub(self.remote_seq) as u32;
            // The old newest becomes bit `shift - 1`.
            self.remote_bits = if shift > 32 {
                0
            } else {
                let shifted = if shift == 32 { 0 } else { self.remote_bits << shift };
                shifted | (1u32 << (shift - 1))
            };
            self.remote_seq = seq;
            true
        } else {
            let behind = self.remote_seq.wrapping_sub(seq) as u32;
            if behind > 32 {
                // Too old to record. Treat as new: if it was a duplicate, the
                // reliable ids inside are deduplicated anyway, and an
                // unreliable message 33 packets late is rare enough to accept.
                return true;
            }
            let bit = 1u32 << (behind - 1);
            if self.remote_bits & bit != 0 {
                return false;
            }
            self.remote_bits |= bit;
            true
        }
    }

    fn on_acks(&mut self, ack: u16, ack_bits: u32, now: Instant) {
        if let Some(packet) = self.sent.remove(&ack) {
            // RTT from the freshest ack only: an older packet acknowledged by a
            // bit may have sat in the peer's ack field for several sends.
            let sample = now.duration_since(packet.at).as_secs_f32();
            if self.rtt_sampled {
                let deviation = (sample - self.rtt).abs();
                self.rtt_jitter += (deviation - self.rtt_jitter) * 0.1;
                self.rtt += (sample - self.rtt) * 0.1;
            } else {
                self.rtt = sample;
                self.rtt_sampled = true;
            }
            self.acked(packet.reliable_ids);
            self.loss *= 0.99;
        }
        for i in 0..32u16 {
            if ack_bits & (1 << i) != 0 {
                if let Some(packet) = self.sent.remove(&ack.wrapping_sub(i + 1)) {
                    self.acked(packet.reliable_ids);
                    self.loss *= 0.99;
                }
            }
        }
    }

    fn acked(&mut self, ids: Vec<u16>) {
        if ids.is_empty() {
            return;
        }
        self.reliable_out.retain(|m| !ids.contains(&m.id));
    }

    /// Whether the peer has gone quiet for longer than `timeout`.
    pub fn timed_out(&self, now: Instant, timeout: Duration) -> bool {
        now.duration_since(self.last_recv) >= timeout
    }

    /// Build the datagrams to send now.
    pub fn flush(&mut self, now: Instant) -> Vec<Packet> {
        self.expire_lost(now);
        self.roll_bandwidth(now);

        let resend_after = MIN_RESEND.max(Duration::from_secs_f32(self.rtt * 1.5));
        let oldest = self.reliable_out.front().map(|m| m.id);
        let mut packets = Vec::new();
        let mut unreliable = std::mem::take(&mut self.unreliable_out).into_iter().peekable();

        loop {
            if packets.len() >= MAX_PACKETS_PER_FLUSH {
                break;
            }
            let mut messages = Vec::new();
            let mut reliable_ids = Vec::new();
            let mut budget = MAX_PACKET_BYTES - 32;

            for m in self.reliable_out.iter_mut() {
                if let Some(oldest) = oldest {
                    if m.id.wrapping_sub(oldest) >= RELIABLE_WINDOW {
                        break;
                    }
                }
                let due = m.last_sent.is_none_or(|t| now.duration_since(t) >= resend_after);
                if !due {
                    continue;
                }
                let msg = WireMessage::Reliable { id: m.id, more: m.more, data: m.data.clone() };
                if msg.size_hint() > budget {
                    break;
                }
                budget -= msg.size_hint();
                m.last_sent = Some(now);
                reliable_ids.push(m.id);
                messages.push(msg);
            }
            while let Some(data) = unreliable.peek() {
                if data.len() + 4 > budget {
                    break;
                }
                budget -= data.len() + 4;
                messages.push(WireMessage::Unreliable(unreliable.next().unwrap_or_default()));
            }

            let keepalive_due = now.duration_since(self.last_send) >= KEEPALIVE;
            if messages.is_empty() && !(self.ack_owed || keepalive_due) {
                break;
            }
            let more_to_send = !messages.is_empty();
            packets.push(self.seal(messages, reliable_ids, now));
            if !more_to_send {
                break;
            }
        }
        // Whatever did not fit this frame is stale by the next one, and
        // sending it then would only delay the fresher state behind it.
        drop(unreliable);
        packets
    }

    fn seal(&mut self, messages: Vec<WireMessage>, reliable_ids: Vec<u16>, now: Instant) -> Packet {
        let seq = self.local_seq;
        self.local_seq = self.local_seq.wrapping_add(1);
        self.sent.insert(seq, SentPacket { at: now, reliable_ids });
        self.ack_owed = false;
        self.last_send = now;
        Packet::Payload { seq, ack: self.remote_seq, ack_bits: self.remote_bits, messages }
    }

    /// Note a sealed packet's encoded size for the bandwidth figures.
    pub fn count_out(&mut self, bytes: usize) {
        self.bytes_out += bytes;
    }

    fn expire_lost(&mut self, now: Instant) {
        let lost_after = PACKET_LOST_AFTER.max(Duration::from_secs_f32(self.rtt * 4.0));
        let before = self.sent.len();
        self.sent.retain(|_, p| now.duration_since(p.at) < lost_after);
        let lost = before - self.sent.len();
        for _ in 0..lost {
            self.loss += (1.0 - self.loss) * 0.01;
        }
    }

    fn roll_bandwidth(&mut self, now: Instant) {
        let elapsed = now.duration_since(self.window_start).as_secs_f32();
        if elapsed >= 1.0 {
            self.bytes_in_rate = self.bytes_in as f32 / elapsed;
            self.bytes_out_rate = self.bytes_out as f32 / elapsed;
            self.bytes_in = 0;
            self.bytes_out = 0;
            self.window_start = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Move every packet from `a` to `b`, keeping those `keep` allows.
    fn deliver(a: &mut Connection, b: &mut Connection, now: Instant, mut keep: impl FnMut(usize) -> bool) {
        for (i, p) in a.flush(now).into_iter().enumerate() {
            if !keep(i) {
                continue;
            }
            if let Packet::Payload { seq, ack, ack_bits, messages } = p {
                b.on_payload(seq, ack, ack_bits, messages, now);
            }
        }
    }

    #[test]
    fn wrapping_order() {
        assert!(seq_newer(1, 0));
        assert!(seq_newer(0, 65535));
        assert!(!seq_newer(65535, 0));
        assert!(!seq_newer(5, 5));
    }

    #[test]
    fn reliable_messages_arrive_in_order_despite_loss() {
        let mut now = Instant::now();
        let mut a = Connection::new(now);
        let mut b = Connection::new(now);
        for i in 0..200u32 {
            a.send(Channel::Reliable, i.to_le_bytes().to_vec());
        }
        let mut got = Vec::new();
        let mut n = 0usize;
        for _ in 0..400 {
            now += Duration::from_millis(20);
            // Drop two packets in three.
            deliver(&mut a, &mut b, now, |_| {
                n += 1;
                n % 3 == 0
            });
            deliver(&mut b, &mut a, now, |_| true);
            got.extend(b.take_received().into_iter().map(|(c, d)| {
                assert_eq!(c, Channel::Reliable);
                u32::from_le_bytes(d.try_into().unwrap())
            }));
        }
        assert_eq!(got, (0..200).collect::<Vec<_>>());
        assert_eq!(a.reliable_backlog(), 0);
    }

    #[test]
    fn a_big_message_is_split_and_put_back_together() {
        let mut now = Instant::now();
        let mut a = Connection::new(now);
        let mut b = Connection::new(now);
        let big: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
        a.send(Channel::Reliable, b"before".to_vec());
        a.send(Channel::Reliable, big.clone());
        a.send(Channel::Reliable, b"after".to_vec());
        let mut got = Vec::new();
        for _ in 0..50 {
            now += Duration::from_millis(60);
            deliver(&mut a, &mut b, now, |i| i % 2 == 0);
            deliver(&mut b, &mut a, now, |_| true);
            got.extend(b.take_received().into_iter().map(|(_, d)| d));
        }
        assert_eq!(got, vec![b"before".to_vec(), big, b"after".to_vec()]);
    }

    #[test]
    fn duplicated_packets_deliver_once() {
        let now = Instant::now();
        let mut a = Connection::new(now);
        let mut b = Connection::new(now);
        a.send(Channel::Reliable, vec![1]);
        a.send(Channel::Unreliable, vec![2]);
        for p in a.flush(now) {
            if let Packet::Payload { seq, ack, ack_bits, messages } = p {
                b.on_payload(seq, ack, ack_bits, messages.clone(), now);
                b.on_payload(seq, ack, ack_bits, messages, now);
            }
        }
        assert_eq!(b.take_received().len(), 2);
    }

    #[test]
    fn an_oversized_unreliable_message_goes_reliably() {
        let now = Instant::now();
        let mut a = Connection::new(now);
        a.send(Channel::Unreliable, vec![0; MAX_UNRELIABLE_BYTES + 1]);
        assert_eq!(a.reliable_backlog(), 2);
    }

    #[test]
    fn round_trip_is_measured() {
        let mut now = Instant::now();
        let mut a = Connection::new(now);
        let mut b = Connection::new(now);
        for _ in 0..100 {
            a.send(Channel::Unreliable, vec![0]);
            let out = a.flush(now);
            now += Duration::from_millis(40);
            for p in out {
                if let Packet::Payload { seq, ack, ack_bits, messages } = p {
                    b.on_payload(seq, ack, ack_bits, messages, now);
                }
            }
            deliver(&mut b, &mut a, now, |_| true);
        }
        let rtt = a.stats().rtt_ms;
        assert!((30.0..=50.0).contains(&rtt), "rtt {rtt}");
    }
}
