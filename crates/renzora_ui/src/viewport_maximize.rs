//! Which viewport slot, if any, is filling the editor on its own.
//!
//! All that survives of `dock_tree`, which held the egui-era dock model:
//! `DockTree`, `DockingState`, `default_layout` and the `save_workspace` /
//! `load_saved_workspace` pair. Every one of those was replaced by
//! `renzora_ember::dock` plus `renzora_shell`, and `DockingState` in particular
//! had stopped being inserted by anything at all, so the twenty-eight sites
//! reading it were `if let Some(..)` guards that never fired.
//!
//! This one is different and is why the file exists rather than the module
//! simply going: it is read and written by the viewport toolbar and play mode,
//! and has nothing to do with docking beyond sitting next to it.

use bevy::prelude::Resource;

/// When set to `Some(slot)`, the editor renders only that viewport slot's panel
/// filling the whole layout. A render-time override: the dock's own tree is left
/// untouched, so toggling off restores the exact layout. `None` means no
/// viewport is maximized.
///
/// Per-slot, so the maximize button in each viewport maximizes the view it
/// belongs to.
#[derive(Resource, Default)]
pub struct ViewportMaximized(pub Option<usize>);
