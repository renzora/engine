//! What the engine knows about the machine and the install it is running from.
//!
//! A layer of the contract hierarchy, above `renzora`. Plugins reach every
//! module and item at the path it always had (`renzora::version`,
//! `renzora::PluginUpdates`, `renzora::RustToolchain`, ...) through
//! `renzora_sdk`. The engine's own crates depend on this crate directly.
//!
//! The modules that used to be flattened into `renzora`'s root are flattened
//! into this crate's root the same way, so a caller writes
//! `renzora_platform_api::PluginUpdates` and never has to know which module it is in.

// ── Engine version / release channel ─────────────────────────────────────
// `ENGINE_VERSION` (`r1-alphaN`) plus the release-tag resolution the export
// downloader needs to ask GitHub for *its own* version's runtime templates
// instead of whatever `releases/latest` happens to be. In the contract because
// the shell, the splash, the command palette and the exporter all need the same
// answer: they used to carry four different ones. Call sites read
// `version::ENGINE_VERSION`.
pub mod version;

// ── Native plugins ───────────────────────────────────────────────────────
// `plugin!`, the scope byte and the ABI record: what a plugin installed into
// `plugins/` emits, and what the loader reads back. `plugin!`'s expansion
// names `$crate::bevy::...`, hence the re-export.
pub use bevy;
mod native_plugin;
pub use native_plugin::{
    native_plugin_abi_record, NativePluginScope, WithEditor, NATIVE_PLUGIN_ABI_MARKER,
    NATIVE_PLUGIN_ABI_RECORD_LEN,
};

/// Whether scripts and plugins can be compiled here. See [`rust_toolchain::RustToolchain`].
pub mod rust_toolchain;
pub use rust_toolchain::{RustToolchain, RustToolchainAnswer};

// Copies of the editor it starts (Play's game, the companion): tied to the
// editor's lifetime and heard in its Console. In the contract because more than
// one crate launches them, and the job object they join must be one per process.
#[cfg(not(target_arch = "wasm32"))]
pub mod child_process;

// NVIDIA's DLSS runtime libraries: where the editor installs them and how the
// engine finds them at startup. In the contract because `main`, the Solari
// inspector and the exporter all need the same answer.
pub mod dlss_runtime;

// `renzora://` links from the website: registering the scheme, handing a link
// to the editor already running, and the queue the marketplace reads. In the
// contract because `main` does the first two before any editor crate is loaded.
// Not flattened: `push`, `take` and `listen` mean nothing without the module.
pub mod app_link;

// Window resizing helpers (not flattened: the names are generic).
pub mod resize;

pub mod auth; // sign-in state mirrored for the title bar
pub mod load_work; // what a freshly loaded scene still has to build
pub mod plugin_inventory; // what plugins were found on disk, and their state
pub mod plugin_updates; // what the marketplace publishes for those plugins
pub mod project_index; // every file in the open project, walked once and kept current
pub mod streaming; // world-streaming gate + camera-position helpers

pub mod companion; // the queue editor crates hand companion jobs to (moved from renzora::core)
pub use auth::*;
pub use companion::*;
pub use load_work::*;
pub use plugin_inventory::*;
pub use plugin_updates::*;
pub use project_index::*;
pub use streaming::*;
