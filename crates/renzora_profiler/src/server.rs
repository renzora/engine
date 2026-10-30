//! The game's side of profiling: announce, accept a companion, stream to it.
//!
//! Runs only while [`ProfilingAccess`] exists. Then:
//!
//! - **Announce.** Once a second a small UDP beacon goes to [`BEACON_PORT`] on
//!   this machine (and to the whole network when `lan` is set), saying which
//!   TCP port to connect to. The companion lists every game it hears.
//! - **Accept.** A companion connects and its first packet must carry the key.
//!   Anything else is dropped. The key never goes in the beacon.
//! - **Stream.** While any connected companion asks to record, recording is on,
//!   and each frame's profile goes to each of them, names and thread names
//!   first. Every client has its own writer thread and a bounded queue, so a
//!   slow companion (a phone on bad wifi) loses frames instead of stalling the
//!   game.

use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::prelude::*;
use renzora::profiling::{self, ProfilingAccess};

use crate::wire::{self, ControlPacket, GamePacket};
use crate::{ProfileFrame, Profiler};

/// Where companions listen for beacons.
pub const BEACON_PORT: u16 = 47_810;

/// The first line of every beacon, so a stray packet on the port is ignored.
pub const BEACON_MAGIC: &str = "RZPROF1";

/// Packets waiting for a client's writer. About two seconds at 60 fps; past
/// that the client is too slow and frames are dropped for it.
const QUEUE: usize = 128;

/// What a writer thread is handed. A frame goes over as the frame itself and
/// is encoded on the writer: encoding one is about 0.2 ms at the editor's load,
/// which on the game's main thread is frame time the profile would then show.
enum Outgoing {
    Packet(Vec<u8>),
    Frame(Arc<ProfileFrame>),
}

/// The running server: the listener's output and the connected companions.
#[derive(Resource)]
pub struct Server {
    /// The TCP port companions connect to; 0 if none could be opened.
    pub port: u16,
    accepted: Mutex<Receiver<TcpStream>>,
    clients: Vec<Client>,
    /// Frames dropped for slow clients, for the log.
    dropped: u64,
}

struct Client {
    queue: SyncSender<Outgoing>,
    /// Whether this companion asked to record; set by its reader thread.
    recording: Arc<AtomicBool>,
    /// Cleared by either thread when the connection ends.
    alive: Arc<AtomicBool>,
    next_frame: u64,
    names_sent: u32,
    threads_sent: u32,
    /// The probes this companion is showing; set by its reader thread.
    watching: Arc<Mutex<Vec<String>>>,
    /// Probe controls it used, waiting for the main thread.
    acts: Arc<Mutex<Vec<(String, String, Option<bool>)>>>,
    /// How many probes it has been told about, so a probe registered late
    /// (a plugin loaded after connecting) is announced.
    probes_sent: usize,
}

/// How often a watched probe's report is rebuilt and sent. A readout people
/// read, not a graph that must be smooth; three times a second keeps the
/// probes' cost to the game negligible.
const PROBE_INTERVAL: Duration = Duration::from_millis(330);

/// Start serving once access exists. Runs every frame but returns at once
/// when there is no access or the server is up.
pub(crate) fn start_server(
    mut commands: Commands,
    access: Option<Res<ProfilingAccess>>,
    server: Option<Res<Server>>,
) {
    let Some(access) = access else { return };
    if server.is_some() {
        return;
    }
    let host = if access.lan { "0.0.0.0" } else { "127.0.0.1" };
    let listener = match TcpListener::bind((host, 0)) {
        Ok(l) => l,
        Err(e) => {
            warn!("[profiler] could not open a port for the companion: {e}");
            // Inserted anyway, empty, so this is not retried every frame.
            commands.insert_resource(Server {
                port: 0,
                accepted: Mutex::new(channel().1),
                clients: Vec::new(),
                dropped: 0,
            });
            return;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let (tx, rx) = channel();
    let key = access.key.clone();
    std::thread::Builder::new()
        .name("profiler-accept".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if accepts(&stream, &key) && tx.send(stream).is_err() {
                    break;
                }
            }
        })
        .ok();
    announce(port, access.lan, access.name.clone());
    info!(
        "[profiler] ready for the companion on port {port}{}",
        if access.lan { " (network)" } else { "" }
    );
    commands.insert_resource(Server {
        port,
        accepted: Mutex::new(rx),
        clients: Vec::new(),
        dropped: 0,
    });
}

/// Whether the connection's first packet carries the key and our version.
fn accepts(stream: &TcpStream, key: &str) -> bool {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Ok(mut reader) = stream.try_clone() else { return false };
    let ok = matches!(
        ControlPacket::read(&mut reader),
        Ok(Some(ControlPacket::Hello { version, key: k })) if version == wire::VERSION && k == key
    );
    let _ = stream.set_read_timeout(None);
    ok
}

/// The beacon, once a second, for the life of the process.
fn announce(port: u16, lan: bool, name: String) {
    let beacon = format!(
        "{BEACON_MAGIC}\n{port}\n{name}\n{}\n{}\n{}\n",
        std::env::consts::OS,
        renzora::version::ENGINE_VERSION,
        std::process::id()
    );
    std::thread::Builder::new()
        .name("profiler-beacon".into())
        .spawn(move || {
            let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)) else { return };
            if lan {
                let _ = socket.set_broadcast(true);
            }
            loop {
                let _ = socket.send_to(beacon.as_bytes(), ("127.0.0.1", BEACON_PORT));
                if lan {
                    let _ = socket.send_to(beacon.as_bytes(), ("255.255.255.255", BEACON_PORT));
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        })
        .ok();
}

/// Take in new companions, follow what they ask for, and send each the frames
/// it has not had. After `collect_frame`, so the newest frame goes out at once.
pub(crate) fn stream(server: Option<ResMut<Server>>, profiler: Res<Profiler>, access: Option<Res<ProfilingAccess>>) {
    let Some(mut server) = server else { return };
    let server = &mut *server;

    let fresh: Vec<TcpStream> = match server.accepted.lock() {
        Ok(rx) => rx.try_iter().collect(),
        Err(_) => Vec::new(),
    };
    for stream in fresh {
        if let Some(client) = connect(stream, access.as_deref(), &profiler) {
            server.clients.push(client);
        }
    }
    server.clients.retain(|c| c.alive.load(Ordering::Relaxed));

    let recording = server.clients.iter().any(|c| c.recording.load(Ordering::Relaxed));
    if recording != profiling::is_recording() {
        profiling::set_recording(recording);
    }
    if !recording {
        return;
    }

    let name_count = profiling::name_count();
    let thread_count = profiling::thread_count();
    let mut dropped = 0;
    for client in server.clients.iter_mut().filter(|c| c.recording.load(Ordering::Relaxed)) {
        let mut send = |packet: Outgoing| match client.queue.try_send(packet) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                dropped += 1;
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        };
        if client.names_sent < name_count {
            if !send(Outgoing::Packet(GamePacket::Names(profiling::names_from(client.names_sent)).encode())) {
                continue;
            }
            client.names_sent = name_count;
        }
        if client.threads_sent < thread_count {
            let threads = (client.threads_sent..thread_count)
                .map(|id| (id, profiling::thread_name(id).unwrap_or_default()))
                .collect();
            if !send(Outgoing::Packet(GamePacket::Threads(threads).encode())) {
                continue;
            }
            client.threads_sent = thread_count;
        }
        for frame in profiler.shared_since(client.next_frame) {
            client.next_frame = frame.index + 1;
            // A frame lost to a full queue is skipped, not retried: the next
            // one is newer and just as useful.
            send(Outgoing::Frame(frame.clone()));
        }
    }
    if dropped > 0 {
        server.dropped += dropped;
        if server.dropped.is_power_of_two() {
            warn!("[profiler] {} frames dropped for a slow companion so far", server.dropped);
        }
    }
}

/// Greet a companion that passed the key check and start its threads.
fn connect(stream: TcpStream, access: Option<&ProfilingAccess>, profiler: &Profiler) -> Option<Client> {
    let mut writer = stream.try_clone().ok()?;
    let _ = stream.set_nodelay(true);
    let hello = GamePacket::Hello {
        version: wire::VERSION,
        name: access.map(|a| a.name.clone()).unwrap_or_default(),
        platform: std::env::consts::OS.to_string(),
        build: renzora::version::ENGINE_VERSION.to_string(),
    };
    wire::write(&mut writer, &hello.encode()).ok()?;

    let alive = Arc::new(AtomicBool::new(true));
    let recording = Arc::new(AtomicBool::new(false));
    let (queue, packets) = sync_channel::<Outgoing>(QUEUE);

    let writer_alive = alive.clone();
    std::thread::Builder::new()
        .name("profiler-send".into())
        .spawn(move || {
            for packet in packets {
                let bytes = match packet {
                    Outgoing::Packet(bytes) => bytes,
                    Outgoing::Frame(frame) => GamePacket::encode_frame(&frame),
                };
                if wire::write(&mut writer, &bytes).is_err() {
                    break;
                }
            }
            writer_alive.store(false, Ordering::Relaxed);
        })
        .ok()?;

    let reader_alive = alive.clone();
    let reader_recording = recording.clone();
    let watching = Arc::new(Mutex::new(Vec::new()));
    let acts = Arc::new(Mutex::new(Vec::new()));
    let (reader_watching, reader_acts) = (watching.clone(), acts.clone());
    let mut reader = stream;
    std::thread::Builder::new()
        .name("profiler-control".into())
        .spawn(move || {
            loop {
                match ControlPacket::read(&mut reader) {
                    Ok(Some(ControlPacket::Record(on))) => reader_recording.store(on, Ordering::Relaxed),
                    Ok(Some(ControlPacket::Watch(ids))) => {
                        if let Ok(mut w) = reader_watching.lock() {
                            *w = ids;
                        }
                    }
                    Ok(Some(ControlPacket::ProbeAct { probe, item, value })) => {
                        if let Ok(mut a) = reader_acts.lock() {
                            a.push((probe, item, value));
                        }
                    }
                    Ok(Some(ControlPacket::Hello { .. })) => {}
                    Ok(None) | Err(_) => break,
                }
            }
            reader_alive.store(false, Ordering::Relaxed);
            reader_recording.store(false, Ordering::Relaxed);
            if let Ok(mut w) = reader_watching.lock() {
                w.clear();
            }
        })
        .ok()?;

    info!("[profiler] a companion connected");
    Some(Client {
        queue,
        recording,
        alive,
        // Only frames from now on: history recorded for someone else is not
        // what a companion that just arrived is looking at.
        next_frame: profiler.frames().next_back().map_or(0, |f| f.index + 1),
        names_sent: 0,
        threads_sent: 0,
        watching,
        acts,
        probes_sent: 0,
    })
}

/// Probes: tell each companion what there is, keep [`WatchedProbes`] to what
/// they are showing (which is what lets probes' data systems run only then),
/// apply the controls they used, and send watched probes' reports every
/// [`PROBE_INTERVAL`].
///
/// Exclusive, because a probe builds its report from the whole `World`.
pub(crate) fn serve_probes(world: &mut World, mut last: Local<Option<std::time::Instant>>) {
    use renzora::probes::{ProbeRegistry, WatchedProbes};
    let Some(registry) = world.get_resource::<ProbeRegistry>().cloned() else { return };
    let Some(mut server) = world.remove_resource::<Server>() else { return };

    // Announce the list to companions that have not had all of it.
    let count = registry.0.len();
    for client in server.clients.iter_mut().filter(|c| c.probes_sent != count) {
        let list = registry
            .0
            .iter()
            .map(|p| wire::ProbeInfo {
                id: p.id.to_string(),
                title: p.title.to_string(),
                icon: p.icon.to_string(),
                category: p.category.to_string(),
            })
            .collect();
        if client.queue.try_send(Outgoing::Packet(GamePacket::Probes(list).encode())).is_ok() {
            client.probes_sent = count;
        }
    }

    // What anyone is watching.
    let mut watched = std::collections::HashSet::new();
    let mut acts = Vec::new();
    for client in &server.clients {
        if let Ok(w) = client.watching.lock() {
            watched.extend(w.iter().cloned());
        }
        if let Ok(mut a) = client.acts.lock() {
            acts.append(&mut a);
        }
    }
    let changed = world.get_resource::<WatchedProbes>().is_none_or(|w| w.0 != watched);
    if changed {
        world.insert_resource(WatchedProbes(watched.clone()));
    }

    for (probe, item, value) in acts {
        if let Some(act) = registry.0.iter().find(|p| p.id == probe).and_then(|p| p.act) {
            act(world, &item, value);
        }
    }

    // Reports, at the interval; at once when the watched set changes so a
    // newly opened probe is not blank for a third of a second.
    let now = std::time::Instant::now();
    let due = changed || last.is_none_or(|t| now.duration_since(t) >= PROBE_INTERVAL);
    if due && !watched.is_empty() {
        *last = Some(now);
        for probe in registry.0.iter().filter(|p| watched.contains(p.id)) {
            let report = (probe.build)(world);
            let Ok(json) = serde_json::to_string(&report) else { continue };
            let packet = GamePacket::ProbeReport { id: probe.id.to_string(), json }.encode();
            for client in &server.clients {
                let wants = client.watching.lock().is_ok_and(|w| w.iter().any(|id| id == probe.id));
                if wants {
                    // A full queue drops this report; the next is a third of a
                    // second away and newer.
                    let _ = client.queue.try_send(Outgoing::Packet(packet.clone()));
                }
            }
        }
    }
    world.insert_resource(server);
}
