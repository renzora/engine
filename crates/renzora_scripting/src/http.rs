//! Async HTTP for scripts — `http_get` / `http_post` Lua verbs.
//!
//! Scripts kick off a request (which becomes a `ScriptCommand::HttpRequest`);
//! `apply_script_commands` spawns a background thread running the blocking
//! `renzora_net` client and pushes the result into [`HttpInbox`]. The execution loop
//! drains the inbox each frame and fires every script's `on_http(name, status,
//! body)` hook (broadcast, like `on_rpc` / `on_ui`). The handling script
//! typically parses the body (`json_parse`) and stashes a value in a variable,
//! which a UI template then binds with `{{ Entity.var }}`.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;

/// Which piece of a streaming response a [`HttpResult`] is.
///
/// Mirrors `renzora_plugin::sys::HttpChunkKind` without depending on it — this
/// module is the script-facing client and predates the plugin boundary; the
/// plugin bridge converts between the two. Keeping them separate is what lets a
/// build without the plugin host still stream to scripts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkKind {
    /// Body bytes; more may follow.
    Data,
    /// The stream ended normally. Empty body.
    End,
    /// The stream failed; the body holds the error text.
    Error,
}

/// One completed HTTP response awaiting dispatch to `on_http` — or one piece of
/// a streaming one.
#[derive(Clone, Debug)]
pub struct HttpResult {
    /// The callback name the script passed to `http_get`/`http_post`.
    pub callback: String,
    /// HTTP status code, or `0` if the request never completed (DNS/connect
    /// error, etc.) — the body then holds the error text.
    pub status: u16,
    /// Response body as a string (or the error text when `status == 0`).
    pub body: String,
    /// `None` for a whole-body response; `Some(kind)` for one piece of a stream
    /// started with [`HttpInbox::request_stream`].
    pub chunk: Option<ChunkKind>,
}

/// Shared landing zone for completed requests. The background request thread
/// pushes here; `drain_http_inbox` empties it each frame. Cloning the resource
/// clones the `Arc`, so threads and systems share one queue.
#[derive(Resource, Clone, Default)]
pub struct HttpInbox {
    results: Arc<Mutex<Vec<HttpResult>>>,
}

impl HttpInbox {
    /// Take everything queued so far (called once per frame by the executor).
    pub fn drain(&self) -> Vec<HttpResult> {
        self.results
            .lock()
            .map(|mut v| std::mem::take(&mut *v))
            .unwrap_or_default()
    }

    /// Take only the results whose callback starts with `prefix`, leaving the
    /// rest queued.
    ///
    /// Scripts and standalone plugins share one client and one queue — there is
    /// no reason to run two — but they have separate consumers, and a plain
    /// [`drain`](Self::drain) by either would swallow the other's responses.
    /// The plugin bridge tags its callbacks with a prefix nothing else uses and
    /// claims only those.
    pub fn drain_matching(&self, prefix: &str) -> Vec<HttpResult> {
        let Ok(mut v) = self.results.lock() else {
            return Vec::new();
        };
        let mut taken = Vec::new();
        let mut i = 0;
        while i < v.len() {
            if v[i].callback.starts_with(prefix) {
                taken.push(v.remove(i));
            } else {
                i += 1;
            }
        }
        taken
    }

    /// Spawn a background thread that performs the request and queues the
    /// result. Returns immediately — the game loop never blocks on the network.
    pub fn request(&self, method: String, url: String, body: Option<String>, callback: String) {
        self.request_with(method, url, body, callback, Vec::new())
    }

    /// As [`request`](Self::request), with extra HTTP headers.
    pub fn request_with(
        &self,
        method: String,
        url: String,
        body: Option<String>,
        callback: String,
        headers: Vec<(String, String)>,
    ) {
        let sink = self.results.clone();
        std::thread::Builder::new()
            .name("renzora-http".into())
            .spawn(move || {
                let (status, body) = run_blocking(&method, &url, body.as_deref(), &headers);
                if let Ok(mut v) = sink.lock() {
                    v.push(HttpResult {
                        callback,
                        status,
                        body,
                        chunk: None,
                    });
                }
            })
            .ok();
    }

    /// As [`request`](Self::request), but queue the response **as it arrives**
    /// instead of once it is complete.
    ///
    /// Each queued [`HttpResult`] carries `chunk: Some(kind)`; the last one for
    /// a callback is `End` or `Error` and has an empty body. A consumer polls
    /// until it sees a terminal kind.
    ///
    /// This exists because a token-streaming chat API is unusable otherwise: the
    /// whole value is seeing the reply appear, and `read_to_string` on a
    /// response that stays open for thirty seconds delivers everything at the
    /// end or not at all.
    pub fn request_stream(
        &self,
        method: String,
        url: String,
        body: Option<String>,
        callback: String,
    ) {
        self.request_stream_with(method, url, body, callback, Vec::new())
    }

    /// As [`request_stream`](Self::request_stream), with extra HTTP headers.
    pub fn request_stream_with(
        &self,
        method: String,
        url: String,
        body: Option<String>,
        callback: String,
        headers: Vec<(String, String)>,
    ) {
        let sink = self.results.clone();
        std::thread::Builder::new()
            .name("renzora-http-stream".into())
            .spawn(move || {
                run_streaming(&method, &url, body.as_deref(), &callback, &sink, &headers);
            })
            .ok();
    }
}

/// Build the request every path below sends.
///
/// The JSON default is a convenience for the common case, but sending it *as
/// well* as a caller's own `Content-Type` would be two of the same header —
/// which some servers reject outright and others resolve unpredictably. So it
/// is applied only when the caller supplied none.
#[cfg(all(not(target_arch = "wasm32"), feature = "script_http"))]
fn build(
    method: &str,
    url: &str,
    body: Option<&str>,
    headers: &[(String, String)],
) -> renzora_net::Request {
    let mut request = renzora_net::Request::new(method, url);
    let has_content_type = headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"));
    if let Some(body) = body.filter(|_| method != "GET") {
        request = if has_content_type {
            // The caller's header is added below; this only carries the bytes.
            let mut r = request;
            r.body = body.as_bytes().to_vec();
            r
        } else {
            request.body("application/json", body.as_bytes())
        };
    }
    for (k, v) in headers {
        request = request.header(k, v);
    }
    request
}

/// Push one chunk onto the shared queue. Separate so both the success and error
/// paths of [`run_streaming`] agree on the shape.
#[allow(dead_code)]
fn push_chunk(
    sink: &Arc<Mutex<Vec<HttpResult>>>,
    callback: &str,
    status: u16,
    body: String,
    kind: ChunkKind,
) {
    if let Ok(mut v) = sink.lock() {
        v.push(HttpResult {
            callback: callback.to_string(),
            status,
            body,
            chunk: Some(kind),
        });
    }
}

/// Perform one blocking request. `(status, body)`; `status == 0` on transport
/// error with the error text in `body`.
///
/// Only compiled with the `script_http` feature (native). The TLS stack no
/// longer lives in this crate — it is `plugins/http`'s, reached through
/// `renzora_net` — so what the feature strips now is the request-building code
/// and the crate's dependency on `renzora_net`, not a megabyte of rustls. It is
/// kept because the lean exporter's `script_http` capability is what tells an
/// exported game it needs no HTTP plugin at all. The `HttpInbox`/`HttpResult`
/// types above stay so `systems::` need no `#[cfg]`.
#[cfg(all(not(target_arch = "wasm32"), feature = "script_http"))]
fn run_blocking(
    method: &str,
    url: &str,
    body: Option<&str>,
    headers: &[(String, String)],
) -> (u16, String) {
    // A status is NOT an error here, and a script sees it as one only if it
    // checks: `renzora_net` reports 4xx as a successful request, which is what
    // lets `on_http` read the error body an API sends with its 400.
    match build(method, url, body, headers).send() {
        Ok(response) => (response.status, response.text()),
        Err(e) => (0, format!("{e}")),
    }
}

/// Perform one request, pushing the body onto `sink` as it arrives.
///
/// Reads into a fixed buffer and emits whatever each `read` returns, rather than
/// trying to split on lines. That is deliberate: NDJSON and SSE frame
/// differently, and a client that understood either would have to buffer partial
/// frames and decide when one ends. The consumer already has to accumulate
/// across chunks, so the transport stays dumb and the plugin keeps whatever
/// framing its API uses.
///
/// A read of 0 is end-of-stream. Every exit path emits exactly one terminal
/// chunk, because a consumer polls until it sees one — a path that returned
/// without it would leave a plugin waiting for a stream that already finished.
#[cfg(all(not(target_arch = "wasm32"), feature = "script_http"))]
fn run_streaming(
    method: &str,
    url: &str,
    body: Option<&str>,
    callback: &str,
    sink: &Arc<Mutex<Vec<HttpResult>>>,
    headers: &[(String, String)],
) {
    let mut stream = match build(method, url, body, headers).send_stream() {
        Ok(s) => s,
        // Never reached a response: one Error chunk carrying the transport error.
        Err(e) => return push_chunk(sink, callback, 0, format!("{e}"), ChunkKind::Error),
    };

    // Every exit path emits exactly one terminal chunk, because a consumer polls
    // until it sees one — a path that returned without it would leave a script
    // waiting on a stream that already finished.
    for chunk in &mut stream {
        // Lossy: a chunk boundary can land mid-codepoint, and refusing the whole
        // stream over a split character would be worse than one replacement
        // char. Framing-sensitive callers should send bytes they can re-split,
        // which both NDJSON and SSE are.
        push_chunk(sink, callback, chunk.status, chunk.text(), ChunkKind::Data);
    }
    let status = stream.status();
    match stream.error() {
        // Mid-stream failure. The status is already known and worth keeping — a
        // 200 that died halfway is a different problem from a 500.
        Some(e) => push_chunk(sink, callback, status, format!("{e}"), ChunkKind::Error),
        None => push_chunk(sink, callback, status, String::new(), ChunkKind::End),
    }
}

/// Streaming counterpart to the disabled `run_blocking` below.
#[cfg(any(target_arch = "wasm32", not(feature = "script_http")))]
fn run_streaming(
    _method: &str,
    _url: &str,
    _body: Option<&str>,
    callback: &str,
    sink: &Arc<Mutex<Vec<HttpResult>>>,
    _headers: &[(String, String)],
) {
    push_chunk(
        sink,
        callback,
        0,
        "http is not available in this build".into(),
        ChunkKind::Error,
    );
}

/// Fallback when script HTTP is unavailable — wasm (no native client yet) or the
/// `script_http` feature stripped by the lean export. `http_get`/`http_post`
/// then resolve to this disabled response instead of reaching the network.
#[cfg(any(target_arch = "wasm32", not(feature = "script_http")))]
fn run_blocking(
    _method: &str,
    _url: &str,
    _body: Option<&str>,
    _headers: &[(String, String)],
) -> (u16, String) {
    (0, "http is not available in this build".into())
}
