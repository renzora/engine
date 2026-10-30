//! The companion: a second copy of the engine the editor starts beside itself.
//!
//! The editor should spend its frames on the game being made. Work that can be
//! done somewhere else (profiling, thumbnails, imports, uploads, compiles) moves
//! here, into `renzora --companion`, one kind at a time. This is phase 0: the
//! process, the link between the two, and a job queue proven by a round trip.
//!
//! # Shape
//!
//! - **The same executable.** The editor relaunches itself with `--companion`,
//!   so the two always come from one build and can never disagree about the
//!   protocol. The companion's code is in the editor image, so a shipped game
//!   carries none of it.
//! - **A child of the editor.** It is tied to the editor's lifetime (a job
//!   object on Windows; elsewhere it exits when its socket closes) and its
//!   output lands in the editor's Console under `Companion`. One editor, one
//!   companion: it never outlives the editor or serves a second one.
//! - **Linked by a loopback socket**, newline-separated JSON, first line a
//!   hello carrying a token from the command line. See [`protocol`].
//! - **It never renders the scene.** The editor keeps the viewport and the
//!   authoritative world. An earlier attempt to move the viewport into a child
//!   process ran out of GPU memory with two copies of a big scene's assets.
//!
//! [`CompanionHostPlugin`] is the editor's side; [`install_companion`] builds
//! the companion's own app.

use bevy::prelude::*;

pub mod protocol;

#[cfg(not(target_arch = "wasm32"))]
mod app;
#[cfg(not(target_arch = "wasm32"))]
mod host;

#[cfg(not(target_arch = "wasm32"))]
pub use host::Companion;

/// The editor's side: starts the companion, keeps the link, shows its state in
/// the status bar and opens its window from the top bar.
#[derive(Default)]
pub struct CompanionHostPlugin;

impl Plugin for CompanionHostPlugin {
    fn build(&self, app: &mut App) {
        // The web has one process and nothing to start.
        #[cfg(not(target_arch = "wasm32"))]
        host::build(app);
        #[cfg(target_arch = "wasm32")]
        let _ = app;
    }
}

/// Build the companion's own app: called through the editor image's
/// `renzora_companion_install` entry point by `renzora --companion`, after
/// `renzora_runtime::add_companion_rendering`.
pub fn install_companion(app: &mut App) {
    #[cfg(not(target_arch = "wasm32"))]
    app::build(app);
    #[cfg(target_arch = "wasm32")]
    let _ = app;
}

renzora::add!(CompanionHostPlugin, Editor);
