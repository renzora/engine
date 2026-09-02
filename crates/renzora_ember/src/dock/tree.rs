//! The dock's data model: a binary tree of `Split`s and `Leaf`s, and every
//! operation the interaction systems perform on it.
//!
//! Deliberately free of bevy_ui — nothing here spawns, queries or reads an
//! entity. That is what makes the layout serialisable (the editor shell
//! persists each workspace's tree to disk) and what lets the tree operations be
//! reasoned about, and tested, without a `World`.

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

    /// Drop every tab named in `ids` from the tree, collapsing any leaf left
    /// empty so the split around it closes up rather than leaving a blank pane.
    ///
    /// For panels that no longer exist. A saved layout outlives the build that
    /// wrote it, and a retired panel id would otherwise sit in it forever as a
    /// tab that opens a placeholder — the dock is happy to render an id it has
    /// no builder for, which is what makes the ghost survivable *and* permanent.
    pub fn retire_panels(&mut self, ids: &[&str]) {
        match self {
            DockTree::Split { first, second, .. } => {
                first.retire_panels(ids);
                second.retire_panels(ids);
                // A split with nothing on one side is just its other side.
                match (first.is_empty(), second.is_empty()) {
                    (true, true) => *self = DockTree::Empty,
                    (true, false) => *self = (**second).clone(),
                    (false, true) => *self = (**first).clone(),
                    (false, false) => {}
                }
            }
            DockTree::Leaf { tabs, active_tab } => {
                // Follow the active tab across the removal rather than resetting
                // to 0: dropping a background tab shouldn't switch the panel the
                // user was last looking at.
                let active_id = tabs.get(*active_tab).cloned();
                tabs.retain(|t| !ids.contains(&t.as_str()));
                *active_tab = active_id
                    .and_then(|id| tabs.iter().position(|t| *t == id))
                    .unwrap_or(0)
                    .min(tabs.len().saturating_sub(1));
                if tabs.is_empty() {
                    *self = DockTree::Empty;
                }
            }
            DockTree::Empty => {}
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

    /// No panels anywhere — either the `Empty` variant or a leaf with no tabs.
    /// Consumers use this to decide whether a dock area is worth showing at
    /// all; an empty one renders as a bare bordered slab.
    pub fn is_empty(&self) -> bool {
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
    /// this to [`crate::dock::GrabRootDivider`] so the live drag adopts the
    /// right divider.
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
pub(crate) const ROOT_DOCK_RATIO: f32 = 0.2;

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
