//! The half of native-plugin compilation that two crates both need.
//!
//! A native plugin gets built in two places, and they have to agree:
//!
//! * **`renzora_plugin_build`** — the editor, compiling a plugin a user
//!   installed into a downloaded build.
//! * **`xtask`** — a source checkout, compiling the repo's own plugins during
//!   staging so an author exercises the real path rather than a dev shortcut.
//!
//! Both assemble the same `rustc` invocation: `-C prefer-dynamic`, the three
//! `--extern`s onto the shared images, the SDK's `-L` paths, and now a plugin's
//! third-party dependencies. Each used to hold its own copy, which meant a
//! change to one silently drifted from the other — and the symptom is a plugin
//! that builds for a developer and not for a user, with neither side looking
//! wrong on its own.
//!
//! # Why this is a separate crate rather than a dependency on the other
//!
//! `xtask` could simply depend on `renzora_plugin_build`. That is *safe*: cargo
//! reads a workspace root to resolve `[lints] workspace = true` but does not
//! validate its sibling members, so an outside crate can path-depend on a member
//! of a broken workspace and still build. Measured directly — with a deliberately
//! unloadable workspace, `cargo metadata` on it fails (exit 101) while an
//! outsider depending on one of its members builds fine. The property xtask
//! protects, that it must still compile when `sync.rs` has left the engine
//! workspace unloadable, survives.
//!
//! It fails on the other constraint. `renzora_plugin_build` pulls `serde`,
//! `serde_json`, `tar` and `lzma-rs`; the last two exist only to unpack
//! `sdk.tar.zst`, which xtask has no business doing. xtask is documented as a
//! "tiny, instant-compiling helper" with zero dependencies, and inheriting four
//! trees to share a hundred lines is the wrong trade.
//!
//! So the shared half lives here with **no dependencies of its own**, and xtask
//! picks up one path dependency that pulls nothing.

pub mod deps;
pub mod install;
pub mod json;
pub mod rustc;

pub use deps::Deps;
pub use rustc::Target;
