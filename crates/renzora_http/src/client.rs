//! The client: one worker thread per request, one shared queue of what they
//! produced.
//!
//! ## Why a thread per request rather than a pool
//!
//! The engine's traffic is a handful of concurrent requests at their peak — a
//! marketplace page and the dozen thumbnails on it — and each is dominated by
//! waiting on a socket. A pool would bound that at some arbitrary width and make
//! the thirteenth thumbnail wait behind the twelfth for no reason, while a
//! thread that spends its life blocked on `read` costs a stack and nothing else.
//!
//! It also keeps cancellation honest: a cancelled request's thread notices at
//! its next read and returns, rather than occupying a pool slot until the server
//! decides to answer.

use std::collections::HashSet;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use renzora_plugin::net::{Backend, BackendInfo, Caps, Event, EventKind, Request};

/// Bytes read per streaming chunk.
///
/// Emitted as whatever each `read` returns rather than split on lines: NDJSON
/// and SSE frame differently, and a client that understood either would have to
/// buffer partial frames and decide when one ended. The consumer already has to
/// accumulate across pieces, so the transport stays dumb and each caller keeps
/// whatever framing its API uses.
const CHUNK: usize = 8192;

/// What a worker thread posts into, and what [`Ureq::poll`] drains.
///
/// A `Mutex<Vec<_>>` rather than a channel because the read side is a single
/// drain-everything call once per frame, which is the one access pattern a
/// channel is worse at: `try_recv` in a loop against a lock taken once.
type Sink = Arc<Mutex<Vec<Event>>>;

/// Tags the host has abandoned. Checked between chunks so a cancelled stream
/// stops reading rather than running to completion with nowhere to go.
type Cancelled = Arc<Mutex<HashSet<u64>>>;

pub struct Ureq {
    agent: ureq::Agent,
    events: Sink,
    cancelled: Cancelled,
}

impl Default for Ureq {
    fn default() -> Self {
        Self {
            agent: build_agent(),
            events: Arc::new(Mutex::new(Vec::new())),
            cancelled: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

/// One agent for the whole process, so connections and TLS sessions are reused
/// across requests.
///
/// `http_status_as_error(false)` is the load-bearing setting: without it `ureq`
/// turns a 4xx into an `Err` and the response body goes with it — and that body
/// is where renzora.com puts `{"error": "…"}`. The engine's error messages used
/// to read "HTTP 400" for exactly this reason.
fn build_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .http_status_as_error(false)
            .user_agent(USER_AGENT)
            .build(),
    )
}

const USER_AGENT: &str = concat!("renzora-http/", env!("CARGO_PKG_VERSION"));

impl Backend for Ureq {
    const NAME: &'static str = "ureq";

    fn init(&mut self) -> Result<BackendInfo, String> {
        Ok(BackendInfo {
            agent: USER_AGENT.to_string(),
            caps: Caps::STREAM | Caps::CANCEL | Caps::HEADERS,
        })
    }

    fn shutdown(&mut self) {
        // The workers are detached and each holds an `Arc` to the sink, so there
        // is nothing to join — they finish their reads and push into a queue
        // nobody drains. Clearing the events drops what is already there;
        // marking every in-flight tag cancelled is what stops the rest.
        if let Ok(mut events) = self.events.lock() {
            events.clear();
        }
    }

    fn start(&mut self, request: &Request, body: &[u8]) -> Result<(), String> {
        // Rejected here, before a thread is spawned, because this is the one
        // failure the host can be told about synchronously — everything after
        // the spawn has to come back as an event.
        if request.url.is_empty() {
            return Err("empty url".to_string());
        }

        let agent = self.agent.clone();
        let events = self.events.clone();
        let cancelled = self.cancelled.clone();
        let request = request.clone();
        let body = body.to_vec();

        std::thread::Builder::new()
            .name("renzora-http".to_string())
            .spawn(move || run(&agent, &request, &body, &events, &cancelled))
            .map_err(|e| format!("could not spawn a request thread: {e}"))?;
        Ok(())
    }

    fn poll(&mut self) -> Vec<Event> {
        self.events
            .lock()
            .map(|mut e| std::mem::take(&mut *e))
            .unwrap_or_default()
    }

    fn cancel(&mut self, tag: u64) {
        if let Ok(mut cancelled) = self.cancelled.lock() {
            cancelled.insert(tag);
        }
    }
}

/// Perform one request on a worker thread.
fn run(
    agent: &ureq::Agent,
    request: &Request,
    body: &[u8],
    events: &Sink,
    cancelled: &Cancelled,
) {
    // Built through the `http` crate rather than `agent.get(..)` / `agent.post(..)`
    // because ureq's own builders are typed by whether the verb carries a body
    // (`RequestBuilder<WithBody>` vs `<WithoutBody>`), which cannot be selected
    // by a runtime string. `Agent::run` takes any method uniformly, which is what
    // this boundary needs — the host may send a verb this build has never heard
    // of, and refusing it here would be a limit invented by the transport.
    let mut builder = ureq::http::Request::builder()
        .method(request.method.as_str())
        .uri(&request.url);
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }

    // Two paths, differing only in the body type. `()` rather than an empty
    // `Vec<u8>` when there are no bytes: ureq reads `BodyMode::NoBody` off the
    // unit type and omits the framing entirely, where an empty vec would send
    // `Content-Length: 0` on every GET — which some servers answer with a 400.
    //
    // A body on a GET is deliberately still possible; this branches on whether
    // there are bytes, not on the verb, because a few APIs do want one.
    let sent = if body.is_empty() {
        builder
            .body(())
            .map_err(|e| e.to_string())
            .and_then(|r| send(agent, r, request.timeout_ms))
    } else {
        builder
            .body(body.to_vec())
            .map_err(|e| e.to_string())
            .and_then(|r| send(agent, r, request.timeout_ms))
    };

    let response = match sent {
        Ok(response) => response,
        // Never reached a response: DNS, connect, TLS, timeout, or a URL the
        // `http` crate would not parse. One Error event carrying the message,
        // which is all the host can report.
        Err(e) => return push(events, error_event(request.tag, 0, &e)),
    };

    let status = response.status().as_u16();
    let headers = collect_headers(&response);

    if !request.stream {
        // `take(limit + 1)` rather than `take(limit)`: reading exactly the limit
        // cannot distinguish a body that just fits from one that was truncated,
        // and silently handing back a truncated image is worse than refusing.
        // The extra byte is the one that proves there was more.
        let limit = cap(request.max_bytes);
        let mut reader = response.into_body().into_reader().take(limit);
        let mut buf = Vec::new();
        return match reader.read_to_end(&mut buf) {
            Ok(_) if over(buf.len() as u64, request.max_bytes) => push(
                events,
                error_event(
                    request.tag,
                    status,
                    &format!("response body exceeded {} bytes", request.max_bytes),
                ),
            ),
            Ok(_) => push(
                events,
                Event {
                    tag: request.tag,
                    kind: EventKind::Response,
                    status,
                    headers,
                    body: buf,
                },
            ),
            // The status is already known and worth keeping — a 200 that died
            // halfway is a different problem from a 500.
            Err(e) => push(events, error_event(request.tag, status, &e.to_string())),
        };
    }

    // Streaming. Every exit path emits exactly one terminal event, because the
    // host polls until it sees one — a path that returned without it would leave
    // a caller parked on a stream that already finished.
    let mut reader = response.into_body().into_reader();
    let mut buf = [0u8; CHUNK];
    let mut headers = Some(headers);
    let mut sent_bytes = 0u64;
    loop {
        if is_cancelled(cancelled, request.tag) {
            // No terminal event: the host cancelled, so it has already stopped
            // listening and `deliver` would drop this anyway.
            return;
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                sent_bytes += n as u64;
                if over(sent_bytes, request.max_bytes) {
                    return push(
                        events,
                        error_event(
                            request.tag,
                            status,
                            &format!("response body exceeded {} bytes", request.max_bytes),
                        ),
                    );
                }
                push(
                    events,
                    Event {
                        tag: request.tag,
                        kind: EventKind::Chunk,
                        status,
                        // Sent once, with the first piece. Repeating them on
                        // every chunk of a long stream would be the same few
                        // hundred bytes copied across the boundary hundreds of
                        // times.
                        headers: headers.take().unwrap_or_default(),
                        body: buf[..n].to_vec(),
                    },
                );
            }
            Err(e) => return push(events, error_event(request.tag, status, &e.to_string())),
        }
    }
    push(
        events,
        Event {
            tag: request.tag,
            kind: EventKind::End,
            status,
            headers: headers.take().unwrap_or_default(),
            body: Vec::new(),
        },
    );
}

/// Apply the per-request timeout and run it.
///
/// Generic over the body type so the two branches in [`run`] — `()` for a
/// bodiless request, `Vec<u8>` for one with bytes — share this rather than
/// duplicating the timeout handling, which is the part that would silently drift.
///
/// A per-request timeout rather than a per-agent one because callers genuinely
/// differ: a thumbnail should give up in seconds, a runtime download should not.
/// `timeout_ms == 0` means the agent's own default.
fn send<S: ureq::AsSendBody>(
    agent: &ureq::Agent,
    request: ureq::http::Request<S>,
    timeout_ms: u32,
) -> Result<ureq::http::Response<ureq::Body>, String> {
    let request = if timeout_ms > 0 {
        agent
            .configure_request(request)
            .timeout_global(Some(Duration::from_millis(timeout_ms as u64)))
            .build()
    } else {
        request
    };
    agent.run(request).map_err(|e| e.to_string())
}

fn collect_headers(response: &ureq::http::Response<ureq::Body>) -> Vec<(String, String)> {
    response
        .headers()
        .iter()
        // Header values are bytes and may legitimately not be UTF-8. Dropping
        // such a header is better than a lossy conversion that would hand the
        // host a value it might send back somewhere.
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect()
}

fn error_event(tag: u64, status: u16, message: &str) -> Event {
    Event {
        tag,
        kind: EventKind::Error,
        status,
        headers: Vec::new(),
        body: message.as_bytes().to_vec(),
    }
}

fn push(events: &Sink, event: Event) {
    if let Ok(mut queue) = events.lock() {
        queue.push(event);
    }
}

fn is_cancelled(cancelled: &Cancelled, tag: u64) -> bool {
    cancelled.lock().map(|c| c.contains(&tag)).unwrap_or(false)
}

/// How many bytes to allow the reader, given a [`Request::max_bytes`] where `0`
/// means unlimited. One past the limit, so the caller can tell "exactly fits"
/// from "was truncated".
fn cap(max_bytes: u32) -> u64 {
    if max_bytes == 0 {
        u64::MAX
    } else {
        max_bytes as u64 + 1
    }
}

/// Whether `read` bytes has passed a [`Request::max_bytes`] of `max`.
fn over(read: u64, max: u32) -> bool {
    max != 0 && read > max as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(tag: u64, url: &str) -> Request {
        Request {
            tag,
            method: "GET".to_string(),
            url: url.to_string(),
            headers: Vec::new(),
            stream: false,
            timeout_ms: 0,
            max_bytes: 0,
        }
    }

    #[test]
    fn init_claims_what_this_backend_actually_does() {
        let info = Ureq::default().init().unwrap();
        assert!(info.caps.contains(Caps::STREAM));
        assert!(info.caps.contains(Caps::CANCEL));
        assert!(info.caps.contains(Caps::HEADERS));
    }

    /// The one failure reportable synchronously — everything after the spawn has
    /// to come back as an event instead.
    #[test]
    fn an_empty_url_is_refused_before_a_thread_is_spawned() {
        let mut backend = Ureq::default();
        assert!(backend.start(&request(1, ""), &[]).is_err());
        assert!(backend.poll().is_empty());
    }

    /// A URL that cannot resolve must produce a terminal Error event, not
    /// silence — a host that never sees one parks until its timeout.
    #[test]
    fn an_unreachable_host_reports_a_terminal_error() {
        let mut backend = Ureq::default();
        // `.invalid` is reserved by RFC 2606 and can never resolve, so this
        // fails without touching the network.
        backend
            .start(&request(1, "http://renzora.invalid/nope"), &[])
            .unwrap();

        let event = wait_for_event(&mut backend);
        assert_eq!(event.tag, 1);
        assert_eq!(event.kind, EventKind::Error);
        assert!(event.kind.is_terminal());
        assert!(!event.body.is_empty(), "an error must say what went wrong");
    }

    #[test]
    fn poll_is_a_drain_rather_than_a_peek() {
        let mut backend = Ureq::default();
        backend
            .start(&request(2, "http://renzora.invalid/nope"), &[])
            .unwrap();
        wait_for_event(&mut backend);
        assert!(backend.poll().is_empty());
    }

    #[test]
    fn cancelling_records_the_tag() {
        let mut backend = Ureq::default();
        backend.cancel(7);
        assert!(is_cancelled(&backend.cancelled, 7));
        assert!(!is_cancelled(&backend.cancelled, 8));
    }

    /// The worker is a real thread, so the test has to wait for it the way the
    /// host does — by polling.
    fn wait_for_event(backend: &mut Ureq) -> Event {
        for _ in 0..600 {
            if let Some(event) = backend.poll().into_iter().next() {
                return event;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the worker never produced an event");
    }
}
