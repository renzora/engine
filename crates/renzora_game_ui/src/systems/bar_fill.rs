//! Drives [`UiBarFill`]'s entity Node from its `value` field.
//!
//! Single-entity widget — the bar IS the entity. No child lookups, no role
//! markers, no parent walking. Whenever `UiBarFill` changes, the system
//! rewrites the entity's `Node` width/height (per `direction`) in place.

use bevy::prelude::*;

use crate::components::{ProgressDirection, UiBarFill};

/// Apply [`UiBarFill::fraction`] to the entity's `Node`.
///
/// Horizontal fills resize `width`; vertical fills resize `height`. The
/// sizing mode is chosen by `max_px`:
///
/// - `max_px > 0` → write `Val::Px(fraction * max_px)`. Works regardless
///   of parent layout. The non-fill axis is left untouched so authoring
///   can set it freely (e.g. fixed pixel height).
/// - `max_px == 0` → write `Val::Percent(fraction * 100)`. Bar fills its
///   parent along the chosen axis. Non-fill axis goes to `100%` to fill
///   the parent's other dimension.
pub fn apply_bar_fill(mut query: Query<(&UiBarFill, &mut Node), Changed<UiBarFill>>) {
    for (fill, mut node) in &mut query {
        let frac = fill.fraction();
        if fill.max_px > 0.0 {
            // Pixel mode — bar grows from 0 to max_px independent of parent.
            let px = frac * fill.max_px;
            match fill.direction {
                ProgressDirection::LeftToRight | ProgressDirection::RightToLeft => {
                    node.width = Val::Px(px);
                }
                ProgressDirection::BottomToTop | ProgressDirection::TopToBottom => {
                    node.height = Val::Px(px);
                }
            }
        } else {
            // Percent mode — bar fills its parent.
            let pct = frac * 100.0;
            match fill.direction {
                ProgressDirection::LeftToRight | ProgressDirection::RightToLeft => {
                    node.width = Val::Percent(pct);
                    node.height = Val::Percent(100.0);
                }
                ProgressDirection::BottomToTop | ProgressDirection::TopToBottom => {
                    node.height = Val::Percent(pct);
                    node.width = Val::Percent(100.0);
                }
            }
        }
    }
}
