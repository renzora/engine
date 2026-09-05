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
    // host is left with no backend — which the contract already models as "a
    // game that carries no network stack", and reports the same way. The web
    // editor is a compile target rather than a usable product today; when the
    // `fetch` backend lands it registers here, by the same `load_static` call.
    #[cfg(target_arch = "wasm32")]
    fn build(&self, _app: &mut bevy::app::App) {}

    #[cfg(not(target_arch = "wasm32"))]
    fn build(&self, app: &mut bevy::app::App) {
        // Installed through `load_static`, the path the lean exporter uses for
        // a plugin it compiled in. Deliberate rather than incidental: the client
        // implements `renzora_plugin::net::Backend`, and the host adopts a
        // backend by descriptor. Going through the ordinary contract means the
        // editor's built-in client and one loaded from `plugins/` arrive by the
        // same route, take the same registration slot and are reported the same
        // way — rather than the built-in one being a special case every other
        // part of the system has to know about.
        //
        // Three arguments, none of them obvious:
        //
        // * `init` is named directly rather than looked up by string, because
        //   `static_link` makes the entry point an ordinary function instead of
        //   an exported symbol.
        // * `Runtime` scope, not `Editor`. Scope says where a backend may RUN,
        //   and this client works perfectly well in a game. Where it actually
        //   ends up is decided by the `add!` below, which is `Editor`.
        // * `true` for `is_editor` gates Editor-scope plugins; it describes the
        //   host, not this plugin.
        let outcome = renzora_plugin::host::loader::load_static(
            app.world_mut(),
            &renzora_plugin::static_link::StaticPlugin {
                id: "http",
                scope: renzora_plugin::sys::PluginScope::Runtime,
                init: backend::renzora_plugin_init,
            },
            true,
        );
        if !matches!(outcome, renzora_plugin::host::loader::LoadOutcome::Loaded) {
            bevy::log::error!("[net] the built-in HTTP backend did not install: {outcome:?}");
        }
    }
}

// Declared, not hand-wired. `cargo renzora sync` reads this and writes the entry
// into the editor's generated plugin list, so adding the crate is the whole job
// — and `Editor` is what keeps the client out of a shipped game, which is the
// property the plugin form was protecting.
renzora::add!(RenzoraHttpPlugin, Editor);

// ── The backend itself ───────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use renzora_plugin::prelude::*;

    // Emits the descriptor and the state it needs. A macro rather than a generic
    // because the entry point must be a bare function pointer with nowhere to
    // carry state, so it needs a `static` — and a `static` cannot be generic
    // over the backend type.
    renzora_plugin::net_backend!(crate::client::Ureq);

    /// The C-ABI plugin `load_static` installs: one call, registering the
    /// descriptor above.
    pub struct Inner;

    impl Plugin for Inner {
        fn build(&self, app: &mut App) {
            app.add_net_backend(net_backend::desc());
        }
    }

    renzora_plugin::add!(Inner);
}
