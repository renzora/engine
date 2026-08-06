//! Async HTTP for scripts — `http_get` / `http_post` Lua verbs.
//!
//! Scripts kick off a request (which becomes a `ScriptCommand::HttpRequest`);
//! `apply_script_commands` spawns a background thread running the blocking
//! `ureq` client and pushes the result into [`HttpInbox`]. The execution loop
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
        let sink = self.results.clone();
        std::thread::Builder::new()
            .name("renzora-http".into())
            .spawn(move || {
                let (status, body) = run_blocking(&method, &url, body.as_deref());
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
        let sink = self.results.clone();
        std::thread::Builder::new()
            .name("renzora-http-stream".into())
            .spawn(move || {
                run_streaming(&method, &url, body.as_deref(), &callback, &sink);
            })
            .ok();
    }
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
/// Only compiled with the `script_http` feature (native): it's the sole user of
/// `ureq`, so gating it here lets the lean exporter drop the whole rustls/ring
/// TLS stack (~1 MiB) for a game that issues no script HTTP requests. The
/// `HttpInbox`/`HttpResult` types above stay so `systems::` need no `#[cfg]`.
#[cfg(all(not(target_arch = "wasm32"), feature = "script_http"))]
fn run_blocking(method: &str, url: &str, body: Option<&str>) -> (u16, String) {
    let result = match method.to_ascii_uppercase().as_str() {
        "POST" => ureq::post(url)
            .header("Content-Type", "application/json")
            .send(body.unwrap_or("").as_bytes()),
        "PUT" => ureq::put(url)
            .header("Content-Type", "application/json")
            .send(body.unwrap_or("").as_bytes()),
        _ => ureq::get(url).call(),
    };
    match result {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.into_body().read_to_string().unwrap_or_default();
            (status, body)
        }
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
) {
    use std::io::Read;

    let result = match method.to_ascii_uppercase().as_str() {
        "POST" => ureq::post(url)
            .header("Content-Type", "application/json")
            .send(body.unwrap_or("").as_bytes()),
        "PUT" => ureq::put(url)
            .header("Content-Type", "application/json")
            .send(body.unwrap_or("").as_bytes()),
        _ => ureq::get(url).call(),
    };

    let resp = match result {
        Ok(r) => r,
        // Never reached a response: one Error chunk carrying the transport error.
        Err(e) => return push_chunk(sink, callback, 0, format!("{e}"), ChunkKind::Error),
    };

    let status = resp.status().as_u16();
    let mut reader = resp.into_body().into_reader();
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                // Lossy: a chunk boundary can land mid-codepoint, and refusing
                // the whole stream over a split character would be worse than
                // one replacement char. Framing-sensitive callers should send
                // bytes they can re-split, which both NDJSON and SSE are.
                let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                push_chunk(sink, callback, status, text, ChunkKind::Data);
            }
            // Mid-stream failure. The status is already known and worth keeping
            // — a 200 that died halfway is a different problem from a 500.
            Err(e) => {
                return push_chunk(sink, callback, status, format!("{e}"), ChunkKind::Error)
            }
        }
    }
    push_chunk(sink, callback, status, String::new(), ChunkKind::End);
}

/// Streaming counterpart to the disabled `run_blocking` below.
#[cfg(any(target_arch = "wasm32", not(feature = "script_http")))]
fn run_streaming(
    _method: &str,
    _url: &str,
    _body: Option<&str>,
    callback: &str,
    sink: &Arc<Mutex<Vec<HttpResult>>>,
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
/// then resolve to this disabled response instead of pulling in `ureq`.
#[cfg(any(target_arch = "wasm32", not(feature = "script_http")))]
fn run_blocking(_method: &str, _url: &str, _body: Option<&str>) -> (u16, String) {
    (0, "http is not available in this build".into())
}
