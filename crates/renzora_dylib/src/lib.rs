//! One shared copy of the `renzora` contract, as a dynamic library.
//!
//! Nothing imports this crate for its API — use `renzora` as normal. A binary
//! links it (via `renzora_app`/`renzora_editor_app`'s `dynamic_linking` feature)
//! purely so that `renzora`'s compiled code ends up in one image instead of a
//! private copy inside every executable and every plugin.
//!
//! # Why one copy is required, not merely tidier
//!
//! `TypeId` is the obvious reason and it is not the real one — with a single
//! `renzora` rlib in the build, statically linked copies agree on type identity
//! anyway. The reason is **process-global state**. The contract crate owns four
//! `static`s that are meant to be exactly one thing per process:
//!
//! * `lang::STORE` / `lang::REVISION` — the translation table `t()` reads.
//! * `runtime_warnings::BUFFER` — what the Problems panel drains.
//! * `core::console_log::GLOBAL_LOG_BUFFER` — what the Console panel shows.
//! * `core::ASSET_BYTE_LOADER` — how anything reads a file.
//!
//! Linked statically, a plugin gets its *own* set. Every one of them then fails
//! silently rather than loudly: `t()` initialises an empty store and hands back
//! the raw key, warnings and log lines land in buffers nobody drains, and asset
//! reads find no loader installed. Nothing errors; the features simply do not
//! happen. Sharing one image is the only fix.
//!
//! # Why the re-export
//!
//! `extern crate renzora;` alone would let the linker drop everything unused.
//! Re-exporting the whole surface keeps the symbols live, which is what makes
//! the image usable by a plugin compiled later against `renzora`'s metadata.
//! Same trick, same reason, as `bevy_dylib`.

pub use renzora::*;

