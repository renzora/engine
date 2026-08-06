//! Plugins compiled **into** the host instead of loaded from `plugins/`.
//!
//! The C ABI does not require a shared library. A plugin exports one function
//! and imports nothing — the interface is passed *in* as a table — so calling
//! that function through a linker-resolved symbol is exactly as valid as calling
//! it through `dlopen`. Everything downstream of `init` is byte-identical; the
//! only thing that changes is how the host got the function pointer.
//!
//! Two consequences make this worth having for a shipped game:
//!
//! * **One file.** A lean export is already a single binary with its assets
//!   appended; shipping a `plugins/` folder beside it puts that back to a folder
//!   of loose libraries a player can break by deleting one.
//! * **Nothing to load at boot.** No directory scan, no shadow copies, no OS
//!   loader work per plugin.
//!
//! What it costs: hot reload (there is no file to watch, and no way to swap code
//! inside a linked binary) and the ability to add a plugin after shipping. Both
//! are development conveniences, which is why this is an export-time choice and
//! never how the editor runs.
//!
//! ## How a plugin ends up here
//!
//! The lean exporter generates a `renzora_static_plugins` crate that depends on
//! the chosen plugins as ordinary rlibs and returns one [`StaticPlugin`] per
//! plugin. The plugins are compiled with this crate's `static_link` feature on,
//! which drops the `#[no_mangle]` from `add!`'s output — without that, two
//! plugins defining `renzora_plugin_init` would fail to link.

use crate::sys;

/// One plugin linked into this binary.
///
/// The dynamic loader's equivalent is a `libloading::Library` plus two symbol
/// lookups; here the compiler has already done both, so a slot is just the two
/// function pointers and a name to log.
pub struct StaticPlugin {
    /// What the plugin would have been called on disk (its crate name). Used for
    /// log lines and as the identity of its registration slot, so a panel or
    /// render pass reads the same whichever way the plugin arrived.
    pub id: &'static str,
    /// Read before `init` is called, exactly as the loader reads
    /// `renzora_plugin_scope` from a library — an Editor-scope plugin linked
    /// into a game binary is skipped rather than run.
    pub scope: sys::PluginScope,
    pub init: sys::ExtensionInit,
}
