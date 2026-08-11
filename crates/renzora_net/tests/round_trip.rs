//! The whole chain, end to end: `fetch` on a background thread → the queue →
//! the frame pump → an `extern "C"` call into a backend → its events → back to
//! the parked thread.
//!
//! Every piece of this is unit-tested on its own, and none of that would have
//! caught the failure this exists for: the pieces agreeing about the *protocol*
//! but not about the *choreography*. A backend that answers on the same frame it
//! was started, a terminal event that never arrives, a waiter registered after
//! the pump already looked — each leaves a thread parked forever and every
//! individual test still green.
//!
//! The backend here is a real one as far as the boundary is concerned: it is
//! reached through `NetEntry`, a bare `extern "C"` function pointer, with the
//! request encoded and the events decoded by the same codec a `dlopen`'d plugin
//! would use. What it is *not* is networked — it answers from a table, so the
//! test has no sockets in it and cannot flake.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use renzora_net::{Error, NetPlugin, Request};
use renzora_plugin::host::{PluginNetBackend, PluginNetBackendEntry};
use renzora_plugin::net::{Backend, BackendInfo, Caps, Event, EventKind};

/// Answers from a table, one frame after the request arrives.
///
/// The delay is the point: a backend that replied inside `start` would make the
/// pump's ordering irrelevant and the test would pass whether or not `Poll` ever
/// ran.
#[derive(Default)]
struct Fake {
    pending: Vec<renzora_plugin::net::Request>,
}

impl Backend for Fake {
    const NAME: &'static str = "fake";

    fn init(&mut self) -> Result<BackendInfo, String> {
        Ok(BackendInfo {
            agent: "fake/1".to_string(),
            caps: Caps::STREAM | Caps::CANCEL | Caps::HEADERS,
        })
    }

    fn start(
        &mut self,
        request: &renzora_plugin::net::Request,
        body: &[u8],
    ) -> Result<(), String> {
        let mut request = request.clone();
        // Echo the request body back on the `/echo` route, so the test can prove
        // that bytes crossed in the `blob` slot rather than being dropped.
        if request.url.ends_with("/echo") {
            request.method = String::from_utf8_lossy(body).into_owned();
        }
        self.pending.push(request);
        Ok(())
    }

    fn poll(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        for request in std::mem::take(&mut self.pending) {
            let tag = request.tag;
            if request.url.ends_with("/hang") {
                // Deliberately never answered, so the request is still in flight
                // when a test takes the backend away.
                continue;
            }
            if request.url.ends_with("/boom") {
                out.push(Event {
                    tag,
                    kind: EventKind::Error,
                    status: 0,
                    headers: Vec::new(),
                    body: b"connection refused".to_vec(),
                });
            } else if request.url.ends_with("/echo") {
                out.push(Event {
                    tag,
                    kind: EventKind::Response,
                    status: 200,
                    headers: Vec::new(),
                    body: request.method.into_bytes(),
                });
            } else if request.stream {
                for piece in ["one ", "two ", "three"] {
                    out.push(Event {
                        tag,
                        kind: EventKind::Chunk,
                        status: 200,
                        headers: Vec::new(),
                        body: piece.as_bytes().to_vec(),
                    });
                }
                out.push(Event {
                    tag,
                    kind: EventKind::End,
                    status: 200,
                    headers: Vec::new(),
                    body: Vec::new(),
                });
            } else {
                out.push(Event {
                    tag,
                    kind: EventKind::Response,
                    status: 404,
                    headers: vec![("Content-Type".to_string(), "application/json".to_string())],
                    body: br#"{"error":"no such asset"}"#.to_vec(),
                });
            }
        }
        out
    }
}

renzora_plugin::net_backend!(Fake);

/// `renzora_net`'s request queue is process-global — it has to be, since
/// `fetch` is called from threads with no access to a `World` — so these tests
/// share it and must not overlap. One test's "the backend went away" would
/// otherwise fail another's in-flight request.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

/// Build an app with the backend already registered, the way the plugin loader
/// would have left it.
fn app_with_backend() -> App {
    let mut app = App::new();
    app.add_plugins(NetPlugin);
    let desc = net_backend::desc();
    app.insert_resource(PluginNetBackend(Some(PluginNetBackendEntry {
        name: "fake".to_string(),
        state: desc.state as usize,
        entry: desc.entry,
        owner: 0,
    })));
    app
}

/// Run frames until `done` says the worker has finished, or give up.
///
/// The worker is a real thread parked on a real channel, so the only way to
/// finish it is to actually run the frames it is waiting for — which is exactly
/// the interaction under test.
fn pump_until(app: &mut App, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        app.update();
        if Instant::now() > deadline {
            panic!("the worker never finished");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_request_crosses_the_boundary_and_comes_back() {
    let _guard = exclusive();
    let mut app = app_with_backend();
    let worker = std::thread::spawn(|| Request::get("https://example.com/asset").send());
    pump_until(&mut app, || worker.is_finished());

    let response = worker.join().unwrap().unwrap();
    // A 404 is a SUCCESSFUL request — the property the whole error type is built
    // around, verified here across the real boundary rather than on a struct
    // literal.
    assert_eq!(response.status, 404);
    assert_eq!(response.header("content-type"), Some("application/json"));
    assert_eq!(
        response.json::<serde_json::Value>().unwrap_err(),
        Error::Transport("no such asset".to_string())
    );
}

/// Bodies ride in `NetCall::blob`, not through the codec. If that slot were
/// dropped, every multipart upload would silently post nothing.
#[test]
fn a_request_body_reaches_the_backend() {
    let _guard = exclusive();
    let mut app = app_with_backend();
    let worker = std::thread::spawn(|| {
        Request::post("https://example.com/echo")
            .body("text/plain", "hello from the host")
            .send()
    });
    pump_until(&mut app, || worker.is_finished());

    let response = worker.join().unwrap().unwrap();
    assert_eq!(response.text(), "hello from the host");
}

#[test]
fn a_streaming_response_arrives_in_pieces() {
    let _guard = exclusive();
    let mut app = app_with_backend();
    let worker = std::thread::spawn(|| {
        let mut stream = Request::get("https://example.com/stream").send_stream()?;
        let mut pieces = Vec::new();
        for chunk in &mut stream {
            pieces.push(chunk.text());
        }
        assert!(stream.error().is_none(), "{:?}", stream.error());
        Ok::<_, Error>(pieces)
    });
    pump_until(&mut app, || worker.is_finished());

    let pieces = worker.join().unwrap().unwrap();
    assert_eq!(pieces, ["one ", "two ", "three"]);
}

/// A transport failure must reach the parked thread as an error. If the terminal
/// event went missing, this hangs — which is the failure mode the whole
/// choreography has to rule out.
#[test]
fn a_transport_failure_reaches_the_caller() {
    let _guard = exclusive();
    let mut app = app_with_backend();
    let worker = std::thread::spawn(|| Request::get("https://example.com/boom").send());
    pump_until(&mut app, || worker.is_finished());

    assert_eq!(
        worker.join().unwrap().unwrap_err(),
        Error::Transport("connection refused".to_string())
    );
}

/// Several requests in flight at once must not cross their answers over. Tags are
/// the only thing keeping them apart.
#[test]
fn concurrent_requests_are_not_confused_with_each_other() {
    let _guard = exclusive();
    let mut app = app_with_backend();
    let workers: Vec<_> = (0..8)
        .map(|i| {
            std::thread::spawn(move || {
                Request::post("https://example.com/echo")
                    .body("text/plain", format!("request {i}"))
                    .send()
                    .map(|r| r.text())
            })
        })
        .collect();
    pump_until(&mut app, || workers.iter().all(|w| w.is_finished()));

    for (i, worker) in workers.into_iter().enumerate() {
        assert_eq!(worker.join().unwrap().unwrap(), format!("request {i}"));
    }
}

/// The backend going away mid-flight — a plugin unloaded or hot-reloaded — must
/// fail everything waiting on it. Without this a parked thread waits out its full
/// timeout for an answer that can no longer come from anywhere.
#[test]
fn losing_the_backend_fails_the_requests_waiting_on_it() {
    let _guard = exclusive();
    let mut app = app_with_backend();
    // A route the fake never answers, so the request is still in flight when the
    // backend is taken away.
    let worker = std::thread::spawn(|| Request::get("https://example.com/hang").send_stream());

    // One frame to hand the request over, then pull the backend out.
    app.update();
    app.update();
    app.insert_resource(PluginNetBackend(None));
    app.update();

    let mut stream = worker.join().unwrap().unwrap();
    // The stream ends immediately and says why, rather than yielding pieces.
    assert!(stream.next().is_none());
    assert!(
        matches!(stream.error(), Some(Error::Transport(_))),
        "{:?}",
        stream.error()
    );
}

/// The regression that broke sign-in: a request issued before the HTTP plugin
/// has registered must WAIT for it, not fail.
///
/// The plugin loader runs during the first frames, so a startup thread can
/// genuinely get there first. Failing those requests turned every early caller
/// into a race — and in `renzora_auth` it read as "your token expired", which
/// deleted the saved session and asked for a sign-in on every launch.
#[test]
fn a_request_made_before_the_backend_loads_waits_for_it() {
    let _guard = exclusive();
    let mut app = App::new();
    app.add_plugins(NetPlugin);

    // Requested first, with no backend registered at all.
    let worker = std::thread::spawn(|| Request::post("https://example.com/echo")
        .body("text/plain", "early")
        .send());
    for _ in 0..5 {
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!worker.is_finished(), "the request should still be waiting");

    // The plugin loads.
    let desc = net_backend::desc();
    app.insert_resource(PluginNetBackend(Some(PluginNetBackendEntry {
        name: "fake".to_string(),
        state: desc.state as usize,
        entry: desc.entry,
        owner: 0,
    })));
    pump_until(&mut app, || worker.is_finished());

    assert_eq!(worker.join().unwrap().unwrap().text(), "early");
}

/// The other side of that grace period: a build that genuinely ships no HTTP
/// plugin has to say so, rather than parking every request until its timeout.
#[test]
fn with_no_backend_at_all_a_request_eventually_fails() {
    let _guard = exclusive();
    let mut app = App::new();
    app.add_plugins(NetPlugin);

    let worker = std::thread::spawn(|| Request::get("https://example.com/asset").send());
    // Past the grace period. Cheaper than sleeping it out — the pump counts
    // frames, not wall-clock.
    for _ in 0..400 {
        app.update();
    }
    pump_until(&mut app, || worker.is_finished());

    assert_eq!(worker.join().unwrap().unwrap_err(), Error::NoBackend);
}
