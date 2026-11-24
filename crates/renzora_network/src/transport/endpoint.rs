//! The two ends of a session, with no sockets in them: a datagram goes in with
//! [`ServerEndpoint::receive`], datagrams come out of
//! [`ServerEndpoint::take_outgoing`]. The UDP driver in `udp.rs` moves bytes
//! between these and the OS; the tests move them through a simulated network
//! that loses, duplicates and reorders. Keeping the protocol free of I/O is
//! what makes those tests possible at all.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use super::connection::{Channel, Connection, ConnectionStats};
use super::packet::{decode, encode, DenyReason, Packet};

/// A peer silent this long is gone.
pub const PEER_TIMEOUT: Duration = Duration::from_secs(8);
/// How often a client repeats its handshake packet while waiting.
const HANDSHAKE_RETRY: Duration = Duration::from_millis(200);
/// How long a client tries to connect before giving up.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The reason a client gives up when the server never answered the handshake.
/// Named, because a `--connect` launch retries on exactly this one.
pub const NO_ANSWER: &str = "no answer from the server";
/// Ids up to here are reserved: 0 is the server, 1 the host's own player.
const FIRST_CLIENT_ID: u64 = 2;

fn keyed_hash(secret: u64, addr: SocketAddr, nonce: u64) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    secret.hash(&mut h);
    addr.hash(&mut h);
    nonce.hash(&mut h);
    h.finish()
}

/// A random-enough 64-bit value: the process hasher's random seed mixed with
/// the clock and a counter. Ids and nonces need to be unpredictable to a
/// stranger and unique within a session, not cryptographic.
pub fn random_u64() -> u64 {
    use std::hash::BuildHasher;
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.finish()
}

// ── Server ──────────────────────────────────────────────────────────────────

/// Something that happened on the server's side of the wire.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerEvent {
    Connected(u64),
    Disconnected(u64),
    Message(u64, Channel, Vec<u8>),
}

struct Slot {
    id: u64,
    conn: Connection,
}

pub struct ServerEndpoint {
    protocol: u64,
    max_clients: usize,
    tick_rate: u16,
    secret: u64,
    slots: HashMap<SocketAddr, Slot>,
    addr_of: HashMap<u64, SocketAddr>,
    outgoing: Vec<(SocketAddr, Vec<u8>)>,
    events: Vec<ServerEvent>,
}

impl ServerEndpoint {
    pub fn new(protocol: u64, max_clients: usize, tick_rate: u16) -> Self {
        Self {
            protocol,
            max_clients,
            tick_rate,
            secret: random_u64(),
            slots: HashMap::new(),
            addr_of: HashMap::new(),
            outgoing: Vec::new(),
            events: Vec::new(),
        }
    }

    fn reply(&mut self, to: SocketAddr, packet: &Packet) {
        self.outgoing.push((to, encode(packet)));
    }

    pub fn receive(&mut self, from: SocketAddr, bytes: &[u8], now: Instant) {
        let Some(packet) = decode(bytes) else { return };
        match packet {
            Packet::ConnectRequest { protocol, nonce } => {
                if protocol != self.protocol {
                    self.reply(from, &Packet::Denied { reason: DenyReason::ProtocolMismatch });
                    return;
                }
                if let Some(slot) = self.slots.get(&from) {
                    // Already in: our Accepted was lost.
                    let id = slot.id;
                    self.reply(from, &Packet::Accepted { client_id: id, tick_rate: self.tick_rate });
                    return;
                }
                let token = keyed_hash(self.secret, from, nonce);
                self.reply(from, &Packet::Challenge { nonce, token });
            }
            Packet::ChallengeResponse { nonce, token } => {
                if token != keyed_hash(self.secret, from, nonce) {
                    return;
                }
                if let Some(slot) = self.slots.get(&from) {
                    let id = slot.id;
                    self.reply(from, &Packet::Accepted { client_id: id, tick_rate: self.tick_rate });
                    return;
                }
                if self.slots.len() >= self.max_clients {
                    self.reply(from, &Packet::Denied { reason: DenyReason::ServerFull });
                    return;
                }
                let id = loop {
                    let candidate = random_u64();
                    if candidate >= FIRST_CLIENT_ID && !self.addr_of.contains_key(&candidate) {
                        break candidate;
                    }
                };
                self.slots.insert(from, Slot { id, conn: Connection::new(now) });
                self.addr_of.insert(id, from);
                self.events.push(ServerEvent::Connected(id));
                self.reply(from, &Packet::Accepted { client_id: id, tick_rate: self.tick_rate });
            }
            Packet::Payload { seq, ack, ack_bits, messages } => {
                let Some(slot) = self.slots.get_mut(&from) else { return };
                slot.conn.count_in(bytes.len());
                slot.conn.on_payload(seq, ack, ack_bits, messages, now);
                let id = slot.id;
                for (channel, data) in slot.conn.take_received() {
                    self.events.push(ServerEvent::Message(id, channel, data));
                }
            }
            Packet::Disconnect => {
                if let Some(slot) = self.slots.remove(&from) {
                    self.addr_of.remove(&slot.id);
                    self.events.push(ServerEvent::Disconnected(slot.id));
                }
            }
            Packet::Challenge { .. } | Packet::Accepted { .. } | Packet::Denied { .. } => {}
        }
    }

    pub fn send(&mut self, client: u64, channel: Channel, data: Vec<u8>) {
        if let Some(slot) = self.addr_of.get(&client).and_then(|a| self.slots.get_mut(a)) {
            slot.conn.send(channel, data);
        }
    }

    pub fn clients(&self) -> impl Iterator<Item = u64> + '_ {
        self.addr_of.keys().copied()
    }

    pub fn stats(&self, client: u64) -> Option<ConnectionStats> {
        self.addr_of.get(&client).and_then(|a| self.slots.get(a)).map(|s| s.conn.stats())
    }

    /// Drop a client, telling it so.
    pub fn kick(&mut self, client: u64) {
        if let Some(addr) = self.addr_of.remove(&client) {
            self.slots.remove(&addr);
            for _ in 0..3 {
                self.reply(addr, &Packet::Disconnect);
            }
            self.events.push(ServerEvent::Disconnected(client));
        }
    }

    /// Time out silent clients and build this frame's datagrams.
    pub fn update(&mut self, now: Instant) {
        let gone: Vec<u64> = self
            .slots
            .values()
            .filter(|s| s.conn.timed_out(now, PEER_TIMEOUT))
            .map(|s| s.id)
            .collect();
        for id in gone {
            if let Some(addr) = self.addr_of.remove(&id) {
                self.slots.remove(&addr);
                self.events.push(ServerEvent::Disconnected(id));
            }
        }
        for (addr, slot) in self.slots.iter_mut() {
            for packet in slot.conn.flush(now) {
                let bytes = encode(&packet);
                slot.conn.count_out(bytes.len());
                self.outgoing.push((*addr, bytes));
            }
        }
    }

    /// Say goodbye to everyone, for a clean shutdown.
    pub fn shutdown(&mut self) {
        let ids: Vec<u64> = self.clients().collect();
        for id in ids {
            self.kick(id);
        }
    }

    pub fn take_outgoing(&mut self) -> Vec<(SocketAddr, Vec<u8>)> {
        std::mem::take(&mut self.outgoing)
    }

    pub fn take_events(&mut self) -> Vec<ServerEvent> {
        std::mem::take(&mut self.events)
    }
}

// ── Client ──────────────────────────────────────────────────────────────────

/// Where a client is in its life.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientState {
    Connecting,
    Connected { client_id: u64, tick_rate: u16 },
    /// Refused, timed out or told to leave. Terminal.
    Disconnected { reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClientEvent {
    Connected { client_id: u64, tick_rate: u16 },
    Disconnected { reason: String },
    Message(Channel, Vec<u8>),
}

pub struct ClientEndpoint {
    server: SocketAddr,
    protocol: u64,
    nonce: u64,
    token: Option<u64>,
    started: Instant,
    last_handshake: Option<Instant>,
    state: ClientState,
    conn: Connection,
    outgoing: Vec<Vec<u8>>,
    events: Vec<ClientEvent>,
}

impl ClientEndpoint {
    pub fn new(server: SocketAddr, protocol: u64, now: Instant) -> Self {
        Self {
            server,
            protocol,
            nonce: random_u64(),
            token: None,
            started: now,
            last_handshake: None,
            state: ClientState::Connecting,
            conn: Connection::new(now),
            outgoing: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn server(&self) -> SocketAddr {
        self.server
    }

    pub fn state(&self) -> &ClientState {
        &self.state
    }

    pub fn stats(&self) -> ConnectionStats {
        self.conn.stats()
    }

    pub fn rtt(&self) -> f32 {
        self.conn.rtt()
    }

    pub fn receive(&mut self, from: SocketAddr, bytes: &[u8], now: Instant) {
        if from != self.server {
            return;
        }
        let Some(packet) = decode(bytes) else { return };
        match (&self.state, packet) {
            (ClientState::Connecting, Packet::Challenge { nonce, token }) if nonce == self.nonce => {
                self.token = Some(token);
                self.last_handshake = None;
            }
            (ClientState::Connecting, Packet::Accepted { client_id, tick_rate }) => {
                // Start the link's clock now, not when the handshake began, or
                // a slow handshake would look like a silent peer.
                self.conn = Connection::new(now);
                self.state = ClientState::Connected { client_id, tick_rate };
                self.events.push(ClientEvent::Connected { client_id, tick_rate });
            }
            (ClientState::Connecting, Packet::Denied { reason }) => {
                self.end(reason.to_string());
            }
            (ClientState::Connected { .. }, Packet::Payload { seq, ack, ack_bits, messages }) => {
                self.conn.count_in(bytes.len());
                self.conn.on_payload(seq, ack, ack_bits, messages, now);
                for (channel, data) in self.conn.take_received() {
                    self.events.push(ClientEvent::Message(channel, data));
                }
            }
            (ClientState::Connected { .. }, Packet::Disconnect) => {
                self.end("the server closed the connection".into());
            }
            _ => {}
        }
    }

    fn end(&mut self, reason: String) {
        if !matches!(self.state, ClientState::Disconnected { .. }) {
            self.state = ClientState::Disconnected { reason: reason.clone() };
            self.events.push(ClientEvent::Disconnected { reason });
        }
    }

    pub fn send(&mut self, channel: Channel, data: Vec<u8>) {
        if matches!(self.state, ClientState::Connected { .. }) {
            self.conn.send(channel, data);
        }
    }

    pub fn update(&mut self, now: Instant) {
        match self.state {
            ClientState::Connecting => {
                if now.duration_since(self.started) >= CONNECT_TIMEOUT {
                    self.end(NO_ANSWER.into());
                    return;
                }
                let due = self.last_handshake.is_none_or(|t| now.duration_since(t) >= HANDSHAKE_RETRY);
                if due {
                    self.last_handshake = Some(now);
                    let packet = match self.token {
                        Some(token) => Packet::ChallengeResponse { nonce: self.nonce, token },
                        None => Packet::ConnectRequest { protocol: self.protocol, nonce: self.nonce },
                    };
                    self.outgoing.push(encode(&packet));
                }
            }
            ClientState::Connected { .. } => {
                if self.conn.timed_out(now, PEER_TIMEOUT) {
                    self.end("the server stopped answering".into());
                    return;
                }
                for packet in self.conn.flush(now) {
                    let bytes = encode(&packet);
                    self.conn.count_out(bytes.len());
                    self.outgoing.push(bytes);
                }
            }
            ClientState::Disconnected { .. } => {}
        }
    }

    /// Leave, telling the server so it frees the slot at once instead of
    /// waiting out the timeout.
    pub fn disconnect(&mut self) {
        if matches!(self.state, ClientState::Connected { .. }) {
            for _ in 0..3 {
                self.outgoing.push(encode(&Packet::Disconnect));
            }
        }
        self.end("disconnected".into());
    }

    pub fn take_outgoing(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.outgoing)
    }

    pub fn take_events(&mut self) -> Vec<ClientEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A network between one server and several clients that can drop,
    /// duplicate and reorder.
    struct Net {
        now: Instant,
        server: ServerEndpoint,
        server_addr: SocketAddr,
        clients: Vec<(SocketAddr, ClientEndpoint)>,
        drop_every: usize,
        n: usize,
    }

    impl Net {
        fn new(clients: usize, max: usize) -> Self {
            let now = Instant::now();
            let server_addr: SocketAddr = "10.0.0.1:7636".parse().unwrap();
            Self {
                now,
                server: ServerEndpoint::new(7, max, 64),
                server_addr,
                clients: (0..clients)
                    .map(|i| {
                        let addr: SocketAddr = format!("10.0.0.{}:5000", i + 2).parse().unwrap();
                        (addr, ClientEndpoint::new(server_addr, 7, now))
                    })
                    .collect(),
                drop_every: 0,
                n: 0,
            }
        }

        fn lost(&mut self) -> bool {
            self.n += 1;
            self.drop_every != 0 && self.n % self.drop_every == 0
        }

        fn step(&mut self) {
            self.now += Duration::from_millis(16);
            let now = self.now;
            let mut to_server = Vec::new();
            for (addr, c) in &mut self.clients {
                c.update(now);
                for bytes in c.take_outgoing() {
                    to_server.push((*addr, bytes));
                }
            }
            // Reordered: newest first.
            to_server.reverse();
            for (addr, bytes) in to_server {
                if !self.lost() {
                    self.server.receive(addr, &bytes, now);
                }
            }
            self.server.update(now);
            for (to, bytes) in self.server.take_outgoing() {
                if self.lost() {
                    continue;
                }
                if let Some((_, c)) = self.clients.iter_mut().find(|(a, _)| *a == to) {
                    c.receive(self.server_addr, &bytes, now);
                    // And once more, as a duplicating router would.
                    c.receive(self.server_addr, &bytes, now);
                }
            }
        }
    }

    #[test]
    fn clients_connect_and_get_distinct_ids() {
        let mut net = Net::new(3, 8);
        for _ in 0..30 {
            net.step();
        }
        let mut ids: Vec<u64> = net
            .clients
            .iter()
            .map(|(_, c)| match c.state() {
                ClientState::Connected { client_id, .. } => *client_id,
                other => panic!("not connected: {other:?}"),
            })
            .collect();
        assert!(ids.iter().all(|id| *id >= FIRST_CLIENT_ID));
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn a_full_server_says_so() {
        let mut net = Net::new(2, 1);
        for _ in 0..30 {
            net.step();
        }
        let refused = net
            .clients
            .iter()
            .filter(|(_, c)| matches!(c.state(), ClientState::Disconnected { reason } if reason.contains("full")))
            .count();
        assert_eq!(refused, 1);
    }

    #[test]
    fn a_different_game_is_refused() {
        let now = Instant::now();
        let server_addr: SocketAddr = "10.0.0.1:7636".parse().unwrap();
        let mut server = ServerEndpoint::new(1, 8, 64);
        let mut client = ClientEndpoint::new(server_addr, 2, now);
        client.update(now);
        let me: SocketAddr = "10.0.0.9:1".parse().unwrap();
        for b in client.take_outgoing() {
            server.receive(me, &b, now);
        }
        for (_, b) in server.take_outgoing() {
            client.receive(server_addr, &b, now);
        }
        assert!(matches!(client.state(), ClientState::Disconnected { .. }));
    }

    #[test]
    fn messages_flow_both_ways_on_a_lossy_link_and_leaving_frees_the_slot() {
        let mut net = Net::new(1, 8);
        net.drop_every = 4;
        for _ in 0..60 {
            net.step();
        }
        let id = match net.clients[0].1.state() {
            ClientState::Connected { client_id, .. } => *client_id,
            s => panic!("{s:?}"),
        };
        for i in 0..50u8 {
            net.clients[0].1.send(Channel::Reliable, vec![i]);
            net.server.send(id, Channel::Reliable, vec![100 + i]);
        }
        let mut at_server = Vec::new();
        let mut at_client = Vec::new();
        for _ in 0..200 {
            net.step();
            for e in net.server.take_events() {
                if let ServerEvent::Message(from, Channel::Reliable, d) = e {
                    assert_eq!(from, id);
                    at_server.push(d[0]);
                }
            }
            for e in net.clients[0].1.take_events() {
                if let ClientEvent::Message(Channel::Reliable, d) = e {
                    at_client.push(d[0]);
                }
            }
        }
        assert_eq!(at_server, (0..50).collect::<Vec<_>>());
        assert_eq!(at_client, (100..150).collect::<Vec<_>>());

        net.drop_every = 0;
        net.clients[0].1.disconnect();
        net.step();
        assert!(net.server.take_events().contains(&ServerEvent::Disconnected(id)));
        assert_eq!(net.server.clients().count(), 0);
    }

    #[test]
    fn a_silent_client_times_out() {
        let mut net = Net::new(1, 8);
        for _ in 0..30 {
            net.step();
        }
        net.server.take_events();
        net.clients.clear();
        for _ in 0..(PEER_TIMEOUT.as_millis() / 16 + 5) {
            net.step();
        }
        assert!(matches!(net.server.take_events().as_slice(), [ServerEvent::Disconnected(_)]));
    }
}
