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

use std::collections::HashMap;

use bevy::prelude::*;

use crate::font::{glyph, icon_text, ui_font, EmberFonts};
use crate::theme::{
    accent, border, close_red, divider, header_bg, panel_bg, rgb, tab_active, text_muted,
    text_primary,
};

// ── Model ────────────────────────────────────────────────────────────────────

/// Direction of a split in the dock tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SplitDirection {
    /// Children are side-by-side (left / right).
    Horizontal,
    /// Children are stacked (top / bottom).
    Vertical,
}

/// A node in the dock tree.
///
/// `Serialize`/`Deserialize` so the editor shell can persist each workspace's
/// layout (split ratios, panel placement, active tabs) to disk and restore it
/// across sessions. The recursive `Box<DockTree>` children serialize fine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DockTree {
    Split {
        direction: SplitDirection,
        /// Fraction of space given to the first child (0.1–0.9).
        ratio: f32,
        first: Box<DockTree>,
        second: Box<DockTree>,
    },
    Leaf {
        /// Panel IDs shown as tabs.
        tabs: Vec<String>,
        /// Index of the currently visible tab.
        active_tab: usize,
    },
    Empty,
}

impl DockTree {
    /// Single-tab leaf.
    pub fn leaf(id: impl Into<String>) -> Self {
        DockTree::Leaf {
            tabs: vec![id.into()],
            active_tab: 0,
        }
    }

    /// A leaf with several tabbed panels.
    pub fn tabs(tabs: &[&str]) -> Self {
        DockTree::Leaf {
            tabs: tabs.iter().map(|s| s.to_string()).collect(),
            active_tab: 0,
        }
    }

    /// Horizontal split (left / right).
    pub fn horizontal(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Horizontal,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Vertical split (top / bottom).
    pub fn vertical(first: DockTree, second: DockTree, ratio: f32) -> Self {
        DockTree::Split {
            direction: SplitDirection::Vertical,
            ratio: ratio.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Set the split ratio at `path` (`false`/`true` = first/second child;
    /// empty path targets this node). Persists a divider drag.
    pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
        if let DockTree::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            match path.split_first() {
                Some((&true, rest)) => second.update_ratio(rest, new_ratio),
                Some((&false, rest)) => first.update_ratio(rest, new_ratio),
                None => *ratio = new_ratio.clamp(0.1, 0.9),
            }
        }
    }

    /// The leaf that contains `panel`, if any.
    pub fn find_leaf_mut(&mut self, panel: &str) -> Option<&mut DockTree> {
        match self {
            DockTree::Split { first, second, .. } => first
                .find_leaf_mut(panel)
                .or_else(|| second.find_leaf_mut(panel)),
            DockTree::Leaf { tabs, .. } => tabs.iter().any(|t| t == panel).then_some(self),
            DockTree::Empty => None,
        }
    }

    /// Collect every panel id in the tree (all tabs of all leaves) into `out`,
    /// in tree order. Used when a floating dock window closes to hand its
    /// panels back to the main dock instead of silently dropping them.
    pub fn collect_panels(&self, out: &mut Vec<String>) {
        match self {
            DockTree::Split { first, second, .. } => {
                first.collect_panels(out);
                second.collect_panels(out);
            }
            DockTree::Leaf { tabs, .. } => out.extend(tabs.iter().cloned()),
            DockTree::Empty => {}
        }
    }

    /// The first panel id in tree order, if any — used to title a floating
    /// dock window after its lead panel.
    pub fn first_panel(&self) -> Option<&str> {
        match self {
            DockTree::Split { first, second, .. } => {
                first.first_panel().or_else(|| second.first_panel())
            }
            DockTree::Leaf { tabs, .. } => tabs.first().map(|s| s.as_str()),
            DockTree::Empty => None,
        }
    }

    /// Add `panel` to the tree wherever it fits: focus it if present, append to
    /// the first leaf, or become the root leaf when the tree is empty (which
    /// `focus_or_add_panel` alone can't do — its walk has no leaf to append to).
    pub fn adopt_panel(&mut self, panel: &str) {
        if self.is_empty() {
            *self = DockTree::leaf(panel);
        } else {
            self.focus_or_add_panel(panel);
        }
    }

    /// Is `panel` present anywhere in the tree (visible or as a background tab)?
    pub fn contains_panel(&self, panel: &str) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                first.contains_panel(panel) || second.contains_panel(panel)
            }
            DockTree::Leaf { tabs, .. } => tabs.iter().any(|t| t == panel),
            DockTree::Empty => false,
        }
    }

    /// Is `panel` the active (visible) tab in its leaf?
    pub fn is_active_tab(&self, panel: &str) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                first.is_active_tab(panel) || second.is_active_tab(panel)
            }
            DockTree::Leaf { tabs, active_tab } => {
                tabs.get(*active_tab).is_some_and(|t| t == panel)
            }
            DockTree::Empty => false,
        }
    }

    /// Collect the active (visible) panel id of every leaf into `out`. The dock
    /// rebuild uses this to decide which preserved content entities the new tree
    /// will reuse — only those may be detached-to-root, because a content node
    /// that is detached *and then despawned* in the same frame corrupts bevy_ui's
    /// taffy tree (the old leaf still lists it as a child while its slotmap key is
    /// freed → `invalid SlotMap key` panic). Contents not in this set stay
    /// attached and die safely with their old leaf.
    pub fn active_tab_ids(&self, out: &mut std::collections::HashSet<String>) {
        match self {
            DockTree::Split { first, second, .. } => {
                first.active_tab_ids(out);
                second.active_tab_ids(out);
            }
            DockTree::Leaf { tabs, active_tab } => {
                if let Some(id) = tabs.get(*active_tab) {
                    out.insert(id.clone());
                }
            }
            DockTree::Empty => {}
        }
    }

    /// Make `panel` the active tab in its leaf.
    pub fn set_active_tab(&mut self, panel: &str) {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(panel) {
            if let Some(idx) = tabs.iter().position(|t| t == panel) {
                *active_tab = idx;
            }
        }
    }

    /// Focus `panel` if it's already somewhere in the tree; otherwise append it
    /// to the first leaf (making it visible). Returns `true` if it was added.
    pub fn focus_or_add_panel(&mut self, panel: &str) -> bool {
        if self.find_leaf_mut(panel).is_some() {
            self.set_active_tab(panel);
            return false;
        }
        fn add_to_first_leaf(tree: &mut DockTree, panel: String) -> bool {
            match tree {
                DockTree::Leaf { tabs, active_tab } => {
                    tabs.push(panel);
                    *active_tab = tabs.len() - 1;
                    true
                }
                DockTree::Split { first, .. } => add_to_first_leaf(first, panel),
                DockTree::Empty => false,
            }
        }
        add_to_first_leaf(self, panel.to_string())
    }

    /// Append `new_panel` as a tab in the leaf containing `sibling`.
    pub fn add_tab(&mut self, sibling: &str, new_panel: String) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            tabs.push(new_panel);
            *active_tab = tabs.len() - 1;
            true
        } else {
            false
        }
    }

    /// Insert `new_panel` into `sibling`'s leaf before `before` (or at the end).
    pub fn add_tab_before(&mut self, sibling: &str, new_panel: String, before: Option<&str>) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            let idx = before
                .and_then(|b| tabs.iter().position(|t| t == b))
                .unwrap_or(tabs.len())
                .min(tabs.len());
            tabs.insert(idx, new_panel);
            *active_tab = idx;
            true
        } else {
            false
        }
    }

    /// Merge several tabs into the leaf containing `sibling`, in order, at
    /// `before` (append when `None`). Group-drag counterpart of
    /// [`Self::add_tab_before`]; the first inserted tab becomes active.
    pub fn add_tabs_before(
        &mut self,
        sibling: &str,
        new_tabs: &[String],
        before: Option<&str>,
    ) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            let idx = before
                .and_then(|b| tabs.iter().position(|t| t == b))
                .unwrap_or(tabs.len())
                .min(tabs.len());
            for (n, t) in new_tabs.iter().enumerate() {
                tabs.insert(idx + n, t.clone());
            }
            if !new_tabs.is_empty() {
                *active_tab = idx;
            }
            true
        } else {
            false
        }
    }

    /// Remove a panel from the tree, collapsing any emptied leaves/splits.
    pub fn remove_panel(&mut self, panel: &str) -> bool {
        let removed = match self {
            DockTree::Split { first, second, .. } => {
                first.remove_panel(panel) || second.remove_panel(panel)
            }
            DockTree::Leaf { tabs, active_tab } => {
                if let Some(idx) = tabs.iter().position(|t| t == panel) {
                    tabs.remove(idx);
                    if *active_tab >= tabs.len() && !tabs.is_empty() {
                        *active_tab = tabs.len() - 1;
                    }
                    true
                } else {
                    false
                }
            }
            DockTree::Empty => false,
        };
        if removed {
            self.cleanup_empty();
        }
        removed
    }

    fn cleanup_empty(&mut self) {
        if let DockTree::Split { first, second, .. } = self {
            first.cleanup_empty();
            second.cleanup_empty();
            let first_empty = first.is_empty();
            let second_empty = second.is_empty();
            if first_empty {
                *self = std::mem::replace(second, DockTree::Empty);
            } else if second_empty {
                *self = std::mem::replace(first, DockTree::Empty);
            }
        } else if let DockTree::Leaf { tabs, .. } = self {
            if tabs.is_empty() {
                *self = DockTree::Empty;
            }
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, DockTree::Empty)
            || matches!(self, DockTree::Leaf { tabs, .. } if tabs.is_empty())
    }

    /// Split the leaf containing `target`, placing `new_panel` on the given side.
    pub fn split_at(&mut self, target: &str, new_panel: String, zone: DropZone) -> bool {
        if let Some(leaf) = self.find_leaf_mut(target) {
            let old = std::mem::replace(leaf, DockTree::Empty);
            let new_leaf = DockTree::leaf(new_panel);
            *leaf = match zone {
                DropZone::Left => DockTree::horizontal(new_leaf, old, 0.5),
                DropZone::Right => DockTree::horizontal(old, new_leaf, 0.5),
                DropZone::Top => DockTree::vertical(new_leaf, old, 0.5),
                DropZone::Bottom => DockTree::vertical(old, new_leaf, 0.5),
                DropZone::Center => {
                    *leaf = old;
                    return false;
                }
            };
            true
        } else {
            false
        }
    }

    /// Split the leaf containing `target`, placing an arbitrary `subtree` on
    /// the given side. Group-drag counterpart of [`Self::split_at`] — the
    /// dragged leaf's whole tab set moves as one unit.
    pub fn split_at_with(&mut self, target: &str, subtree: DockTree, zone: DropZone) -> bool {
        if subtree.is_empty() {
            return true;
        }
        if let Some(leaf) = self.find_leaf_mut(target) {
            let old = std::mem::replace(leaf, DockTree::Empty);
            *leaf = match zone {
                DropZone::Left => DockTree::horizontal(subtree, old, 0.5),
                DropZone::Right => DockTree::horizontal(old, subtree, 0.5),
                DropZone::Top => DockTree::vertical(subtree, old, 0.5),
                DropZone::Bottom => DockTree::vertical(old, subtree, 0.5),
                DropZone::Center => {
                    *leaf = old;
                    return false;
                }
            };
            true
        } else {
            false
        }
    }

    /// Root-edge split with an arbitrary `subtree` — group-drag counterpart of
    /// [`Self::split_root`]. `Center` merges the subtree's tabs into the tree
    /// (adopting each panel) since a root center-drop has no side to take.
    pub fn split_root_with(&mut self, subtree: DockTree, zone: DropZone) {
        if subtree.is_empty() {
            return;
        }
        if self.is_empty() {
            *self = subtree;
            return;
        }
        if matches!(zone, DropZone::Center) {
            let mut ids = Vec::new();
            subtree.collect_panels(&mut ids);
            for id in ids {
                self.adopt_panel(&id);
            }
            return;
        }
        let old = std::mem::replace(self, DockTree::Empty);
        *self = match zone {
            DropZone::Left => DockTree::horizontal(subtree, old, ROOT_DOCK_RATIO),
            DropZone::Right => DockTree::horizontal(old, subtree, 1.0 - ROOT_DOCK_RATIO),
            DropZone::Top => DockTree::vertical(subtree, old, ROOT_DOCK_RATIO),
            DropZone::Bottom => DockTree::vertical(old, subtree, 1.0 - ROOT_DOCK_RATIO),
            DropZone::Center => unreachable!(),
        };
    }

    /// Split the WHOLE tree against one edge: the new panel gets a full-height
    /// column (`Left`/`Right`) or full-width row (`Top`/`Bottom`) spanning the
    /// entire dock, regardless of the existing split structure. This is the
    /// edge/corner-drop gesture; `Center` (not a root gesture) just adds the
    /// panel as a tab in the first leaf.
    pub fn split_root(&mut self, new_panel: String, zone: DropZone) {
        if self.is_empty() {
            *self = DockTree::leaf(new_panel);
            return;
        }
        if matches!(zone, DropZone::Center) {
            self.focus_or_add_panel(&new_panel);
            return;
        }
        let old = std::mem::replace(self, DockTree::Empty);
        let new_leaf = DockTree::leaf(new_panel);
        *self = match zone {
            DropZone::Left => DockTree::horizontal(new_leaf, old, ROOT_DOCK_RATIO),
            DropZone::Right => DockTree::horizontal(old, new_leaf, 1.0 - ROOT_DOCK_RATIO),
            DropZone::Top => DockTree::vertical(new_leaf, old, ROOT_DOCK_RATIO),
            DropZone::Bottom => DockTree::vertical(old, new_leaf, 1.0 - ROOT_DOCK_RATIO),
            DropZone::Center => unreachable!(),
        };
    }

    /// Detach the tree's bottom region: when the root is a vertical split,
    /// remove and return its bottom child plus the split's ratio (the top
    /// share, so a re-attach restores the exact height). `None` when the root
    /// has no bottom region. Inverse of [`Self::attach_bottom`] — the editor's
    /// collapsible bottom panel stashes the subtree this returns.
    pub fn detach_bottom(&mut self) -> Option<(DockTree, f32)> {
        let DockTree::Split {
            direction: SplitDirection::Vertical,
            ratio,
            first,
            second,
        } = self
        else {
            return None;
        };
        let r = *ratio;
        let bottom = std::mem::replace(&mut **second, DockTree::Empty);
        *self = std::mem::replace(&mut **first, DockTree::Empty);
        Some((bottom, r))
    }

    /// Re-attach `bottom` as a full-width bottom region under the whole tree,
    /// giving the existing content `ratio` of the space. Inverse of
    /// [`Self::detach_bottom`]. No-op for an empty `bottom`; an empty tree just
    /// becomes `bottom`.
    pub fn attach_bottom(&mut self, bottom: DockTree, ratio: f32) {
        if bottom.is_empty() {
            return;
        }
        if self.is_empty() {
            *self = bottom;
            return;
        }
        let old = std::mem::replace(self, DockTree::Empty);
        *self = DockTree::vertical(old, bottom, ratio);
    }

    /// Detach a bottom region even when it isn't the root split: find the
    /// shallowest vertical split whose *bottom* child (`second`) contains
    /// `panel`, remove and return that child plus the split's ratio, and
    /// collapse the split so its top child takes its place. Also returns the
    /// panel ids that were in that top child — an **anchor** so
    /// [`Self::attach_bottom_at`] can restore the region under the same
    /// neighbour instead of full-width at the root.
    ///
    /// This generalises [`Self::detach_bottom`] (which only fires when the
    /// bottom region spans the whole width at the root) to a strip that sits
    /// under one column with a full-height panel beside it. `None` when `panel`
    /// is nowhere below a vertical divider.
    pub fn detach_bottom_containing(
        &mut self,
        panel: &str,
    ) -> Option<(DockTree, f32, Vec<String>)> {
        // Is *this* split the one hosting the strip (panel in its bottom child)?
        if let DockTree::Split {
            direction: SplitDirection::Vertical,
            ratio,
            first,
            second,
        } = self
        {
            if second.contains_panel(panel) {
                let r = *ratio;
                let mut anchor = Vec::new();
                first.collect_panels(&mut anchor);
                let bottom = std::mem::replace(&mut **second, DockTree::Empty);
                *self = std::mem::replace(&mut **first, DockTree::Empty);
                return Some((bottom, r, anchor));
            }
        }
        // Otherwise recurse into whichever child still holds the panel.
        match self {
            DockTree::Split { first, second, .. } => first
                .detach_bottom_containing(panel)
                .or_else(|| second.detach_bottom_containing(panel)),
            _ => None,
        }
    }

    /// Re-attach `bottom` beneath the region identified by `anchor`: descend to
    /// the smallest subtree that still contains all of the (surviving) anchor
    /// panels — their common ancestor — and re-split it vertically, giving that
    /// region `ratio` of the height. Inverse of [`Self::detach_bottom_containing`].
    ///
    /// An empty anchor, or one whose panels have all left the tree, falls back
    /// to a full-width root attach (identical to [`Self::attach_bottom`]) — so a
    /// stash saved before anchors existed still reopens sensibly.
    ///
    /// Returns the **path** of the vertical split it created, in the divider
    /// path convention (`false` = descended into the first child) — empty for
    /// a root attach. The shell's drag-the-collapsed-strip-open gesture hands
    /// this to [`GrabRootDivider`] so the live drag adopts the right divider.
    pub fn attach_bottom_at(&mut self, bottom: DockTree, ratio: f32, anchor: &[String]) -> Vec<bool> {
        if bottom.is_empty() {
            return Vec::new();
        }
        if self.is_empty() {
            *self = bottom;
            return Vec::new();
        }
        let present: Vec<&str> = anchor
            .iter()
            .map(String::as_str)
            .filter(|p| self.contains_panel(p))
            .collect();
        // Walk down to the anchor panels' common ancestor. Each step descends
        // into the child that holds *all* of them; when neither does, this node
        // is that ancestor. Empty anchor → stay at the root (full width).
        let mut target: &mut DockTree = self;
        let mut split_path = Vec::new();
        if !present.is_empty() {
            loop {
                let descend = match &*target {
                    DockTree::Split { first, second, .. } => {
                        if present.iter().all(|p| first.contains_panel(p)) {
                            Some(true)
                        } else if present.iter().all(|p| second.contains_panel(p)) {
                            Some(false)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                match descend {
                    Some(true) => {
                        let DockTree::Split { first, .. } = target else { unreachable!() };
                        // Divider path convention: `false` = first child.
                        split_path.push(false);
                        target = first.as_mut();
                    }
                    Some(false) => {
                        let DockTree::Split { second, .. } = target else { unreachable!() };
                        split_path.push(true);
                        target = second.as_mut();
                    }
                    None => break,
                }
            }
        }
        let old = std::mem::replace(target, DockTree::Empty);
        *target = DockTree::vertical(old, bottom, ratio);
        split_path
    }

    /// Remove and return the whole leaf containing `panel` — tab set and active
    /// tab intact — collapsing the split it leaves behind. Unlike
    /// [`Self::remove_panel`] (one tab), this moves a leaf wholesale, so a
    /// multi-tab strip survives being stashed and re-attached as one unit.
    pub fn take_leaf_containing(&mut self, panel: &str) -> Option<DockTree> {
        let taken = {
            let leaf = self.find_leaf_mut(panel)?;
            std::mem::replace(leaf, DockTree::Empty)
        };
        self.cleanup_empty();
        Some(taken)
    }
}

/// Share of the dock a root-edge drop claims for the new panel — a side rail,
/// not a half split, since the gesture targets toolbars/inspectors.
const ROOT_DOCK_RATIO: f32 = 0.2;

/// Where a dragged panel will land on a leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    /// Add as a tab in the target leaf.
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

// ── Plugin + public seam ─────────────────────────────────────────────────────

/// Adds the dock reconciler + interaction systems and the resources they need.
pub struct DockPlugin;

impl Plugin for DockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dock>()
            .init_resource::<DockDirty>()
            .init_resource::<DockWindows>()
            .init_resource::<DockWindowRequests>()
            .init_resource::<DockWindowCloseRequests>()
            .init_resource::<FloatDrag>()
            .init_resource::<GlobalCursor>()
            .init_resource::<FocusPanelRequest>()
            .init_resource::<FocusedPanel>()
            .init_resource::<PendingSwitch>()
            .init_resource::<DraggedDivider>()
            .init_resource::<BottomSnapRequest>()
            .init_resource::<BottomStripMarkers>()
            .init_resource::<GrabRootDivider>()
            .init_resource::<TabDrag>()
            .init_resource::<DockDragWatch>()
            .init_resource::<crate::font::FontRegistry>()
            .add_systems(
                Update,
                (
                    crate::font::load_fonts,
                    crate::font::scan_project_fonts,
                    // Screen-space cursor first: divider/tab drags and the
                    // tear-off window follow all read it this frame.
                    track_global_cursor,
                    divider_drag,
                    tab_drag,
                    // Grip-press tear-offs land before the spawn below, so the
                    // window appears the same frame, like the Ctrl+drag path.
                    tab_grip_interact,
                    // Same frame as the tear-off gesture so the new window
                    // appears on the very next present.
                    spawn_dock_windows,
                    // After spawn (picks up the tear-off grab the same frame).
                    float_window_drag,
                    apply_focus_request,
                    apply_tab_switch,
                    tab_hover,
                    tab_close_hover,
                    tab_close_click,
                    // Rebuild last, in the same frame the model mutates, so the
                    // dock doesn't show a stale layout for a frame (flicker).
                    rebuild_dock,
                    // Toggle per-tab pane visibility after any rebuild.
                    sync_panes,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    add_panel_click,
                    track_focused_panel,
                    tab_strip_wheel,
                    apply_dock_style,
                    float_window_controls,
                    tab_grip_hover,
                    tab_context_menu,
                    bottom_collapse_click,
                ),
            )
            // In PostUpdate, before `camera_system` evaluates render targets: a
            // dock window and its camera must die in the same frame (see
            // [`DockWindowCloseRequests`]). This also catches windows despawned
            // externally (e.g. Alt+F4 via bevy's `close_when_requested`) in the
            // same frame their `Window` component is removed.
            .add_systems(
                PostUpdate,
                (process_dock_window_closes, guard_dock_target_cameras)
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
) -> impl Fn(Option<Res<Dock>>, Option<Res<DockWindows>>) -> bool + Clone {
    move |dock: Option<Res<Dock>>, wins: Option<Res<DockWindows>>| {
        dock.is_some_and(|d| d.tree.is_active_tab(id))
            || wins.is_some_and(|w| w.0.iter().any(|s| s.tree.is_active_tab(id)))
    }
}

/// Is `panel` the active (visible) tab in the primary dock or any floating dock
/// window? The non-run-condition companion to [`panel_active`] for callers that
/// check from a `World` or with resources in hand.
pub fn panel_visible_anywhere(
    id: &str,
    dock: Option<&Dock>,
    wins: Option<&DockWindows>,
) -> bool {
    dock.is_some_and(|d| d.tree.is_active_tab(id))
        || wins.is_some_and(|w| w.0.iter().any(|s| s.tree.is_active_tab(id)))
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            tree: DockTree::Empty,
        }
    }
}

/// Tag the node the dock should render into. Flip [`DockDirty`] to (re)build.
#[derive(Component)]
pub struct DockArea;

/// Set to rebuild the dock subtree (structure changed, or first build). Tab
/// switches do NOT set this — they update in place.
#[derive(Resource, Default)]
pub struct DockDirty(pub bool);

// ── Floating dock windows ────────────────────────────────────────────────────

/// One floating OS window hosting its own [`DockTree`] — created by
/// Ctrl+dragging a tab out of a dock (tear-off) or programmatically via
/// [`DockWindowRequests`]. Each window is a full dock: tabs, splits and
/// drag-docking all work inside it, and tabs drag between windows.
pub struct DockWindowState {
    /// The OS window entity.
    pub window: Entity,
    /// The `Camera2d` rendering this window's UI.
    pub camera: Entity,
    /// The root UI node in this window (title bar + dock area + resize grip).
    pub root: Entity,
    /// The [`DockArea`] node the tree reconciles into.
    pub area: Entity,
    /// This window's live dock layout.
    pub tree: DockTree,
    /// Per-window rebuild flag — the floating counterpart of [`DockDirty`].
    pub dirty: bool,
}

/// All live floating dock windows. The editor shell reads this to persist
/// floating layouts; everything else goes through the dock's own systems.
#[derive(Resource, Default)]
pub struct DockWindows(pub Vec<DockWindowState>);

/// Request to open a floating dock window. Push onto [`DockWindowRequests`];
/// the dock spawns the OS window + camera + chrome next frame (or the same
/// frame for the tear-off gesture, which runs before the spawn system).
pub struct DockWindowRequest {
    /// The layout the new window opens with.
    pub tree: DockTree,
    /// Desired client-area origin in physical screen px (`None` = OS default).
    pub position: Option<IVec2>,
    /// Client-area size in physical px.
    pub size: UVec2,
    /// Tear-off: the window follows the held cursor until mouse release.
    pub grab: bool,
}

/// Queue of pending [`DockWindowRequest`]s (public seam — the shell uses this
/// to restore persisted floating windows at startup).
#[derive(Resource, Default)]
pub struct DockWindowRequests(pub Vec<DockWindowRequest>);

/// A floating dock window being dragged: the tear-off follow (until the
/// gesture's button releases) and title-bar drags share this. Title-bar drags
/// also resolve a re-dock drop — release over a tab bar or a dock root
/// edge/corner in the main window and the panel docks back there.
struct FloatDragState {
    window: Entity,
    /// Cursor → window-origin offset in logical px (scaled per frame, so the
    /// grab survives the window crossing monitors with different DPI).
    grab: Vec2,
    /// Whether releasing over a dock target re-docks the panel. `false` for
    /// the tear-off gesture (its release point is still inside the tab bar it
    /// just left — re-docking there would undo the tear-off).
    redock: bool,
    action: Option<DropAction>,
    shown_overlay: Option<Entity>,
}

/// The active floating-window drag, if any.
#[derive(Resource, Default)]
struct FloatDrag(Option<FloatDragState>);

/// Marks a floating window's [`DockArea`] and links it back to its OS window.
/// The primary dock area (the editor shell's) does NOT carry this — that's how
/// dock systems tell "mutate `Dock.tree`" from "mutate this window's tree".
#[derive(Component)]
pub struct FloatingDockArea {
    pub window: Entity,
}

/// A floating dock window's title bar — press starts a window drag (which can
/// end in a re-dock, see [`FloatDragState`]).
#[derive(Component)]
struct FloatWindowBar(Entity);

/// A floating dock window's close button (×).
#[derive(Component)]
struct FloatWindowClose(Entity);

/// A floating dock window's edge/corner resize zone — press starts an OS
/// resize toward the given octant.
#[derive(Component)]
struct FloatWindowResize {
    window: Entity,
    octant: bevy::math::CompassOctant,
}

/// Floating dock windows queued to close (window entities). All close paths —
/// the title-bar ×, dragging the last panel out, a re-dock — go through this
/// queue instead of despawning the window directly: [`process_dock_window_closes`]
/// tears the window, its camera and its UI root down **together, before bevy's
/// camera update**. A `Camera` whose `RenderTarget::Window` entity is already
/// gone panics `camera_system` ("RenderTarget::Window missing"), so the camera
/// must never outlive its window across that boundary — not even one frame.
#[derive(Resource, Default)]
struct DockWindowCloseRequests(Vec<Entity>);

/// Last known cursor position in physical **screen** coordinates, tracked from
/// [`bevy::window::CursorMoved`] messages (window client origin + event
/// position). Unlike `Window::cursor_position()`, this keeps updating during a
/// mouse-capture drag even once the cursor leaves the source window's bounds —
/// bevy clamps the readable position to `None` outside the window, but the
/// underlying move events keep flowing to the captured window with
/// out-of-bounds coordinates. Cross-window tab drops and the tear-off window
/// follow both depend on that.
#[derive(Resource, Default)]
pub struct GlobalCursor {
    pub pos: Option<Vec2>,
}

/// External request to focus (make active) a panel by id. Other crates set this
/// to programmatically bring a tab to the foreground — e.g. the viewport crate
/// focusing the **Game** tab when play starts — and [`apply_focus_request`]
/// routes it through the same in-place tab-switch a click performs (so labels
/// recolor and the right pane shows). Consumed (reset to `None`) each frame.
#[derive(Resource, Default)]
pub struct FocusPanelRequest(pub Option<String>);

/// Id of the dock panel the user last interacted with (clicked into, switched
/// to, or programmatically focused). Distinct from "visible" — several leaves
/// are visible at once, but only one has focus. Consumers use this to route
/// focus-sensitive behaviour (e.g. undo/redo picking the active document's
/// stack). `None` until the first interaction, letting consumers fall back to
/// another signal (like the active document tab).
#[derive(Resource, Default)]
pub struct FocusedPanel(pub Option<String>);

// ── Components ───────────────────────────────────────────────────────────────

/// A panel tab. Click switches the active tab in place; drag re-docks. Holds
/// its leaf + child entities so consumers can restyle (e.g. real titles/icons).
#[derive(Component)]
pub struct DockTab {
    pub id: String,
    pub leaf: Entity,
    pub label: Entity,
    pub icon: Entity,
    /// Vertical insertion marker shown at this tab's edge during a drag.
    marker: Entity,
}

/// A dock leaf — the drop target for tab drags, and where consumers put panel
/// content. Fill the `content` node based on `active` (the visible panel id);
/// ember leaves `content` empty so the consumer owns it.
#[derive(Component)]
pub struct DockLeaf {
    pub tabs: Vec<String>,
    /// The container to put the active panel's content into.
    pub content: Entity,
    /// Id of the currently-visible tab — drives what content to show.
    pub active: String,
    /// The [`DockArea`] this leaf was built into — routes tree mutations to the
    /// primary dock or the owning floating window.
    pub area: Entity,
    overlay: Entity,
}

/// A per-tab content pane that lives inside a leaf's `content` container. Panes
/// are built **once** (lazily, when their tab is first activated) and kept; tab
/// switches just toggle their [`Node::display`] via [`sync_panes`] instead of
/// despawning + rebuilding. This is the persistent-content model — it avoids the
/// despawn/rebuild churn (and entity-reuse races) of swapping a single content
/// node on every tab switch.
#[derive(Component)]
pub struct TabPane {
    pub id: String,
}

/// Wrap built panel `content` in a tab pane for tab `id`. `scroll` adds a
/// wheel-scrollable viewport + scrollbar (use `false` for panels that manage
/// their own scrolling, e.g. the console or a viewport). Panes are built only
/// when their tab is active, so they start visible; [`sync_panes`] toggles
/// visibility thereafter. Add the returned pane to the leaf's `content`.
pub fn tab_pane(commands: &mut Commands, id: &str, content: Entity, scroll: bool) -> Entity {
    let outer = if scroll {
        crate::widgets::scroll_view(commands, content)
    } else {
        let p = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::clip(),
                    ..default()
                },
                Name::new(format!("pane:{id}")),
            ))
            .id();
        commands.entity(p).add_child(content);
        p
    };
    commands.entity(outer).insert(TabPane { id: id.to_string() });
    outer
}

/// Toggle each pane's visibility to match its leaf's active tab, and drop panes
/// whose tab has left the leaf (e.g. moved to another leaf). Runs every frame;
/// writes are guarded so it never churns.
fn sync_panes(
    mut commands: Commands,
    leaves: Query<&DockLeaf>,
    children: Query<&Children>,
    panes: Query<&TabPane>,
    mut nodes: Query<&mut Node>,
) {
    for leaf in &leaves {
        let Ok(kids) = children.get(leaf.content) else {
            continue;
        };
        for child in kids.iter() {
            let Ok(pane) = panes.get(child) else {
                continue;
            };
            if !leaf.tabs.contains(&pane.id) {
                commands.entity(child).despawn();
                continue;
            }
            let display = if pane.id == leaf.active {
                Display::Flex
            } else {
                Display::None
            };
            if let Ok(mut n) = nodes.get_mut(child) {
                if n.display != display {
                    n.display = display;
                }
            }
        }
    }
}

#[derive(Component)]
struct InsertMarker;

#[derive(Component)]
struct TabClose;

#[derive(Component)]
struct DropOverlay;

/// The dock-wide drop preview for root edge/corner docking — one overlay child
/// per [`DockArea`] node, shown while a drag targets a full-height / full-width
/// root split of that area. Recreated on every dock rebuild.
#[derive(Component)]
struct RootDropOverlay {
    /// The dock area this overlay previews for.
    area: Entity,
}

#[derive(Component)]
struct TabBarOf(Entity);

#[derive(Component)]
struct TabGhost;

/// Tags a split's divider with everything its drag needs.
#[derive(Component)]
struct Divider {
    container: Entity,
    first_wrap: Entity,
    horizontal: bool,
    path: Vec<bool>,
    /// The dock area this divider's split belongs to — routes the ratio
    /// persist to the right tree (primary vs a floating window's).
    area: Entity,
    /// True in a floating dock window — those never publish
    /// [`BottomSnapRequest`] (the snap-closed gesture is a primary-dock
    /// bottom-panel affair).
    floating: bool,
    /// `Some(panel)` when this is a vertical divider whose bottom pane is a
    /// leaf holding one of the [`BottomStripMarkers`] panels — i.e. the
    /// collapsible bottom strip even when it isn't the root region. Overshoot
    /// this divider downward and the strip snap-closes, same as the root one;
    /// the panel id tells the consumer which region to collapse.
    strip: Option<String>,
}

/// Panel ids that identify the consumer's collapsible bottom strip by
/// content (the editor shell registers `assets`/`console`). The dock is
/// otherwise content-agnostic — the *root* bottom region always collapses —
/// but a strip nested under one column (a full-height panel beside it) can
/// only be recognized by what it holds. A leaf tabbing one of these as the
/// bottom child of a vertical split gets the collapse chevron and the
/// divider snap-closed gesture, wherever it sits in the tree.
#[derive(Resource, Default)]
pub struct BottomStripMarkers(pub Vec<String>);

/// Published when the user drags a collapsible bottom region's divider hard
/// down — past the ratio clamp, squeezing the bottom pane under
/// [`BOTTOM_SNAP_PX`] — or clicks the region's collapse chevron. The dock
/// itself can't collapse anything (the bottom-panel stash is editor-shell
/// state), so it hands the intent to the consumer via this seam. Mirrors
/// [`DockDragWatch`]'s outside-consumer pattern.
#[derive(Resource, Default)]
pub struct BottomSnapRequest(pub Option<BottomSnap>);

/// One collapse intent (see [`BottomSnapRequest`]).
#[derive(Clone)]
pub struct BottomSnap {
    /// Ratio (top share) to record for reopening. Divider snaps pass the
    /// **pre-drag** ratio — by the time the consumer detaches, the live tree
    /// holds the squished mid-drag one. `None` (chevron clicks) means the
    /// ratio in the tree is accurate; use whatever the detach returns.
    pub restore: Option<f32>,
    /// `Some(panel)`: collapse the vertical region containing this panel (a
    /// nested strip, see [`BottomStripMarkers`]). `None`: the root bottom
    /// region.
    pub target: Option<String>,
}

/// Set by an outside consumer right after it re-attaches the bottom region
/// while the mouse button is still held (the shell's "drag the collapsed
/// strip open" gesture): once the rebuilt tree's vertical divider at this
/// path exists (empty path = the primary root divider; a non-empty one is
/// where a nested strip re-attached — [`DockTree::attach_bottom_at`] returns
/// it), [`divider_drag`] adopts it as the live drag, so the just-reopened
/// panel keeps sizing under the held cursor. Cleared on adoption or when the
/// button is released.
#[derive(Resource, Default)]
pub struct GrabRootDivider(pub Option<Vec<bool>>);

/// The collapse chevron at the right end of a collapsible bottom region's
/// tab bar: clicking publishes [`BottomSnapRequest`] so the consumer
/// collapses the region (reopening restores its height).
#[derive(Component)]
struct BottomCollapseBtn {
    /// Mirrors [`BottomSnap::target`]: `None` for the root bottom region,
    /// `Some(panel)` for a nested strip.
    target: Option<String>,
}

/// The whole-leaf drag handle at the far left of a tab bar: dragging it
/// moves the leaf's entire tab set as one unit — same drop zones as a
/// single-tab drag, but the drop inserts the whole leaf (see the `group`
/// paths in [`tab_drag`]).
#[derive(Component)]
struct LeafGrip {
    leaf: Entity,
}

/// Snap threshold for [`BottomSnapRequest`]: the second pane's would-be
/// height in physical px below which a root-divider drag reads as "close it".
/// The ratio clamp already floors the pane at 10% of the area, so reaching
/// this takes a deliberate overshoot well past where the divider stops.
const BOTTOM_SNAP_PX: f32 = 48.0;

/// Info passed to a leaf so its tab-bar empty area can act as a secondary
/// resize handle for the parent split's boundary ("more to grip").
#[derive(Clone)]
struct ParentSplit {
    container: Entity,
    first_wrap: Entity,
    horizontal: bool,
    is_second: bool,
    path: Vec<bool>,
}

impl ParentSplit {
    fn aligned(&self) -> bool {
        (!self.horizontal && self.is_second) || (self.horizontal && !self.is_second)
    }
}

#[derive(Resource, Default)]
struct DraggedDivider(Option<DividerDrag>);

struct DividerDrag {
    handle: Entity,
    /// Physical screen-space grab point (from [`GlobalCursor`]) — screen space
    /// keeps the drag alive even when the cursor leaves the window mid-drag.
    start_cursor: Vec2,
    start_ratio: f32,
}

#[derive(Resource, Default)]
struct TabDrag(Option<TabDragState>);

/// Public seam letting an outside consumer (the editor shell) observe an
/// in-flight tab drag and divert its drop somewhere the dock knows nothing
/// about — e.g. dropping a panel on the workspace ribbon to spawn a new
/// workspace from it.
///
/// `dragging` carries the panel id once a drag passes the move threshold and is
/// cleared on release. A consumer that wants to handle the drop itself sets
/// `claim` (typically while the cursor is over its own drop target); on release
/// the dock then skips its own re-dock/tab-switch and leaves the panel to the
/// claimant, which is responsible for removing it from the live [`Dock`] tree.
/// The dock clears both fields on release.
#[derive(Resource, Default)]
pub struct DockDragWatch {
    /// Id of the panel currently being dragged (`None` when no active drag).
    pub dragging: Option<String>,
    /// Set by an external consumer to take ownership of the pending drop.
    pub claim: bool,
}

struct TabDragState {
    id: String,
    leaf: Entity,
    /// True when the drag started on the leaf's [`LeafGrip`]: the whole tab
    /// set moves as one unit instead of just `id` (which is then the leaf's
    /// active tab, used for the ghost and target resolution).
    group: bool,
    /// The dock area the drag started in — the tree the panel is removed from.
    source_area: Entity,
    /// The OS window hosting `source_area`. During the drag this window holds
    /// the mouse capture, so it's the only window whose cursor stays readable.
    source_window: Entity,
    /// Cursor at press, in the source window's logical coords.
    start_cursor: Vec2,
    active: bool,
    action: Option<DropAction>,
    ghost: Option<Entity>,
    shown_overlay: Option<Entity>,
    shown_marker: Option<Entity>,
}

/// Where a drop lands. Every variant carries the **target** dock area, which
/// may differ from the drag's source area (cross-window drops).
enum DropAction {
    Split { area: Entity, rep: String, zone: DropZone },
    /// Full-height / full-width split against the whole dock (edge/corner drop).
    RootSplit { area: Entity, zone: DropZone },
    Tab { area: Entity, rep: String, before: Option<String> },
}

impl DropAction {
    fn area(&self) -> Entity {
        match self {
            DropAction::Split { area, .. }
            | DropAction::RootSplit { area, .. }
            | DropAction::Tab { area, .. } => *area,
        }
    }
}

/// A pending in-place tab switch: (leaf entity, panel id to activate).
#[derive(Resource, Default)]
struct PendingSwitch(Option<(Entity, String)>);

// ── Tree routing (primary dock vs floating windows) ─────────────────────────

/// The live tree owning dock area `area`: a floating window's tree if `area`
/// belongs to one, else the primary [`Dock`]'s.
fn area_tree_mut<'a>(
    area: Entity,
    dock: &'a mut Dock,
    wins: &'a mut DockWindows,
) -> &'a mut DockTree {
    match wins.0.iter_mut().find(|s| s.area == area) {
        Some(st) => &mut st.tree,
        None => &mut dock.tree,
    }
}

/// Flag `area`'s tree for rebuild — [`DockDirty`] for the primary dock, the
/// per-window flag for a floating one.
fn flag_area_dirty(area: Entity, dirty: &mut DockDirty, wins: &mut DockWindows) {
    match wins.0.iter_mut().find(|s| s.area == area) {
        Some(st) => st.dirty = true,
        None => dirty.0 = true,
    }
}

/// The OS window hosting dock area `area` (the floating window's, else the
/// primary window).
fn area_window(area: Entity, wins: &DockWindows, primary: Option<Entity>) -> Option<Entity> {
    wins.0
        .iter()
        .find(|s| s.area == area)
        .map(|s| s.window)
        .or(primary)
}

/// Does `global` (physical screen px) land inside `win`'s client area? Only
/// answerable once winit has reported the window's position (`At`).
fn window_contains(win: &Window, global: Vec2) -> bool {
    let bevy::window::WindowPosition::At(origin) = win.position else {
        return false;
    };
    let local = global - origin.as_vec2();
    local.x >= 0.0
        && local.y >= 0.0
        && local.x < win.physical_width() as f32
        && local.y < win.physical_height() as f32
}

/// `global` (physical screen px) converted into `win`-local physical px, if the
/// window's position is known.
fn window_local(win: &Window, global: Vec2) -> Option<Vec2> {
    let bevy::window::WindowPosition::At(origin) = win.position else {
        return None;
    };
    Some(global - origin.as_vec2())
}

/// Track the cursor in physical screen space from raw move messages — see
/// [`GlobalCursor`] for why this can't just read `Window::cursor_position()`.
fn track_global_cursor(
    mut cursor: ResMut<GlobalCursor>,
    mut moves: MessageReader<bevy::window::CursorMoved>,
    windows: Query<&Window>,
) {
    for ev in moves.read() {
        let Ok(win) = windows.get(ev.window) else {
            continue;
        };
        // `position` is `Automatic` until winit reports the first `Moved` for
        // this window. Falling back to a zero origin keeps single-window
        // coordinates self-consistent (everything is relative to that window)
        // so divider/tab drags still work before the position resolves.
        let origin = match win.position {
            bevy::window::WindowPosition::At(origin) => origin.as_vec2(),
            _ => Vec2::ZERO,
        };
        cursor.pos = Some(origin + ev.position * win.scale_factor());
    }
}

// ── Defaults a consumer overrides ────────────────────────────────────────────

/// Default tab title + icon for a panel id (humanized + a neutral dot).
/// Consumers override with real metadata (e.g. the editor's panel registry).
fn tab_meta(id: &str) -> (String, &'static str) {
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

// ── Systems ──────────────────────────────────────────────────────────────────

const TAB_DRAG_THRESHOLD: f32 = 6.0;

fn as_ratio(v: Val) -> Option<f32> {
    if let Val::Percent(p) = v {
        Some(p / 100.0)
    } else {
        None
    }
}

/// Drag a divider handle to resize its split. Latches on press (continues off
/// the handle), moves by cursor delta (no snap), resizes live + persists.
///
/// Works in physical screen space ([`GlobalCursor`] vs the container's
/// physical size) so it's window-agnostic — the same code drives splits in the
/// primary dock and in floating dock windows, and the drag survives the cursor
/// briefly leaving the window (mouse capture keeps the move events coming).
fn divider_drag(
    mut dragged: ResMut<DraggedDivider>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<GlobalCursor>,
    dividers: Query<(Entity, &Interaction, &Divider)>,
    computed: Query<&bevy::ui::ComputedNode>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<Dock>,
    mut wins: ResMut<DockWindows>,
    mut snap: ResMut<BottomSnapRequest>,
    mut grab: ResMut<GrabRootDivider>,
) {
    if mouse.just_released(MouseButton::Left) {
        dragged.0 = None;
        grab.0 = None;
    }
    let Some(cursor) = cursor.pos else {
        return;
    };

    // Adopt a consumer-initiated grab (the shell's drag-the-collapsed-strip-
    // open gesture): the mouse is already held, so take the vertical divider
    // at the grab's path — where the bottom region just re-attached — as the
    // live drag the moment the rebuilt tree provides it, no press on the
    // handle needed.
    if dragged.0.is_none() && mouse.pressed(MouseButton::Left) {
        if let Some(path) = grab.0.as_ref() {
            if let Some((entity, _, div)) = dividers
                .iter()
                .find(|(_, _, d)| !d.floating && !d.horizontal && &d.path == path)
            {
                let start_ratio = nodes
                    .get(div.first_wrap)
                    .ok()
                    .and_then(|n| as_ratio(n.height))
                    .unwrap_or(0.5);
                dragged.0 = Some(DividerDrag {
                    handle: entity,
                    start_cursor: cursor,
                    start_ratio,
                });
                grab.0 = None;
            }
        }
    }

    if dragged.0.is_none() {
        if !mouse.just_pressed(MouseButton::Left) {
            return;
        }
        for (entity, interaction, div) in &dividers {
            if *interaction == Interaction::Pressed {
                let start_ratio = nodes
                    .get(div.first_wrap)
                    .ok()
                    .and_then(|n| as_ratio(if div.horizontal { n.width } else { n.height }))
                    .unwrap_or(0.5);
                dragged.0 = Some(DividerDrag {
                    handle: entity,
                    start_cursor: cursor,
                    start_ratio,
                });
                break;
            }
        }
    }

    let Some(drag) = dragged.0.as_ref() else {
        return;
    };
    let Ok((_, _, div)) = dividers.get(drag.handle) else {
        dragged.0 = None;
        return;
    };
    let Ok(cn) = computed.get(div.container) else {
        return;
    };
    // Physical px on both sides of the division, so per-window scale factors
    // (mixed-DPI monitors) cancel out.
    let extent = if div.horizontal {
        cn.size().x
    } else {
        cn.size().y
    };
    if extent <= 0.0 {
        return;
    }
    let moved = if div.horizontal {
        cursor.x - drag.start_cursor.x
    } else {
        cursor.y - drag.start_cursor.y
    };
    let raw = drag.start_ratio + moved / extent;
    // Snap-closed gesture: overshooting a collapsible bottom region's divider
    // (the primary root vertical one, or a nested strip's — `strip`) far
    // enough that the bottom pane would drop under the threshold ends the
    // drag and asks the consumer (the shell's bottom panel) to collapse the
    // region — the clamp below would otherwise just pin the pane at 10%.
    if !div.horizontal
        && !div.floating
        && (div.path.is_empty() || div.strip.is_some())
        && (1.0 - raw) * extent < BOTTOM_SNAP_PX
    {
        snap.0 = Some(BottomSnap {
            restore: Some(drag.start_ratio),
            // The root region collapses content-agnostically; only a nested
            // strip needs the panel id to be found again.
            target: if div.path.is_empty() { None } else { div.strip.clone() },
        });
        dragged.0 = None;
        return;
    }
    let ratio = raw.clamp(0.1, 0.9);

    let first_wrap = div.first_wrap;
    let horizontal = div.horizontal;
    let dpath = div.path.clone();
    let darea = div.area;

    // Resize the first pane; the divider strip is a flex sibling, so flexbox
    // re-places it on the new boundary automatically — no handle to move.
    if let Ok(mut n) = nodes.get_mut(first_wrap) {
        if horizontal {
            n.width = Val::Percent(ratio * 100.0);
        } else {
            n.height = Val::Percent(ratio * 100.0);
        }
    }
    area_tree_mut(darea, &mut dock, &mut wins).update_ratio(&dpath, ratio);
}

/// Drag a tab to re-dock; a plain click switches the active tab.
///
/// Multi-window aware:
/// - **Ctrl+drag** tears the panel off into a new floating OS window that
///   follows the cursor until release (see [`DockWindowRequest::grab`]).
/// - A plain drag re-docks **across windows**: while the cursor is inside the
///   source window the usual `RelativeCursorPosition` machinery resolves the
///   drop; once it leaves (the pressed window holds the mouse capture, so no
///   other window receives cursor events) the drop target is resolved manually
///   from [`GlobalCursor`] against each window's leaf rects.
#[allow(clippy::too_many_arguments)]
fn tab_drag(
    mut drag: ResMut<TabDrag>,
    mut pending: ResMut<PendingSwitch>,
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    input: (
        Res<ButtonInput<MouseButton>>,
        Res<ButtonInput<KeyCode>>,
        Res<GlobalCursor>,
    ),
    windows: Query<&Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    tabs: Query<(Entity, &Interaction, &DockTab, &bevy::ui::RelativeCursorPosition)>,
    grips: Query<(&Interaction, &LeafGrip)>,
    // Bundled into one tuple param so `tab_drag` stays under Bevy's 16-param
    // system cap: `.0` = tab bars (drop-onto-bar targeting), `.1` = tab strips
    // (in-place reorder re-sorts the strip's tab children).
    bars: (
        Query<(Entity, &TabBarOf, &bevy::ui::RelativeCursorPosition)>,
        Query<(Entity, &TabScrollStrip)>,
    ),
    mut leaves: Query<(
        Entity,
        &mut DockLeaf,
        &bevy::ui::RelativeCursorPosition,
        &bevy::ui::ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
    areas: Query<
        (
            Entity,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
            Option<&FloatingDockArea>,
        ),
        With<DockArea>,
    >,
    root_overlays: Query<(Entity, &RootDropOverlay)>,
    mut nodes: Query<&mut Node>,
    model: (
        ResMut<Dock>,
        ResMut<DockDirty>,
        ResMut<DockWindows>,
        ResMut<DockWindowRequests>,
        ResMut<DockWindowCloseRequests>,
    ),
    mut watch: ResMut<DockDragWatch>,
) {
    let (mouse, keys, global) = input;
    let (mut dock, mut dirty, mut wins, mut requests, mut close_queue) = model;

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        // A grip press outranks a tab press (the grip sits inside the bar but
        // not inside a tab, so both can't be Pressed at once anyway).
        let pressed = grips
            .iter()
            .find(|(i, _)| **i == Interaction::Pressed)
            .and_then(|(_, grip)| {
                let (_, ld, ..) = leaves.get(grip.leaf).ok()?;
                // Drag the whole tab set; `id` is the active tab (or the
                // first) — it stands in for the leaf in target resolution.
                let id = if ld.tabs.iter().any(|t| t == &ld.active) {
                    ld.active.clone()
                } else {
                    ld.tabs.first()?.clone()
                };
                Some((id, grip.leaf, true))
            })
            .or_else(|| {
                tabs.iter()
                    .find(|(_, i, ..)| **i == Interaction::Pressed)
                    .map(|(_, _, tab, _)| (tab.id.clone(), tab.leaf, false))
            });
        if let Some((id, leaf_e, group)) = pressed {
            if let Ok((_, ld, ..)) = leaves.get(leaf_e) {
                let source_area = ld.area;
                if let Some(source_window) =
                    area_window(source_area, &wins, primary.single().ok())
                {
                    if let Some(cur) = windows
                        .get(source_window)
                        .ok()
                        .and_then(|w| w.cursor_position())
                    {
                        drag.0 = Some(TabDragState {
                            id,
                            leaf: leaf_e,
                            group,
                            source_area,
                            source_window,
                            start_cursor: cur,
                            active: false,
                            action: None,
                            ghost: None,
                            shown_overlay: None,
                            shown_marker: None,
                        });
                    }
                }
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        if let Some(state) = drag.0.take() {
            if let Some(ghost) = state.ghost {
                commands.entity(ghost).despawn();
            }
            for e in [state.shown_overlay, state.shown_marker].into_iter().flatten() {
                if let Ok(mut n) = nodes.get_mut(e) {
                    n.display = Display::None;
                }
            }
            // An external consumer (e.g. the workspace ribbon) claimed this drop:
            // it owns moving the panel, so the dock does nothing here — no re-dock,
            // no tab switch. Clear the watch and bail.
            if watch.claim {
                watch.dragging = None;
                watch.claim = false;
                return;
            }
            if state.active {
                if let Some(action) = state.action {
                    let target_area = action.area();
                    let same_area = target_area == state.source_area;
                    // Group drag: take the WHOLE leaf out (tab set + active
                    // index travel together) and insert it per the action.
                    // Target resolution never offers the source leaf itself
                    // (see the `group` skips below), so the take can't
                    // invalidate the drop target.
                    if state.group {
                        let taken = area_tree_mut(state.source_area, &mut dock, &mut wins)
                            .take_leaf_containing(&state.id);
                        if let Some(leaf_tree) = taken {
                            let target = area_tree_mut(target_area, &mut dock, &mut wins);
                            insert_leaf_action(target, leaf_tree, &action);
                            flag_area_dirty(target_area, &mut dirty, &mut wins);
                            if !same_area {
                                flag_area_dirty(state.source_area, &mut dirty, &mut wins);
                                close_empty_dock_window(
                                    state.source_area,
                                    &wins,
                                    &mut close_queue,
                                );
                            }
                        }
                        watch.dragging = None;
                        watch.claim = false;
                        return;
                    }
                    // A reorder within the same leaf only changes tab order, not
                    // structure — do it in place (move the tab entity) instead of
                    // rebuilding the dock subtree, which avoids the layout flicker.
                    let inplace = match &action {
                        DropAction::Tab { rep, before, .. } if same_area => leaves
                            .get(state.leaf)
                            .ok()
                            .filter(|(_, ld, ..)| ld.tabs.contains(rep))
                            .map(|(_, ld, ..)| reordered(&ld.tabs, &state.id, before.as_deref())),
                        _ => None,
                    };
                    // Move the panel: out of the source tree, into the target's
                    // (the same tree when the drop stayed in-window).
                    area_tree_mut(state.source_area, &mut dock, &mut wins)
                        .remove_panel(&state.id);
                    insert_action(
                        area_tree_mut(target_area, &mut dock, &mut wins),
                        &state.id,
                        &action,
                    );
                    match inplace {
                        Some(new_tabs) => {
                            if let Ok((_, mut ld, ..)) = leaves.get_mut(state.leaf) {
                                ld.tabs = new_tabs.clone();
                            }
                            // Re-sort the *strip's* children (the tabs) — not the
                            // tab bar's, which also holds the grip, `+`, filler,
                            // and collapse chevron a wholesale reorder would drop.
                            if let Some((strip, _)) =
                                bars.1.iter().find(|(_, s)| s.leaf == state.leaf)
                            {
                                let ordered: Vec<Entity> = new_tabs
                                    .iter()
                                    .filter_map(|id| {
                                        tabs.iter()
                                            .find(|(_, _, t, _)| &t.id == id)
                                            .map(|(e, _, _, _)| e)
                                    })
                                    .collect();
                                // `replace_children`, not `insert_children(0, …)`:
                                // reordering existing children via Bevy 0.19's
                                // `place` can panic ("insertion index should be
                                // <= len") because it clamps the index before
                                // removing the moved child. `replace_children`
                                // rebuilds the collection wholesale and avoids it.
                                if !ordered.is_empty() {
                                    commands.entity(strip).replace_children(&ordered);
                                }
                            }
                        }
                        None => {
                            flag_area_dirty(target_area, &mut dirty, &mut wins);
                            if !same_area {
                                flag_area_dirty(state.source_area, &mut dirty, &mut wins);
                                // Dragging the last panel out of a floating
                                // window leaves an empty shell — close it.
                                close_empty_dock_window(
                                    state.source_area,
                                    &wins,
                                    &mut close_queue,
                                );
                            }
                        }
                    }
                }
            } else if !state.group {
                pending.0 = Some((state.leaf, state.id));
            }
        }
        watch.dragging = None;
        watch.claim = false;
        return;
    }

    let Some(state) = drag.0.as_mut() else {
        return;
    };
    // The source window's cursor — `None` once the cursor leaves its bounds
    // (the drag keeps running on [`GlobalCursor`] then).
    let cursor = windows
        .get(state.source_window)
        .ok()
        .and_then(|w| w.cursor_position());

    if !state.active {
        let Some(cur) = cursor else {
            return;
        };
        if cur.distance(state.start_cursor) <= TAB_DRAG_THRESHOLD {
            return;
        }
        // Ctrl+drag: tear the panel off into a floating OS window that follows
        // the cursor until release, instead of a re-dock drag. Not for group
        // drags — floating windows are chromeless single-panel hosts, so a
        // whole tab set torn off would leave its background tabs unreachable.
        if !state.group
            && (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight))
        {
            let id = state.id.clone();
            let scale = windows
                .get(state.source_window)
                .map(|w| w.scale_factor())
                .unwrap_or(1.0);
            let leaf_size = leaves.get(state.leaf).map(|(_, _, _, cn, _)| cn.size()).ok();
            tear_off_panel(
                &id,
                state.source_area,
                leaf_size,
                scale,
                global.pos,
                true,
                (&mut dock, &mut dirty, &mut wins, &mut requests, &mut close_queue),
            );
            drag.0 = None;
            watch.dragging = None;
            watch.claim = false;
            return;
        }
        state.active = true;
        // Publish the dragged panel id so external drop targets (the workspace
        // ribbon) can react while the drag is in flight. Group drags don't
        // publish — the ribbon's claim moves a single panel, which would tear
        // the group apart.
        if !state.group {
            watch.dragging = Some(state.id.clone());
        }
        if let Some(fonts) = &fonts {
            // A drag from a floating window renders its ghost on that window's
            // camera (a root node with no target lands on the primary window).
            let camera = wins
                .0
                .iter()
                .find(|s| s.area == state.source_area)
                .map(|s| s.camera);
            // Group ghosts show how many background tabs ride along.
            let extra = if state.group {
                leaves
                    .get(state.leaf)
                    .map(|(_, ld, ..)| ld.tabs.len().saturating_sub(1))
                    .unwrap_or(0)
            } else {
                0
            };
            state.ghost = Some(spawn_ghost(&mut commands, &fonts.ui, &state.id, extra, cur, camera));
        }
    }

    if let Some(ghost) = state.ghost {
        if let Ok(mut n) = nodes.get_mut(ghost) {
            match cursor {
                Some(cur) => {
                    n.display = Display::Flex;
                    n.left = Val::Px(cur.x + 12.0);
                    n.top = Val::Px(cur.y + 12.0);
                }
                // Cursor outside the source window (cross-window drag) — the
                // ghost would pin to a stale spot, so hide it.
                None => n.display = Display::None,
            }
        }
    }

    let mut action: Option<DropAction> = None;
    // (overlay entity, zone, is_root) — root hits use the area-wide overlay
    // with `set_root_zone_rect`, leaf hits the leaf overlay with `set_zone_rect`.
    let mut new_overlay: Option<(Entity, DropZone, bool)> = None;
    let mut new_marker: Option<(Entity, bool)> = None;

    if cursor.is_some() {
        // ── In-window drop resolution (`RelativeCursorPosition` machinery).
        // Only nodes in the cursor's window report `cursor_over`, so this
        // naturally scopes to the window the drag is currently inside.

        // Root edge/corner hit, in dock-local logical px. Corners outrank tab
        // bars (the top edge is lined with them); plain edge bands don't.
        let root_hit = areas
            .iter()
            .find(|(_, rcp, _, _)| rcp.cursor_over)
            .and_then(|(area_e, rcp, computed, _)| {
                let norm = rcp.normalized?;
                let size = computed.size() * computed.inverse_scale_factor();
                let (zone, corner) =
                    pick_root_zone((norm.x + 0.5) * size.x, (norm.y + 0.5) * size.y, size)?;
                Some((area_e, zone, corner))
            });
        let overlay_for = |area_e: Entity| {
            root_overlays
                .iter()
                .find(|(_, o)| o.area == area_e)
                .map(|(e, _)| e)
        };

        let over_bar = bars
            .0
            .iter()
            .find(|(_, _, rcp)| rcp.cursor_over)
            .map(|(_, bar, _)| bar.0);
        if let Some((area_e, zone, overlay)) = root_hit
            .filter(|(_, _, corner)| *corner)
            .and_then(|(a, z, _)| overlay_for(a).map(|o| (a, z, o)))
        {
            action = Some(DropAction::RootSplit { area: area_e, zone });
            new_overlay = Some((overlay, zone, true));
        } else if let Some(leaf_ent) =
            // A group drag can't drop on its own bar — the "target" is the
            // very leaf being moved.
            over_bar.filter(|e| !(state.group && *e == state.leaf))
        {
            if let Some((_, ld, ..)) = leaves.iter().find(|(e, ..)| *e == leaf_ent) {
                let mut before: Option<String> = None;
                let mut marker: Option<(Entity, bool)> = None;
                for id in ld.tabs.iter() {
                    if let Some((_, _, tab, rcp)) = tabs.iter().find(|(_, _, t, _)| &t.id == id) {
                        let nx = rcp.normalized.map_or(f32::INFINITY, |n| n.x);
                        if nx < 0.0 {
                            before = Some(id.clone());
                            marker = Some((tab.marker, false));
                            break;
                        }
                    }
                }
                if marker.is_none() {
                    if let Some(last) = ld.tabs.last() {
                        marker = tabs
                            .iter()
                            .find(|(_, _, t, _)| &t.id == last)
                            .map(|(_, _, t, _)| (t.marker, true));
                    }
                    before = None;
                }
                if let Some(rep) = ld.tabs.iter().find(|t| **t != state.id).cloned() {
                    action = Some(DropAction::Tab {
                        area: ld.area,
                        rep,
                        before,
                    });
                    new_marker = marker;
                }
            }
        } else if let Some((area_e, zone, overlay)) =
            root_hit.and_then(|(a, z, _)| overlay_for(a).map(|o| (a, z, o)))
        {
            action = Some(DropAction::RootSplit { area: area_e, zone });
            new_overlay = Some((overlay, zone, true));
        } else {
            for (leaf_e, ld, rcp, ..) in &leaves {
                if rcp.cursor_over {
                    // A group drag over its own leaf has no valid zone —
                    // splitting a leaf against itself is a no-op at best.
                    if state.group && leaf_e == state.leaf {
                        break;
                    }
                    if let Some(norm) = rcp.normalized {
                        let (x, y) = (norm.x + 0.5, norm.y + 0.5);
                        let zone = pick_zone(x, y);
                        if let Some(rep) = ld.tabs.iter().find(|t| **t != state.id).cloned() {
                            action = Some(if matches!(zone, DropZone::Center) {
                                DropAction::Tab {
                                    area: ld.area,
                                    rep,
                                    before: None,
                                }
                            } else {
                                DropAction::Split {
                                    area: ld.area,
                                    rep,
                                    zone,
                                }
                            });
                            new_overlay = Some((ld.overlay, zone, false));
                        }
                    }
                    break;
                }
            }
        }
    } else if let Some(gpos) = global.pos {
        // ── Cross-window drop resolution. The source window holds the mouse
        // capture, so windows under the cursor receive no cursor events and
        // their `RelativeCursorPosition` never updates — hit-test dock leaves
        // manually in physical screen space instead. Only the PRIMARY window
        // is a target: floating windows are chromeless single-panel hosts (no
        // tab bar), so extra tabs dropped into one would be unreachable.
        let mut target: Option<(Entity, Entity)> = None; // (window, area)
        if let Ok(pw) = primary.single() {
            if pw != state.source_window
                && windows.get(pw).is_ok_and(|w| window_contains(w, gpos))
            {
                if let Some((area_e, ..)) = areas.iter().find(|(.., f)| f.is_none()) {
                    target = Some((pw, area_e));
                }
            }
        }
        if let Some((win_e, area_e)) = target {
            if let Some(local) = windows.get(win_e).ok().and_then(|w| window_local(w, gpos)) {
                for (_, ld, _, cn, gt) in &leaves {
                    if ld.area != area_e || !cn.contains_point(*gt, local) {
                        continue;
                    }
                    if let Some(norm) = cn.normalize_point(*gt, local) {
                        let zone = pick_zone(norm.x + 0.5, norm.y + 0.5);
                        if let Some(rep) = ld.tabs.iter().find(|t| **t != state.id).cloned() {
                            action = Some(if matches!(zone, DropZone::Center) {
                                DropAction::Tab {
                                    area: area_e,
                                    rep,
                                    before: None,
                                }
                            } else {
                                DropAction::Split {
                                    area: area_e,
                                    rep,
                                    zone,
                                }
                            });
                            new_overlay = Some((ld.overlay, zone, false));
                        }
                    }
                    break;
                }
            }
        }
    }
    state.action = action;

    let new_overlay_e = new_overlay.map(|(e, _, _)| e);
    if state.shown_overlay != new_overlay_e {
        if let Some(old) = state.shown_overlay {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        state.shown_overlay = new_overlay_e;
    }
    if let Some((e, zone, is_root)) = new_overlay {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::Flex;
            if is_root {
                set_root_zone_rect(&mut n, zone);
            } else {
                set_zone_rect(&mut n, zone);
            }
        }
    }

    let new_marker_e = new_marker.map(|(e, _)| e);
    if state.shown_marker != new_marker_e {
        if let Some(old) = state.shown_marker {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        state.shown_marker = new_marker_e;
    }
    if let Some((e, right)) = new_marker {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::Flex;
            if right {
                n.left = Val::Auto;
                n.right = Val::Px(-2.0);
            } else {
                n.left = Val::Px(-2.0);
                n.right = Val::Auto;
            }
        }
    }
}

fn spawn_ghost(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    id: &str,
    extra: usize,
    cursor: Vec2,
    camera: Option<Entity>,
) -> Entity {
    let (mut title, icon) = tab_meta(id);
    // A whole-leaf drag: show how many background tabs travel with it.
    if extra > 0 {
        title = format!("{title} +{extra}");
    }
    let ghost = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(cursor.x + 12.0),
                top: Val::Px(cursor.y + 12.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            bevy::ui::GlobalZIndex(1000),
            bevy::ui::FocusPolicy::Pass,
            TabGhost,
            renzora::HideInHierarchy,
            Name::new("tab-ghost"),
        ))
        .id();
    // Ghosts for drags out of a floating dock window render on that window's
    // camera; without a target a root node lands on the primary window.
    if let Some(camera) = camera {
        commands.entity(ghost).insert(bevy::ui::UiTargetCamera(camera));
    }
    let gi = glyph(commands, icon, text_primary(), 13.0);
    let gl = commands
        .spawn((
            Text::new(title),
            ui_font(font, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(ghost).add_children(&[gi, gl]);
    ghost
}

/// Corner squares this big (logical px) at the dock's four corners always
/// root-dock — even over a tab bar, so the gesture stays reachable along the
/// top edge. Inside a corner, the nearer edge wins: closer to the side →
/// full-height column, closer to the top/bottom → full-width row.
const ROOT_CORNER_PX: f32 = 48.0;
/// Thin bands along the dock's left/right edges that also root-dock (lower
/// priority than tab bars). No top band: the topmost tab bars line that edge
/// and tab drops there are far more common — the top corners still give
/// access to a full-width top dock.
const ROOT_EDGE_BAND_PX: f32 = 16.0;
/// The bottom band is much taller than the side bands: "drag it to the bottom"
/// is the gesture that docks a panel (or a torn-off floating window) into the
/// full-width bottom panel, so it must be an easy target — nothing else
/// competes for the dock's bottom edge the way tab bars compete for the top.
const ROOT_BOTTOM_BAND_PX: f32 = 48.0;

/// Root edge/corner hit-test over the whole dock area. `(x, y)` is the cursor
/// in dock-local logical px, `size` the dock's logical size. Returns the root
/// zone and whether it was a corner hit (corners outrank tab bars).
fn pick_root_zone(x: f32, y: f32, size: Vec2) -> Option<(DropZone, bool)> {
    if x < 0.0 || y < 0.0 || x > size.x || y > size.y {
        return None;
    }
    let (dl, dr) = (x, size.x - x);
    let (dt, db) = (y, size.y - y);
    let dx = dl.min(dr);
    let dy = dt.min(db);
    let zone_x = if dl <= dr { DropZone::Left } else { DropZone::Right };
    let zone_y = if dt <= db { DropZone::Top } else { DropZone::Bottom };
    if dx <= ROOT_CORNER_PX && dy <= ROOT_CORNER_PX {
        // Corner: the diagonal decides between the column and the row.
        return Some((if dx <= dy { zone_x } else { zone_y }, true));
    }
    if dx <= ROOT_EDGE_BAND_PX {
        return Some((zone_x, false));
    }
    if dy <= ROOT_BOTTOM_BAND_PX && matches!(zone_y, DropZone::Bottom) {
        return Some((zone_y, false));
    }
    None
}

/// Preview rect for a root edge drop — mirrors the `ROOT_DOCK_RATIO` the split
/// will actually use, so the highlight shows the real resulting area.
fn set_root_zone_rect(n: &mut Node, zone: DropZone) {
    let r = ROOT_DOCK_RATIO * 100.0;
    let (l, t, w, h) = match zone {
        DropZone::Center => (0.0, 0.0, 100.0, 100.0),
        DropZone::Left => (0.0, 0.0, r, 100.0),
        DropZone::Right => (100.0 - r, 0.0, r, 100.0),
        DropZone::Top => (0.0, 0.0, 100.0, r),
        DropZone::Bottom => (0.0, 100.0 - r, 100.0, r),
    };
    n.left = Val::Percent(l);
    n.top = Val::Percent(t);
    n.width = Val::Percent(w);
    n.height = Val::Percent(h);
}

fn pick_zone(x: f32, y: f32) -> DropZone {
    const EDGE: f32 = 0.25;
    if x < EDGE {
        DropZone::Left
    } else if x > 1.0 - EDGE {
        DropZone::Right
    } else if y < EDGE {
        DropZone::Top
    } else if y > 1.0 - EDGE {
        DropZone::Bottom
    } else {
        DropZone::Center
    }
}

fn set_zone_rect(n: &mut Node, zone: DropZone) {
    let (l, t, w, h) = match zone {
        DropZone::Center => (0.0, 0.0, 100.0, 100.0),
        DropZone::Left => (0.0, 0.0, 50.0, 100.0),
        DropZone::Right => (50.0, 0.0, 50.0, 100.0),
        DropZone::Top => (0.0, 0.0, 100.0, 50.0),
        DropZone::Bottom => (0.0, 50.0, 100.0, 50.0),
    };
    n.left = Val::Percent(l);
    n.top = Val::Percent(t);
    n.width = Val::Percent(w);
    n.height = Val::Percent(h);
}

/// The new tab order for a same-leaf reorder: `dragged` removed and re-inserted
/// before `before` (or appended).
fn reordered(old: &[String], dragged: &str, before: Option<&str>) -> Vec<String> {
    let mut v: Vec<String> = old.iter().filter(|t| t.as_str() != dragged).cloned().collect();
    match before.and_then(|b| v.iter().position(|t| t == b)) {
        Some(idx) => v.insert(idx, dragged.to_string()),
        None => v.push(dragged.to_string()),
    }
    v
}

/// Insert `dragged` into `tree` per `action`. The removal from the source tree
/// already happened (possibly in a *different* window's tree — cross-window
/// drops split the old remove+insert into two tree mutations). Total: if the
/// stated sibling can't take the panel, it's adopted somewhere in the tree
/// rather than silently dropped.
fn insert_action(tree: &mut DockTree, dragged: &str, action: &DropAction) {
    match action {
        DropAction::Split { rep, zone, .. } => {
            if rep == dragged || !tree.split_at(rep, dragged.to_string(), *zone) {
                tree.adopt_panel(dragged);
            }
        }
        DropAction::RootSplit { zone, .. } => tree.split_root(dragged.to_string(), *zone),
        DropAction::Tab { rep, before, .. } => {
            if rep == dragged || !tree.add_tab_before(rep, dragged.to_string(), before.as_deref())
            {
                tree.adopt_panel(dragged);
            }
        }
    }
}

/// Insert a whole dragged leaf (`leaf_tree`) into `tree` per `action` — the
/// group-drag counterpart of [`insert_action`]. Total in the same way: if the
/// stated sibling can't take the subtree, every panel in it is adopted
/// somewhere in the tree rather than silently dropped.
fn insert_leaf_action(tree: &mut DockTree, leaf_tree: DockTree, action: &DropAction) {
    let adopt_all = |tree: &mut DockTree, sub: &DockTree| {
        let mut ids = Vec::new();
        sub.collect_panels(&mut ids);
        for id in ids {
            tree.adopt_panel(&id);
        }
    };
    match action {
        DropAction::Split { rep, zone, .. } => {
            if !tree.split_at_with(rep, leaf_tree.clone(), *zone) {
                adopt_all(tree, &leaf_tree);
            }
        }
        DropAction::RootSplit { zone, .. } => tree.split_root_with(leaf_tree, *zone),
        DropAction::Tab { rep, before, .. } => {
            let merged = match &leaf_tree {
                DockTree::Leaf { tabs, .. } => {
                    tree.add_tabs_before(rep, tabs, before.as_deref())
                }
                _ => false,
            };
            if !merged {
                adopt_all(tree, &leaf_tree);
            }
        }
    }
}

/// Height of a floating dock window's title bar, logical px.
const FLOAT_TITLEBAR_H: f32 = 26.0;

/// If `area` belongs to a floating dock window whose tree just emptied, queue
/// that window for close — [`process_dock_window_closes`] tears it down at the
/// end of the frame. An empty floating shell has no way to gain panels, so
/// leaving it open is just clutter.
fn close_empty_dock_window(area: Entity, wins: &DockWindows, closes: &mut DockWindowCloseRequests) {
    if let Some(st) = wins.0.iter().find(|s| s.area == area) {
        if st.tree.is_empty() {
            closes.0.push(st.window);
        }
    }
}

/// Undock `id` from `source_area` into a new floating window: remove it from
/// the owning tree, size the window like the leaf it came from (`leaf_size` in
/// physical px, plus the title bar), and queue the window request. `grab` makes
/// the new window follow the held cursor until release (the drag gestures);
/// the context-menu path passes `false` so the window just opens under the
/// cursor. Shared by Ctrl+drag, the header grip, and the right-click menu.
fn tear_off_panel(
    id: &str,
    source_area: Entity,
    leaf_size: Option<Vec2>,
    scale: f32,
    cursor: Option<Vec2>,
    grab: bool,
    (dock, dirty, wins, requests, closes): (
        &mut Dock,
        &mut DockDirty,
        &mut DockWindows,
        &mut DockWindowRequests,
        &mut DockWindowCloseRequests,
    ),
) {
    let size = leaf_size.unwrap_or(Vec2::new(480.0, 360.0) * scale);
    let size = UVec2::new(
        (size.x.clamp(240.0 * scale, 1600.0 * scale)) as u32,
        (size.y + FLOAT_TITLEBAR_H * scale).clamp(160.0 * scale, 1200.0 * scale) as u32,
    );
    area_tree_mut(source_area, dock, wins).remove_panel(id);
    flag_area_dirty(source_area, dirty, wins);
    close_empty_dock_window(source_area, wins, closes);
    // Put the title bar under the cursor so the tear-off reads as grabbing the
    // new window by its header.
    let position = cursor
        .map(|p| (p - Vec2::new(60.0, FLOAT_TITLEBAR_H * 0.5) * scale).round().as_ivec2());
    requests.0.push(DockWindowRequest {
        tree: DockTree::leaf(id),
        position,
        size,
        grab,
    });
}

// ── Floating dock window lifecycle ───────────────────────────────────────────

/// Drain [`DockWindowRequests`]: spawn the OS window, a `Camera2d` targeting
/// it, and its chrome (title bar with close ×, the [`DockArea`], a corner
/// resize grip). The window is undecorated to match the editor's borderless
/// look — the title bar drives OS move via `start_drag_move`, the grip OS
/// resize — which also keeps its `position` exactly the client-area origin
/// (no title-bar offset in the screen-space math).
fn spawn_dock_windows(
    mut requests: ResMut<DockWindowRequests>,
    mut wins: ResMut<DockWindows>,
    mut drag: ResMut<FloatDrag>,
    fonts: Option<Res<EmberFonts>>,
    splash: Option<Res<State<renzora::SplashState>>>,
    registry: Option<Res<renzora::core::ShellPanelRegistry>>,
    mut commands: Commands,
) {
    if requests.0.is_empty() {
        return;
    }
    // Keep requests queued until fonts exist (startup restore can race them),
    // and — in the editor — until the splash/loading phases are over, so
    // restored floating windows don't pop up over the splash screen. Games
    // don't register `SplashState`; they spawn immediately.
    let Some(fonts) = fonts else {
        return;
    };
    if splash.is_some_and(|s| *s.get() != renzora::SplashState::Editor) {
        return;
    }
    for req in requests.0.drain(..) {
        let lead = req.tree.first_panel().unwrap_or("panels");
        let title = registry
            .as_ref()
            .and_then(|r| r.panels.get(lead))
            .map(|info| renzora::lang::t_or(&format!("panel.{lead}"), &info.title))
            .unwrap_or_else(|| humanize(lead));

        let window = commands
            .spawn((
                Window {
                    title: format!("Renzora — {title}"),
                    resolution: bevy::window::WindowResolution::new(req.size.x, req.size.y),
                    position: req
                        .position
                        .map(bevy::window::WindowPosition::At)
                        .unwrap_or(bevy::window::WindowPosition::Automatic),
                    decorations: false,
                    resizable: true,
                    // Normal window level (NOT always-on-top): floats layer
                    // like any OS window so they can live on other monitors
                    // without sitting over everything. Clicking the maximized
                    // main window will raise it over a float on the SAME
                    // monitor — that's standard OS behavior; alt-tab or click
                    // the float's taskbar entry to bring it back.
                    ..default()
                },
                // Seed a cursor icon so the undecorated window always shows
                // one; `apply_cursor_icon` retargets it on hover after that.
                bevy::window::CursorIcon::System(bevy::window::SystemCursorIcon::Default),
                // Engine chrome, not scene content: keep it out of the
                // hierarchy panel AND out of the scene-clear sweep (which
                // despawns named entities without this marker).
                renzora::HideInHierarchy,
                Name::new("dock-window"),
            ))
            .id();
        let camera = commands
            .spawn((
                Camera2d,
                Camera::default(),
                bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
                renzora::HideInHierarchy,
                Name::new("dock-window-camera"),
            ))
            .id();

        // Root column: title bar / dock area, framed by a dark border so the
        // undecorated window reads as a panel against whatever is behind it.
        // `UiTargetCamera` on the root routes the whole subtree onto this
        // window's camera.
        let root = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(rgb(crate::theme::window_bg())),
                BorderColor::all(rgb(border())),
                bevy::ui::UiTargetCamera(camera),
                renzora::HideInHierarchy,
                Name::new("dock-window-root"),
            ))
            .id();

        let bar = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(FLOAT_TITLEBAR_H),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    flex_shrink: 0.0,
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(rgb(header_bg())),
                BorderColor::all(rgb(divider())),
                Interaction::default(),
                FloatWindowBar(window),
                Name::new("dock-window-titlebar"),
            ))
            .id();
        let bar_title = commands
            .spawn((
                Text::new(title),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_primary())),
                bevy::text::TextLayout::no_wrap(),
            ))
            .id();
        let bar_fill = commands
            .spawn((Node {
                flex_grow: 1.0,
                ..default()
            },))
            .id();
        let close = icon_text(&mut commands, &fonts.phosphor, "x", text_muted(), 12.0);
        commands.entity(close).insert((
            Interaction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            // Block so the press closes instead of starting a window drag.
            bevy::ui::FocusPolicy::Block,
            FloatWindowClose(window),
        ));
        commands.entity(bar).add_children(&[bar_title, bar_fill, close]);

        let area = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_basis: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                DockArea,
                FloatingDockArea { window },
                Name::new("dock-window-area"),
            ))
            .id();

        let mut kids = vec![bar, area];
        kids.extend(spawn_float_resize_zones(&mut commands, window));
        commands.entity(root).add_children(&kids);

        bevy::log::info!(
            "[dock] spawned floating window {window} (camera {camera}, root {root}) for '{lead}'"
        );
        wins.0.push(DockWindowState {
            window,
            camera,
            root,
            area,
            tree: req.tree,
            dirty: true,
        });
        if req.grab {
            drag.0 = Some(FloatDragState {
                window,
                grab: Vec2::new(60.0, FLOAT_TITLEBAR_H * 0.5),
                // Tear-off: the release point is still over the tab bar the
                // panel just left — re-docking there would undo the gesture.
                redock: false,
                action: None,
                shown_overlay: None,
            });
        }
    }
}

/// Edge + corner resize zones around a floating window's perimeter, each with
/// the matching resize cursor. Thin absolute strips overlaid on the border;
/// corners are spawned last so they win the hit-test where they overlap edges.
fn spawn_float_resize_zones(commands: &mut Commands, window: Entity) -> Vec<Entity> {
    use bevy::math::CompassOctant as O;
    use bevy::window::SystemCursorIcon as C;
    const EDGE: f32 = 5.0;
    const CORNER: f32 = 12.0;
    let full = Val::Percent(100.0);
    let zones: [(O, C, Val, Val, Val, Val, Val, Val); 8] = [
        // (octant, cursor, left, right, top, bottom, width, height)
        (O::North, C::NsResize, Val::Px(CORNER), Val::Auto, Val::Px(0.0), Val::Auto, full, Val::Px(EDGE)),
        (O::South, C::NsResize, Val::Px(CORNER), Val::Auto, Val::Auto, Val::Px(0.0), full, Val::Px(EDGE)),
        (O::West, C::EwResize, Val::Px(0.0), Val::Auto, Val::Px(CORNER), Val::Auto, Val::Px(EDGE), full),
        (O::East, C::EwResize, Val::Auto, Val::Px(0.0), Val::Px(CORNER), Val::Auto, Val::Px(EDGE), full),
        (O::NorthWest, C::NwResize, Val::Px(0.0), Val::Auto, Val::Px(0.0), Val::Auto, Val::Px(CORNER), Val::Px(CORNER)),
        (O::NorthEast, C::NeResize, Val::Auto, Val::Px(0.0), Val::Px(0.0), Val::Auto, Val::Px(CORNER), Val::Px(CORNER)),
        (O::SouthWest, C::SwResize, Val::Px(0.0), Val::Auto, Val::Auto, Val::Px(0.0), Val::Px(CORNER), Val::Px(CORNER)),
        (O::SouthEast, C::SeResize, Val::Auto, Val::Px(0.0), Val::Auto, Val::Px(0.0), Val::Px(CORNER), Val::Px(CORNER)),
    ];
    zones
        .into_iter()
        .map(|(octant, cursor, left, right, top, bottom, width, height)| {
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left,
                        right,
                        top,
                        bottom,
                        width,
                        height,
                        ..default()
                    },
                    Interaction::default(),
                    crate::cursor_icon::HoverCursor(cursor),
                    FloatWindowResize { window, octant },
                    bevy::ui::GlobalZIndex(200),
                    Name::new("dock-window-resize"),
                ))
                .id()
        })
        .collect()
}

/// Floating window chrome: title-bar press starts a window drag (with re-dock
/// on release, see [`float_window_drag`]), a resize zone starts an OS resize,
/// the × queues the window's close (its panel returns to the main dock via
/// [`process_dock_window_closes`]).
fn float_window_controls(
    bars: Query<(&Interaction, &FloatWindowBar), Changed<Interaction>>,
    resizes: Query<(&Interaction, &FloatWindowResize), Changed<Interaction>>,
    closes: Query<(&Interaction, &FloatWindowClose), Changed<Interaction>>,
    mut windows: Query<&mut Window>,
    mut drag: ResMut<FloatDrag>,
    mut close_queue: ResMut<DockWindowCloseRequests>,
) {
    for (i, bar) in &bars {
        if *i == Interaction::Pressed && drag.0.is_none() {
            if let Ok(w) = windows.get_mut(bar.0) {
                // Grab offset = where inside the window the cursor pressed, so
                // the window doesn't jump under the cursor.
                let grab = w.cursor_position().unwrap_or(Vec2::new(60.0, FLOAT_TITLEBAR_H * 0.5));
                drag.0 = Some(FloatDragState {
                    window: bar.0,
                    grab,
                    redock: true,
                    action: None,
                    shown_overlay: None,
                });
            }
        }
    }
    for (i, rz) in &resizes {
        if *i == Interaction::Pressed {
            if let Ok(mut w) = windows.get_mut(rz.window) {
                w.start_drag_resize(rz.octant);
            }
        }
    }
    for (i, close) in &closes {
        if *i == Interaction::Pressed {
            close_queue.0.push(close.0);
        }
    }
}

/// Show each tab's undock handle while its tab (or the handle itself — once
/// shown, it blocks the tab's own hover) is hovered; hide it otherwise so
/// unhovered tabs keep their exact look and hit-testing.
fn tab_grip_hover(
    grips: Query<(Entity, &Interaction, &TabGrip)>,
    tabs: Query<&Interaction, With<DockTab>>,
    mut nodes: Query<&mut Node>,
) {
    for (grip_entity, grip_interaction, grip) in &grips {
        let hovered = matches!(grip_interaction, Interaction::Hovered | Interaction::Pressed)
            || tabs
                .get(grip.tab)
                .is_ok_and(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
        if let Ok(mut node) = nodes.get_mut(grip_entity) {
            let display = if hovered { Display::Flex } else { Display::None };
            if node.display != display {
                node.display = display;
            }
        }
    }
}

/// Press a tab's undock handle → tear that panel off into a floating window
/// that follows the cursor until release (the same gesture as Ctrl+dragging
/// the tab, without the modifier).
#[allow(clippy::too_many_arguments)]
fn tab_grip_interact(
    grips: Query<(&Interaction, &TabGrip), Changed<Interaction>>,
    leaves: Query<&bevy::ui::ComputedNode, With<DockLeaf>>,
    windows: Query<&Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    global: Res<GlobalCursor>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    mut wins: ResMut<DockWindows>,
    mut requests: ResMut<DockWindowRequests>,
    mut close_queue: ResMut<DockWindowCloseRequests>,
) {
    for (i, grip) in &grips {
        if *i != Interaction::Pressed {
            continue;
        }
        let scale = area_window(grip.area, &wins, primary.single().ok())
            .and_then(|w| windows.get(w).ok())
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        let leaf_size = leaves.get(grip.leaf).map(|cn| cn.size()).ok();
        tear_off_panel(
            &grip.id.clone(),
            grip.area,
            leaf_size,
            scale,
            global.pos,
            true,
            (&mut dock, &mut dirty, &mut wins, &mut requests, &mut close_queue),
        );
    }
}

/// Right-click a tab → context menu with **Undock**: tears that panel off into
/// a floating window opened under the cursor (no drag required).
fn tab_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    fonts: Option<Res<EmberFonts>>,
    windows: Query<&Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    wins: Res<DockWindows>,
    tabs: Query<(&DockTab, &bevy::ui::RelativeCursorPosition)>,
    leaves: Query<(&DockLeaf, &bevy::ui::ComputedNode)>,
    global: Res<GlobalCursor>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };
    for (tab, rcp) in &tabs {
        if !rcp.cursor_over {
            continue;
        }
        let Ok((ld, cn)) = leaves.get(tab.leaf) else {
            break;
        };
        // Menu coordinates are the tab's window's logical cursor (tabs only
        // exist in the primary window — floats are chromeless).
        let win = area_window(ld.area, &wins, primary.single().ok())
            .and_then(|w| windows.get(w).ok());
        let Some(cur) = win.and_then(|w| w.cursor_position()) else {
            break;
        };
        let scale = win.map(|w| w.scale_factor()).unwrap_or(1.0);
        let id = tab.id.clone();
        let area = ld.area;
        let leaf_size = cn.size();
        let gpos = global.pos;

        // Cloned for the second (Close) menu item, since the Undock closure
        // moves `id`. `area` is `Copy`, so it stays available to both.
        let close_id = id.clone();
        let menu = crate::widgets::screen_menu(&mut commands, cur.x, cur.y);
        let undock = crate::widgets::menu_item(
            &mut commands,
            &fonts,
            "arrow-square-out",
            &renzora::lang::t_or("menu.undock", "Undock"),
            move |w: &mut World| {
                // World-side mirror of `tear_off_panel` (menu items run as
                // world closures). Route to the tree owning the leaf's area —
                // in practice the primary dock, since floats have no tabs.
                let floating = w
                    .get_resource::<DockWindows>()
                    .and_then(|ws| ws.0.iter().position(|s| s.area == area));
                match floating {
                    Some(idx) => {
                        if let Some(mut ws) = w.get_resource_mut::<DockWindows>() {
                            ws.0[idx].tree.remove_panel(&id);
                            ws.0[idx].dirty = true;
                        }
                    }
                    None => {
                        if let Some(mut dock) = w.get_resource_mut::<Dock>() {
                            dock.tree.remove_panel(&id);
                        }
                        if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
                            d.0 = true;
                        }
                    }
                }
                let size = UVec2::new(
                    leaf_size.x.clamp(240.0 * scale, 1600.0 * scale) as u32,
                    (leaf_size.y + FLOAT_TITLEBAR_H * scale).clamp(160.0 * scale, 1200.0 * scale)
                        as u32,
                );
                let position = gpos.map(|p| {
                    (p - Vec2::new(60.0, FLOAT_TITLEBAR_H * 0.5) * scale).round().as_ivec2()
                });
                if let Some(mut req) = w.get_resource_mut::<DockWindowRequests>() {
                    req.0.push(DockWindowRequest {
                        tree: DockTree::leaf(id.clone()),
                        position,
                        size,
                        grab: false,
                    });
                }
            },
        );
        // Close: remove the panel from whichever tree owns its area, then close
        // the floating window if that emptied it — the world-side mirror of
        // `tab_close_click` (the per-tab × button).
        let close = crate::widgets::menu_item(
            &mut commands,
            &fonts,
            "x",
            &renzora::lang::t_or("menu.close_panel", "Close panel"),
            move |w: &mut World| {
                let floating = w
                    .get_resource::<DockWindows>()
                    .and_then(|ws| ws.0.iter().position(|s| s.area == area));
                let mut emptied = None;
                match floating {
                    Some(idx) => {
                        if let Some(mut ws) = w.get_resource_mut::<DockWindows>() {
                            if ws.0[idx].tree.remove_panel(&close_id) {
                                ws.0[idx].dirty = true;
                                if ws.0[idx].tree.is_empty() {
                                    emptied = Some(ws.0[idx].window);
                                }
                            }
                        }
                    }
                    None => {
                        if let Some(mut dock) = w.get_resource_mut::<Dock>() {
                            dock.tree.remove_panel(&close_id);
                        }
                        if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
                            d.0 = true;
                        }
                    }
                }
                if let Some(win) = emptied {
                    if let Some(mut closes) = w.get_resource_mut::<DockWindowCloseRequests>() {
                        closes.0.push(win);
                    }
                }
            },
        );
        commands.entity(menu).add_children(&[undock, close]);
        break;
    }
}

/// Drive an in-flight floating-window drag: keep the window under the cursor
/// (physical screen space, so it crosses monitors), and — for title-bar drags —
/// resolve a re-dock target in the main window. Only *small, deliberate*
/// targets re-dock: a leaf's tab bar (dock as a tab there) or the dock's root
/// edge/corner bands (full-height/width split). Leaf centers don't — the main
/// window is usually maximized, so a greedy target would re-dock every drop.
#[allow(clippy::too_many_arguments)]
fn float_window_drag(
    mut drag: ResMut<FloatDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<GlobalCursor>,
    mut windows: Query<&mut Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    areas: Query<
        (
            Entity,
            &bevy::ui::ComputedNode,
            &bevy::ui::UiGlobalTransform,
            Option<&FloatingDockArea>,
        ),
        With<DockArea>,
    >,
    tabbars: Query<(&TabBarOf, &bevy::ui::ComputedNode, &bevy::ui::UiGlobalTransform)>,
    leaves: Query<&DockLeaf>,
    root_overlays: Query<(Entity, &RootDropOverlay)>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    mut wins: ResMut<DockWindows>,
    mut close_queue: ResMut<DockWindowCloseRequests>,
) {
    if drag.0.is_none() {
        return;
    }

    // ── Release: apply the re-dock (if any) and end the drag. ──
    if !mouse.pressed(MouseButton::Left) {
        let Some(state) = drag.0.take() else {
            return;
        };
        if let Some(e) = state.shown_overlay {
            if let Ok(mut n) = nodes.get_mut(e) {
                n.display = Display::None;
            }
        }
        if let Some(action) = state.action {
            // Move the float's panel (floats host exactly one) into the target
            // tree, then close the emptied window.
            let panel = wins
                .0
                .iter()
                .find(|s| s.window == state.window)
                .and_then(|s| s.tree.first_panel().map(str::to_string));
            if let Some(panel) = panel {
                if let Some(st) = wins.0.iter_mut().find(|s| s.window == state.window) {
                    st.tree.remove_panel(&panel);
                }
                insert_action(
                    area_tree_mut(action.area(), &mut dock, &mut wins),
                    &panel,
                    &action,
                );
                flag_area_dirty(action.area(), &mut dirty, &mut wins);
                close_queue.0.push(state.window);
            }
        }
        return;
    }

    let Some(state) = drag.0.as_mut() else {
        return;
    };

    // ── Move the window under the cursor. ──
    let Some(pos) = cursor.pos else {
        return;
    };
    let Ok(mut window) = windows.get_mut(state.window) else {
        return;
    };
    let scale = window.scale_factor();
    let target = (pos - state.grab * scale).round().as_ivec2();
    if window.position != bevy::window::WindowPosition::At(target) {
        window.position = bevy::window::WindowPosition::At(target);
    }

    // ── Re-dock targeting (title-bar drags only), in the primary window. ──
    let mut action: Option<DropAction> = None;
    let mut overlay: Option<(Entity, DropZone, bool)> = None;
    if state.redock {
        let primary_local = primary.single().ok().and_then(|pw| {
            windows
                .get(pw)
                .ok()
                .filter(|w| window_contains(w, pos))
                .and_then(|w| window_local(w, pos))
        });
        if let Some(local) = primary_local {
            let primary_area = areas.iter().find(|(.., f)| f.is_none());
            // Tab bars first: dock as a tab in that leaf.
            let bar_hit = tabbars.iter().find_map(|(bar, cn, gt)| {
                cn.contains_point(*gt, local).then_some(bar.0)
            });
            if let Some(leaf_ent) = bar_hit {
                if let Ok(ld) = leaves.get(leaf_ent) {
                    // Only main-window leaves are targets.
                    if primary_area.is_some_and(|(a, ..)| a == ld.area) {
                        if let Some(rep) = ld.tabs.first().cloned() {
                            action = Some(DropAction::Tab {
                                area: ld.area,
                                rep,
                                before: None,
                            });
                            overlay = Some((ld.overlay, DropZone::Center, false));
                        }
                    }
                }
            } else if let Some((area_e, cn, gt, _)) = primary_area {
                // Root edge/corner bands of the main dock.
                if let Some(norm) = cn.normalize_point(*gt, local) {
                    let size = cn.size() * cn.inverse_scale_factor();
                    let (x, y) = ((norm.x + 0.5) * size.x, (norm.y + 0.5) * size.y);
                    if let Some((zone, _)) = pick_root_zone(x, y, size) {
                        action = Some(DropAction::RootSplit { area: area_e, zone });
                        overlay = root_overlays
                            .iter()
                            .find(|(_, o)| o.area == area_e)
                            .map(|(e, _)| (e, zone, true));
                    }
                }
            }
        }
    }
    state.action = action;

    // Show/hide the drop preview (same mechanics as the tab drag).
    let overlay_e = overlay.map(|(e, _, _)| e);
    if state.shown_overlay != overlay_e {
        if let Some(old) = state.shown_overlay {
            if let Ok(mut n) = nodes.get_mut(old) {
                n.display = Display::None;
            }
        }
        state.shown_overlay = overlay_e;
    }
    if let Some((e, zone, is_root)) = overlay {
        if let Ok(mut n) = nodes.get_mut(e) {
            n.display = Display::Flex;
            if is_root {
                set_root_zone_rect(&mut n, zone);
            } else {
                set_zone_rect(&mut n, zone);
            }
        }
    }
}

/// Tear down closing floating dock windows: everything queued in
/// [`DockWindowCloseRequests`] (the ×, last-panel-out, re-docks), plus
/// `WindowCloseRequested` messages for our windows (Alt+F4 — handled here so
/// the teardown is atomic; bevy's `close_when_requested` marker dance would
/// despawn the window a frame before we'd notice), plus any window despawned
/// externally (caught via `RemovedComponents` the same frame).
///
/// The window's panels return to the primary dock so they're never lost, and
/// the window, its camera and its UI root despawn in ONE command batch. This
/// system runs in `PostUpdate` **before** `CameraUpdateSystems`: a camera whose
/// `RenderTarget::Window` entity is already gone panics `camera_system`, so the
/// camera must never survive its window into that set — not even one frame.
///
/// Also despawns all remaining floating windows when the PRIMARY window is
/// closed, so the app's `ExitCondition::OnAllClosed` fires instead of the
/// process lingering with orphaned tool windows.
fn process_dock_window_closes(
    mut queue: ResMut<DockWindowCloseRequests>,
    mut close_requested: MessageReader<bevy::window::WindowCloseRequested>,
    mut removed: RemovedComponents<Window>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    windows: Query<(), With<Window>>,
    mut last_primary: Local<Option<Entity>>,
    mut wins: ResMut<DockWindows>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    mut commands: Commands,
) {
    // Remember the primary window entity while it exists — once it's despawned
    // it can't be queried, and the removal event alone doesn't say whose it
    // was. (Our own float despawns surface here as removals a frame later, so
    // "any removed window that isn't ours" would misread them as the primary.)
    if let Ok(pw) = primary.single() {
        *last_primary = Some(pw);
    }

    let mut to_close: Vec<(Entity, &'static str)> =
        queue.0.drain(..).map(|e| (e, "requested")).collect();
    // Alt+F4 / OS close on one of OUR windows only — the primary window's
    // close is the shell chrome's (and bevy's) business.
    for ev in close_requested.read() {
        if wins.0.iter().any(|s| s.window == ev.window) {
            to_close.push((ev.window, "os-close"));
        }
    }
    let externally_removed: Vec<Entity> = removed.read().collect();
    to_close.extend(externally_removed.iter().map(|e| (*e, "despawned-externally")));
    // Belt-and-braces: a state whose window entity no longer exists but never
    // surfaced through the paths above (missed events). Whatever despawned it,
    // the camera + root must not linger.
    for st in &wins.0 {
        if !windows.contains(st.window) {
            to_close.push((st.window, "window-vanished"));
        }
    }

    let primary_gone = last_primary
        .is_some_and(|pw| externally_removed.contains(&pw))
        && !wins.0.is_empty();

    for (e, why) in to_close {
        let Some(idx) = wins.0.iter().position(|s| s.window == e) else {
            continue;
        };
        let st = wins.0.swap_remove(idx);
        bevy::log::info!(
            "[dock] closing floating window {e} ({why}); camera {}, root {}",
            st.camera,
            st.root
        );
        let mut panels = Vec::new();
        st.tree.collect_panels(&mut panels);
        for p in panels {
            if !dock.tree.contains_panel(&p) {
                dock.tree.adopt_panel(&p);
                dirty.0 = true;
            }
        }
        commands.entity(st.window).try_despawn();
        commands.entity(st.camera).try_despawn();
        commands.entity(st.root).try_despawn();
    }

    // A non-dock window was despawned — that's the primary closing. Take every
    // floating window down with it (their layouts are already persisted by the
    // shell, and lingering windows would keep the app alive).
    if primary_gone {
        for st in wins.0.drain(..) {
            bevy::log::info!("[dock] primary window closed; closing floating window {}", st.window);
            commands.entity(st.window).try_despawn();
            commands.entity(st.camera).try_despawn();
            commands.entity(st.root).try_despawn();
        }
    }
}

/// Last line of defense against the `camera_system` panic ("RenderTarget::
/// Window missing"): if ANY camera still targets a window entity that no
/// longer exists — whatever despawned it, through whatever path — despawn the
/// camera before bevy's camera update evaluates it. This never false-positives
/// (a camera and its window spawn in the same command batch) and the warning
/// makes an otherwise-invisible teardown bug diagnosable from the console log
/// instead of a crash report.
fn guard_dock_target_cameras(
    cameras: Query<(Entity, &bevy::camera::RenderTarget), With<Camera>>,
    windows: Query<(), With<Window>>,
    mut commands: Commands,
) {
    for (cam, rt) in &cameras {
        if let bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(w)) = rt {
            if !windows.contains(*w) {
                bevy::log::warn!(
                    "[dock] camera {cam} targets missing window {w} — despawning the camera \
                     to avoid a render panic (something despawned the window out from under it)"
                );
                commands.entity(cam).try_despawn();
            }
        }
    }
}

/// (Re)build each dirty dock: the primary [`DockArea`] when [`DockDirty`], and
/// every floating dock window whose per-window flag is set.
fn rebuild_dock(
    mut dirty: ResMut<DockDirty>,
    mut wins: ResMut<DockWindows>,
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    dock: Res<Dock>,
    markers: Res<BottomStripMarkers>,
    areas: Query<(Entity, Option<&Children>, Option<&FloatingDockArea>), With<DockArea>>,
    leaves: Query<&DockLeaf>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    if dirty.0 {
        if let Some((area_entity, children, _)) = areas.iter().find(|(.., f)| f.is_none()) {
            rebuild_area(
                &mut commands, &fonts, &markers.0, &dock.tree, area_entity, children, &leaves,
                false,
            );
            dirty.0 = false;
        }
    }
    for st in wins.0.iter_mut().filter(|s| s.dirty) {
        // The area may not exist yet the frame the window spawns (commands
        // apply at the end of the frame) — leave it dirty and retry next frame.
        if let Ok((area_entity, children, _)) = areas.get(st.area) {
            rebuild_area(
                &mut commands, &fonts, &markers.0, &st.tree, area_entity, children, &leaves, true,
            );
            st.dirty = false;
        }
    }
}

/// Rebuild one dock area's subtree from `tree`. Shared by the primary dock and
/// floating dock windows; `floating` leaves are chromeless (no tab bar — the
/// window's own title bar plays that role).
#[allow(clippy::too_many_arguments)]
fn rebuild_area(
    commands: &mut Commands,
    fonts: &EmberFonts,
    markers: &[String],
    tree: &DockTree,
    area_entity: Entity,
    children: Option<&Children>,
    leaves: &Query<&DockLeaf>,
    floating: bool,
) {
    // Preserve each leaf's content entity (keyed by its active panel) and detach
    // it from the hierarchy so the despawn below doesn't take it — `build_tree`
    // re-parents it, so reordering/moving tabs keeps the panel (and its state)
    // instead of recreating it.
    //
    // Only detach contents the *new* tree will reuse. A content node that is
    // detached-to-root and then despawned in the same frame (e.g. every non-
    // viewport panel when maximizing) corrupts bevy_ui's taffy tree: it gets
    // reparented to the implicit-viewport root and its slotmap key freed, while
    // its old leaf still lists it as a child — so taffy panics with
    // `invalid SlotMap key` when removing that leaf. Leaving non-reused contents
    // attached lets them despawn safely with their old leaf instead.
    //
    // Scoped to THIS area's leaves: with floating dock windows, another window
    // can legitimately show the same panel id — detaching its content here
    // would steal a live panel out of that window.
    let mut reusable = std::collections::HashSet::new();
    tree.active_tab_ids(&mut reusable);
    let mut preserved: HashMap<String, Entity> = HashMap::new();
    for leaf in leaves.iter().filter(|l| l.area == area_entity) {
        if !leaf.active.is_empty() && reusable.contains(&leaf.active) {
            preserved.insert(leaf.active.clone(), leaf.content);
            commands.entity(leaf.content).remove::<ChildOf>();
        }
    }

    // `try_despawn`: a child may already have been despawned this frame by
    // another system (e.g. a panel tearing down its own content, or a chrome
    // rebuild racing this one) — a plain `despawn` on that stale handle panics.
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).try_despawn();
        }
    }
    let tree_root = build_tree(
        commands,
        &fonts.ui,
        &fonts.phosphor,
        markers,
        area_entity,
        floating,
        None,
        Vec::new(),
        tree,
        &mut preserved,
    );
    commands.entity(area_entity).add_child(tree_root);

    // Root drop overlay + cursor tracking for edge/corner docking. Added after
    // the tree so the overlay draws on top; recreated with every rebuild (the
    // child-despawn above took the previous one).
    commands
        .entity(area_entity)
        .insert(bevy::ui::RelativeCursorPosition::default());
    let root_overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            RootDropOverlay { area: area_entity },
            Name::new("root-drop-overlay"),
        ))
        .id();
    commands.entity(area_entity).add_child(root_overlay);

    // Any preserved content not reused (its panel no longer exists) → despawn.
    // `try_despawn` for the same stale-handle reason as the child sweep above.
    for (_, content) in preserved.drain() {
        commands.entity(content).try_despawn();
    }
}

/// Track which panel the user last clicked into as [`FocusedPanel`]. A left
/// press over a leaf makes that leaf's visible panel the focused one. Panel
/// content nodes block picking (`FocusPolicy::Block`), so leaf `Interaction`
/// is unreliable here — we test the leaf's own `RelativeCursorPosition`, which
/// is true whenever the cursor is anywhere inside that leaf. Tab switches and
/// programmatic focus update [`FocusedPanel`] separately (see
/// [`apply_tab_switch`] / [`apply_focus_request`]).
fn track_focused_panel(
    mouse: Res<ButtonInput<MouseButton>>,
    mut focused: ResMut<FocusedPanel>,
    leaves: Query<(&DockLeaf, &bevy::ui::RelativeCursorPosition)>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    for (leaf, rcp) in &leaves {
        if rcp.cursor_over {
            if focused.0.as_deref() != Some(leaf.active.as_str()) {
                focused.0 = Some(leaf.active.clone());
            }
            return;
        }
    }
}

/// Turn a [`FocusPanelRequest`] into a pending tab switch: locate the leaf that
/// holds the requested panel and, if it isn't already the active tab, queue the
/// same in-place switch a click would. Runs just before [`apply_tab_switch`] so
/// the switch lands the same frame.
fn apply_focus_request(
    mut req: ResMut<FocusPanelRequest>,
    mut pending: ResMut<PendingSwitch>,
    mut focused: ResMut<FocusedPanel>,
    leaves: Query<(Entity, &DockLeaf)>,
) {
    let Some(id) = req.0.take() else {
        return;
    };
    for (entity, leaf) in &leaves {
        if leaf.tabs.iter().any(|t| t == &id) {
            if leaf.active != id {
                pending.0 = Some((entity, id.clone()));
            }
            // Programmatic focus counts as focusing the panel even when it's
            // already the active tab (no switch queued).
            focused.0 = Some(id);
            return;
        }
    }
}

/// Apply a pending tab switch in place: recolor label/icon and update the
/// leaf's `active` id (the consumer's content system reacts to that).
fn apply_tab_switch(
    mut pending: ResMut<PendingSwitch>,
    mut dock: ResMut<Dock>,
    mut wins: ResMut<DockWindows>,
    mut focused: ResMut<FocusedPanel>,
    tabs: Query<(Entity, &DockTab)>,
    mut leaves: Query<&mut DockLeaf>,
    mut colors: Query<&mut TextColor>,
) {
    let Some((leaf, id)) = pending.0.take() else {
        return;
    };
    // Switching to a tab focuses it.
    focused.0 = Some(id.clone());
    if let Ok(ld) = leaves.get(leaf) {
        area_tree_mut(ld.area, &mut dock, &mut wins).set_active_tab(&id);
    }

    for (_tab_entity, tab) in &tabs {
        if tab.leaf != leaf {
            continue;
        }
        let fg = rgb(if tab.id == id { text_primary() } else { text_muted() });
        if let Ok(mut c) = colors.get_mut(tab.label) {
            c.0 = fg;
        }
        if let Ok(mut c) = colors.get_mut(tab.icon) {
            c.0 = fg;
        }
    }

    if let Ok(mut ld) = leaves.get_mut(leaf) {
        ld.active = id;
    }
}

/// Tab background follows hover + active state (active wins).
fn tab_hover(
    leaves: Query<&DockLeaf>,
    theme: Res<crate::style::Theme>,
    mut tabs: Query<(&Interaction, &DockTab, &mut BackgroundColor)>,
    mut texts: Query<&mut TextColor>,
) {
    let d = &theme.dock;
    for (interaction, tab, mut bg) in &mut tabs {
        // Active per the tab's own leaf (not a tree lookup): with floating dock
        // windows the same panel id can exist in several trees at once.
        let active = leaves.get(tab.leaf).is_ok_and(|l| l.active == tab.id);
        let hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        let target = if active {
            d.tab_active.color()
        } else if hovered {
            d.tab_hover.color()
        } else {
            d.tab_inactive.color()
        };
        if bg.0 != target {
            bg.0 = target;
        }
        let tc = if active {
            d.tab_text_active.color()
        } else {
            d.tab_text.color()
        };
        for ent in [tab.label, tab.icon] {
            if let Ok(mut t) = texts.get_mut(ent) {
                if t.0 != tc {
                    t.0 = tc;
                }
            }
        }
    }
}

/// A tab's close × reddens while the cursor is over it.
fn tab_close_hover(
    mut closes: Query<(&bevy::ui::RelativeCursorPosition, &mut TextColor), With<TabClose>>,
) {
    for (rcp, mut color) in &mut closes {
        let target = rgb(if rcp.cursor_over { close_red() } else { text_muted() });
        if color.0 != target {
            color.0 = target;
        }
    }
}

/// Click a tab's × → remove that panel from its dock tree (primary or a
/// floating window's; closing a floating window's last tab closes the window).
fn tab_close_click(
    mouse: Res<ButtonInput<MouseButton>>,
    closes: Query<(&bevy::ui::RelativeCursorPosition, &ChildOf), With<TabClose>>,
    tabs: Query<&DockTab>,
    leaves: Query<&DockLeaf>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    mut wins: ResMut<DockWindows>,
    mut close_queue: ResMut<DockWindowCloseRequests>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    for (rcp, parent) in &closes {
        if !rcp.cursor_over {
            continue;
        }
        if let Ok(tab) = tabs.get(parent.parent()) {
            let area = leaves.get(tab.leaf).map(|l| l.area).ok();
            let Some(area) = area else { break };
            if area_tree_mut(area, &mut dock, &mut wins).remove_panel(&tab.id) {
                flag_area_dirty(area, &mut dirty, &mut wins);
                close_empty_dock_window(area, &wins, &mut close_queue);
            }
        }
        break;
    }
}

/// Click a bottom region's collapse chevron → publish [`BottomSnapRequest`]
/// so the consumer (the shell's bottom panel) collapses that region. No
/// ratio in the payload: unlike a divider snap, a click leaves the tree's
/// ratio accurate, so the consumer records whatever the detach returns.
fn bottom_collapse_click(
    q: Query<(&Interaction, &BottomCollapseBtn), Changed<Interaction>>,
    mut snap: ResMut<BottomSnapRequest>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        snap.0 = Some(BottomSnap {
            restore: None,
            target: btn.target.clone(),
        });
    }
}

// ── Reconciler ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn build_tree(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    phosphor: &Handle<Font>,
    markers: &[String],
    area: Entity,
    floating: bool,
    parent: Option<ParentSplit>,
    path: Vec<bool>,
    tree: &DockTree,
    preserved: &mut HashMap<String, Entity>,
) -> Entity {
    match tree {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let row = matches!(direction, SplitDirection::Horizontal);
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        min_width: Val::Px(0.0),
                        min_height: Val::Px(0.0),
                        flex_direction: if row {
                            FlexDirection::Row
                        } else {
                            FlexDirection::Column
                        },
                        position_type: PositionType::Relative,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    Name::new("split"),
                ))
                .id();

            let pct = ratio.clamp(0.1, 0.9) * 100.0;

            let mut wa = Node {
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                ..default()
            };
            if row {
                wa.width = Val::Percent(pct);
                wa.height = Val::Percent(100.0);
            } else {
                wa.height = Val::Percent(pct);
                wa.width = Val::Percent(100.0);
            }
            let wrap_a = commands.spawn((wa, Name::new("split-first"))).id();
            let mut path_a = path.clone();
            path_a.push(false);
            let info_a = ParentSplit {
                container,
                first_wrap: wrap_a,
                horizontal: row,
                is_second: false,
                path: path.clone(),
            };
            let child_a = build_tree(
                commands, font, phosphor, markers, area, floating, Some(info_a), path_a, first,
                preserved,
            );
            commands.entity(wrap_a).add_child(child_a);

            // The divider is a flush 1px line laid out as a flex sibling *between*
            // the two panes, so the panels stay flush (no gap) and the line sits
            // exactly on the boundary at any dock position / nesting / UI scale.
            //
            // The grab area is a wider, transparent, absolutely-positioned child of
            // the line. Centering it uses only negative *px* insets (no percent +
            // margin combo, which taffy doesn't honor on abs-pos nodes — the bug
            // that made the old handle drift off the line). It overhangs both panes
            // without taking layout space; a `ZIndex` lift puts the line + handle
            // above the neighbouring pane so the whole grab width is clickable.
            const GRAB: f32 = 11.0;
            let mut line = Node {
                flex_shrink: 0.0,
                position_type: PositionType::Relative,
                ..default()
            };
            if row {
                line.width = Val::Px(1.0);
                line.height = Val::Percent(100.0);
            } else {
                line.height = Val::Px(1.0);
                line.width = Val::Percent(100.0);
            }
            let divider = commands
                .spawn((
                    line,
                    BackgroundColor(rgb(divider())),
                    bevy::ui::ZIndex(1),
                    DockPart::Divider,
                    Name::new("divider"),
                ))
                .id();

            let cursor = crate::cursor_icon::parse_cursor(if row {
                "ew-resize"
            } else {
                "ns-resize"
            })
            .unwrap();
            let mut hit = Node {
                position_type: PositionType::Absolute,
                ..default()
            };
            if row {
                hit.left = Val::Px(-(GRAB - 1.0) / 2.0);
                hit.width = Val::Px(GRAB);
                hit.top = Val::Px(0.0);
                hit.height = Val::Percent(100.0);
            } else {
                hit.top = Val::Px(-(GRAB - 1.0) / 2.0);
                hit.height = Val::Px(GRAB);
                hit.left = Val::Px(0.0);
                hit.width = Val::Percent(100.0);
            }
            // A vertical split whose bottom pane is a leaf tabbing one of the
            // bottom-strip marker panels is a collapsible region: tag its
            // divider so overshooting it downward snap-closes the strip (the
            // root divider does this content-agnostically via its empty path).
            let strip = if !row && !floating {
                match &**second {
                    DockTree::Leaf { tabs, .. } => {
                        tabs.iter().find(|t| markers.contains(*t)).cloned()
                    }
                    _ => None,
                }
            } else {
                None
            };
            let handle = commands
                .spawn((
                    hit,
                    Interaction::default(),
                    crate::cursor_icon::HoverCursor(cursor),
                    Divider {
                        container,
                        first_wrap: wrap_a,
                        horizontal: row,
                        path: path.clone(),
                        area,
                        floating,
                        strip,
                    },
                    Name::new("divider-handle"),
                ))
                .id();
            commands.entity(divider).add_child(handle);

            let mut wb = Node {
                overflow: Overflow::clip(),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                // Without a zero minimum, this flex child's automatic min-size is
                // its (tall) content, so it inflates instead of shrinking to the
                // remaining space — tall panels then clip at the bottom with no
                // scroll. (The first child is capped by its explicit pct size.)
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            };
            if row {
                wb.height = Val::Percent(100.0);
            } else {
                wb.width = Val::Percent(100.0);
            }
            let wrap_b = commands.spawn((wb, Name::new("split-second"))).id();
            let mut path_b = path.clone();
            path_b.push(true);
            let info_b = ParentSplit {
                container,
                first_wrap: wrap_a,
                horizontal: row,
                is_second: true,
                path: path.clone(),
            };
            let child_b = build_tree(
                commands, font, phosphor, markers, area, floating, Some(info_b), path_b, second,
                preserved,
            );
            commands.entity(wrap_b).add_child(child_b);

            commands
                .entity(container)
                .add_children(&[wrap_a, divider, wrap_b]);
            container
        }
        DockTree::Leaf { tabs, active_tab } => build_leaf(
            commands, font, phosphor, markers, area, floating, parent, tabs, *active_tab,
            preserved,
        ),
        DockTree::Empty => {
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Name::new("empty"),
                ))
                .id();
            let btn = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(tab_active())),
                    BorderColor::all(rgb(border())),
                    Interaction::default(),
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                    // No leaf yet — picking sets the tree's root leaf.
                    AddPanelButton {
                        leaf: Entity::PLACEHOLDER,
                        area,
                    },
                    Name::new("empty-add-panel"),
                ))
                .id();
            let ic = icon_text(commands, phosphor, "plus", text_muted(), 14.0);
            let t = commands
                .spawn((
                    Text::new(renzora::lang::t("menu.add_panel")),
                    ui_font(font, 13.0),
                    TextColor(rgb(text_muted())),
                ))
                .id();
            commands.entity(btn).add_children(&[ic, t]);
            commands.entity(container).add_child(btn);
            container
        }
    }
}

/// Which dock element a node styles from [`crate::style::DockStyle`].
#[derive(Component, Clone, Copy)]
pub(crate) enum DockPart {
    Leaf,
    TabBar,
    Divider,
    /// A tab button — geometry (radius/padding) from the style; its bg + text
    /// colors are state-driven by `tab_hover`.
    Tab,
}

/// Paint every [`DockPart`] from the live `Theme.dock` on theme change / spawn —
/// the dock's panel chrome (leaf bg/border/radius/padding + drop shadow, tab bar,
/// dividers) follows the theme and is editable in the Theme tab.
pub(crate) fn apply_dock_style(
    theme: Res<crate::style::Theme>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        Ref<DockPart>,
        &mut BackgroundColor,
        Option<&mut BorderColor>,
        &mut Node,
    )>,
) {
    let repaint = theme.is_changed();
    let d = &theme.dock;
    for (e, part, mut bg, border, mut node) in &mut q {
        if !repaint && !part.is_added() {
            continue;
        }
        match *part {
            DockPart::Leaf => {
                bg.0 = d.leaf_bg.color();
                node.border = UiRect::all(Val::Px(d.leaf_border_width));
                node.border_radius = BorderRadius::all(Val::Px(d.leaf_radius));
                node.padding = UiRect::all(Val::Px(d.leaf_padding));
                node.margin = UiRect::all(Val::Px(d.leaf_margin));
                if let Some(mut bc) = border {
                    *bc = BorderColor::all(d.leaf_border.color());
                }
                if d.shadow {
                    commands.entity(e).insert(BoxShadow::new(
                        d.shadow_color.color().with_alpha(d.shadow_alpha),
                        Val::Px(d.shadow_x),
                        Val::Px(d.shadow_y),
                        Val::Px(d.shadow_spread),
                        Val::Px(d.shadow_blur),
                    ));
                } else {
                    commands.entity(e).remove::<BoxShadow>();
                }
            }
            DockPart::TabBar => {
                bg.0 = d.tabbar_bg.color();
                node.border = UiRect::bottom(Val::Px(d.header_border_width));
                node.border_radius = BorderRadius::all(Val::Px(d.header_radius));
                node.padding = UiRect::axes(Val::Px(d.header_pad_x), Val::Px(d.header_pad_y));
                if let Some(mut bc) = border {
                    *bc = BorderColor::all(d.header_border.color());
                }
            }
            DockPart::Tab => {
                node.border_radius = BorderRadius::all(Val::Px(d.tab_radius));
                node.padding = UiRect::axes(Val::Px(d.tab_pad_x), Val::Px(d.tab_pad_y));
            }
            DockPart::Divider => bg.0 = d.divider.color(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_leaf(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    phosphor: &Handle<Font>,
    markers: &[String],
    area: Entity,
    floating: bool,
    parent: Option<ParentSplit>,
    tabs: &[String],
    active: usize,
    preserved: &mut HashMap<String, Entity>,
) -> Entity {
    let leaf = commands
        .spawn((
            Node {
                // Flex-fill the cell (equivalent to 100%/100% when margin is 0)
                // so a `leaf_margin` insets the panel cleanly instead of
                // overflowing the way a fixed 100% size + margin would.
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                align_self: AlignSelf::Stretch,
                // Force a zero minimum so tall panel content can't push the
                // leaf's content-based min-size up and disturb sibling splits.
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(Color::NONE),
            DockPart::Leaf,
            crate::widgets::ThemeShaderSurface {
                surface: crate::widgets::ThemeSurface::Panel,
            },
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("leaf"),
        ))
        .id();
    populate_leaf(
        commands, font, phosphor, markers, area, floating, parent, leaf, tabs, active, preserved,
    );
    leaf
}

/// The tab bar's `+` button — opens the Add-Panel picker for its leaf.
#[derive(Component)]
struct AddPanelButton {
    leaf: Entity,
    /// The dock area the leaf lives in — routes the add to the right tree.
    area: Entity,
}

/// The horizontally-clipping container holding a leaf's tabs. When the tabs
/// overflow the leaf width it clips them (`Overflow::scroll_x`) and the wheel
/// pans it ([`tab_strip_wheel`]) — there is no visible scrollbar, so it reads
/// as tabs that quietly slide under the pinned `+`/collapse controls. Carries
/// its leaf so the in-place tab reorder ([`tab_drag`]) can re-sort *the strip's*
/// children (the tabs) rather than the tab bar's — the bar also holds the grip,
/// `+`, and collapse chevron, which a wholesale reorder would drop.
#[derive(Component)]
struct TabScrollStrip {
    leaf: Entity,
}

/// The undock handle at the left of a tab: press it to tear that panel off
/// into a floating window — no Ctrl needed. Overlaid absolutely on the tab's
/// left padding so it takes NO layout space (no gap in unhovered tabs), and
/// kept `Display::None` until the tab is hovered so the hidden handle can't
/// intercept clicks either. `FocusPolicy::Block` so pressing it undocks
/// instead of switching/dragging the tab (the same trick the close × uses).
#[derive(Component)]
struct TabGrip {
    /// The tab this handle sits in (drives hover visibility).
    tab: Entity,
    /// Panel id to undock.
    id: String,
    leaf: Entity,
    area: Entity,
}

/// Click the tab bar `+` → open the shared search overlay of panels not already
/// in that leaf; selecting one adds it as a tab (mirrors the egui panel picker).
fn add_panel_click(
    q: Query<(&Interaction, &AddPanelButton), Changed<Interaction>>,
    leaves: Query<&DockLeaf>,
    fonts: Option<Res<EmberFonts>>,
    registry: Option<Res<renzora::core::ShellPanelRegistry>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let leaf = btn.leaf;
        let area = btn.area;
        let existing: std::collections::HashSet<String> = leaves
            .get(leaf)
            .map(|l| l.tabs.iter().cloned().collect())
            .unwrap_or_default();
        let mut entries: Vec<crate::widgets::SearchEntry> = Vec::new();
        if let Some(reg) = &registry {
            let mut items: Vec<(&String, &renzora::core::ShellPanelInfo)> =
                reg.panels.iter().collect();
            items.sort_by(|a, b| a.1.title.cmp(&b.1.title));
            for (id, info) in items {
                if existing.contains(id.as_str()) {
                    continue;
                }
                let id = id.clone();
                let icon = if info.icon.is_empty() {
                    "circle".to_string()
                } else {
                    info.icon.clone()
                };
                let category_en = if info.category.is_empty() {
                    "General".to_string()
                } else {
                    info.category.clone()
                };
                // Localized category header. The slug (lowercased, non-alphanumerics
                // → '_') keys `panel.cat.<slug>`; grouping still keys off this string,
                // so all entries in one English category share one localized header.
                let slug: String = category_en
                    .trim()
                    .to_lowercase()
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                let category = renzora::lang::t_or(&format!("panel.cat.{}", slug), &category_en);
                // Localized panel display name — reuses the `panel.<id>` keys seeded
                // by the chrome pass; falls back to the registry's English title.
                let label = renzora::lang::t_or(&format!("panel.{}", id), &info.title);
                entries.push(crate::widgets::SearchEntry::new(
                    icon,
                    label,
                    category,
                    move |w: &mut World| {
                        let sibling = w.get::<DockLeaf>(leaf).and_then(|l| l.tabs.first().cloned());
                        // Route to the tree owning this leaf's area — a floating
                        // dock window's, else the primary dock's.
                        let floating = w
                            .get_resource::<DockWindows>()
                            .and_then(|ws| ws.0.iter().position(|s| s.area == area));
                        match floating {
                            Some(idx) => {
                                if let Some(mut ws) = w.get_resource_mut::<DockWindows>() {
                                    let st = &mut ws.0[idx];
                                    match &sibling {
                                        Some(sib) => {
                                            st.tree.add_tab(sib, id.clone());
                                        }
                                        None => st.tree = DockTree::leaf(id.clone()),
                                    }
                                    st.dirty = true;
                                }
                            }
                            None => {
                                if let Some(mut dock) = w.get_resource_mut::<Dock>() {
                                    match &sibling {
                                        Some(sib) => {
                                            dock.tree.add_tab(sib, id.clone());
                                        }
                                        None => dock.tree = DockTree::leaf(id.clone()),
                                    }
                                }
                                if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
                                    d.0 = true;
                                }
                            }
                        }
                    },
                ));
            }
        }
        let title = renzora::lang::t("menu.add_panel");
        crate::widgets::grid_overlay(&mut commands, &fonts, &title, entries);
    }
}

/// Wheel over a tab strip pans it horizontally. The strip clips overflowing
/// tabs but draws no scrollbar, so without this the tabs pushed past the leaf's
/// right edge would be unreachable. Vertical wheel maps to horizontal because a
/// 28px-tall strip has no vertical range; a trackpad's horizontal wheel passes
/// through. Bevy clamps `ScrollPosition` to the scrollable range during layout,
/// so we needn't clamp the far edge ourselves — only guard against going
/// negative past the first tab.
fn tab_strip_wheel(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut strips: Query<
        (&bevy::ui::RelativeCursorPosition, &mut bevy::ui::ScrollPosition),
        With<TabScrollStrip>,
    >,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let mut delta = 0.0;
    for ev in wheel.read() {
        // Line events are ~1 notch each; scale to about one tab's width. Pixel
        // events (precision trackpads) are already in logical px.
        let unit = if matches!(ev.unit, MouseScrollUnit::Line) { 24.0 } else { 1.0 };
        // Wheel down (negative y) reveals tabs to the right; horizontal wheel
        // adds directly.
        delta += (-ev.y + ev.x) * unit;
    }
    if delta == 0.0 {
        return;
    }
    for (rcp, mut scroll) in &mut strips {
        if rcp.cursor_over {
            scroll.x = (scroll.x + delta).max(0.0);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn populate_leaf(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    phosphor: &Handle<Font>,
    markers: &[String],
    area: Entity,
    floating: bool,
    parent: Option<ParentSplit>,
    leaf: Entity,
    tabs: &[String],
    active: usize,
    preserved: &mut HashMap<String, Entity>,
) {
    // Floating windows are chromeless single-panel hosts: the OS window's
    // own title bar plays the tab bar's role, so no tabs, no add-panel
    // button, no tab-bar resize filler.
    let tabbar = if floating {
        None
    } else {
        let tabbar = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(28.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(2.0),
                    // Snug: tabs sit flush to the leaf edges (no outer padding).
                    flex_shrink: 0.0,
                    // Hairline separator under the header.
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(rgb(header_bg())),
                BorderColor::all(rgb(divider())),
                // Subtle drop shadow so the header reads as lifted above the content.
                BoxShadow::new(
                    Color::srgba(0.0, 0.0, 0.0, 0.30),
                    Val::Px(0.0),
                    Val::Px(2.0),
                    Val::Px(0.0),
                    Val::Px(4.0),
                ),
                bevy::ui::RelativeCursorPosition::default(),
                TabBarOf(leaf),
                DockPart::TabBar,
                crate::widgets::ThemeShaderSurface {
                    surface: crate::widgets::ThemeSurface::PanelHeader,
                },
                Name::new("tabbar"),
            ))
            .id();

        let mut bar_kids: Vec<Entity> = Vec::new();
        // Whole-leaf drag handle at the far left of the bar: grab it to move
        // the leaf's entire tab set as one unit (see [`LeafGrip`]).
        let leaf_grip_icon =
            icon_text(commands, phosphor, "dots-six-vertical", text_muted(), 12.0);
        let leaf_grip = commands
            .spawn((
                Node {
                    height: Val::Percent(100.0),
                    width: Val::Px(14.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Grab),
                LeafGrip { leaf },
                Name::new("leaf-grip"),
            ))
            .id();
        commands.entity(leaf_grip).add_child(leaf_grip_icon);
        bar_kids.push(leaf_grip);
        // Tabs live inside a horizontally-clipping strip so a leaf with more
        // tabs than fit never pushes the "+"/collapse controls off the bar:
        // the overflowing tabs scroll instead (see [`tab_strip_wheel`]). The
        // strip shows no scrollbar — the wheel is the only affordance, hence
        // "invisible scroll".
        let mut tab_kids: Vec<Entity> = Vec::new();
        for (i, id) in tabs.iter().enumerate() {
            let is_active = i == active;
            let fg = if is_active { text_primary() } else { text_muted() };
            let (title, icon) = tab_meta(id);
            let tab = commands
                .spawn((
                    Node {
                        // Fill the full bar height so the active tab is snug to the
                        // top (content stays vertically centered via align_items).
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.0),
                        padding: UiRect::horizontal(Val::Px(9.0)),
                        position_type: PositionType::Relative,
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(if is_active {
                        rgb(tab_active())
                    } else {
                        Color::NONE
                    }),
                    Interaction::default(),
                    bevy::ui::RelativeCursorPosition::default(),
                    DockPart::Tab,
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                    Name::new(format!("tab:{id}")),
                ))
                .id();
            // Undock handle overlaid on the tab's left padding — hidden (and
            // non-hit-testable) until the tab is hovered ([`tab_grip_hover`]);
            // press to tear the panel off into a floating window
            // ([`tab_grip_interact`]). Absolute so unhovered tabs keep their
            // exact size — no reserved gap.
            let grip_icon = icon_text(commands, phosphor, "dots-six-vertical", text_muted(), 10.0);
            let grip = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(10.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        display: Display::None,
                        ..default()
                    },
                    Interaction::default(),
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Grab),
                    // Block so pressing the handle undocks instead of
                    // selecting/dragging the tab (mirrors the close ×).
                    bevy::ui::FocusPolicy::Block,
                    Name::new("tab-undock-grip"),
                ))
                .id();
            commands.entity(grip).add_child(grip_icon);
            let tab_icon = icon_text(commands, phosphor, icon, fg, 13.0);
            let tab_label = commands
                .spawn((
                    Text::new(title),
                    ui_font(font, 12.0),
                    TextColor(rgb(fg)),
                    bevy::text::TextLayout::no_wrap(),
                ))
                .id();
            let close = icon_text(commands, phosphor, "x", text_muted(), 11.0);
            commands.entity(close).insert((
                TabClose,
                bevy::ui::RelativeCursorPosition::default(),
                // Block so clicking the × closes the tab instead of selecting it.
                bevy::ui::FocusPolicy::Block,
            ));
            let marker = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-2.0),
                        top: Val::Px(0.0),
                        height: Val::Percent(100.0),
                        width: Val::Px(2.0),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(rgb(accent())),
                    bevy::ui::FocusPolicy::Pass,
                    InsertMarker,
                    Name::new("insert-marker"),
                ))
                .id();
            commands.entity(tab).insert(DockTab {
                id: id.clone(),
                leaf,
                label: tab_label,
                icon: tab_icon,
                marker,
            });
            commands.entity(grip).insert(TabGrip {
                tab,
                id: id.clone(),
                leaf,
                area,
            });
            commands
                .entity(tab)
                .add_children(&[grip, tab_icon, tab_label, close, marker]);
            tab_kids.push(tab);
        }
        let tab_strip = commands
            .spawn((
                Node {
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(2.0),
                    // Allowed to shrink below the tabs' natural width and clip
                    // the overflow (Overflow::scroll_x), rather than letting the
                    // tabs spill past the "+" and collapse controls.
                    min_width: Val::Px(0.0),
                    flex_shrink: 1.0,
                    overflow: Overflow::scroll_x(),
                    ..default()
                },
                bevy::ui::ScrollPosition::default(),
                bevy::ui::RelativeCursorPosition::default(),
                TabScrollStrip { leaf },
                Name::new("tab-strip"),
            ))
            .id();
        commands.entity(tab_strip).add_children(&tab_kids);
        bar_kids.push(tab_strip);
        // "+" add-panel button, pinned to the bar *outside* the scroll strip so
        // it stays put and clickable no matter how many tabs the leaf holds.
        let add_icon = icon_text(commands, phosphor, "plus", text_muted(), 13.0);
        let add_btn = commands
            .spawn((
                Node {
                    height: Val::Percent(100.0),
                    width: Val::Px(22.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                AddPanelButton { leaf, area },
                Name::new("dock-add-panel"),
            ))
            .id();
        commands.entity(add_btn).add_child(add_icon);
        bar_kids.push(add_btn);
        let filler = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(100.0),
                    ..default()
                },
                Name::new("tabbar-filler"),
            ))
            .id();
        // Is this leaf a collapsible bottom region? Either the primary dock's
        // whole bottom region (root vertical split, second child — content-
        // agnostic), or a nested bottom strip: the bottom child of any
        // vertical split whose tabs include a bottom-strip marker panel. Both
        // get a collapse chevron at the bar's right end — the click mirror of
        // Ctrl+Space / the divider snap — so the toggle survives the strip
        // being docked under one column instead of full-width.
        let is_root_bottom = !floating
            && parent
                .as_ref()
                .is_some_and(|p| p.path.is_empty() && !p.horizontal && p.is_second);
        let strip = if !floating
            && !is_root_bottom
            && parent.as_ref().is_some_and(|p| !p.horizontal && p.is_second)
        {
            tabs.iter().find(|t| markers.contains(*t)).cloned()
        } else {
            None
        };
        if let Some(p) = parent.filter(|p| p.aligned()) {
            let cursor = crate::cursor_icon::parse_cursor(if p.horizontal {
                "ew-resize"
            } else {
                "ns-resize"
            })
            .unwrap();
            commands.entity(filler).insert((
                Interaction::default(),
                crate::cursor_icon::HoverCursor(cursor),
                Divider {
                    container: p.container,
                    first_wrap: p.first_wrap,
                    horizontal: p.horizontal,
                    path: p.path,
                    area,
                    floating,
                    // The filler doubles as the parent split's resize handle,
                    // so a strip leaf's filler snap-closes like its divider.
                    strip: strip.clone(),
                },
            ));
        }
        bar_kids.push(filler);
        if is_root_bottom || strip.is_some() {
            let chev = icon_text(commands, phosphor, "caret-down", text_muted(), 13.0);
            let btn = commands
                .spawn((
                    Node {
                        height: Val::Percent(100.0),
                        width: Val::Px(24.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Interaction::default(),
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                    BottomCollapseBtn {
                        // Root region collapses content-agnostically; a nested
                        // strip is found again by one of its panels.
                        target: if is_root_bottom { None } else { strip },
                    },
                    Name::new("bottom-collapse"),
                ))
                .id();
            commands.entity(btn).add_child(chev);
            bar_kids.push(btn);
        }
        commands.entity(tabbar).add_children(&bar_kids);
        Some(tabbar)
    };

    // Content region. Reuse the active panel's preserved content entity (kept
    // across this rebuild) so reordering/moving tabs doesn't recreate the panel;
    // otherwise spawn an empty node for the consumer to fill.
    let active_id = tabs.get(active).cloned().unwrap_or_default();
    let content = preserved.remove(&active_id).unwrap_or_else(|| {
        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    // Zero minimum so the active panel's content size never
                    // reserves space in the leaf's column layout.
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_basis: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Name::new("content"),
            ))
            .id()
    });

    let overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            DropOverlay,
            Name::new("drop-overlay"),
        ))
        .id();

    commands.entity(leaf).insert(DockLeaf {
        tabs: tabs.to_vec(),
        content,
        active: active_id,
        area,
        overlay,
    });
    match tabbar {
        Some(tabbar) => {
            commands
                .entity(leaf)
                .add_children(&[tabbar, content, overlay]);
        }
        None => {
            commands.entity(leaf).add_children(&[content, overlay]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The editor's bottom-panel toggle round-trips the bottom region through
    /// detach + attach; the stash must come back bit-identical (tabs, active
    /// tab, ratio) or a closed panel would reopen subtly rearranged.
    #[test]
    fn detach_attach_bottom_round_trips() {
        let mut tree = DockTree::vertical(
            DockTree::tabs(&["viewport", "code"]),
            DockTree::tabs(&["assets", "console"]),
            0.72,
        );
        let (bottom, ratio) = tree.detach_bottom().expect("root vertical split");
        assert!(matches!(&tree, DockTree::Leaf { tabs, .. } if tabs == &["viewport", "code"]));
        assert!(matches!(&bottom, DockTree::Leaf { tabs, .. } if tabs == &["assets", "console"]));
        assert_eq!(ratio, 0.72);

        tree.attach_bottom(bottom, ratio);
        let DockTree::Split {
            direction: SplitDirection::Vertical,
            ratio,
            second,
            ..
        } = &tree
        else {
            panic!("expected root vertical split after attach");
        };
        assert_eq!(*ratio, 0.72);
        assert!(second.contains_panel("assets") && second.contains_panel("console"));
    }

    #[test]
    fn detach_bottom_requires_vertical_root() {
        let mut tree = DockTree::horizontal(DockTree::leaf("a"), DockTree::leaf("b"), 0.5);
        assert!(tree.detach_bottom().is_none());
        assert!(tree.contains_panel("a") && tree.contains_panel("b"));
    }

    /// A bottom strip that isn't full width (sits under one column, with a
    /// full-height panel beside it) detaches, collapses its column, and reopens
    /// under that same column — not full-width at the root.
    #[test]
    fn nested_bottom_detach_reattach_round_trips() {
        let mut tree = DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::tabs(&["assets", "console"]),
                0.7,
            ),
            DockTree::leaf("inspector"),
            0.8,
        );
        let (bottom, ratio, anchor) = tree
            .detach_bottom_containing("assets")
            .expect("nested bottom detaches");
        assert_eq!(ratio, 0.7);
        assert_eq!(anchor, vec!["viewport".to_string()]);
        // Collapsed: only the lone viewport column and the side panel remain.
        assert!(!tree.contains_panel("assets") && !tree.contains_panel("console"));
        assert!(matches!(
            &tree,
            DockTree::Split { direction: SplitDirection::Horizontal, .. }
        ));

        let path = tree.attach_bottom_at(bottom, ratio, &anchor);
        // The split re-appeared under the root's first child — the path the
        // drag-open gesture needs to adopt the right divider.
        assert_eq!(path, vec![false]);
        let DockTree::Split { direction: SplitDirection::Horizontal, first, .. } = &tree else {
            panic!("root should stay horizontal, not become a full-width vertical split");
        };
        let DockTree::Split { direction: SplitDirection::Vertical, second, .. } = &**first else {
            panic!("the viewport column should regain its bottom strip");
        };
        assert!(second.contains_panel("assets") && second.contains_panel("console"));
    }

    /// An empty anchor (a full-width stash, or one whose neighbours all left the
    /// tree) reattaches full-width at the root — the pre-anchor behaviour.
    #[test]
    fn attach_bottom_at_empty_anchor_is_full_width() {
        let mut tree = DockTree::horizontal(DockTree::leaf("a"), DockTree::leaf("b"), 0.5);
        tree.attach_bottom_at(DockTree::leaf("assets"), 0.7, &[]);
        let DockTree::Split { direction: SplitDirection::Vertical, ratio, second, .. } = &tree
        else {
            panic!("empty anchor should attach a full-width root vertical split");
        };
        assert_eq!(*ratio, 0.7);
        assert!(second.contains_panel("assets"));
    }

    /// Taking a leaf must move its whole tab set as one unit and collapse the
    /// split it vacates (no `Empty` husk left for the reconciler).
    #[test]
    fn take_leaf_containing_moves_whole_leaf() {
        let mut tree = DockTree::horizontal(
            DockTree::leaf("viewport"),
            DockTree::tabs(&["assets", "console", "mixer"]),
            0.7,
        );
        let leaf = tree.take_leaf_containing("console").expect("leaf exists");
        assert!(
            matches!(&leaf, DockTree::Leaf { tabs, .. } if tabs == &["assets", "console", "mixer"])
        );
        assert!(matches!(&tree, DockTree::Leaf { tabs, .. } if tabs == &["viewport"]));
        assert!(tree.take_leaf_containing("nonexistent").is_none());
    }

    /// The group-drag insert: splitting a leaf with a whole taken subtree
    /// keeps the subtree's tab set (and active tab) intact.
    #[test]
    fn split_at_with_moves_group_intact() {
        let mut tree = DockTree::horizontal(
            DockTree::leaf("viewport"),
            DockTree::tabs(&["hierarchy", "scenes", "shapes"]),
            0.7,
        );
        let group = tree.take_leaf_containing("scenes").expect("leaf exists");
        assert!(tree.split_at_with("viewport", group, DropZone::Bottom));
        let DockTree::Split { second, .. } = &tree else {
            panic!("expected split at root");
        };
        assert!(
            matches!(&**second, DockTree::Leaf { tabs, .. } if tabs == &["hierarchy", "scenes", "shapes"])
        );
        // Center is not a split; the caller falls back to a tab merge.
        let group2 = DockTree::tabs(&["a", "b"]);
        assert!(!tree.split_at_with("viewport", group2, DropZone::Center));
    }

    /// A group Tab-drop merges every dragged tab into the target leaf at the
    /// insertion point, in order, and activates the first.
    #[test]
    fn add_tabs_before_merges_group() {
        let mut tree = DockTree::tabs(&["viewport", "code"]);
        let dragged = ["hierarchy".to_string(), "scenes".to_string()];
        assert!(tree.add_tabs_before("viewport", &dragged, Some("code")));
        assert!(
            matches!(&tree, DockTree::Leaf { tabs, active_tab: 1, .. }
                if tabs == &["viewport", "hierarchy", "scenes", "code"])
        );
        assert!(!tree.add_tabs_before("nonexistent", &dragged, None));
    }

    /// A group root-edge drop rails the whole subtree against the dock edge;
    /// a Center root drop (no side to take) adopts each panel instead.
    #[test]
    fn split_root_with_rails_group() {
        let mut tree = DockTree::leaf("viewport");
        tree.split_root_with(DockTree::tabs(&["assets", "console"]), DropZone::Bottom);
        let DockTree::Split {
            direction: SplitDirection::Vertical,
            second,
            ..
        } = &tree
        else {
            panic!("expected root vertical split");
        };
        assert!(
            matches!(&**second, DockTree::Leaf { tabs, .. } if tabs == &["assets", "console"])
        );

        let mut center = DockTree::leaf("viewport");
        center.split_root_with(DockTree::tabs(&["a", "b"]), DropZone::Center);
        assert!(center.contains_panel("a") && center.contains_panel("b"));
    }
}
