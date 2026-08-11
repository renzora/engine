//! Implement the HTTP client as a plugin.
//!
//! The engine ships a networking *API* — the request builder, the blocking
//! facade its background threads call, the inbox scripts read — and no client.
//! This module is the contract between that and whatever actually opens a
//! socket, so an HTTP stack is an ordinary `plugins/<name>/` cdylib built with
//! no engine checkout.
//!
//! ## Why the engine gave up its own client
//!
//! Not architecture for its own sake. `ureq` and the TLS stack under it —
//! rustls, ring, webpki, the platform verifiers — are **twenty packages** that
//! every build of the engine compiled, including a 2D mobile game that never
//! makes a request. Behind this boundary they are compiled by the plugin, once,
//! and a game that ships without the plugin ships without the stack.
//!
//! It also makes the client *replaceable*, which turned out to matter more than
//! expected: a browser build wants `fetch`, a console build wants the platform's
//! own certified HTTP library (and on some of them, shipping your own is a
//! certification failure), and a studio behind a corporate proxy wants theirs.
//! None of those were expressible when the client was a dependency.
//!
//! ## The shape of a call
//!
//! ```text
//!   host                                   plugin
//!   ────                                   ──────
//!   Init                     ────────────▶  build a client
//!                            ◀──────────── BackendInfo (agent + capabilities)
//!   Start (tag, url, method) ────────────▶  spawn the transfer, return at once
//!   Poll                     ─── /frame ──▶
//!                            ◀──────────── whatever finished since last frame
//!   Cancel (tag)             ────────────▶  abandon it
//! ```
//!
//! ### The plugin must not block
//!
//! [`Start`](crate::sys::NetOp::Start) is called from the engine's frame. A
//! backend that performed the transfer inline would stall the editor for a round
//! trip, which is why this is queue-and-poll rather than a `fetch(url) ->
//! Response`. See [`Backend`].
//!
//! The engine layers a *blocking* facade on top for the code that wants one —
//! `renzora_net::fetch` — and it works by parking the calling thread while the
//! frame keeps running. That is only sound because this side stays asynchronous.
//!
//! ### This is not [`crate::http`]
//!
//! [`crate::http`] is the same protocol pointed the other way: a plugin asking
//! the engine to fetch something. Both exist, and a plugin's `http_get` now ends
//! up routed back out to whoever registered here. See `sys/net.rs`.
//!
//! ## Ops, not one function pointer per operation
//!
//! A backend registers a single `extern "C"` entry point and the operation is
//! selected by [`NetOp`](crate::sys::NetOp), for the same reason
//! [`crate::audio`] does it: a struct of named function pointers would make
//! adding a sixth an ABI change, while an unknown op code is merely a capability
//! that backend does not have.

mod backend;
pub mod protocol;

pub use backend::{decode_events, desc_for, dispatch, Backend};

pub use protocol::{
    read_events, write_events, BackendInfo, Caps, Event, EventKind, Request,
};

/// The boundary half, re-exported so a plugin author names one module.
///
/// These live in [`sys`](crate::sys) because they are named by
/// [`Interface`](crate::sys::Interface), and anything in that table is part of
/// the frozen mechanism. Everything else here — the requests, the events, the
/// codec — is vocabulary layered on top and can grow without moving the ABI.
pub use crate::sys::{NetBackendDesc, NetCall, NetEntry, NetOp, NetStatus};
