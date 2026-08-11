//! The engine's networking API — and no networking.
//!
//! Renzora does not contain an HTTP client. It contains a way to *describe* a
//! request and a way to wait for the answer; something on the other side of the
//! C-ABI plugin boundary opens the socket. In a normal build that something is
//! `plugins/http`, which links `ureq` and the TLS stack under it.
//!
//! This is the same split `renzora_audio` is: the engine keeps everything with a
//! Bevy type in it, the plugin keeps everything with a dependency in it.
//!
//! ## Why
//!
//! `ureq` plus rustls, ring, webpki and the platform certificate verifiers is
//! **twenty packages** that every build of the engine used to compile —
//! including a 2D mobile game that never makes a request. Behind the boundary
//! they are the plugin's dependencies, compiled once, and a game exported
//! without the plugin ships without the stack at all.
//!
//! The replaceability turned out to matter as much as the size. A browser build
//! wants `fetch`; a console build wants the platform's own certified HTTP
//! library, and on some of them shipping your own is a certification failure; a
//! studio behind a corporate proxy wants theirs. None of that was expressible
//! while the client was a dependency of the engine.
//!
//! ## Using it
//!
//! ```ignore
//! use renzora_net::Request;
//!
//! // On a background thread — see below.
//! let assets: Vec<Asset> = Request::get(&url)
//!     .maybe_bearer(token.as_deref())
//!     .send()?
//!     .json()?;
//! ```
//!
//! [`fetch`] blocks the thread it is called on, which is what every network
//! call site in this engine already wanted: a thread that does one request and
//! posts the result into a resource. It does **not** block the frame — the
//! request is queued, the calling thread parks, and [`pump`] hands the answer
//! back a frame or more later.
//!
//! The one rule is the corollary: **do not call it from a system.** A system
//! runs inside the frame that the pump needs in order to make progress. Blocking
//! there waits for something that cannot happen until you return, and you get
//! [`Error::NoPump`] after two seconds rather than a hang.

mod api;
mod pump;

use bevy::prelude::*;

pub use api::{fetch, fetch_stream, is_available, Chunk, Error, Request, Response, Stream};
pub use pump::NetLink;

/// Capabilities a backend may claim, re-exported so a caller checking one does
/// not have to name `renzora_plugin`.
pub use renzora_plugin::net::Caps;

/// Wires the frame pump in. Add it once; everything else is free functions,
/// because the callers are background threads with no access to a `World`.
#[derive(Default)]
pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetLink>().add_systems(
            Last,
            // Chained: adopting a backend runs `Init`, and a request handed to a
            // backend that has not been initialised is a request to a client
            // that does not exist yet. The same ordering matters on the way out
            // — releasing a vanished backend must happen before anything calls
            // through a function pointer into an unmapped library.
            (pump::adopt_backend, pump::pump).chain(),
        );
    }
}

// Runtime, not Editor: a shipped game fetches too — a leaderboard, a patch
// check, whatever a script's `http_get` asks for. A game that ships without the
// HTTP plugin still runs this; it just has no backend to hand requests to, and
// `fetch` says so.
renzora::add!(NetPlugin);
