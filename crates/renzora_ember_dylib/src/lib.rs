//! One shared copy of `renzora_ember`, as a dynamic library.
//!
//! Nothing imports this crate for its API — use `renzora_ember` as normal. The
//! two binaries link it (via `renzora_runtime`'s `dynamic_linking` feature)
//! purely so that ember's compiled code ends up in one image instead of a
//! private copy inside every executable and every native plugin.
//!
//! # Why one copy is required, not merely smaller
//!
//! Size is the visible half: a plugin registering a single panel measured
//! 32.2 MB with ember linked as an rlib, and 0.09 MB once it resolved to this
//! image. That alone would not justify a crate.
//!
//! The reason is **process-global state**, exactly as in `renzora_dylib`. Ember
//! keeps several `static`s that are meant to be one thing per process:
//!
//! * `theme::CURRENT` — the palette every widget colours itself from.
//! * `theme::SYNTAX` / `theme::SHEET` — the syntax palette and the per-widget
//!   stylesheet loaded from `themes/*.toml`.
//! * `font::UI_FONT_SCALE` / `font::THEME_UI_FONT` — UI scale and the theme's
//!   font override.
//! * `toolbar::VIEWPORT_TRAILING` / `VIEWPORT_TOP_STRIP` / `VIEWPORT_TOOL_GROUPS`
//!   — the viewport toolbar contribution lists.
//!
//! Linked statically, a plugin gets its *own* set, and every one of them fails
//! **silently**. The plugin's panel renders in ember's default palette while the
//! rest of the editor is on the user's theme — not an error, just a panel that
//! looks wrong and cannot be made to look right. A plugin calling
//! `register_viewport_tool_group` pushes into a list the shell never reads, so
//! its toolbar button simply never appears. Nothing logs; the features do not
//! happen.
//!
//! Note what is *not* the reason: `TypeId`. A plugin compiled against the same
//! `librenzora_ember-<hash>.rlib` agrees with the host about what
//! `NativePanelBuilders` is either way, because a `TypeId` comes from the crate's
//! stable id and not from which final artifact swallowed it. So
//! `register_panel_content` would reach the right resource even in the broken
//! arrangement — which is precisely what makes it broken rather than obviously
//! broken.
//!
//! # Why the re-export
//!
//! `extern crate renzora_ember;` alone would let the linker drop everything
//! unused. Re-exporting the whole surface keeps the symbols live, which is what
//! makes the image usable by a plugin compiled later against ember's metadata.
//! Same trick, same reason, as `renzora_dylib` and `bevy_dylib`.

// Linked for its side effect only, and load-bearing: it is what makes ember's
// own `renzora` dependency resolve to the shared contract image rather than
// being embedded here a second time. See the dependency comment in `Cargo.toml`
// — rustc rejects the alternative rather than allowing it, naming the contract
// crate's whole dependency closure and nothing about the cause.
extern crate renzora_dylib;

pub use renzora_ember::*;
