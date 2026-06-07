//! Editor bundle — a single loadable cdylib that statically links every
//! editor-only plugin crate (as rlibs) and exposes them through ONE FFI entry
//! point (`plugin_install_scope`) via [`renzora::export_plugin_bundle!`].
//!
//! This is the "editor as a removable dylib" half of Operation Merge: present
//! beside the host → the binary is the editor; remove the dll → the same
//! binary is the exported game. Both share one `bevy_dylib` (built in one
//! `--workspace` invocation), so a community plugin built once loads in both.
//!
//! Step A (this): the bundle door + skeleton. The macro emits the single
//! collision-free `plugin_install_scope` symbol; the `dynamic_plugin_loader`
//! prefers it over the per-plugin `plugin_create`. Until Step B adds the
//! editor-crate dependencies below, the inventory it replays is empty.
//!
//! ## Hard preconditions (from the Step-A adversarial review)
//!
//! - **Build via `--workspace` only** (never `cargo build -p renzora_editor_bundle`).
//!   `dynamic_linking` reaches this crate's `bevy` solely through resolver-2
//!   feature unification with `renzora_app` under one `--workspace` build. Built
//!   in isolation it would link a *separate, static* bevy → a different
//!   `World` TypeId → `plugin_bevy_hash` mismatch → the loader silently rejects
//!   the bundle ("incompatible bevy version"). The user must verify the built
//!   bundle's hash equals the host's.
//! - **Deployment contract:** the inventory `plugin_install_scope` replays is the
//!   ONE global registry in the shared `renzora` dylib. So a build either
//!   statically links the editor plugins (`add_editor_plugins`) OR ships them as
//!   this bundle — *never both*, or they install twice and Bevy panics. The
//!   runtime-shaped host (Step C) must stop statically registering editor-scope
//!   plugins. Until Step C, this bundle is not safe to drop into a current
//!   *editor* build's `plugins/`.
//! - **Step B keepalive:** when the ~44 editor crates are added as plain deps,
//!   the linker may drop rlibs nothing references, so their `inventory::submit!`
//!   ctors never run (empty bundle). Replicate `renzora_runtime`'s `pub use
//!   renzora_<crate>;` keepalive, and install the SDK foundation in order via the
//!   `export_plugin_bundle!(foundation = [...])` form.

renzora::export_plugin_bundle!();
