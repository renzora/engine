//! `renzora_ember` — Renzora's unified **bevy_ui** UI framework.
//!
//! One crate, used by both the editor and exported games, so UI is authored the
//! same way everywhere (Godot-style). Deliberately **no feature flags**: feature
//! deltas would shift `bevy_dylib`'s SVH and re-split the editor/runtime builds
//! that the plugin ABI relies on (see `docs/editor-runtime-plugin-architecture.md`),
//! so everything ships together.
//!
//! Planned modules (migrating in):
//! - [`dock`] — the docking layout component (model is here; the bevy_ui
//!   reconciler + interactions follow from `renzora_shell`).
//! - `markup` — folds in `renzora_hui` (`.html` → bevy_ui, data-binding, vello).
//! - `widgets` — the bevy_ui rewrites of the egui `renzora_ui` widgets.
//! - `cinder` — the particle UI.

pub mod dock;
