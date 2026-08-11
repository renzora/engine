//! What the rest of the engine calls: a request builder, a blocking [`fetch`],
//! and a streaming one.
//!
//! ## Why blocking, when the boundary underneath is not
//!
//! Every network call in this engine already runs on a thread somebody spawned
//! for it — the marketplace search, the thumbnail fetcher, the sign-in flow, the
//! update check. They were written against a blocking client and their shape is
//! right: a background thread that does one request and posts the result into a
//! Bevy resource is simpler than any callback arrangement, and it is what the
//! surrounding code already expects.
//!
//! So this keeps that shape. [`fetch`] blocks *its own thread* while the frame
//! carries on: it queues the request, parks on a channel, and the per-frame pump
//! ([`crate::pump`]) hands the answer back. What changed underneath is only who
//! opens the socket.
//!
//! **It must therefore not be called from a system.** A system runs inside the
//! frame the pump needs in order to make progress, so blocking there waits for
//! something that cannot happen until you return. [`Error::NoPump`] is what you
//! get instead of a hang — see [`WATCHDOG`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use renzora_plugin::net::{Event, EventKind};

/// How long a wait goes with the pump making no progress at all before it
/// concludes that it never will.
///
/// This is not the request timeout — that is [`Request::timeout`], and the
/// backend enforces it. This catches the two ways the *host* side can stop:
/// being called from inside a system (the frame cannot advance while you block
/// in it) and the app shutting down with a request in flight. Both used to be
/// indistinguishable from a slow server; now they are a message in two seconds.
const WATCHDOG: Duration = Duration::from_secs(2);

/// The request timeout when a caller does not name one.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// What the pump puts in the body of the failure it delivers when a request was
/// queued and there is no backend to hand it to.
///
/// A sentinel rather than an eager `is_available()` check in [`fetch`], because
/// that check had a race: a request made between the plugin loading and the
/// first frame that adopts it would be refused for a backend that was about to
/// exist. Queueing and letting the pump decide removes the window entirely — the
/// cost is that "there is no backend" has to travel back as an event like every
/// other outcome, and this is what marks it.
///
/// Both ends are in this crate, so it is a shared constant rather than string
/// matching across a boundary. It never reaches a caller: [`fetch`] and
/// [`Stream`] both turn it back into [`Error::NoBackend`].
pub(crate) const NO_BACKEND: &str = "\u{0}renzora-net:no-backend";

/// Why a request did not produce a response.
///
/// Note that an HTTP error status is **not** in here. A 404 is a successful
/// request whose [`Response::status`] is 404, and the body is whatever the
/// server sent — which is the only way to read the `{"error": …}` an API returns
/// with its 400. Use [`Response::is_ok`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No plugin has registered as the network backend, so the engine has no way
    /// to make a request at all. In a normal build `plugins/http` provides one;
    /// this is what a game exported without it reports.
    NoBackend,
    /// The frame loop is not running, so a queued request can never be handed to
    /// the backend. Almost always means this was called from a system — see the
    /// module docs.
    NoPump,
    /// The transport failed: DNS, connect, TLS, timeout, a read that died
    /// mid-body.
    Transport(String),
    /// The response arrived but would not parse as what the caller asked for.
    Decode(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBackend => f.write_str(
                "no network backend is loaded — this build has no HTTP client (plugins/http)",
            ),
            Self::NoPump => {
                f.write_str("the frame loop is not running — was this called from a system?")
            }
            Self::Transport(e) | Self::Decode(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for Error {}

/// One request, before it is sent.
///
/// Built rather than passed positionally because the common call has two
/// interesting parts and five uninteresting ones, and
/// `Request::get(url).bearer(token).send()` says which is which.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub timeout: Duration,
    /// Fail the request once the response body passes this many bytes. `0` means
    /// no limit. See [`max_bytes`](Self::max_bytes).
    pub max_bytes: u32,
    /// Set by [`json`](Self::json) when the value would not serialise, and
    /// turned back into an `Err` by [`fetch`]. Deferred so a builder chain stays
    /// a chain — a type that will not serialise is a bug at the call site, not a
    /// runtime condition worth branching on mid-expression.
    bad_body: Option<String>,
}

impl Request {
    pub fn new(method: &str, url: &str) -> Self {
        Self {
            // Uppercased once, here, so no backend has to think about it.
            method: method.to_ascii_uppercase(),
            url: url.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
            max_bytes: 0,
            bad_body: None,
        }
    }

    pub fn get(url: &str) -> Self {
        Self::new("GET", url)
    }

    pub fn post(url: &str) -> Self {
        Self::new("POST", url)
    }

    pub fn put(url: &str) -> Self {
        Self::new("PUT", url)
    }

    pub fn delete(url: &str) -> Self {
        Self::new("DELETE", url)
    }

    /// Add a header. Repeatable.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// `Authorization: Bearer <token>`, which is what every authenticated call
    /// in this engine wants.
    pub fn bearer(self, token: &str) -> Self {
        self.header("Authorization", &format!("Bearer {token}"))
    }

    /// As [`bearer`](Self::bearer), for a token that may not be there.
    ///
    /// Skips the header entirely rather than sending `Bearer ` with nothing
    /// after it — some servers treat that as a malformed credential and answer
    /// 400 where they would have served an anonymous request. Several call sites
    /// in this engine take an `Option<&str>` token for exactly that reason.
    pub fn maybe_bearer(self, token: Option<&str>) -> Self {
        match token {
            Some(t) => self.bearer(t),
            None => self,
        }
    }

    /// Raw body bytes with an explicit content type.
    pub fn body(mut self, content_type: &str, bytes: impl Into<Vec<u8>>) -> Self {
        self.body = bytes.into();
        self.header("Content-Type", content_type)
    }

    /// Serialise `value` as a JSON body. See [`bad_body`](Self::bad_body) for
    /// why a failure here does not return a `Result`.
    pub fn json(mut self, value: &impl serde::Serialize) -> Self {
        match serde_json::to_vec(value) {
            Ok(bytes) => self.body("application/json", bytes),
            Err(e) => {
                self.bad_body = Some(e.to_string());
                self
            }
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Refuse a response body larger than this, while it is arriving.
    ///
    /// For the fetches whose URL a server chose rather than we did — a
    /// marketplace thumbnail, an avatar, an image embedded in a README. The
    /// backend stops reading at the limit and reports a transport error, so an
    /// enormous or endless body costs the cap rather than the whole of it.
    pub fn max_bytes(mut self, bytes: u32) -> Self {
        self.max_bytes = bytes;
        self
    }

    /// Send it and block this thread until the whole response is in.
    pub fn send(self) -> Result<Response, Error> {
        fetch(self)
    }

    /// Send it and get the body in pieces as they arrive.
    pub fn send_stream(self) -> Result<Stream, Error> {
        fetch_stream(self)
    }
}

/// A completed response.
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    /// `Vec<u8>` rather than `String`: half the callers are fetching PNGs, and a
    /// lossy conversion here would corrupt every one of them. [`text`](Self::text)
    /// is one call away for the other half.
    pub body: Vec<u8>,
}

impl Response {
    pub fn is_ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The body as text, replacing anything that is not valid UTF-8.
    ///
    /// Lossy rather than fallible: a server that returns a malformed byte inside
    /// an error message should not make the message unreadable, and a caller has
    /// nothing useful to do with the distinction.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// One response header, matched case-insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Parse the body as JSON.
    ///
    /// **Non-2xx is an error**, and the message is the server's own `{"error":
    /// …}` string when it sent one. That is the behaviour a dozen call sites in
    /// this engine hand-rolled identically before this existed, including the
    /// detail that makes it worth having: the alternative is reporting "HTTP
    /// 400" and discarding the actual reason.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, Error> {
        if !self.is_ok() {
            let text = self.text();
            let message = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
                .unwrap_or_else(|| format!("HTTP {}", self.status));
            return Err(Error::Transport(message));
        }
        serde_json::from_slice(&self.body)
            .map_err(|e| Error::Decode(format!("failed to parse response: {e}")))
    }
}

/// One piece of a streaming response.
#[derive(Clone, Debug)]
pub struct Chunk {
    /// HTTP status, repeated on every piece so a consumer that keeps only the
    /// latest still knows it.
    pub status: u16,
    pub data: Vec<u8>,
}

impl Chunk {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.data).into_owned()
    }
}

/// A streaming response, in progress.
///
/// Iterate it; it ends when the stream does. A transport failure mid-body ends
/// it too, and [`error`](Self::error) then says what happened — check it
/// **after** iterating, because a stream that fails halfway delivers the pieces
/// it got and then stops, which is indistinguishable from success until you ask.
/// Dropping it cancels the request.
pub struct Stream {
    tag: u64,
    events: Receiver<Event>,
    status: u16,
    error: Option<Error>,
    done: bool,
}

impl Stream {
    /// The HTTP status, once the first piece has arrived. `0` before that.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Why the stream stopped early, if it did. `None` for a clean end.
    pub fn error(&self) -> Option<&Error> {
        self.error.as_ref()
    }

    /// Everything, concatenated. For a caller that wanted streaming for
    /// liveness but has decided to wait after all.
    pub fn collect_body(mut self) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        for chunk in &mut self {
            out.extend_from_slice(&chunk.data);
        }
        match self.error.take() {
            Some(e) => Err(e),
            None => Ok(out),
        }
    }
}

impl Iterator for Stream {
    type Item = Chunk;

    fn next(&mut self) -> Option<Chunk> {
        if self.done {
            return None;
        }
        let event = match wait_for(&self.events) {
            Ok(event) => event,
            Err(e) => {
                self.done = true;
                self.error = Some(e);
                return None;
            }
        };
        if event.status != 0 {
            self.status = event.status;
        }
        match event.kind {
            EventKind::Chunk => Some(Chunk {
                status: self.status,
                data: event.body,
            }),
            // A backend without `Caps::STREAM` answers a streaming request with
            // one whole body. Deliver it as a single piece rather than dropping
            // it: a caller that accumulates gets the same bytes either way,
            // which is what makes that capability optional rather than required.
            EventKind::Response => {
                self.done = true;
                Some(Chunk {
                    status: self.status,
                    data: event.body,
                })
            }
            EventKind::End => {
                self.done = true;
                None
            }
            EventKind::Error => {
                self.done = true;
                self.error = Some(transport_error(&event.body));
                None
            }
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        shared().forget(self.tag);
        if !self.done {
            shared().cancel(self.tag);
        }
    }
}

// ── The queue between callers and the pump ───────────────────────────────────

/// A request waiting to be handed to the backend.
pub(crate) struct Submission {
    pub request: renzora_plugin::net::Request,
    pub body: Vec<u8>,
}

/// Process-global, because [`fetch`] is called from threads that have no Bevy
/// world and no way to reach a resource.
///
/// A `static` rather than something threaded through every caller: the
/// alternative is handing a channel to each of the dozen background threads the
/// editor spawns, down call stacks that exist only to pass it along. There is
/// one engine per process and one backend, so a global is what this actually is.
pub(crate) struct Shared {
    queue: Mutex<Vec<Submission>>,
    /// Where to deliver each in-flight request's events.
    waiters: Mutex<HashMap<u64, Sender<Event>>>,
    /// Tags the caller has abandoned, for the pump to pass to the backend.
    cancels: Mutex<Vec<u64>>,
    next_tag: AtomicU64,
    /// Bumped by the pump every frame. [`wait_for`] watches it to tell a slow
    /// server from a frame loop that has stopped.
    ticks: AtomicU64,
    /// Whether a backend is registered right now.
    available: AtomicBool,
}

pub(crate) fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| Shared {
        queue: Mutex::new(Vec::new()),
        waiters: Mutex::new(HashMap::new()),
        cancels: Mutex::new(Vec::new()),
        next_tag: AtomicU64::new(1),
        ticks: AtomicU64::new(0),
        available: AtomicBool::new(false),
    })
}

impl Shared {
    /// Queue a request and return its tag and the channel its events arrive on.
    ///
    /// The tag is allocated and the waiter registered *before* the submission is
    /// queued, so the pump cannot pick the request up and deliver an event for a
    /// tag nobody is listening to yet.
    fn submit(&self, request: Request, stream: bool) -> (u64, Receiver<Event>) {
        let tag = self.next_tag.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = channel();
        if let Ok(mut waiters) = self.waiters.lock() {
            waiters.insert(tag, tx);
        }
        if let Ok(mut queue) = self.queue.lock() {
            queue.push(Submission {
                request: renzora_plugin::net::Request {
                    tag,
                    method: request.method,
                    url: request.url,
                    headers: request.headers,
                    stream,
                    // Saturating rather than wrapping: a caller asking for a
                    // 50-day timeout gets the longest one expressible, not a
                    // short one from a truncated cast.
                    timeout_ms: request.timeout.as_millis().min(u32::MAX as u128) as u32,
                    max_bytes: request.max_bytes,
                },
                body: request.body,
            });
        }
        (tag, rx)
    }

    /// Take everything queued since the last frame. Called by the pump.
    pub(crate) fn take_queued(&self) -> Vec<Submission> {
        self.queue
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default()
    }

    pub(crate) fn take_cancels(&self) -> Vec<u64> {
        self.cancels
            .lock()
            .map(|mut c| std::mem::take(&mut *c))
            .unwrap_or_default()
    }

    /// Deliver one event to whoever is waiting on its tag.
    ///
    /// A tag with no waiter is dropped, which is the right handling of a
    /// response whose caller gave up: the [`Stream`] was dropped, or the panel
    /// it belonged to closed.
    pub(crate) fn deliver(&self, event: Event) {
        let Ok(mut waiters) = self.waiters.lock() else {
            return;
        };
        let terminal = event.kind.is_terminal();
        let tag = event.tag;
        if let Some(tx) = waiters.get(&tag) {
            // A send failure means the receiver is gone — the same case as no
            // waiter at all, so fall through to the cleanup below.
            let _ = tx.send(event);
        }
        if terminal {
            waiters.remove(&tag);
        }
    }

    /// Fail everything in flight, and everything queued behind it.
    ///
    /// Used when the backend goes away — a plugin unloaded or hot-reloaded
    /// mid-transfer. Without this, every parked thread would wait out its full
    /// timeout for an answer that can no longer come from anywhere.
    pub(crate) fn fail_all(&self, reason: &str) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.clear();
        }
        let Ok(mut waiters) = self.waiters.lock() else {
            return;
        };
        for (tag, tx) in waiters.drain() {
            let _ = tx.send(Event {
                tag,
                kind: EventKind::Error,
                status: 0,
                headers: Vec::new(),
                body: reason.as_bytes().to_vec(),
            });
        }
    }

    /// Stop expecting events for `tag`.
    fn forget(&self, tag: u64) {
        if let Ok(mut waiters) = self.waiters.lock() {
            waiters.remove(&tag);
        }
    }

    /// Ask the backend to abandon `tag`.
    fn cancel(&self, tag: u64) {
        if let Ok(mut cancels) = self.cancels.lock() {
            cancels.push(tag);
        }
    }

    /// Called once per frame by the pump. What [`wait_for`] watches.
    pub(crate) fn tick(&self) {
        self.ticks.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn set_available(&self, available: bool) {
        self.available.store(available, Ordering::Relaxed);
    }
}

/// Block until the next event arrives, or decide that none ever will.
///
/// The loop exists for the watchdog. A plain `recv_timeout(request timeout)`
/// cannot tell a sixty-second download from a caller that deadlocked itself by
/// blocking inside a system, and the second should not take sixty seconds to
/// report. So this wakes regularly and checks whether the pump has ticked since
/// the last look: a frame loop that is running makes the wait effectively
/// unbounded, and one that is not fails in two seconds.
fn wait_for(events: &Receiver<Event>) -> Result<Event, Error> {
    let shared = shared();
    let mut last_tick = shared.ticks.load(Ordering::Relaxed);
    loop {
        match events.recv_timeout(WATCHDOG) {
            Ok(event) => return Ok(event),
            Err(RecvTimeoutError::Timeout) => {
                let tick = shared.ticks.load(Ordering::Relaxed);
                if tick == last_tick {
                    // Logged rather than left to the caller. This is a bug in
                    // the calling code, not a network condition, and it is
                    // exactly the kind a caller swallows: `renzora_auth` read a
                    // failure here as "your token expired" and deleted the
                    // saved session, so the editor asked for a sign-in on every
                    // launch and nothing in the log said why.
                    bevy::log::error!(
                        "[net] a request was made with no frame loop running — \
                         `renzora_net::fetch` blocks its own thread and cannot be \
                         called from a system or from `Plugin::build`. Spawn a thread."
                    );
                    return Err(Error::NoPump);
                }
                last_tick = tick;
            }
            // The sender was dropped without a terminal event. `fail_all` sends
            // a reason first in every case it handles, so reaching here means
            // the world itself went away mid-request.
            Err(RecvTimeoutError::Disconnected) => {
                return Err(Error::Transport("the engine shut down".to_string()))
            }
        }
    }
}

/// Turn an error event's body into an [`Error`], recognising the pump's
/// no-backend sentinel. See [`NO_BACKEND`].
fn transport_error(body: &[u8]) -> Error {
    let text = String::from_utf8_lossy(body);
    if text == NO_BACKEND {
        Error::NoBackend
    } else {
        Error::Transport(text.into_owned())
    }
}

/// Perform one request, blocking this thread until the whole response is in.
///
/// **Not from a system** — see the module docs.
///
/// ```ignore
/// std::thread::spawn(move || {
///     let assets: Vec<Asset> = Request::get(&url)
///         .maybe_bearer(token.as_deref())
///         .send()?
///         .json()?;
/// });
/// ```
pub fn fetch(request: Request) -> Result<Response, Error> {
    if let Some(e) = request.bad_body {
        return Err(Error::Decode(format!("failed to encode request body: {e}")));
    }
    let (_, events) = shared().submit(request, false);
    let mut body = Vec::new();
    let mut status = 0u16;
    let mut headers = Vec::new();
    loop {
        let event = wait_for(&events)?;
        if event.status != 0 {
            status = event.status;
        }
        if !event.headers.is_empty() {
            headers = event.headers;
        }
        match event.kind {
            EventKind::Response => {
                body = event.body;
                break;
            }
            // A backend may answer a non-streaming request in pieces anyway.
            // Accumulating rather than refusing costs nothing and means the host
            // does not care which shape comes back.
            EventKind::Chunk => body.extend_from_slice(&event.body),
            EventKind::End => break,
            EventKind::Error => return Err(transport_error(&event.body)),
        }
    }
    Ok(Response {
        status,
        headers,
        body,
    })
}

/// Perform one request, delivering the body in pieces as it arrives.
///
/// What a token-streaming chat API needs: the whole value is seeing the reply
/// appear, and a response that stays open for thirty seconds delivers everything
/// at the end or not at all.
pub fn fetch_stream(request: Request) -> Result<Stream, Error> {
    if let Some(e) = request.bad_body {
        return Err(Error::Decode(format!("failed to encode request body: {e}")));
    }
    let (tag, events) = shared().submit(request, true);
    Ok(Stream {
        tag,
        events,
        status: 0,
        error: None,
        done: false,
    })
}

/// Whether a backend is loaded.
///
/// For UI that wants to say "offline" rather than fire a request to find out —
/// and for the callers that check before spawning a thread at all.
pub fn is_available() -> bool {
    shared().available.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_builder_produces_the_headers_it_was_given() {
        let request = Request::post("https://example.com/x")
            .bearer("tok")
            .header("Accept", "application/json");
        assert_eq!(request.method, "POST");
        assert_eq!(
            request.headers[0],
            ("Authorization".into(), "Bearer tok".into())
        );
        assert_eq!(
            request.headers[1],
            ("Accept".into(), "application/json".into())
        );
    }

    /// `Bearer ` with nothing after it is a malformed credential some servers
    /// reject outright, where they would have answered an anonymous request.
    #[test]
    fn a_missing_token_omits_the_header_entirely() {
        let request = Request::get("https://example.com").maybe_bearer(None);
        assert!(request.headers.is_empty());
    }

    #[test]
    fn a_lowercase_method_is_normalised() {
        assert_eq!(Request::new("post", "https://example.com").method, "POST");
    }

    /// The distinction the whole error type is built around: a 404 is a
    /// successful request, and its body is the server's own message.
    #[test]
    fn a_non_2xx_json_response_surfaces_the_servers_message() {
        let response = Response {
            status: 400,
            headers: Vec::new(),
            body: br#"{"error":"asset name already taken"}"#.to_vec(),
        };
        assert!(!response.is_ok());
        assert_eq!(
            response.json::<serde_json::Value>().unwrap_err(),
            Error::Transport("asset name already taken".to_string())
        );
    }

    #[test]
    fn a_non_2xx_without_an_error_field_falls_back_to_the_status() {
        let response = Response {
            status: 503,
            headers: Vec::new(),
            body: b"<html>down</html>".to_vec(),
        };
        assert_eq!(
            response.json::<serde_json::Value>().unwrap_err(),
            Error::Transport("HTTP 503".to_string())
        );
    }

    #[test]
    fn headers_are_matched_case_insensitively() {
        let response = Response {
            status: 200,
            headers: vec![("Content-Type".into(), "image/png".into())],
            body: Vec::new(),
        };
        assert_eq!(response.header("content-type"), Some("image/png"));
    }

    /// The reason the body is bytes: a thumbnail is not UTF-8.
    #[test]
    fn a_binary_body_is_not_mangled_into_text() {
        let png = vec![0x89, b'P', b'N', b'G', 0xff, 0xfe];
        let response = Response {
            status: 200,
            headers: Vec::new(),
            body: png.clone(),
        };
        assert_eq!(response.body, png);
    }

    /// A body that would not serialise must reach the caller as an error rather
    /// than being sent as an empty request.
    #[test]
    fn an_unserialisable_json_body_fails_the_send() {
        // A map with non-string keys is the classic `serde_json` refusal.
        let mut bad = std::collections::HashMap::new();
        bad.insert(vec![1u8, 2], "value");
        let request = Request::post("https://example.com").json(&bad);
        assert!(matches!(fetch(request), Err(Error::Decode(_))));
    }

    /// `Shared` is process-global and cargo runs tests in parallel, so the two
    /// tests below would otherwise flip each other's availability flag mid-run.
    /// Held for the length of each, rather than per-assertion, because what they
    /// are testing IS the global's state.
    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The sentinel the pump uses for "there is no backend" must arrive as the
    /// typed error, not as a transport message with a control character in it.
    /// The end-to-end path is covered by `tests/round_trip.rs`.
    #[test]
    fn the_no_backend_sentinel_maps_back_to_a_typed_error() {
        assert_eq!(transport_error(NO_BACKEND.as_bytes()), Error::NoBackend);
        assert_eq!(
            transport_error(b"dns failure"),
            Error::Transport("dns failure".to_string())
        );
    }

    /// The watchdog: nothing pumping at all (the deadlock a caller creates by
    /// fetching from inside a system) fails with a message in seconds rather
    /// than parking the thread for the full request timeout.
    #[test]
    fn a_request_with_no_frame_loop_reports_it_rather_than_hanging() {
        let _guard = exclusive();
        let started = std::time::Instant::now();
        let err = fetch(Request::get("https://example.com/never")).unwrap_err();

        assert_eq!(err, Error::NoPump);
        // Comfortably under the 60 s default request timeout, which is the
        // failure mode this replaces.
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
