//! The contract between the engine's networking API and whatever actually opens
//! a socket.
//!
//! The engine ships a networking *API* — the request builder, the blocking
//! facade its background threads call, the inbox scripts read — and no client.
//! A client implements [`Backend`] and hands itself to
//! [`AppNetBackendExt::add_net_backend`].
//!
//! ## Why the engine gave up its own client
//!
//! Not architecture for its own sake. `ureq` and the TLS stack under it —
//! rustls, ring, webpki, the platform verifiers — are twenty packages that every
//! build of the engine would otherwise compile, including a 2D mobile game that
//! never makes a request. Behind this boundary they are a dependency of one
//! crate, and a build without that crate ships without the stack.
//!
//! It also makes the client replaceable, which turned out to matter more than
//! expected: a browser build wants `fetch`, a console build wants the platform's
//! own certified HTTP library (and on some of them, shipping your own is a
//! certification failure), and a studio behind a corporate proxy wants theirs.
//!
//! ## The shape of a call
//!
//! ```text
//!   engine                                 backend
//!   ──────                                 ───────
//!   init()                   ────────────▶  build a client
//!                            ◀──────────── BackendInfo (agent + capabilities)
//!   start(request, body)     ────────────▶  spawn the transfer, return at once
//!   poll()                   ─── /frame ──▶
//!                            ◀──────────── whatever finished since last frame
//!   cancel(tag)              ────────────▶  abandon it
//! ```
//!
//! ### A backend must not block
//!
//! [`Backend::start`] is called from the engine's frame. A backend that
//! performed the transfer inline would stall the editor for a round trip, which
//! is why this is queue-and-poll rather than `fetch(url) -> Response`.
//!
//! The engine layers a *blocking* facade on top for the code that wants one, and
//! it works by parking the calling thread while the frame keeps running. That is
//! only sound because this side stays asynchronous.

use bevy::prelude::*;

/// What a backend promises it can do.
///
/// Asked rather than assumed, because backends genuinely differ: a browser build
/// going through `fetch` cannot set several headers and has no control over
/// redirects. An engine that assumed otherwise would sit waiting for chunks that
/// never come.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Caps(pub u32);

impl Caps {
    /// Bodies can be delivered in pieces — [`Request::stream`] is honoured
    /// rather than ignored. Without it a streaming request still works, it just
    /// arrives as one [`EventKind::Response`] at the end.
    pub const STREAM: Self = Self(1 << 0);
    /// [`Backend::cancel`] actually abandons the transfer. A backend without it
    /// may still accept the call and simply drop the answer.
    pub const CANCEL: Self = Self(1 << 1);
    /// Requests may carry arbitrary headers. A `fetch`-based backend cannot
    /// promise this — the browser forbids setting several of them.
    pub const HEADERS: Self = Self(1 << 2);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl core::ops::BitOr for Caps {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// What a backend answers [`Backend::init`] with.
#[derive(Clone, Debug)]
pub struct BackendInfo {
    /// What this client calls itself — the string that goes in logs, and the
    /// basis of the default `User-Agent`.
    pub agent: String,
    pub caps: Caps,
}

/// One HTTP request.
///
/// The body is **not** here: it is passed alongside, so a multipart upload is
/// not copied into the request just to be read out again.
#[derive(Clone, Debug, Default)]
pub struct Request {
    /// The engine's identifier for this request, echoed on every event it
    /// produces. The engine picks it; the backend only carries it.
    pub tag: u64,
    /// `GET`, `POST`, `PUT`, `DELETE`, … Uppercased before it gets here, so a
    /// backend can compare directly.
    pub method: String,
    pub url: String,
    /// Sent as given, in order. A backend that did not claim [`Caps::HEADERS`]
    /// may drop them.
    pub headers: Vec<(String, String)>,
    /// Deliver the body in pieces rather than in one event. Ignored — not
    /// refused — by a backend without [`Caps::STREAM`].
    pub stream: bool,
    /// Give up after this many milliseconds. `0` means the backend's own
    /// default, which is what almost every caller wants.
    pub timeout_ms: u32,
    /// Stop reading after this many bytes and fail the request. `0` means no
    /// limit.
    ///
    /// Enforced *during* the transfer, not after, which is the whole point: the
    /// callers that want it are fetching images from URLs a server chose — a
    /// marketplace thumbnail, an avatar, an image in a markdown README — and a
    /// cap applied once the bytes are already in memory protects nothing.
    pub max_bytes: u32,
}

/// Which kind of thing happened to a request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventKind {
    /// The whole body, for a non-streaming request. Terminal.
    Response,
    /// One piece of a streaming body. More follow.
    Chunk,
    /// A streaming body finished. Empty body. Terminal.
    End,
    /// The request failed — DNS, connect, TLS, timeout, a read that died
    /// mid-stream. The body holds the error text. Terminal.
    ///
    /// **A 404 is not this.** A server that answered is a successful request;
    /// the status is on the event and the body is whatever it sent. Reserving
    /// this for transport failure is what lets a caller read the `{"error": …}`
    /// body an API returns with its 400.
    Error,
}

impl EventKind {
    /// Whether nothing more will arrive for this tag. A consumer stops polling
    /// once it sees one.
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Chunk)
    }
}

/// Something that happened to a request, on its way back to the engine.
#[derive(Clone, Debug)]
pub struct Event {
    /// The [`Request::tag`] this belongs to.
    pub tag: u64,
    pub kind: EventKind,
    /// HTTP status, or `0` when the request never reached a response. Repeated
    /// on every chunk of a stream so a consumer that keeps only the latest still
    /// knows it.
    pub status: u16,
    /// Response headers. Empty on [`EventKind::Chunk`] after the first — they
    /// are sent once, with the first event that has them.
    pub headers: Vec<(String, String)>,
    /// The bytes. `Vec<u8>` rather than `String` because half the callers are
    /// fetching PNGs, and a lossy UTF-8 conversion at the boundary would corrupt
    /// every one of them.
    pub body: Vec<u8>,
}

/// An HTTP client the engine can drive.
///
/// `Send + Sync` because the engine holds it in a resource and polls it from the
/// frame; a backend that keeps its transfers on their own threads (every real
/// one does) needs no interior mutability beyond what it already has.
pub trait Backend: Send + Sync + 'static {
    /// Human-readable, for logs and the editor's network settings.
    fn name(&self) -> &str;

    /// Bring the client up.
    ///
    /// The returned [`Caps`] is a promise the engine relies on: it will not call
    /// [`cancel`](Self::cancel) on a backend that did not claim [`Caps::CANCEL`],
    /// and it *will* assume a streaming request arrives in pieces if
    /// [`Caps::STREAM`] is set. Claiming something you do not do produces an
    /// editor that hangs waiting for chunks that never come.
    fn init(&mut self) -> Result<BackendInfo, String>;

    /// Cancel everything in flight and release the client.
    fn shutdown(&mut self) {}

    /// Begin a request. Return immediately — see the trait docs.
    ///
    /// `body` is empty for a GET. An `Err` here means the request could not be
    /// *started* (a URL that would not parse); a request that starts and then
    /// fails reports [`EventKind::Error`] through [`poll`](Self::poll) instead,
    /// because by then the engine is already waiting on the tag and an error it
    /// never hears about is a caller that waits forever.
    fn start(&mut self, request: &Request, body: &[u8]) -> Result<(), String>;

    /// Take everything that has arrived since the last call.
    ///
    /// Called once per frame. Returning the same event twice delivers it twice;
    /// this is a drain, not a peek.
    fn poll(&mut self) -> Vec<Event>;

    /// Abandon a request. Only called when [`Caps::CANCEL`] was claimed.
    ///
    /// The engine has stopped caring about the answer. Any event still emitted
    /// for this tag is discarded, so the only thing at stake is whether the
    /// transfer keeps consuming bandwidth.
    fn cancel(&mut self, tag: u64) {
        let _ = tag;
    }
}

/// The registered client, if one has been installed.
///
/// One, not several: a request carries no key that would choose between two, and
/// a pair of clients would each hold half of one session's cookies and
/// connection pool. Scripting can hold several backends because a script names
/// its language by file extension; this cannot.
#[derive(Resource, Default)]
pub struct NetBackend(pub Option<Box<dyn Backend>>);

/// Install an HTTP client.
pub trait AppNetBackendExt {
    /// Register `backend` as the engine's HTTP client.
    ///
    /// First claim wins, and a second is refused with an error rather than
    /// replacing the first — a client swapped out from under in-flight requests
    /// would strand every one of them.
    fn add_net_backend(&mut self, backend: impl Backend) -> &mut Self;
}

impl AppNetBackendExt for App {
    fn add_net_backend(&mut self, backend: impl Backend) -> &mut Self {
        let name = backend.name().to_string();
        let mut slot = self
            .world_mut()
            .get_resource_or_insert_with(NetBackend::default);
        match &slot.0 {
            Some(existing) => error!(
                "network backend `{name}` is ignored — `{}` is already registered",
                existing.name()
            ),
            None => {
                info!("[net] backend `{name}` registered");
                slot.0 = Some(Box::new(backend));
            }
        }
        self
    }
}
