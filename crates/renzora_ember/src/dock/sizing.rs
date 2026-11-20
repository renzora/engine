//! Panels keep their size when the window does not.
//!
//! A split stores a ratio, and drawing it as a percentage scaled every panel
//! with the window: shrink the editor and the Inspector went with it, from a
//! usable 410 px to an unreadable 190, though the viewport beside it had
//! plenty to give. Panels like the Inspector need a width in pixels, not a
//! share of the screen, so this holds one.
//!
//! ## Which side holds
//!
//! The **smaller** side of each split keeps its size in logical pixels and the
//! larger side takes the change. That is content-agnostic on purpose: the dock
//! knows no panel ids, and in every workspace the pane worth growing (the
//! viewport, a graph, the code editor) is the big one. A side panel the user
//! has made larger than the centre becomes the flexible one, which is what
//! "larger" means to them anyway.
//!
//! The held side only gives up space once the two are equal: past that, both
//! shrink together, so a very small window still shows everything, and the
//! larger side never becomes the smaller one (the rule stays stable).
//!
//! ## Remembering the size
//!
//! Each split remembers the size it was asked to hold (`want`). Shrinking the
//! window below it and growing it back returns the panel to that size rather
//! than leaving it squashed. A divider drag, a rebuilt layout or a workspace
//! switch all show up as the tree's ratio no longer being the one this module
//! last wrote, and the split is simply seeded again from what is on screen.
//! That one check is what makes the memory safe to key by tree path: a path
//! that now names a different split will not match.
//!
//! The tree's ratio is rewritten as well as the node, so a layout saved after
//! a resize reopens as it looks. The shell only writes the layout once the
//! mouse is released, so a live window resize costs no disk churn.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::dock::components::Divider;
use crate::dock::routing::{area_tree, area_tree_mut};
use crate::dock::tree::SplitDirection;
use crate::dock::{Dock, DockWindows, FixedDock};

/// What one split is holding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Held {
    horizontal: bool,
    /// The split's size along its axis, in logical px, when last looked at.
    extent: f32,
    /// The ratio this module last wrote (or found, when seeding).
    ratio: f32,
    /// Whether the first side is the held one.
    keep_first: bool,
    /// The held side's size in logical px: what it returns to when there is
    /// room again.
    want: f32,
}

impl Held {
    fn seed(horizontal: bool, ratio: f32, extent: f32) -> Self {
        let keep_first = ratio < 0.5;
        let share = if keep_first { ratio } else { 1.0 - ratio };
        Self {
            horizontal,
            extent,
            ratio,
            keep_first,
            want: share * extent,
        }
    }

    /// Whether this still describes the split at its path: same axis, and the
    /// tree holds the ratio last written. Anything else means the user or a
    /// rebuild changed it, and the split is seeded again.
    fn matches(&self, horizontal: bool, ratio: f32) -> bool {
        self.horizontal == horizontal && (self.ratio - ratio).abs() < 1e-4
    }

    /// The ratio for a split that is now `extent` long, or `None` when it has
    /// not changed size or the ratio comes out the same.
    fn resize(&mut self, extent: f32) -> Option<f32> {
        if (extent - self.extent).abs() < 0.5 {
            return None;
        }
        self.extent = extent;
        // At most half, so the held side never outgrows the flexible one.
        let share = (self.want / extent).min(0.5);
        let ratio = if self.keep_first { share } else { 1.0 - share }.clamp(0.1, 0.9);
        if (ratio - self.ratio).abs() < 1e-4 {
            return None;
        }
        self.ratio = ratio;
        Some(ratio)
    }
}

/// Every split being held, by dock area and tree path.
#[derive(Resource, Default)]
pub(crate) struct HeldSizes(HashMap<(Entity, Vec<bool>), Held>);

/// Hold the smaller side of every split at its size as the dock resizes. See
/// the module docs.
///
/// Runs after the rebuild, reading last frame's layout: a just-rebuilt split
/// has no size yet and is skipped until it does.
pub(crate) fn hold_panel_sizes(
    dividers: Query<&Divider>,
    computed: Query<&ComputedNode>,
    mut nodes: Query<&mut Node>,
    mut dock: ResMut<Dock>,
    mut fixed: ResMut<FixedDock>,
    mut wins: ResMut<DockWindows>,
    mut held: ResMut<HeldSizes>,
) {
    let mut seen = Vec::new();
    for div in &dividers {
        let Ok(cn) = computed.get(div.container) else { continue };
        let size = cn.size() * cn.inverse_scale_factor;
        let extent = if div.horizontal { size.x } else { size.y };
        if extent < 1.0 {
            continue;
        }
        // Read through the plain refs: `ResMut` would mark the tree changed
        // every frame even when nothing is written.
        let Some((direction, ratio)) =
            area_tree(div.area, &dock, &fixed, &wins).split_ratio_at(&div.path)
        else {
            continue;
        };
        let horizontal = matches!(direction, SplitDirection::Horizontal);
        let key = (div.area, div.path.clone());
        seen.push(key.clone());

        let entry = held.0.entry(key).or_insert_with(|| Held::seed(horizontal, ratio, extent));
        if !entry.matches(horizontal, ratio) {
            *entry = Held::seed(horizontal, ratio, extent);
            continue;
        }
        let Some(new_ratio) = entry.resize(extent) else { continue };

        if let Ok(mut n) = nodes.get_mut(div.first_wrap) {
            if div.horizontal {
                n.width = Val::Percent(new_ratio * 100.0);
            } else {
                n.height = Val::Percent(new_ratio * 100.0);
            }
        }
        area_tree_mut(div.area, &mut dock, &mut fixed, &mut wins).update_ratio(&div.path, new_ratio);
    }
    // Splits that are gone (a closed window, a merged leaf) take their memory
    // with them, so a later split at the same path starts fresh.
    if held.0.len() != seen.len() {
        held.0.retain(|k, _| seen.contains(k));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The case from the report: a 22% Inspector beside the viewport. It keeps
    /// its 405 px as the window narrows, instead of shrinking to 22% of it.
    #[test]
    fn the_smaller_side_keeps_its_pixels() {
        let mut h = Held::seed(true, 0.78, 1840.0);
        let r = h.resize(1340.0).unwrap();
        let inspector = (1.0 - r) * 1340.0;
        assert!((inspector - 0.22 * 1840.0).abs() < 0.5, "inspector {inspector}");
    }

    /// Past the point where the two sides are equal they shrink together, so
    /// the viewport is never squeezed out by the panel beside it.
    #[test]
    fn the_held_side_never_outgrows_the_other() {
        let mut h = Held::seed(true, 0.78, 1840.0);
        assert_eq!(h.resize(600.0), Some(0.5));
    }

    /// Shrinking past the panel's size and growing back returns it to the size
    /// it had, rather than leaving it at the squeezed one.
    #[test]
    fn growing_back_restores_the_size_it_was_given() {
        let mut h = Held::seed(true, 0.78, 1840.0);
        h.resize(600.0);
        let r = h.resize(1840.0).unwrap();
        assert!((r - 0.78).abs() < 1e-4, "ratio {r}");
    }

    /// The first side can be the held one too (a panel docked on the left).
    #[test]
    fn a_left_panel_is_held_the_same_way() {
        let mut h = Held::seed(true, 0.2, 1000.0);
        let r = h.resize(500.0).unwrap();
        assert!((r * 500.0 - 200.0).abs() < 0.5);
    }

    /// A size change under half a pixel is not a resize: no write, no churn.
    #[test]
    fn a_subpixel_change_writes_nothing() {
        let mut h = Held::seed(true, 0.78, 1840.0);
        assert_eq!(h.resize(1840.3), None);
    }

    /// A divider drag (or a rebuilt tree) leaves a ratio this module did not
    /// write, and the split must then be re-measured rather than snapped back
    /// to the remembered size.
    #[test]
    fn a_ratio_written_by_someone_else_is_not_ours() {
        let h = Held::seed(true, 0.78, 1840.0);
        assert!(h.matches(true, 0.78));
        assert!(!h.matches(true, 0.6));
        assert!(!h.matches(false, 0.78));
    }
}
