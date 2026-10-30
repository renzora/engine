//! Finding games to profile, and the connection to the one being profiled.
//!
//! Games announce themselves with a UDP beacon once a second (see
//! `renzora_profiler::server`). This listens for those beacons on
//! `BEACON_PORT` and keeps [`Games`]: every game heard recently, on this
//! machine or the network. Connecting opens a TCP stream to the one chosen,
//! presents the [`PairingKey`], and hands every packet it sends to
//! [`super::capture`], waking the window as each arrives: the companion
//! otherwise sleeps between redraws to stay out of the way.

use std::collections::BTreeMap;
use std::io::Write;
use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::winit::{EventLoopProxy, EventLoopProxyWrapper, WinitUserEvent};
use renzora_profiler::server::{BEACON_MAGIC, BEACON_PORT};
use renzora_profiler::wire::{self, ControlPacket, GamePacket};

use super::PairingKey;

/// A game not heard from for this long has gone (closed, or left the network).
const FORGET_AFTER: Duration = Duration::from_secs(4);

/// A game that announced itself.
#[derive(Clone, Debug, PartialEq)]
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
    pub fn place(&self) -> String {
        if self.local {
            format!("this PC · {}", self.platform)
        } else {
            format!("{} · {}", self.address.ip(), self.platform)
        }
    }
}

/// Every game heard from recently, keyed by where to connect.
#[derive(Resource, Default)]
pub(crate) struct Games {
    pub found: BTreeMap<SocketAddr, (GameInfo, Instant)>,
    beacons: Option<Mutex<Receiver<GameInfo>>>,
}

/// The game being profiled, if any.
#[derive(Resource, Default)]
pub(crate) struct Connection {
    pub game: Option<GameInfo>,
    pub state: ConnState,
    writer: Option<TcpStream>,
    incoming: Option<Mutex<Receiver<Incoming>>>,
    /// Whether this companion has asked the game to record.
    pub recording: bool,
}

#[derive(Default, Clone, Debug, PartialEq)]
pub(crate) enum ConnState {
    #[default]
    Idle,
    Connecting,
    Connected,
    Failed(String),
}

pub(crate) enum Incoming {
    Packet(GamePacket),
    Failed(String),
    Closed,
}

/// Something asking for a connection change, from the UI.
#[derive(Message, Clone)]
pub(crate) enum ConnRequest {
    Connect(GameInfo),
    Disconnect,
    Record(bool),
    /// The probes the Debug screen is showing.
    Watch(Vec<String>),
    /// A probe's toggle or button was used.
    ProbeAct { probe: String, item: String, value: Option<bool> },
}

pub(crate) fn build(app: &mut App) {
    app.init_resource::<Games>()
        .init_resource::<Connection>()
        .add_message::<ConnRequest>()
        .add_systems(Startup, listen)
        .add_systems(Update, (track_games, handle_requests, pump_connection).chain());
}

/// Listen for beacons on a thread, waking the window for each new one.
fn listen(mut games: ResMut<Games>, proxy: Option<Res<EventLoopProxyWrapper>>) {
    let socket = match UdpSocket::bind(("0.0.0.0", BEACON_PORT)) {
        Ok(s) => s,
        Err(e) => {
            warn!("[companion] cannot hear games on port {BEACON_PORT}: {e}; another companion may be running");
            return;
        }
    };
    let proxy: Option<EventLoopProxy<WinitUserEvent>> = proxy.map(|p| (**p).clone());
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("companion-beacons".into())
        .spawn(move || {
            let mut buf = [0u8; 1024];
            while let Ok((len, from)) = socket.recv_from(&mut buf) {
                let Some(info) = parse_beacon(&buf[..len], from.ip()) else { continue };
                if tx.send(info).is_err() {
                    return;
                }
                if let Some(proxy) = &proxy {
                    let _ = proxy.send_event(WinitUserEvent::WakeUp);
                }
            }
        })
        .ok();
    games.beacons = Some(Mutex::new(rx));
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
    Some(GameInfo {
        address: SocketAddr::new(from, port),
        name,
        platform,
        build,
        pid,
        local: from.is_loopback(),
    })
}

fn track_games(mut games: ResMut<Games>) {
    let heard: Vec<GameInfo> = match games.beacons.as_ref().and_then(|b| b.lock().ok()) {
        Some(rx) => rx.try_iter().collect(),
        None => Vec::new(),
    };
    let now = Instant::now();
    for info in heard {
        games.found.insert(info.address, (info, now));
    }
    games.found.retain(|_, (_, seen)| now.duration_since(*seen) < FORGET_AFTER);
}

fn handle_requests(
    mut requests: MessageReader<ConnRequest>,
    mut conn: ResMut<Connection>,
    key: Res<PairingKey>,
    proxy: Option<Res<EventLoopProxyWrapper>>,
    mut capture: ResMut<super::capture::Capture>,
) {
    for request in requests.read() {
        match request.clone() {
            ConnRequest::Connect(game) => {
                disconnect(&mut conn);
                capture.start(&game);
                let proxy = proxy.as_ref().map(|p| (***p).clone());
                match open(&game, &key.0, proxy) {
                    Ok((writer, rx)) => {
                        conn.writer = Some(writer);
                        conn.incoming = Some(Mutex::new(rx));
                        conn.state = ConnState::Connecting;
                    }
                    Err(e) => conn.state = ConnState::Failed(e),
                }
                conn.game = Some(game);
            }
            ConnRequest::Disconnect => disconnect(&mut conn),
            ConnRequest::Record(on) => {
                conn.recording = on;
                if let Some(writer) = conn.writer.as_mut() {
                    let _ = writer.write_all(&ControlPacket::Record(on).encode());
                }
            }
            ConnRequest::Watch(ids) => {
                if let Some(writer) = conn.writer.as_mut() {
                    let _ = writer.write_all(&ControlPacket::Watch(ids).encode());
                }
            }
            ConnRequest::ProbeAct { probe, item, value } => {
                if let Some(writer) = conn.writer.as_mut() {
                    let _ = writer.write_all(&ControlPacket::ProbeAct { probe, item, value }.encode());
                }
            }
        }
    }
}

fn disconnect(conn: &mut Connection) {
    if let Some(writer) = conn.writer.take() {
        let _ = writer.shutdown(std::net::Shutdown::Both);
    }
    conn.incoming = None;
    conn.game = None;
    conn.recording = false;
    conn.state = ConnState::Idle;
}

/// Connect, present the key, and read packets on a thread.
fn open(
    game: &GameInfo,
    key: &str,
    proxy: Option<EventLoopProxy<WinitUserEvent>>,
) -> Result<(TcpStream, Receiver<Incoming>), String> {
    let mut stream = TcpStream::connect_timeout(&game.address, Duration::from_secs(3))
        .map_err(|e| format!("could not connect: {e}"))?;
    let _ = stream.set_nodelay(true);
    stream
        .write_all(&ControlPacket::Hello { version: wire::VERSION, key: key.to_string() }.encode())
        .map_err(|e| format!("could not send the key: {e}"))?;
    let mut reader = stream.try_clone().map_err(|e| e.to_string())?;
    let (tx, rx) = channel();
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
                    let _ = tx.send(Incoming::Failed(format!(
                        "it was built with a different engine (stream {version}, this companion {})",
                        wire::VERSION
                    )));
                    return;
                }
                Ok(Some(hello @ GamePacket::Hello { .. })) => {
                    let _ = tx.send(Incoming::Packet(hello));
                }
                _ => {
                    let _ = tx.send(Incoming::Failed(
                        "it did not accept this editor's key; it was exported from another editor".into(),
                    ));
                    return;
                }
            }
            loop {
                match GamePacket::read(&mut reader) {
                    Ok(Some(packet)) => {
                        if tx.send(Incoming::Packet(packet)).is_err() {
                            return;
                        }
                        if let Some(proxy) = &proxy {
                            let _ = proxy.send_event(WinitUserEvent::WakeUp);
                        }
                    }
                    Ok(None) => break,
                    // A game that exits (Stop, or its window closed) usually
                    // resets the socket rather than closing it: that is the
                    // game closing, not something failing, and was reported as
                    // "An existing connection was forcibly closed".
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
                        let _ = tx.send(Incoming::Failed(e.to_string()));
                        return;
                    }
                }
            }
            let _ = tx.send(Incoming::Closed);
            if let Some(proxy) = &proxy {
                let _ = proxy.send_event(WinitUserEvent::WakeUp);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok((stream, rx))
}

/// Hand what arrived to the capture and follow the connection's state.
fn pump_connection(
    mut conn: ResMut<Connection>,
    mut capture: ResMut<super::capture::Capture>,
    queue: Res<super::jobs::JobQueue>,
) {
    // A frame that arrives while a job runs shared the machine with it. The
    // game's clock is not ours, so arrival is the measure: a local stream
    // arrives within a millisecond, so it marks the right frames.
    capture.companion_busy = queue.busy();
    let arrived: Vec<Incoming> = match conn.incoming.as_ref().and_then(|rx| rx.lock().ok()) {
        Some(rx) => rx.try_iter().collect(),
        None => return,
    };
    for incoming in arrived {
        match incoming {
            Incoming::Packet(GamePacket::Hello { .. }) => {
                conn.state = ConnState::Connected;
                // Ask straight away: connecting to a game is asking to see it.
                let recording = true;
                conn.recording = recording;
                if let Some(writer) = conn.writer.as_mut() {
                    let _ = writer.write_all(&ControlPacket::Record(recording).encode());
                }
            }
            Incoming::Packet(packet) => capture.receive(packet),
            Incoming::Failed(why) => {
                conn.state = ConnState::Failed(why);
                conn.writer = None;
                conn.recording = false;
            }
            Incoming::Closed => {
                // Idle, not failed: a game that closed did nothing wrong, and
                // the capture stays for looking at.
                conn.state = ConnState::Idle;
                conn.writer = None;
                conn.recording = false;
            }
        }
    }
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
}
