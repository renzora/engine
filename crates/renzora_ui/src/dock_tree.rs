//! Dock tree data structure
//!
//! Binary tree representing the editor panel layout. Each node is a Split (dividing
//! space between two children) or a Leaf (containing string-based tab IDs).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Direction of a split in the dock tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    /// Children are side-by-side (left / right).
    Horizontal,
    /// Children are stacked (top / bottom).
    Vertical,
}

/// A node in the dock tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DockTree {
    Split {
        direction: SplitDirection,
        /// Fraction of space given to the first child (0.0–1.0).
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

    /// Find a mutable reference to the leaf that contains `panel`.
    pub fn find_leaf_mut(&mut self, panel: &str) -> Option<&mut DockTree> {
        match self {
            DockTree::Split { first, second, .. } => first
                .find_leaf_mut(panel)
                .or_else(|| second.find_leaf_mut(panel)),
            DockTree::Leaf { tabs, .. } => {
                if tabs.iter().any(|t| t == panel) {
                    Some(self)
                } else {
                    None
                }
            }
            DockTree::Empty => None,
        }
    }

    /// Remove a panel tab from the tree, cleaning up empty leaves.
    pub fn remove_panel(&mut self, panel: &str) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                if first.remove_panel(panel) || second.remove_panel(panel) {
                    self.cleanup_empty();
                    true
                } else {
                    false
                }
            }
            DockTree::Leaf { tabs, active_tab } => {
                if let Some(idx) = tabs.iter().position(|t| t == panel) {
                    tabs.remove(idx);
                    if *active_tab >= tabs.len() && !tabs.is_empty() {
                        *active_tab = tabs.len() - 1;
                    }
                    if tabs.is_empty() {
                        // Root leaf with no tabs left → collapse to Empty so
                        // the empty-workspace prompt renders.
                        *self = DockTree::Empty;
                    }
                    true
                } else {
                    false
                }
            }
            DockTree::Empty => false,
        }
    }

    /// Set the active tab for the leaf that contains `panel`.
    pub fn set_active_tab(&mut self, panel: &str) {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(panel) {
            if let Some(idx) = tabs.iter().position(|t| t == panel) {
                *active_tab = idx;
            }
        }
    }

    /// Update the split ratio at the given tree path.
    pub fn update_ratio(&mut self, path: &[bool], new_ratio: f32) {
        if path.is_empty() {
            if let DockTree::Split { ratio, .. } = self {
                *ratio = new_ratio.clamp(0.1, 0.9);
            }
            return;
        }
        if let DockTree::Split { first, second, .. } = self {
            if path[0] {
                second.update_ratio(&path[1..], new_ratio);
            } else {
                first.update_ratio(&path[1..], new_ratio);
            }
        }
    }

    /// Does the tree contain a panel with this ID?
    pub fn contains_panel(&self, panel: &str) -> bool {
        match self {
            DockTree::Split { first, second, .. } => {
                first.contains_panel(panel) || second.contains_panel(panel)
            }
            DockTree::Leaf { tabs, .. } => tabs.iter().any(|t| t == panel),
            DockTree::Empty => false,
        }
    }

    /// Is `panel` the currently-selected tab in its leaf (i.e. actually
    /// visible to the user, not just present somewhere in the layout)?
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

    /// Collapse empty leaves and single-child splits.
    fn cleanup_empty(&mut self) {
        match self {
            DockTree::Split { first, second, .. } => {
                first.cleanup_empty();
                second.cleanup_empty();

                let first_empty = matches!(first.as_ref(), DockTree::Empty)
                    || matches!(first.as_ref(), DockTree::Leaf { tabs, .. } if tabs.is_empty());
                let second_empty = matches!(second.as_ref(), DockTree::Empty)
                    || matches!(second.as_ref(), DockTree::Leaf { tabs, .. } if tabs.is_empty());

                if first_empty {
                    *self = std::mem::replace(second.as_mut(), DockTree::Empty);
                } else if second_empty {
                    *self = std::mem::replace(first.as_mut(), DockTree::Empty);
                }
            }
            DockTree::Leaf { tabs, .. } => {
                if tabs.is_empty() {
                    *self = DockTree::Empty;
                }
            }
            DockTree::Empty => {}
        }
    }

    /// Focus an already-open panel, or add it to the first leaf in
    /// tree-traversal order if it isn't open. Returns true if the tree
    /// changed (panel added) or if the active tab was switched.
    pub fn focus_or_add_panel(&mut self, panel: &str) -> bool {
        if self.find_leaf_mut(panel).is_some() {
            self.set_active_tab(panel);
            return true;
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

    /// Add a tab to the leaf containing `sibling`, at the end.
    pub fn add_tab(&mut self, sibling: &str, new_panel: String) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            tabs.push(new_panel);
            *active_tab = tabs.len() - 1;
            return true;
        }
        false
    }

    /// Add a tab at a specific index within its leaf.
    pub fn add_tab_at(&mut self, sibling: &str, new_panel: String, index: usize) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(sibling) {
            let idx = index.min(tabs.len());
            tabs.insert(idx, new_panel);
            *active_tab = idx;
            return true;
        }
        false
    }

    /// Split the leaf containing `target` and place `new_panel` in the given direction.
    pub fn split_at(&mut self, target: &str, new_panel: String, zone: DropZone) -> bool {
        if let Some(leaf) = self.find_leaf_mut(target) {
            let old = std::mem::replace(leaf, DockTree::Empty);
            let new_leaf = DockTree::leaf(new_panel);
            *leaf = match zone {
                DropZone::Left => DockTree::horizontal(new_leaf, old, 0.5),
                DropZone::Right => DockTree::horizontal(old, new_leaf, 0.5),
                DropZone::Top => DockTree::vertical(new_leaf, old, 0.5),
                DropZone::Bottom => DockTree::vertical(old, new_leaf, 0.5),
                // Center and Tab should use add_tab instead
                _ => {
                    *leaf = old;
                    return false;
                }
            };
            return true;
        }
        false
    }

    /// Reorder a tab within its leaf (same leaf, different index).
    pub fn reorder_tab(&mut self, panel: &str, new_index: usize) -> bool {
        if let Some(DockTree::Leaf { tabs, active_tab }) = self.find_leaf_mut(panel) {
            if let Some(old_idx) = tabs.iter().position(|t| t == panel) {
                let panel_id = tabs.remove(old_idx);
                let idx = new_index.min(tabs.len());
                tabs.insert(idx, panel_id);
                *active_tab = idx;
                return true;
            }
        }
        false
    }

    /// Collect all panel IDs in the tree.
    pub fn all_panels(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_panels(&mut out);
        out
    }

    fn collect_panels(&self, out: &mut Vec<String>) {
        match self {
            DockTree::Split { first, second, .. } => {
                first.collect_panels(out);
                second.collect_panels(out);
            }
            DockTree::Leaf { tabs, .. } => {
                out.extend(tabs.iter().cloned());
            }
            DockTree::Empty => {}
        }
    }
}

/// Where a dragged tab will be dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    /// Insert as a tab at the given index.
    Tab(usize),
    Left,
    Right,
    Top,
    Bottom,
    /// Add as tab at end.
    Center,
}

/// Default layout:
/// ```text
/// Hierarchy (15%) | Viewport (70% top)   | Inspector (remaining)
///                 | Assets / Console (30%)|
/// ```
pub fn default_layout() -> DockTree {
    DockTree::horizontal(
        DockTree::leaf("hierarchy"),
        DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("viewport"),
                DockTree::Leaf {
                    tabs: vec!["assets".into(), "console".into()],
                    active_tab: 0,
                },
                0.7,
            ),
            DockTree::leaf("inspector"),
            0.75,
        ),
        0.15,
    )
}

/// Bevy resource holding the current docking layout.
#[derive(Resource)]
pub struct DockingState {
    pub tree: DockTree,
}

/// When set to `Some(slot)`, the editor renders only that viewport slot's panel
/// filling the whole layout (a render-time override — the real [`DockingState`]
/// tree is left untouched, so toggling off restores the exact layout). `None`
/// means no viewport is maximized. Per-slot so the maximize button in each
/// viewport maximizes the view it belongs to.
#[derive(Resource, Default)]
pub struct ViewportMaximized(pub Option<usize>);

impl Default for DockingState {
    fn default() -> Self {
        // Use the Scene layout from LayoutManager so they start in sync.
        Self {
            tree: crate::layouts::scene_layout(),
        }
    }
}

// ── Layout persistence ─────────────────────────────────────────────────────
//
// The user's in-progress dock layout is auto-saved whenever panels are moved
// or added, and restored on next launch. Storage is a single TOML file in
// the user config dir — not per-project, so layouts follow the user across
// projects.

/// Path to the persisted layout file. Returns `None` if the user config
/// directory is unavailable (rare — sandboxed environments).
#[cfg(not(target_arch = "wasm32"))]
pub fn layout_config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("renzora").join("layout.toml"))
}

#[cfg(target_arch = "wasm32")]
pub fn layout_config_path() -> Option<std::path::PathBuf> {
    None
}

/// Load the last-saved workspace (all layouts + active index) from disk.
/// Returns `None` if nothing is saved, the file is corrupt, or we're on
/// a platform without filesystem access.
pub fn load_saved_workspace() -> Option<crate::layouts::LayoutManager> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = layout_config_path()?;
        let content = std::fs::read_to_string(&path).ok()?;
        toml::from_str::<crate::layouts::LayoutManager>(&content).ok()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Persist the workspace (all layouts + active index) to the user config
/// file. Errors are logged but not propagated.
pub fn save_workspace(manager: &crate::layouts::LayoutManager) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = layout_config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!("[dock] couldn't create layout config dir: {e}");
                return;
            }
        }
        match toml::to_string_pretty(manager) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    warn!("[dock] couldn't save workspace: {e}");
                }
            }
            Err(e) => warn!("[dock] couldn't serialise workspace: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = manager;
    }
}

/// Delete the saved workspace file (used by "Reset Layout").
pub fn delete_saved_workspace() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(path) = layout_config_path() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Hierarchy | (Viewport / Console) | Inspector`, the shape most editor
    /// interactions actually operate on.
    fn tree() -> DockTree {
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::horizontal(
                DockTree::vertical(DockTree::leaf("viewport"), DockTree::leaf("console"), 0.7),
                DockTree::leaf("inspector"),
                0.8,
            ),
            0.15,
        )
    }

    fn tabs_of(tree: &DockTree, panel: &str) -> Vec<String> {
        fn find(tree: &DockTree, panel: &str) -> Option<Vec<String>> {
            match tree {
                DockTree::Split { first, second, .. } => {
                    find(first, panel).or_else(|| find(second, panel))
                }
                DockTree::Leaf { tabs, .. } if tabs.iter().any(|t| t == panel) => {
                    Some(tabs.clone())
                }
                _ => None,
            }
        }
        find(tree, panel).unwrap_or_default()
    }

    // ── construction ─────────────────────────────────────────────────────────

    /// A ratio outside 0.1..0.9 leaves a pane too small to grab the resize grip
    /// of — effectively lost, with no way back short of resetting the layout.
    #[test]
    fn split_ratios_are_clamped_to_a_grabbable_range() {
        for bad in [-5.0f32, 0.0, 0.05, 0.95, 1.0, 42.0] {
            for tree in [
                DockTree::horizontal(DockTree::leaf("a"), DockTree::leaf("b"), bad),
                DockTree::vertical(DockTree::leaf("a"), DockTree::leaf("b"), bad),
            ] {
                let DockTree::Split { ratio, .. } = tree else { panic!("not a split") };
                assert!((0.1..=0.9).contains(&ratio), "{bad} became {ratio}");
            }
        }
    }

    #[test]
    fn a_leaf_starts_on_its_only_tab() {
        let DockTree::Leaf { tabs, active_tab } = DockTree::leaf("viewport") else {
            panic!("not a leaf");
        };
        assert_eq!(tabs, vec!["viewport"]);
        assert_eq!(active_tab, 0);
    }

    // ── queries ──────────────────────────────────────────────────────────────

    #[test]
    fn containment_reaches_every_depth() {
        let t = tree();
        for panel in ["hierarchy", "viewport", "console", "inspector"] {
            assert!(t.contains_panel(panel), "{panel} not found");
        }
        assert!(!t.contains_panel("nope"));
        assert!(!DockTree::Empty.contains_panel("anything"));
    }

    /// `contains_panel` and `is_active_tab` answer different questions, and
    /// conflating them is why a background tab can be told to render: a panel can
    /// be present in the layout while another tab in its leaf is the one on
    /// screen.
    #[test]
    fn a_background_tab_is_present_but_not_active() {
        let mut t = tree();
        t.add_tab("viewport", "game".to_string());

        assert!(t.contains_panel("viewport"));
        assert!(t.contains_panel("game"));
        assert!(t.is_active_tab("game"), "the newly added tab should be shown");
        assert!(!t.is_active_tab("viewport"), "viewport is now behind `game`");
    }

    #[test]
    fn all_panels_lists_the_tree_in_traversal_order() {
        assert_eq!(
            tree().all_panels(),
            vec!["hierarchy", "viewport", "console", "inspector"]
        );
        assert!(DockTree::Empty.all_panels().is_empty());
    }

    // ── adding tabs ──────────────────────────────────────────────────────────

    #[test]
    fn adding_a_tab_appends_it_and_brings_it_to_the_front() {
        let mut t = tree();
        assert!(t.add_tab("console", "output".to_string()));
        assert_eq!(tabs_of(&t, "console"), vec!["console", "output"]);
        assert!(t.is_active_tab("output"));
    }

    #[test]
    fn adding_a_tab_at_an_index_inserts_there() {
        let mut t = tree();
        t.add_tab("console", "output".to_string());
        assert!(t.add_tab_at("console", "problems".to_string(), 1));
        assert_eq!(tabs_of(&t, "console"), vec!["console", "problems", "output"]);
        assert!(t.is_active_tab("problems"));
    }

    /// A drop past the end of the strip is what a drag released in empty space
    /// to the right produces — it must clamp rather than panic on insert.
    #[test]
    fn an_out_of_range_insert_index_clamps_to_the_end() {
        let mut t = tree();
        assert!(t.add_tab_at("console", "output".to_string(), 999));
        assert_eq!(tabs_of(&t, "console"), vec!["console", "output"]);
    }

    #[test]
    fn adding_next_to_an_unknown_panel_does_nothing() {
        let mut t = tree();
        assert!(!t.add_tab("nope", "x".to_string()));
        assert!(!t.add_tab_at("nope", "x".to_string(), 0));
        assert_eq!(t.all_panels().len(), 4);
    }

    // ── splitting ────────────────────────────────────────────────────────────

    #[test]
    fn splitting_places_the_new_panel_on_the_dropped_side() {
        for (zone, expect_first) in [
            (DropZone::Left, "new"),
            (DropZone::Right, "inspector"),
            (DropZone::Top, "new"),
            (DropZone::Bottom, "inspector"),
        ] {
            let mut t = DockTree::leaf("inspector");
            assert!(t.split_at("inspector", "new".to_string(), zone));
            let DockTree::Split { first, .. } = &t else { panic!("{zone:?} did not split") };
            assert_eq!(first.all_panels(), vec![expect_first], "{zone:?}");
        }
    }

    #[test]
    fn splitting_left_or_right_is_horizontal_and_top_or_bottom_vertical() {
        for (zone, want) in [
            (DropZone::Left, SplitDirection::Horizontal),
            (DropZone::Right, SplitDirection::Horizontal),
            (DropZone::Top, SplitDirection::Vertical),
            (DropZone::Bottom, SplitDirection::Vertical),
        ] {
            let mut t = DockTree::leaf("a");
            t.split_at("a", "b".to_string(), zone);
            let DockTree::Split { direction, .. } = &t else { panic!() };
            assert_eq!(*direction, want, "{zone:?}");
        }
    }

    /// Center and Tab mean "become a tab here", not "split". If `split_at`
    /// accepted them it would silently produce a split for a drop the user
    /// intended as a tab — and the early return must leave the leaf intact
    /// rather than the `Empty` it was temporarily swapped for.
    #[test]
    fn a_center_or_tab_drop_is_refused_without_damaging_the_leaf() {
        for zone in [DropZone::Center, DropZone::Tab(0)] {
            let mut t = tree();
            assert!(!t.split_at("inspector", "new".to_string(), zone), "{zone:?}");
            assert!(t.contains_panel("inspector"), "{zone:?} destroyed the leaf");
            assert!(!t.contains_panel("new"));
            assert_eq!(t.all_panels().len(), 4, "{zone:?}");
        }
    }

    #[test]
    fn splitting_an_unknown_panel_does_nothing() {
        let mut t = tree();
        assert!(!t.split_at("nope", "new".to_string(), DropZone::Left));
        assert_eq!(t.all_panels().len(), 4);
    }

    // ── removing, and the collapse that follows ──────────────────────────────

    #[test]
    fn removing_a_tab_leaves_its_siblings() {
        let mut t = tree();
        t.add_tab("console", "output".to_string());
        assert!(t.remove_panel("output"));
        assert_eq!(tabs_of(&t, "console"), vec!["console"]);
    }

    /// Removing the tab that was on screen must leave a valid selection. An
    /// `active_tab` past the end is an out-of-bounds index into the strip.
    #[test]
    fn removing_the_active_tab_reselects_within_bounds() {
        let mut t = tree();
        t.add_tab("console", "output".to_string());
        assert!(t.is_active_tab("output"));

        t.remove_panel("output");

        let mut probe = t.clone();
        let leaf = probe.find_leaf_mut("console").expect("console leaf missing").clone();
        let DockTree::Leaf { tabs, active_tab } = leaf else {
            panic!("expected a leaf")
        };
        assert!(active_tab < tabs.len(), "active_tab {active_tab} is out of bounds");
        assert!(t.is_active_tab("console"));
    }

    /// Emptying a leaf must collapse its parent split, promoting the sibling.
    /// Leaving the split in place is what produces a dead region the layout
    /// still reserves space for.
    #[test]
    fn emptying_a_leaf_collapses_its_parent_split() {
        let mut t = tree();
        assert!(t.remove_panel("console"));

        assert!(!t.contains_panel("console"));
        assert_eq!(t.all_panels(), vec!["hierarchy", "viewport", "inspector"]);

        // The viewport/console vertical split should be gone, leaving the
        // viewport leaf directly in its place.
        let DockTree::Split { second, .. } = &t else { panic!() };
        let DockTree::Split { first, .. } = second.as_ref() else { panic!() };
        assert!(
            matches!(first.as_ref(), DockTree::Leaf { .. }),
            "the split should have collapsed to a leaf"
        );
    }

    /// Closing the last panel has to leave `Empty`, which is what renders the
    /// empty-workspace prompt. A zero-tab `Leaf` would render a blank tab strip
    /// instead, with no way to open anything.
    #[test]
    fn removing_the_last_panel_collapses_the_tree_to_empty() {
        let mut t = DockTree::leaf("only");
        assert!(t.remove_panel("only"));
        assert!(matches!(t, DockTree::Empty));
    }

    #[test]
    fn cascading_removal_collapses_all_the_way_down() {
        let mut t = tree();
        for panel in ["hierarchy", "viewport", "console", "inspector"] {
            t.remove_panel(panel);
        }
        assert!(matches!(t, DockTree::Empty), "got {t:?}");
        assert!(t.all_panels().is_empty());
    }

    #[test]
    fn removing_an_unknown_panel_reports_no_change() {
        let mut t = tree();
        assert!(!t.remove_panel("nope"));
        assert_eq!(t.all_panels().len(), 4);
    }

    // ── focus / reorder / ratios ─────────────────────────────────────────────

    #[test]
    fn focusing_an_open_panel_brings_it_forward_without_duplicating_it() {
        let mut t = tree();
        t.add_tab("console", "output".to_string());
        assert!(t.is_active_tab("output"));

        assert!(t.focus_or_add_panel("console"));

        assert!(t.is_active_tab("console"));
        assert_eq!(tabs_of(&t, "console"), vec!["console", "output"], "duplicated");
    }

    #[test]
    fn focusing_a_closed_panel_adds_it() {
        let mut t = tree();
        assert!(t.focus_or_add_panel("profiler"));
        assert!(t.contains_panel("profiler"));
    }

    #[test]
    fn reordering_moves_a_tab_and_keeps_it_selected() {
        let mut t = DockTree::Leaf {
            tabs: vec!["a".into(), "b".into(), "c".into()],
            active_tab: 0,
        };
        assert!(t.reorder_tab("a", 2));
        assert_eq!(tabs_of(&t, "a"), vec!["b", "c", "a"]);
        assert!(t.is_active_tab("a"), "a dragged tab should stay selected");
    }

    #[test]
    fn reordering_past_the_end_clamps() {
        let mut t = DockTree::Leaf {
            tabs: vec!["a".into(), "b".into()],
            active_tab: 0,
        };
        assert!(t.reorder_tab("a", 99));
        assert_eq!(tabs_of(&t, "a"), vec!["b", "a"]);
    }

    #[test]
    fn a_ratio_can_be_updated_by_path_and_is_clamped() {
        let mut t = tree();
        t.update_ratio(&[], 0.42);
        let DockTree::Split { ratio, .. } = &t else { panic!() };
        assert!((ratio - 0.42).abs() < 1e-6);

        t.update_ratio(&[], 5.0);
        let DockTree::Split { ratio, .. } = &t else { panic!() };
        assert!((0.1..=0.9).contains(ratio), "{ratio}");
    }

    #[test]
    fn a_ratio_path_selects_the_right_child() {
        let mut t = tree();
        t.update_ratio(&[true], 0.3); // the second child's split
        let DockTree::Split { second, .. } = &t else { panic!() };
        let DockTree::Split { ratio, .. } = second.as_ref() else { panic!() };
        assert!((ratio - 0.3).abs() < 1e-6);
    }

    /// A path that runs off the end of the tree happens whenever a drag is still
    /// in flight while the layout changes underneath it.
    #[test]
    fn a_ratio_path_into_nothing_is_ignored() {
        let mut t = DockTree::leaf("a");
        t.update_ratio(&[true, false, true], 0.5);
        assert_eq!(t.all_panels(), vec!["a"]);
    }

    // ── the shipped layouts ──────────────────────────────────────────────────

    /// A duplicate panel id in a shipped layout means the same panel is docked
    /// twice, and the second copy renders over nothing.
    #[test]
    fn the_default_layout_has_no_duplicate_panels() {
        let panels = default_layout().all_panels();
        let unique: std::collections::HashSet<&String> = panels.iter().collect();
        assert_eq!(panels.len(), unique.len(), "duplicates in {panels:?}");
        assert!(!panels.is_empty());
    }

    #[test]
    fn the_default_layout_docks_the_core_panels() {
        let panels = default_layout().all_panels();
        for expected in ["viewport", "hierarchy", "inspector"] {
            assert!(
                panels.iter().any(|p| p == expected),
                "{expected} missing from {panels:?}"
            );
        }
    }
}
