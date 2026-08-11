//! The Renzora HTTP client.
//!
//! The engine ships a networking *API* — the request builder, the blocking
//! facade its background threads call, the frame pump — and no client. This
//! crate is the client: it resolves, connects, negotiates TLS and reads bodies.
//! Drop it in `plugins/` and the editor can reach renzora.com; leave it out and
//! the same binary runs with no network stack at all, carrying none of the cost.
//!
//! That cost is the point. `ureq` and the rustls/ring/webpki stack under it are
//! about twenty packages, and before this plugin existed every build of the
//! engine compiled them — including a 2D mobile game that never makes a request.
//!
//! ## What is not here, deliberately
//!
//! **Anything that knows what a URL is *for*.** No marketplace endpoints, no
//! auth token handling, no JSON shapes, no retry policy. The engine composes the
//! request and interprets the answer; this crate moves bytes. That is what lets
//! a second backend — `fetch` on the web, a console's own certified HTTP library
//! — implement the same contract without sharing a line of code with this one.
//!
//! **Blocking the caller.** [`Backend::start`](renzora_plugin::net::Backend::start)
//! is invoked from the engine's frame, so it spawns and returns; the answer is
//! collected by `poll` on a later frame. See [`client`].

/// The engine's network backend, over `ureq`.
pub mod client;

pub use client::Ureq;

// ── The plugin entry point ───────────────────────────────────────────────────
mod plugin {
    use renzora_plugin::prelude::*;

    // Emits the `extern "C"` entry point and the state it needs. A macro rather
    // than a generic because the entry point must be a bare function pointer
    // with nowhere to carry state, so it needs a `static` — and a `static`
    // cannot be generic over the backend type.
    renzora_plugin::net_backend!(crate::client::Ureq);

    pub struct RenzoraHttpPlugin;

    impl Plugin for RenzoraHttpPlugin {
        fn build(&self, app: &mut App) {
            app.add_net_backend(net_backend::desc());
        }
    }

    renzora_plugin::add!(RenzoraHttpPlugin);
}
