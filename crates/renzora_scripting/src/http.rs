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

/// One completed HTTP response awaiting dispatch to `on_http`.
#[derive(Clone, Debug)]
pub struct HttpResult {
    /// The callback name the script passed to `http_get`/`http_post`.
    pub callback: String,
    /// HTTP status code, or `0` if the request never completed (DNS/connect
    /// error, etc.) — the body then holds the error text.
    pub status: u16,
    /// Response body as a string (or the error text when `status == 0`).
    pub body: String,
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
                    });
                }
            })
            .ok();
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

/// Fallback when script HTTP is unavailable — wasm (no native client yet) or the
/// `script_http` feature stripped by the lean export. `http_get`/`http_post`
/// then resolve to this disabled response instead of pulling in `ureq`.
#[cfg(any(target_arch = "wasm32", not(feature = "script_http")))]
fn run_blocking(_method: &str, _url: &str, _body: Option<&str>) -> (u16, String) {
    (0, "http is not available in this build".into())
}
