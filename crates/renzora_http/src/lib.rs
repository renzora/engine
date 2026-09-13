//! The Renzora HTTP client, as a crate the editor links rather than a plugin it
//! loads.
//!
//! `renzora_net` ships the networking *API* — the request builder, the blocking
//! facade background threads call, the frame pump — and no client. This crate is
//! the client: it resolves, connects, negotiates TLS and reads bodies.
//!
//! # Why this is not only a plugin
//!
//! It was one, and the reasoning was good: `ureq` and the rustls/ring/webpki
//! stack under it are about twenty packages, and every build of the engine used
//! to compile them — including a 2D mobile game that never makes a request.
//! Keeping the client out of the binary is what makes that game cheap.
//!
//! That argument is about **games**, and it still holds — nothing here changes
//! how a game gets its client. What it never covered is the editor, which is not
//! a program that might want a network stack. Nine crates reach for
//! `renzora_net::Request`: the marketplace, the update check, the update
//! installer, engine-source and runtime-template downloads, toolchain
//! provisioning, the splash's release feed, the markdown widget and scripting.
//! Remove the client and all nine fail at once, in nine unrelated-looking ways.
//!
//! The second reason is newer and decides it. Plugins are now built on the
//! machine that installs them, and this is the only one whose dependency graph
//! is heavy enough to matter — and whose first build needs crates.io. An editor
//! whose networking depends on a plugin that must be downloaded to build is an
//! editor that cannot recover from starting offline.
//!
//! # Where it is installed
//!
//! Declared `renzora::add!(.., Editor)`, so the generator wires it into the
//! editor's plugin list and nowhere else. A game export ships the shared
//! libraries and not the editor bundle, so a shipped game is exactly as it was:
//! no client unless it asks for one.
//!
//! # What is not here, deliberately
//!
//! Anything that knows what a URL is *for*. No marketplace endpoints, no auth
//! tokens, no JSON shapes, no retry policy. The engine composes the request and
//! interprets the answer; this crate moves bytes. That is what lets a second
//! backend — `fetch` on the web, a console's own certified library — implement
//! the same contract without sharing a line with this one.

/// The engine's network backend, over `ureq`.
///
/// Absent on wasm, where `ureq`'s rustls/ring stack does not compile (see
/// `Cargo.toml`). The web backend is `fetch`, which reaches the host through the
/// same contract and shares no code with this one — which is the property the
/// module docs above describe.
#[cfg(not(target_arch = "wasm32"))]
pub mod client;

#[cfg(not(target_arch = "wasm32"))]
pub use client::Ureq;

/// Registers the HTTP backend. An ordinary Bevy plugin, so it is declared and
/// installed exactly like every other crate in this workspace.
#[derive(Default)]
pub struct RenzoraHttpPlugin;

impl bevy::app::Plugin for RenzoraHttpPlugin {
    // On wasm there is no `ureq` to register, so this installs nothing and the
    // engine is left with no backend — which the contract already models as "a
    // game that carries no network stack", and reports the same way. The web
    // editor is a compile target rather than a usable product today; when the
    // `fetch` backend lands it registers here, by the same one call.
    #[cfg(target_arch = "wasm32")]
    fn build(&self, _app: &mut bevy::app::App) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn build(&self, app: &mut bevy::app::App) {
        // One call. `Ureq` implements `renzora::net_backend::Backend` and this
        // hands it over; the pump adopts it on the next frame and runs `init`.
        renzora_net::AppNetBackendExt::add_net_backend(app, crate::client::Ureq::default());
    }
}

// Declared, not hand-wired. `cargo renzora sync` reads this and writes the entry
// into the editor's generated plugin list, so adding the crate is the whole job
// — and `Editor` is what keeps the client out of a shipped game, which is the
// property the plugin form was protecting.
renzora::add!(RenzoraHttpPlugin, Editor);
