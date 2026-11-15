//! Finding games to profile, and the connection to the one being profiled.
//!
//! Games announce themselves with a UDP beacon once a second (see
//! `renzora_profiler::server`). A thread listens for those on `BEACON_PORT`
//! and hands each to the server, which keeps every game heard recently, on
//! this machine or the network. Connecting opens a TCP stream to the one
//! chosen, presents the pairing key, and hands every packet the game sends to
//! the server as an [`Event::Profile`], which keeps it in the [`Capture`].
//!
//! Ported from the engine companion's `app::games`; the Bevy resources became
//! [`Profiling`], which the server thread owns.

use std::collections::BTreeMap;
use std::io::Write;
use std::net::{IpAddr, Shutdown, SocketAddr, TcpStream, UdpSocket};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use renzora_companion_proto::wire::{self, ControlPacket, GamePacket, BEACON_MAGIC, BEACON_PORT};
use serde::{Deserialize, Serialize};

use crate::capture::Capture;
use crate::log;
use crate::server::Event;

/// A game not heard from for this long has gone (closed, or left the network).
const FORGET_AFTER: Duration = Duration::from_secs(4);

/// A game that announced itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct GameInfo {
    pub address: SocketAddr,
    pub name: String,
    pub platform: String,
    pub build: String,
    pub pid: u32,
    /// On this machine rather than the network.
    pub local: bool,
}

impl GameInfo {
    /// A short line for lists: "this PC" or the address, and the platform.
    pub(crate) fn place(&self) -> String {
        if self.local {
            format!("this PC · {}", self.platform)
        } else {
            format!("{} · {}", self.address.ip(), self.platform)
        }
    }
}

/// How the connection to the profiled game stands.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) enum ConnState {
    #[default]
    Idle,
    Connecting,
    Connected,
    Failed(String),
}

/// What the reader thread of a connection reports.
pub(crate) enum Incoming {
    Packet(GamePacket),
    Failed(String),
    Closed,
}

/// Everything about profiling: the games heard, the connection, the capture.
pub(crate) struct Profiling {
    /// What this companion presents to every game it profiles.
    key: String,
    pub games: BTreeMap<SocketAddr, (GameInfo, Instant)>,
    pub game: Option<GameInfo>,
    pub state: ConnState,
    /// Whether this companion has asked the game to record.
    pub recording: bool,
    writer: Option<TcpStream>,
    /// Counts connections, so packets still arriving from one that was
    /// replaced are recognised and dropped.
    serial: u64,
    pub capture: Capture,
    /// Counts captures: a new game or an opened file replaces the capture,
    /// and the window starts its copy over when this moves.
    pub generation: u64,
}

impl Default for Profiling {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl Profiling {
    pub(crate) fn new(key: String) -> Self {
        Self {
            key,
            games: BTreeMap::new(),
            game: None,
            state: ConnState::Idle,
            recording: false,
            writer: None,
            serial: 0,
            capture: Capture::default(),
            generation: 0,
        }
    }

    pub(crate) fn heard(&mut self, info: GameInfo) {
        self.games.insert(info.address, (info, Instant::now()));
    }

    /// Drop games not heard from lately. True when the list changed.
    pub(crate) fn forget_quiet(&mut self) -> bool {
        let before = self.games.len();
        let now = Instant::now();
        self.games.retain(|_, (_, seen)| now.duration_since(*seen) < FORGET_AFTER);
        self.games.len() != before
    }

    /// Connect to `game`, starting a new capture. Recording starts by itself
    /// once the game answers.
    pub(crate) fn connect(&mut self, game: GameInfo, events: &Sender<Event>) {
        self.disconnect();
        self.capture.reset(&game.name, None);
        self.generation += 1;
        self.serial += 1;
        match open(&game, &self.key, self.serial, events) {
            Ok(writer) => {
                self.writer = Some(writer);
                self.state = ConnState::Connecting;
            }
            Err(e) => self.state = ConnState::Failed(e),
        }
        self.game = Some(game);
    }

    pub(crate) fn disconnect(&mut self) {
        if let Some(writer) = self.writer.take() {
            let _ = writer.shutdown(Shutdown::Both);
        }
        self.game = None;
        self.recording = false;
        self.state = ConnState::Idle;
    }

    pub(crate) fn record(&mut self, on: bool) {
        self.recording = on && self.writer.is_some();
        self.send(&ControlPacket::Record(on));
    }

    pub(crate) fn watch(&mut self, ids: Vec<String>) {
        self.send(&ControlPacket::Watch(ids));
    }

    pub(crate) fn probe_act(&mut self, probe: String, item: String, value: Option<bool>) {
        self.send(&ControlPacket::ProbeAct { probe, item, value });
    }

    /// Replace the capture with one saved earlier, dropping any connection.
    pub(crate) fn open_capture(&mut self, path: &Path) -> String {
        match Capture::open(path) {
            Ok(mut opened) => {
                self.disconnect();
                opened.revision = self.capture.revision + 1;
                self.capture = opened;
                self.generation += 1;
                format!("Opened {}", path.display())
            }
            Err(e) => format!("Could not open {}: {e}", path.display()),
        }
    }

    fn send(&mut self, packet: &ControlPacket) {
        if let Some(writer) = self.writer.as_mut() {
            if let Err(e) = writer.write_all(&packet.encode()) {
                log::warn(format!("could not reach the profiled game: {e}"));
            }
        }
    }

    /// Something arrived from connection `serial`. Returns the packet when it
    /// is one for the capture, for the caller to keep (and pass on to the
    /// window); a stale connection's packets are dropped here.
    pub(crate) fn incoming(&mut self, serial: u64, incoming: Incoming) -> Option<GamePacket> {
        if serial != self.serial {
            return None;
        }
        match incoming {
            Incoming::Packet(GamePacket::Hello { .. }) => {
                self.state = ConnState::Connected;
                // Connecting to a game is asking to see it.
                self.record(true);
                None
            }
            Incoming::Packet(packet) => Some(packet),
            Incoming::Failed(why) => {
                self.state = ConnState::Failed(why);
                self.writer = None;
                self.recording = false;
                None
            }
            Incoming::Closed => {
                // Idle, not failed: a game that closed did nothing wrong, and
                // the capture stays for looking at.
                self.state = ConnState::Idle;
                self.writer = None;
                self.recording = false;
                None
            }
        }
    }
}

/// Listen for beacons on a thread, handing each to the server.
pub(crate) fn listen(events: Sender<Event>) {
    let socket = match UdpSocket::bind(("0.0.0.0", BEACON_PORT)) {
        Ok(s) => s,
        Err(e) => {
            log::warn(format!("cannot hear games on port {BEACON_PORT}: {e}; another companion may be running"));
            return;
        }
    };
    let spawned = std::thread::Builder::new().name("companion-beacons".into()).spawn(move || {
        let mut buf = [0u8; 1024];
        while let Ok((len, from)) = socket.recv_from(&mut buf) {
            let Some(info) = parse_beacon(&buf[..len], from.ip()) else { continue };
            if events.send(Event::Heard(info)).is_err() {
                return;
            }
        }
    });
    if let Err(e) = spawned {
        log::warn(format!("cannot listen for games: {e}"));
    }
}

/// Read a beacon: magic, port, name, platform, build, pid, one per line.
pub(crate) fn parse_beacon(bytes: &[u8], from: IpAddr) -> Option<GameInfo> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.lines();
    if lines.next()? != BEACON_MAGIC {
        return None;
    }
    let port: u16 = lines.next()?.parse().ok()?;
    let name = lines.next()?.to_string();
    let platform = lines.next()?.to_string();
    let build = lines.next()?.to_string();
    let pid = lines.next()?.parse().ok()?;
    Some(GameInfo { address: SocketAddr::new(from, port), name, platform, build, pid, local: from.is_loopback() })
}

/// Connect, present the key, and read packets on a thread.
fn open(game: &GameInfo, key: &str, serial: u64, events: &Sender<Event>) -> Result<TcpStream, String> {
    let mut stream =
        TcpStream::connect_timeout(&game.address, Duration::from_secs(3)).map_err(|e| format!("could not connect: {e}"))?;
    let _ = stream.set_nodelay(true);
    stream
        .write_all(&ControlPacket::Hello { version: wire::VERSION, key: key.to_string() }.encode())
        .map_err(|e| format!("could not send the key: {e}"))?;
    let mut reader = stream.try_clone().map_err(|e| e.to_string())?;
    let tx = events.clone();
    let send = move |incoming| tx.send(Event::Profile(serial, incoming)).is_ok();
    std::thread::Builder::new()
        .name("companion-profile".into())
        .spawn(move || {
            // A game that rejects the key closes without a word; the hello is
            // what tells the two apart.
            let _ = reader.set_read_timeout(Some(Duration::from_secs(6)));
            let first = GamePacket::read(&mut reader);
            let _ = reader.set_read_timeout(None);
            match first {
                Ok(Some(GamePacket::Hello { version, .. })) if version != wire::VERSION => {
                    send(Incoming::Failed(format!(
                        "it was built with a different engine (stream {version}, this companion {})",
                        wire::VERSION
                    )));
                    return;
                }
                Ok(Some(hello @ GamePacket::Hello { .. })) => {
                    if !send(Incoming::Packet(hello)) {
                        return;
                    }
                }
                _ => {
                    send(Incoming::Failed(
                        "it did not accept this editor's key; it was exported from another editor".into(),
                    ));
                    return;
                }
            }
            loop {
                match GamePacket::read(&mut reader) {
                    Ok(Some(packet)) => {
                        if !send(Incoming::Packet(packet)) {
                            return;
                        }
                    }
                    Ok(None) => break,
                    // A game that exits usually resets the socket rather than
                    // closing it: that is the game closing, not a failure.
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::ConnectionAborted
                                | std::io::ErrorKind::UnexpectedEof
                        ) =>
                    {
                        break
                    }
                    Err(e) => {
                        send(Incoming::Failed(e.to_string()));
                        return;
                    }
                }
            }
            send(Incoming::Closed);
        })
        .map_err(|e| e.to_string())?;
    Ok(stream)
}

/// The pairing key: `--profiling-key` (the editor always passes its machine
/// key), or else the machine key file the editor keeps.
pub(crate) fn pairing_key() -> String {
    let args: Vec<String> = std::env::args().collect();
    if let Some(key) = args.iter().position(|a| a == "--profiling-key").and_then(|i| args.get(i + 1)) {
        return key.clone();
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| std::path::PathBuf::from(home).join(".renzora").join("profiling_key"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|k| k.trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_beacon_reads_back() {
        let text = format!("{BEACON_MAGIC}\n51234\nforest_demo\nandroid\nr1-alpha8\n4242\n");
        let info = parse_beacon(text.as_bytes(), IpAddr::from([192, 168, 1, 42])).unwrap();
        assert_eq!(info.address, SocketAddr::from(([192, 168, 1, 42], 51234)));
        assert_eq!(info.name, "forest_demo");
        assert_eq!(info.platform, "android");
        assert_eq!(info.pid, 4242);
        assert!(!info.local);
        assert_eq!(info.place(), "192.168.1.42 · android");
    }

    #[test]
    fn stray_packets_are_not_games() {
        assert!(parse_beacon(b"hello", IpAddr::from([127, 0, 0, 1])).is_none());
        let short = format!("{BEACON_MAGIC}\n51234\n");
        assert!(parse_beacon(short.as_bytes(), IpAddr::from([127, 0, 0, 1])).is_none());
    }

    #[test]
    fn a_replaced_connection_is_not_heard() {
        let mut profiling = Profiling::new(String::new());
        profiling.serial = 2;
        assert!(profiling.incoming(1, Incoming::Packet(GamePacket::Names(Vec::new()))).is_none());
        assert!(profiling.incoming(2, Incoming::Packet(GamePacket::Names(Vec::new()))).is_some());
    }
}
