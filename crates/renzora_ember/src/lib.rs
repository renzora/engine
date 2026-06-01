//! `renzora_ember` — Renzora's unified **bevy_ui** UI framework.
//!
//! One crate, used by both the editor and exported games, so UI is authored the
//! same way everywhere (Godot-style). Deliberately **no feature flags**: feature
//! deltas would shift `bevy_dylib`'s SVH and re-split the editor/runtime builds
//! that the plugin ABI relies on (see `docs/editor-runtime-plugin-architecture.md`),
//! so everything ships together.
//!
//! Modules:
//! - [`theme`] — the bevy-native palette (shared colors).
//! - [`font`] — fonts + text/icon helpers.
//! - [`dock`] — the dockable panel layout component ([`dock::DockPlugin`]).
//!
//! Migrating in next: `markup` (folds in `renzora_hui`), `widgets`, and
//! `cinder` (particle UI).

pub mod dock;
pub mod font;
pub mod theme;
