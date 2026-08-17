//! A [`Link`] — one duplex connection to one peer, with the socket kept off the
//! main thread.
//!
//! Everything above this module talks in [`CollabMsg`] and never in bytes. That
//! is the seam the transport choice sits behind: today a [`crate::tcp`] socket
//! fills a `Link`, and a relayed WebSocket to renzora.com can fill the same one
//! later without a single caller changing. Direct TCP is what ships first
//! because it needs no server deployed to test against, but it only reaches
//! peers on a LAN or behind a forwarded port — the relay is what will make
//! "invite a friend" work across the open internet, and this is the shape that
//! keeps that a drop-in.
//!
//! ## Threads, not async
//!
//! A `Link` owns two worker threads (one reading, one writing) and hands the
//! main thread a pair of channels. There is no executor and nothing to poll,
//! because the editor's frame loop is the only scheduler here: systems drain
//! whatever arrived since the last frame and move on. It mirrors the social
//! WebSocket worker, which reached the same shape for the same reason.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};

use crate::protocol::CollabMsg;

/// Something that happened on a link, in the order it happened.
#[derive(Debug)]
pub enum LinkEvent {
    /// The socket is up. For a guest this is the first sign the address was even
    /// reachable — `Link::connect` returns before the connection is made, so
    /// that a slow or wrong address cannot stall a frame.
    Connected,
    Message(CollabMsg),
    /// The link is finished, for the stated reason. No further events follow.
    Closed(String),
}

/// One peer connection. Cloneable senders are handed out; the link itself is
/// owned by the session.
pub struct Link {
    out: Sender<CollabMsg>,
    inbox: Receiver<LinkEvent>,
    shutdown: Arc<AtomicBool>,
    /// Forces the reader out of a blocking read so the link can be torn down.
    ///
    /// A transport-shaped hole rather than a `TcpStream`, because the reader
    /// blocks in whatever the transport blocks in, and only the transport knows
    /// how to interrupt that. TCP closes the socket; a relay will abort its
    /// request. Without one, tearing down a session would leave a thread parked
    /// forever in a read that nothing is ever going to satisfy.
    hangup: Option<Box<dyn Fn() + Send + Sync>>,
    /// Human-readable peer address, for the panel and logs.
    pub addr: String,
    /// Set once `Closed` has been observed, so a caller that drains after the
    /// fact doesn't keep queueing sends into a dead socket.
    closed: bool,
}

impl Link {
    /// Wire a link around channels a transport has already spawned threads for.
    pub(crate) fn new(
        out: Sender<CollabMsg>,
        inbox: Receiver<LinkEvent>,
        shutdown: Arc<AtomicBool>,
        addr: String,
    ) -> Self {
        Self { out, inbox, shutdown, hangup: None, addr, closed: false }
    }

    /// Give the link a way to interrupt its blocked reader. Called by the
    /// transport once it has something to interrupt.
    pub(crate) fn on_hangup(&mut self, hangup: impl Fn() + Send + Sync + 'static) {
        self.hangup = Some(Box::new(hangup));
    }

    /// Queue a message. Never blocks and never fails loudly: a send into a link
    /// whose worker has already exited is a no-op, because every caller of this
    /// is a system that has no useful way to handle "the socket died half a
    /// frame ago" and will see the `Closed` event on its next drain regardless.
    pub fn send(&self, msg: CollabMsg) {
        if self.closed {
            return;
        }
        let _ = self.out.send(msg);
    }

    /// Take everything that arrived since the last call.
    ///
    /// Drains to empty rather than taking one per frame: presence messages are
    /// state, and processing a stale one when a newer is already queued would
    /// render peers a frame behind for no reason.
    pub fn drain(&mut self) -> Vec<LinkEvent> {
        let mut out = Vec::new();
        loop {
            match self.inbox.try_recv() {
                Ok(event) => {
                    if matches!(event, LinkEvent::Closed(_)) {
                        self.closed = true;
                    }
                    out.push(event);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // Both workers are gone without a Closed event — the thread
                    // panicked or the process is tearing down. Synthesise one so
                    // the session still notices.
                    if !self.closed {
                        self.closed = true;
                        out.push(LinkEvent::Closed("connection lost".into()));
                    }
                    break;
                }
            }
        }
        out
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// A handle a worker thread can send through.
    ///
    /// File transfer is the reason this exists: a multi-gigabyte project cannot
    /// be read and chunked from a system without stalling the frame, so the
    /// streaming happens on its own thread and pushes into the same queue the
    /// main thread uses. Sends through it are ordered against everything else on
    /// the link, which is what keeps a chunk from overtaking the message that
    /// announced the file.
    pub fn sender(&self) -> Sender<CollabMsg> {
        self.out.clone()
    }
}

impl Drop for Link {
    /// Dropping a link hangs up.
    ///
    /// The flag alone is not enough and must not be made enough. The reader
    /// blocks indefinitely on purpose, because **every way of making a read
    /// return early has already broken this transport once**: a non-blocking
    /// socket returning `WouldBlock`, and a short read timeout so the flag could
    /// be polled between frames. Either one can land mid-frame, and `read_exact`
    /// does not report how much it consumed before failing — so "just retry"
    /// resumes in the middle of a payload and reads it as a length prefix. The
    /// reader therefore stays blocked and the *transport* interrupts it.
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(hangup) = &self.hangup {
            hangup();
        }
    }
}

/// A source of inbound connections — what a host holds while a session is open.
///
/// Both transports produce one: [`crate::tcp::listen`] from a socket accepting
/// on a port, [`crate::relay::host`] from a single outbound WebSocket that the
/// server multiplexes several guests onto. The session only ever asks it for
/// links, which is what lets "who can reach me" change without the session
/// knowing.
pub struct Acceptor {
    pub incoming: Receiver<Link>,
    /// Human-readable description of where peers arrive — a port, or a room
    /// code. Shown in the panel.
    pub origin: String,
    /// The port actually bound, for transports that bind one.
    ///
    /// Transport-specific in a transport-agnostic type, which is a small wart
    /// paid for a real need: asking the OS for a port (by requesting 0) is how
    /// you avoid collisions, and the caller then has to be told which one it
    /// got. `None` for the relay, which binds nothing.
    pub local_port: Option<u16>,
    hangup: Option<Box<dyn Fn() + Send + Sync>>,
}

impl Acceptor {
    pub(crate) fn new(
        incoming: Receiver<Link>,
        origin: String,
        hangup: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self { incoming, origin, local_port: None, hangup: Some(Box::new(hangup)) }
    }

    pub(crate) fn with_port(mut self, port: u16) -> Self {
        self.local_port = Some(port);
        self
    }
}

impl Drop for Acceptor {
    fn drop(&mut self) {
        if let Some(hangup) = &self.hangup {
            hangup();
        }
    }
}

/// The channel ends a transport hands back when it spawns a link's workers.
pub(crate) struct LinkPlumbing {
    pub out_rx: Receiver<CollabMsg>,
    pub inbox_tx: Sender<LinkEvent>,
    pub shutdown: Arc<AtomicBool>,
}

/// Build the channels for a link, returning the caller-facing half and the
/// worker-facing half.
pub(crate) fn plumbing(addr: String) -> (Link, LinkPlumbing) {
    let (out_tx, out_rx) = unbounded();
    let (inbox_tx, inbox_rx) = unbounded();
    let shutdown = Arc::new(AtomicBool::new(false));
    let link = Link::new(out_tx, inbox_rx, shutdown.clone(), addr);
    (link, LinkPlumbing { out_rx, inbox_tx, shutdown })
}
