# Network backends

The engine ships a networking **API** and no network client. What opens a socket
is a backend behind a Rust trait, exactly the way a mixer or a scripting language
is — see [Audio backends](./audio-backends.md) and
[Script backends](./script-backends.md), whose shape this mirrors deliberately.

Build with `renzora_http` present and the editor can reach the marketplace.
Without it the same binary runs offline: nothing panics, no request hangs, every
call reports "no network backend is loaded" and the UI carries on.

## Why HTTP is behind a boundary and not part of the engine

**It was twenty packages every build paid for.** `ureq` plus the TLS stack under
it — rustls, ring, webpki, the platform certificate verifiers — used to be
compiled by every build of the engine, including a 2D mobile game that never
makes a request. Behind the boundary they belong to one crate a build opts into.

**Different platforms need genuinely different implementations.** A browser build
wants `fetch`, where TLS, certificates and the connection pool all come free and
none of it belongs in the wasm blob. A console build wants the platform's own
certified HTTP library — and on some of them, shipping your own is a
certification failure, not a preference. A studio behind a corporate proxy wants
theirs. None of that was expressible while the client was a dependency of the
engine.

## What each side owns

| | |
|---|---|
| **The engine** (`renzora::net` + `renzora_net`) | the request builder, the blocking facade, the frame pump, the tag bookkeeping, and every decision about *what* to fetch and what the answer means |
| **The backend** (`renzora_http`) | DNS, connect, TLS, headers, reading bodies |

The backend knows nothing about the marketplace, auth tokens, JSON shapes or
retry policy. It moves bytes.

### It must not block

`Backend::start` is called from inside the engine's frame. A backend that
performed the transfer there would stall the editor for a round trip, so the
contract is queue-and-poll: `start` spawns and returns, and `poll` is called once
per frame to collect whatever finished.

The engine layers a *blocking* facade on top for the code that wants one, and it
works by parking the **calling** thread while the frame keeps running. That only
holds together because this side stays asynchronous.

### Capabilities are answered, not assumed

`Backend::init` returns a `Caps` bitfield: `STREAM`, `CANCEL`, `HEADERS`. The
engine will not send a cancellation to a backend that did not claim `CANCEL`, and
it *will* expect a streaming request to arrive in pieces if `STREAM` is set.

Claiming honestly matters. A backend that says it streams and then delivers one
body at the end leaves the AI chat panel waiting for tokens that never come.

### An HTTP error status is not an error

A 404 is a **successful** request whose response says 404. Transport failure —
DNS, connect, TLS, timeout, a read that died mid-body — is `EventKind::Error`,
and nothing else is.

This is load-bearing rather than pedantic. renzora.com answers a failed call with
`{"error": "asset name already taken"}` and a 400; a client that turned the
status into an error would throw that body away, and the editor would show "HTTP
400". It used to.

## Using it from the engine

```rust
use renzora_net::Request;

// On a background thread — never from a system, see below.
let assets: Vec<Asset> = Request::get(&url)
    .maybe_bearer(token.as_deref())
    .send()?
    .json()?;
```

`Response::json` treats a non-2xx as an error and surfaces the server's own
`{"error": …}` message, falling back to `HTTP <status>`. `Response::text` and
`Response::body` are there for everything else — the body is `Vec<u8>` because
half the callers are fetching PNGs.

Streaming, for a token-by-token chat reply:

```rust
let mut stream = Request::post(&endpoint).json(&payload).send_stream()?;
for chunk in &mut stream {
    print!("{}", chunk.text());
}
if let Some(e) = stream.error() { /* died halfway */ }
```

Chunks are whatever each read returned, so a frame can straddle two of them. The
transport is deliberately dumb about framing — NDJSON and SSE end a frame
differently — so a consumer that parses lines must reassemble them across chunk
boundaries. `renzora_ai_chat` is the worked example.

### Never call `fetch` from a system

A system runs inside the frame the pump needs in order to make progress, so
blocking there waits for something that cannot happen until you return. You get
`Error::NoPump` after two seconds rather than a hang — but the fix is to spawn a
thread, or use the `HttpInbox` pattern in `renzora_scripting`.

## Writing one

```rust
use bevy::prelude::*;
use renzora::net_backend::*;

#[derive(Default)]
struct MyClient { /* … */ }

impl Backend for MyClient {
    fn name(&self) -> &str { "my_client" }

    fn init(&mut self) -> Result<BackendInfo, String> { /* build a client */ }
    fn start(&mut self, request: &Request, body: &[u8]) -> Result<(), String> { /* spawn */ }
    fn poll(&mut self) -> Vec<Event> { /* drain */ }
    // `shutdown` and `cancel` have defaults
}

pub struct MyHttpPlugin;

impl Plugin for MyHttpPlugin {
    fn build(&self, app: &mut App) {
        app.add_net_backend(MyClient::default());
    }
}

renzora::plugin!(MyHttpPlugin, Runtime);
```

Four required methods.

**A panicking backend is contained, then disabled.** The pump wraps every call in
`catch_unwind`, so one malformed header does not take the editor down — and the
requests already handed over are failed immediately rather than left parked until
their own timeouts. A backend that panicked once is poisoned rather than retried.

**One backend loads.** Two scripting languages coexist because a script picks one
by its file extension; a request carries no such key, and splitting a session's
cookies and connection pool across two clients would break both. The engine keeps
the first registration and logs an error on the second.

## The bundled backend

`renzora_http` is the native one: `ureq`, with rustls underneath. One worker
thread per request — the engine's traffic peaks at a marketplace page and the
dozen thumbnails on it, each dominated by waiting on a socket, so a pool would
bound that arbitrarily for no gain.

Two things worth knowing about it:

- **`http_status_as_error(false)`.** Without it `ureq` turns a 4xx into an `Err`
  and the response body goes with it, which is the behaviour the section above
  exists to prevent.
- **`max_bytes` is enforced as the body arrives**, not after. The callers that
  set it are fetching images from URLs a server chose — a thumbnail, an avatar,
  an image in a README — and a cap applied once the bytes are in memory protects
  nothing.

## What is not covered yet

**WebSockets.** Nothing in the engine opens one any more. The only live
connection was the social layer's, and it was deleted along with the feed,
messages and friends panels — which took `tungstenite`, rustls and ring out of
the dependency graph with it. A backend that wants one still has nowhere to put
it: adding a `send` method with `Open`/`Message`/`Close` event kinds would be a
small, additive change, but there is no longer a caller asking for it.

**Game replication.** `renzora_network` stays where it is, deliberately. It is
per-frame, latency-critical and high-frequency; routing every packet through a
once-a-frame poll and a request queue would cost a frame of latency and a copy
per packet. It wants a ring buffer, not a request queue.
