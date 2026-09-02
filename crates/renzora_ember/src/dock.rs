//! Dockable panel layout — a reusable bevy_ui component.
//!
//! The [`DockTree`] model (a binary tree of `Split`s and `Leaf`s) plus the
//! bevy_ui reconciler and interaction systems that render and drive it:
//! draggable split dividers, tab drag-docking with drop zones + insertion
//! markers, in-place tab switching, hover, and a tab-bar secondary resize
//! handle. Used by the editor shell and available to games.
//!
//! ## Using it
//! Add [`DockPlugin`], set [`Dock::tree`], spawn a node tagged [`DockArea`]
//! where the dock should render, and flip [`DockDirty`] to build it. Each leaf's
//! content is left to the consumer — query the public [`DockLeaf`] and fill it
//! (the editor fills it with panels; a game with whatever).
//!
//! ## Layout
//! This file is the plugin and the public seam. The rest is split by concern:
//!
//! - [`tree`] — the model, and **nothing else**: no bevy_ui, so the layout is
//!   serialisable and the tree surgery is testable without a `World`
//! - [`reconcile`] — turning a tree into entities, and theming them
//! - [`drag`] — divider resize + tab/leaf drag and drop-zone resolution
//! - [`windows`] — floating dock windows, from tear-off to teardown
//! - [`interact`] — tab switch, focus, hover, close, collapse, undock
//! - [`components`] — what tabs/leaves/dividers carry, and the consumer seams
//! - [`routing`] — which of the three tree kinds a dock area belongs to

use bevy::prelude::*;

pub mod components;
pub mod drag;
mod interact;
mod reconcile;
mod routing;
pub mod tree;
pub mod windows;

// The dock's surface has always been flat (`renzora_ember::dock::DockTree`,
// `::DockLeaf`, `::panel_active`, …) and a dozen crates spell it that way.
// Splitting the file is an internal concern, so the seam is re-exported rather
// than pushed onto every consumer.
pub use components::{
    tab_pane, BottomSnap, BottomSnapRequest, BottomStripMarkers, DockLeaf, DockTab,
    FixedAreaHeader, FocusPanelRequest, FocusedPanel, GlobalCursor, GrabRootDivider, TabPane,
};
pub use drag::DockDragWatch;
pub use tree::{DockTree, DropZone, SplitDirection};
pub use windows::{DockWindowRequest, DockWindowRequests, DockWindowState, DockWindows, FloatingDockArea};

use components::sync_panes;
use windows::DockWindowCloseRequests;

/// Adds the dock reconciler + interaction systems and the resources they need.
pub struct DockPlugin;

impl Plugin for DockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dock>()
            .init_resource::<DockDirty>()
            .init_resource::<FixedDock>()
            .init_resource::<DockWindows>()
            .init_resource::<DockWindowRequests>()
            .init_resource::<DockWindowCloseRequests>()
            .init_resource::<windows::FloatDrag>()
            .init_resource::<GlobalCursor>()
            .init_resource::<FocusPanelRequest>()
            .init_resource::<FocusedPanel>()
            .init_resource::<drag::PendingSwitch>()
            .init_resource::<drag::DraggedDivider>()
            .init_resource::<BottomSnapRequest>()
            .init_resource::<BottomStripMarkers>()
            .init_resource::<GrabRootDivider>()
            .init_resource::<drag::TabDrag>()
            .init_resource::<DockDragWatch>()
            .init_resource::<crate::font::FontRegistry>()
            .add_systems(
                Update,
                (
                    crate::font::load_fonts,
                    crate::font::scan_project_fonts,
                    // Before anything that routes by area: until the fixed
                    // area's entity is known, its leaves look like the
                    // primary's to `area_tree_mut` and a drag would edit the
                    // wrong tree.
                    track_fixed_dock_area,
                    // Screen-space cursor first: divider/tab drags and the
                    // tear-off window follow all read it this frame.
                    routing::track_global_cursor,
                    drag::divider_drag,
                    drag::tab_drag,
                    // Grip-press tear-offs land before the spawn below, so the
                    // window appears the same frame, like the Ctrl+drag path.
                    interact::tab_grip_interact,
                    // Same frame as the tear-off gesture so the new window
                    // appears on the very next present.
                    windows::spawn_dock_windows,
                    // After spawn (picks up the tear-off grab the same frame).
                    windows::float_window_drag,
                    interact::apply_focus_request,
                    interact::apply_tab_switch,
                    interact::tab_hover,
                    interact::tab_close_hover,
                    interact::tab_close_click,
                    // Rebuild last, in the same frame the model mutates, so the
                    // dock doesn't show a stale layout for a frame (flicker).
                    reconcile::rebuild_dock,
                    // Toggle per-tab pane visibility after any rebuild.
                    sync_panes,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    reconcile::add_panel_click,
                    interact::track_focused_panel,
                    reconcile::tab_strip_wheel,
                    reconcile::apply_dock_style,
                    windows::float_window_controls,
                    interact::tab_grip_hover,
                    interact::tab_context_menu,
                    interact::bottom_collapse_click,
                ),
            )
            // In PostUpdate, before `camera_system` evaluates render targets: a
            // dock window and its camera must die in the same frame (see
            // `DockWindowCloseRequests`). This also catches windows despawned
            // externally (e.g. Alt+F4 via bevy's `close_when_requested`) in the
            // same frame their `Window` component is removed.
            .add_systems(
                PostUpdate,
                (windows::process_dock_window_closes, windows::guard_dock_target_cameras)
                    .chain()
                    .before(bevy::camera::CameraUpdateSystems),
            );
    }
}

/// The live dock layout. Set [`Dock::tree`]; divider/tab drags mutate it in
/// place; it persists across rebuilds.
#[derive(Resource)]
pub struct Dock {
    pub tree: DockTree,
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            tree: DockTree::Empty,
        }
    }
}

/// Run-condition: `true` while `id` is the active (visible) tab in its dock
/// leaf — `false` when it's a background tab or not in the dock at all.
///
/// Gate a panel's per-frame *view* systems with `.run_if(panel_active("id"))` so
/// they stand down while the panel isn't on screen. This is the systems-level
/// companion to the reactive layer's hidden-pane skip: reactions/keyed-lists are
/// gated automatically, but plain `Update` systems (directory scans, thumbnail
/// loading, per-tile layout) need an explicit run-condition, or a backgrounded
/// panel keeps burning frame time over entities nobody can see.
///
/// Gate only *view* work — leave a panel's always-on systems (e.g. the console's
/// log capture, which must keep collecting while hidden) ungated.
pub fn panel_active(
    id: &'static str,
) -> impl Fn(Option<Res<Dock>>, Option<Res<FixedDock>>, Option<Res<DockWindows>>) -> bool + Clone {
    move |dock: Option<Res<Dock>>,
          fixed: Option<Res<FixedDock>>,
          wins: Option<Res<DockWindows>>| {
        dock.is_some_and(|d| d.tree.is_active_tab(id))
            || fixed.is_some_and(|f| f.tree.is_active_tab(id))
            || wins.is_some_and(|w| w.0.iter().any(|s| s.tree.is_active_tab(id)))
    }
}

/// Is `panel` the active (visible) tab in the primary dock, the [`FixedDock`],
/// or any floating dock window? The non-run-condition companion to
/// [`panel_active`] for callers that check from a `World` or with resources in
/// hand.
///
/// The fixed area must be counted here or a panel docked into it would be
/// gated off by its own `panel_active` run-condition — visible on screen and
/// not updating, which is worse than the wasted work the gate exists to avoid.
pub fn panel_visible_anywhere(
    id: &str,
    dock: Option<&Dock>,
    fixed: Option<&FixedDock>,
    wins: Option<&DockWindows>,
) -> bool {
    dock.is_some_and(|d| d.tree.is_active_tab(id))
        || fixed.is_some_and(|f| f.tree.is_active_tab(id))
        || wins.is_some_and(|w| w.0.iter().any(|s| s.tree.is_active_tab(id)))
}

/// A second, non-floating dock area pinned wherever the consumer puts its
/// [`FixedDockArea`] node — the editor shell renders it as the global bottom
/// panel, overlaid on the primary dock and shared by every workspace.
///
/// It is a peer of [`Dock`], not a region inside it. That is the whole point:
/// the primary tree belongs to whichever workspace is active and is swapped
/// wholesale on a workspace switch, so anything living inside it is
/// per-workspace by construction. A tree held out here survives the swap, and
/// resizing it cannot perturb the workspace's split ratios because it isn't in
/// that tree to begin with.
///
/// `area` is filled in by [`track_fixed_dock_area`] once the consumer's node
/// exists; the routing helpers use it to tell this area's leaves apart from the
/// primary's. `dirty` is the per-area rebuild flag — the fixed counterpart of
/// [`DockDirty`], mirroring how each floating window carries its own.
#[derive(Resource)]
pub struct FixedDock {
    pub tree: DockTree,
    /// The [`FixedDockArea`] node, once it has been spawned. `None` before then
    /// (and after a chrome rebuild despawns it, until the next one is tracked).
    pub area: Option<Entity>,
    pub dirty: bool,
}

impl Default for FixedDock {
    fn default() -> Self {
        Self {
            tree: DockTree::Empty,
            area: None,
            dirty: false,
        }
    }
}

/// Tag the node the [`FixedDock`] should render into. The consumer owns where
/// this sits and how big it is; the dock only fills it.
#[derive(Component)]
pub struct FixedDockArea;

/// Keep [`FixedDock::area`] pointing at the live [`FixedDockArea`] node.
///
/// The shell despawns and respawns its whole chrome (theme switches, DPI
/// changes), so this cannot be a one-shot at startup — the entity changes
/// identity underneath us. Re-tracking also re-arms `dirty`, because a freshly
/// spawned area is empty and nothing else would ask for it to be filled.
fn track_fixed_dock_area(
    mut fixed: ResMut<FixedDock>,
    areas: Query<Entity, Added<FixedDockArea>>,
) {
    if let Some(area) = areas.iter().next() {
        fixed.area = Some(area);
        fixed.dirty = true;
    }
}

/// Tag the node the dock should render into. Flip [`DockDirty`] to (re)build.
#[derive(Component)]
pub struct DockArea;

/// Set to rebuild the dock subtree (structure changed, or first build). Tab
/// switches do NOT set this — they update in place.
#[derive(Resource, Default)]
pub struct DockDirty(pub bool);

/// Open (or focus) `panel` in the live dock and flag a rebuild.
///
/// The bevy_ui shell renders from this [`Dock`] model, so a panel added
/// programmatically (a "go to Marketplace" button in another panel, the
/// asset-uploader button, …) only becomes visible once [`DockDirty`] is armed.
/// Mutating the tree via `focus_or_add_panel` alone leaves the rebuild
/// un-triggered until something else (e.g. a theme switch) sets the flag — that
/// was the "nothing happens until I change the theme" bug. Always go through
/// this so both steps happen together.
pub fn open_or_focus_panel(world: &mut World, panel: &str) {
    if let Some(mut dock) = world.get_resource_mut::<Dock>() {
        dock.tree.adopt_panel(panel);
    }
    if let Some(mut dirty) = world.get_resource_mut::<DockDirty>() {
        dirty.0 = true;
    }
}

// ── Defaults a consumer overrides ────────────────────────────────────────────

/// Default tab title + icon for a panel id (humanized + a neutral dot).
/// Consumers override with real metadata (e.g. the editor's panel registry).
pub(crate) fn tab_meta(id: &str) -> (String, &'static str) {
    (humanize(id), "circle")
}

/// `code_editor` → `Code Editor`.
pub fn humanize(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
