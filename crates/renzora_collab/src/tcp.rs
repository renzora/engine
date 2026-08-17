//! The TCP transport: a listener for the host, a connector for the guest.
//!
//! TCP rather than the engine's existing UDP transport (`renzora_network`)
//! because the two carry opposite traffic. A game replicates *state* — a
//! transform that arrives late is worthless, so UDP drops it and sends the next
//! one. An editor session replicates *edits*, and an edit that arrives late is
//! still the edit; dropping one leaves the two projects permanently different.
//! Reliable ordered delivery is the requirement, and rebuilding it over UDP is
//! rebuilding TCP.

use std::io::Write;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};

use crate::link::{plumbing, Acceptor, Link, LinkEvent, LinkPlumbing};
use crate::protocol::{read_frame, write_frame, CollabMsg};

/// How long the writer parks on its queue before re-checking the shutdown flag.
///
/// This is a *channel* timeout, not a socket one, and the difference matters:
/// waking up early on an empty channel loses nothing, whereas waking up early
/// mid-`read_exact` on a socket discards bytes it has already taken off the
/// stream. The reader therefore has no timeout at all — see [`read_loop`].
const WRITER_IDLE: Duration = Duration::from_millis(500);

/// A socket that may not exist yet, shared with whoever needs to close it.
///
/// The guest's socket is opened on a worker thread, so the `Link` it belongs to
/// is handed back before there is anything to hang up on. This is the box the
/// thread drops it into once connected, and the box the hangup looks in.
#[derive(Default, Clone)]
struct SocketHandle(Arc<Mutex<Option<TcpStream>>>);

impl SocketHandle {
    fn set(&self, stream: TcpStream) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(stream);
        }
    }

    /// Unblock the reader, and *only* the reader.
    ///
    /// `Shutdown::Read`, not `Both`, because a link is often dropped with
    /// something still queued to send: rejecting a peer sends `Rejected` and then
    /// drops the link in the same breath. Closing the write half here would beat
    /// the writer to it and turn an explained refusal into a bare disconnect —
    /// which is the exact failure the `Rejected` message exists to prevent. The
    /// writer closes the socket properly once it has drained.
    fn hangup(&self) {
        if let Ok(slot) = self.0.lock() {
            if let Some(stream) = slot.as_ref() {
                let _ = stream.shutdown(Shutdown::Read);
            }
        }
    }
}

/// Start accepting peers on `port`.
///
/// Binds to `0.0.0.0` so a peer on the LAN can reach it. That is a deliberate
/// exposure and the reason the handshake refuses unknown protocol versions
/// before anything else is read — the port is open to whatever else is on the
/// network, and the first frame from a stranger must not be able to do more than
/// get itself hung up on.
pub fn listen(port: u16) -> std::io::Result<Acceptor> {
    let listener = TcpListener::bind(("0.0.0.0", port))?;
    let port = listener.local_addr()?.port();
    // Non-blocking accept, so the loop can honour the shutdown flag rather than
    // parking in `accept()` until someone happens to connect.
    listener.set_nonblocking(true)?;

    let (tx, incoming) = unbounded();
    let shutdown = Arc::new(AtomicBool::new(false));
    let flag = shutdown.clone();

    std::thread::Builder::new()
        .name("collab-listen".into())
        .spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        log::info!("[collab] peer connected from {addr}");
                        match spawn_link(stream, addr.to_string(), true) {
                            Ok(link) => {
                                if tx.send(link).is_err() {
                                    return; // session gone
                                }
                            }
                            Err(e) => log::warn!("[collab] rejecting {addr}: {e}"),
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        log::warn!("[collab] accept failed: {e}");
                        return;
                    }
                }
            }
        })?;

    let stop = shutdown.clone();
    Ok(Acceptor::new(incoming, format!("port {port}"), move || {
        stop.store(true, Ordering::Relaxed)
    })
    .with_port(port))
}

/// Connect to a host. Returns immediately — the connection itself is made on the
/// worker thread and reported as [`LinkEvent::Connected`] or
/// [`LinkEvent::Closed`], so a wrong address costs the guest a message rather
/// than a frozen editor.
pub fn connect(addr: String) -> Link {
    let (mut link, plumb) = plumbing(addr.clone());
    let LinkPlumbing { out_rx, inbox_tx, shutdown } = plumb;

    let socket = SocketHandle::default();
    let for_hangup = socket.clone();
    link.on_hangup(move || for_hangup.hangup());

    let spawned = std::thread::Builder::new()
        .name("collab-connect".into())
        .spawn(move || {
            let resolved: SocketAddr = match resolve(&addr) {
                Ok(a) => a,
                Err(e) => {
                    let _ = inbox_tx.send(LinkEvent::Closed(e));
                    return;
                }
            };
            let stream = match TcpStream::connect_timeout(&resolved, Duration::from_secs(10)) {
                Ok(s) => s,
                Err(e) => {
                    let _ = inbox_tx.send(LinkEvent::Closed(format!("could not reach {addr}: {e}")));
                    return;
                }
            };
            // `connect_timeout` goes non-blocking internally to implement the
            // timeout and restores blocking afterwards, but say so explicitly:
            // the reader below is only correct on a blocking socket, and that
            // requirement should not rest on a detail of someone else's
            // implementation.
            if let Err(e) = stream.set_nonblocking(false) {
                let _ = inbox_tx.send(LinkEvent::Closed(format!("could not configure socket: {e}")));
                return;
            }
            if let Ok(handle) = stream.try_clone() {
                socket.set(handle);
            }
            let _ = inbox_tx.send(LinkEvent::Connected);
            run_stream(stream, out_rx, inbox_tx, shutdown);
        });

    if let Err(e) = spawned {
        log::error!("[collab] could not spawn connect thread: {e}");
    }
    link
}

/// Parse `host:port`, defaulting the port when it is omitted.
fn resolve(addr: &str) -> Result<SocketAddr, String> {
    use std::net::ToSocketAddrs;
    let with_port =
        if addr.contains(':') { addr.to_string() } else { format!("{addr}:{}", crate::DEFAULT_PORT) };
    with_port
        .to_socket_addrs()
        .map_err(|e| format!("could not resolve {with_port}: {e}"))?
        .next()
        .ok_or_else(|| format!("{with_port} resolved to no address"))
}

/// Wrap an accepted stream in a link.
fn spawn_link(stream: TcpStream, addr: String, connected: bool) -> std::io::Result<Link> {
    // **Put the socket back into blocking mode.** On Windows an accepted socket
    // inherits the listener's non-blocking flag, and the listener is
    // non-blocking so its accept loop can poll for shutdown. Inheriting it here
    // is the bug that corrupted the first working session: every `read_exact`
    // returned `WouldBlock` part-way through a frame, having already taken bytes
    // off the stream that there is no way to put back, and the reader resumed
    // mid-payload. It is a one-line fix for a fault that presented as the *peer*
    // announcing a 1.5 GB frame.
    stream.set_nonblocking(false)?;

    let (mut link, plumb) = plumbing(addr);
    let LinkPlumbing { out_rx, inbox_tx, shutdown } = plumb;

    let socket = SocketHandle::default();
    socket.set(stream.try_clone()?);
    link.on_hangup(move || socket.hangup());

    if connected {
        let _ = inbox_tx.send(LinkEvent::Connected);
    }
    std::thread::Builder::new()
        .name("collab-peer".into())
        .spawn(move || run_stream(stream, out_rx, inbox_tx, shutdown))?;
    Ok(link)
}

/// Drive one connected socket until it closes: a reader thread and a writer
/// thread over two handles to the same stream.
///
/// Two threads rather than one because the alternative is polling. A single
/// thread would have to alternate between "is a frame readable?" and "is a
/// message queued?", and the only way to do that without a busy loop is a short
/// read timeout — which puts a floor of half a timeout on outbound latency for
/// every message, including the presence stream. Splitting them lets both sides
/// block properly.
fn run_stream(
    stream: TcpStream,
    out_rx: Receiver<CollabMsg>,
    inbox_tx: Sender<LinkEvent>,
    shutdown: Arc<AtomicBool>,
) {
    // Nagle batches small writes, which is exactly wrong here: presence and
    // lease messages are small and latency-sensitive, and the delay it adds is
    // felt directly as a peer's cursor lagging behind their actual position.
    let _ = stream.set_nodelay(true);
    // Deliberately NO read timeout, and (see `spawn_link`) deliberately not a
    // non-blocking socket. Both make a read return early, both can do it
    // mid-frame, and `read_exact` does not report how much it consumed before
    // failing — so retrying resumes in the middle of a payload. That is what
    // desynchronised the first working session, reported as the peer announcing
    // a 1.5 GB frame. The reader blocks until data or EOF; `Link`'s hangup
    // closes the socket to end it.

    let write_half = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            let _ = inbox_tx.send(LinkEvent::Closed(format!("could not split socket: {e}")));
            return;
        }
    };

    let writer_flag = shutdown.clone();
    let writer = std::thread::Builder::new()
        .name("collab-write".into())
        .spawn(move || write_loop(write_half, out_rx, writer_flag));

    read_loop(stream, &inbox_tx, &shutdown);

    // The reader has finished, so the link is over. Signal the writer and let it
    // drain what is already queued — a `Rejected` explaining why we are hanging
    // up is the last thing written, and losing it would turn an explained
    // refusal into a silent disconnect.
    shutdown.store(true, Ordering::Relaxed);
    if let Ok(handle) = writer {
        let _ = handle.join();
    }
}

/// Read frames until the socket ends.
///
/// Blocks indefinitely inside `read_frame` and is woken only by data, by EOF, or
/// by the link's hangup closing the socket underneath it. There is no timeout
/// and there must not be one: `read_exact` does not report how much it consumed
/// before failing, so any error it can recover from is an error that has already
/// eaten part of a frame.
///
/// Reads through a `BufReader` so the 8-byte header does not cost its own
/// syscall — with Nagle off, headers and payloads arrive as separate segments,
/// and unbuffered that is two reads per message at minimum.
fn read_loop(stream: TcpStream, inbox_tx: &Sender<LinkEvent>, shutdown: &Arc<AtomicBool>) {
    let mut reader = std::io::BufReader::with_capacity(64 * 1024, stream);
    loop {
        match read_frame(&mut reader) {
            Ok(msg) => {
                if inbox_tx.send(LinkEvent::Message(msg)).is_err() {
                    return; // session dropped the link
                }
            }
            Err(e) => {
                // A shutdown we asked for surfaces here as a read error on a
                // socket somebody closed. Report it as the ordinary end of a
                // session rather than as a fault.
                if shutdown.load(Ordering::Relaxed) {
                    return;
                }
                let reason = match e.kind() {
                    std::io::ErrorKind::UnexpectedEof => "peer disconnected".to_string(),
                    // A blocking read cannot return this. Seeing it means the
                    // socket is in non-blocking mode, which has happened here
                    // once already (accepted sockets inherit the listener's flag
                    // on Windows) and cost a session's worth of confusing
                    // "the peer announced an absurd frame" reports. Name it.
                    std::io::ErrorKind::WouldBlock => {
                        "socket is in non-blocking mode — this is a transport bug, \
                         not a peer problem"
                            .to_string()
                    }
                    _ => format!("read failed: {e}"),
                };
                let _ = inbox_tx.send(LinkEvent::Closed(reason));
                return;
            }
        }
    }
}

fn write_loop(mut stream: TcpStream, out_rx: Receiver<CollabMsg>, shutdown: Arc<AtomicBool>) {
    loop {
        match out_rx.recv_timeout(WRITER_IDLE) {
            Ok(msg) => {
                if let Err(e) = write_frame(&mut stream, &msg) {
                    log::debug!("[collab] write failed: {e}");
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if shutdown.load(Ordering::Relaxed) {
                    // Flush anything already queued before going, then stop.
                    while let Ok(msg) = out_rx.try_recv() {
                        if write_frame(&mut stream, &msg).is_err() {
                            break;
                        }
                    }
                    let _ = stream.flush();
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                let _ = stream.flush();
                let _ = stream.shutdown(Shutdown::Both);
                return;
            }
        }
    }
}

/// Best-effort LAN address to show a host, so they can read a peer the address
/// to type. `UdpSocket::connect` on a UDP socket sends nothing — it only asks
/// the routing table which local interface would be used to reach that
/// destination, which is the one a peer on the same network can reach back on.
pub fn local_address_hint() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip().to_string())
}
