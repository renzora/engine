//! Scrolling a component card into view when you open it.
//!
//! Opening a card near the bottom of the panel used to grow its rows below the
//! fold, so the click appeared to do nothing until you scrolled down to find
//! them. Now the panel eases down just far enough to show the whole card, and
//! never so far that its header leaves the top: a card taller than the panel
//! lands with its header at the top instead.
//!
//! Only a single, deliberate open does this. Expand-all, the expand policy
//! applied on a selection change, and a culled body refilling as it scrolls
//! back into view all open or fill sections without anyone asking to look at
//! one, and scrolling then would yank the panel away from where you were.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};

use renzora_ember::widgets::{EmberScroll, Section};

use super::section::SectionBodySpec;
use super::{InspectorRoot, InspectorSectionHeader};

/// The card waiting to be scrolled into view, once its rows have laid out.
#[derive(Default)]
pub(super) struct PendingReveal {
    /// The card's root (the header's parent) and its body.
    card: Option<(Entity, Entity)>,
    /// Frames spent waiting, and the body height seen last frame. The body is
    /// filled the frame it opens, but it lays out after that, and a native
    /// drawer can grow over a frame or two more, so the reveal waits for the
    /// height to hold still before measuring.
    frames: u32,
    last_h: f32,
}

/// Give up on a reveal after this many frames, whatever the height is doing.
const MAX_WAIT_FRAMES: u32 = 8;

/// Note a card the user has just opened. Runs before
/// `reconcile_section_bodies` in the same chain, so a section that opened
/// this frame still has its unfilled body: that is how a real open is told
/// apart from any other change to a `Section`.
pub(super) fn note_opened_section(
    headers: Query<(Ref<Section>, &ChildOf), (With<InspectorSectionHeader>, Changed<Section>)>,
    bodies: Query<&SectionBodySpec>,
) -> Option<(Entity, Entity)> {
    let mut opened = None;
    let mut count = 0;
    for (sec, parent) in &headers {
        // A header built this frame (a rebuild on selection change) carries
        // its open state from the start; nobody opened it.
        if sec.is_added() || !sec.is_open() {
            continue;
        }
        if bodies.get(sec.body()).is_ok_and(|s| !s.filled) {
            count += 1;
            opened = Some((parent.parent(), sec.body()));
        }
    }
    // Several at once is expand-all or the expand policy, not a click.
    if count == 1 {
        opened
    } else {
        None
    }
}

/// Ease the inspector's scroll view to show the card noted by
/// [`note_opened_section`], once it has laid out.
pub(super) fn reveal_opened_section(
    opened: In<Option<(Entity, Entity)>>,
    mut pending: Local<PendingReveal>,
    root: Query<Entity, With<InspectorRoot>>,
    parents: Query<&ChildOf>,
    nodes: Query<(&ComputedNode, &UiGlobalTransform)>,
    mut viewports: Query<(&mut EmberScroll, &ScrollPosition, &ComputedNode, &UiGlobalTransform)>,
) {
    if let Some(card) = opened.0 {
        *pending = PendingReveal { card: Some(card), frames: 0, last_h: -1.0 };
        // Its rows are queued this frame; there is nothing to measure yet.
        return;
    }
    let Some((card, body)) = pending.card else {
        return;
    };
    pending.frames += 1;
    let (Ok((body_node, _)), Ok((card_node, card_xf))) = (nodes.get(body), nodes.get(card)) else {
        pending.card = None;
        return;
    };
    let inv = card_node.inverse_scale_factor();
    let body_h = body_node.size().y * inv;
    let settled = body_h > 0.0 && (body_h - pending.last_h).abs() < 0.5;
    pending.last_h = body_h;
    if !settled && pending.frames < MAX_WAIT_FRAMES {
        return;
    }
    pending.card = None;

    let Ok(root) = root.single() else {
        return;
    };
    let Ok(vp) = parents.get(root).map(|c| c.parent()) else {
        return;
    };
    let Ok((mut scroll, pos, vp_node, vp_xf)) = viewports.get_mut(vp) else {
        return;
    };
    let vp_h = vp_node.size().y * inv;
    if vp_h <= 0.0 {
        return;
    }
    // Both rects already carry the scroll offset, so they compare directly.
    let vp_top = vp_xf.translation.y * inv - vp_h * 0.5;
    let card_h = card_node.size().y * inv;
    let card_top = card_xf.translation.y * inv - card_h * 0.5;
    let delta = reveal_delta(card_top, card_h, vp_top, vp_h);
    if delta.abs() > 0.5 {
        scroll.scroll_to(pos.y + delta);
    }
}

/// How far to scroll (positive is down) to show a card at `card_top` of
/// height `card_h` in a viewport at `vp_top` of height `vp_h`: zero when it
/// already fits, enough to bring its bottom into view otherwise, but never so
/// much that its top goes above the viewport's.
fn reveal_delta(card_top: f32, card_h: f32, vp_top: f32, vp_h: f32) -> f32 {
    let (card_bot, vp_bot) = (card_top + card_h, vp_top + vp_h);
    if card_top < vp_top {
        card_top - vp_top
    } else if card_bot > vp_bot {
        (card_bot - vp_bot).min(card_top - vp_top)
    } else {
        0.0
    }
}

#[cfg(test)]
mod reveal_tests {
    use super::reveal_delta;

    #[test]
    fn a_card_that_fits_does_not_scroll() {
        assert_eq!(reveal_delta(100.0, 200.0, 0.0, 600.0), 0.0);
    }

    #[test]
    fn a_card_below_the_fold_scrolls_just_enough() {
        // Bottom at 700 in a 600px view: 100px down shows it all.
        assert_eq!(reveal_delta(500.0, 200.0, 0.0, 600.0), 100.0);
    }

    #[test]
    fn a_card_taller_than_the_view_lands_header_first() {
        // Showing the bottom would need 600px; the header allows only 400.
        assert_eq!(reveal_delta(400.0, 800.0, 0.0, 600.0), 400.0);
    }

    #[test]
    fn a_card_whose_header_is_cut_off_scrolls_up_to_it() {
        assert_eq!(reveal_delta(-30.0, 200.0, 0.0, 600.0), -30.0);
    }
}
