//! Throwing away the rows of open sections that have scrolled off screen.
//!
//! Collapsing a section is the single biggest win available to this panel —
//! measured at ~3.3 ms/frame, 60 fps → 50 fps, for one entity's worth of open
//! components — because a collapsed body *builds nothing* ([`super::section`]
//! explains why hiding is not enough). Scrolling a section out of view makes it
//! exactly as invisible as collapsing it does, but the rows stayed built and kept
//! charging taffy for a full tree walk every frame. This applies the same trick
//! on the second axis.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};

use renzora_ember::widgets::Section;

use super::section::SectionBodySpec;
use super::{InspectorRoot, InspectorSectionHeader};

/// How far outside the viewport a section body is kept built, as a fraction of
/// the viewport's own height.
///
/// Culling exactly at the viewport edge would rebuild a section the instant one
/// pixel of it scrolls into view, so a slow drag would pay a rebuild every frame.
/// Half a screen of slack on each side means normal scrolling crosses the
/// boundary rarely, and a section is built and laid out well before it is
/// readable.
///
/// Relative rather than a fixed pixel count because both things it trades off
/// scale with the panel: a tall inspector is scrolled in bigger jumps, and a
/// fixed slack generous enough for a tall one would exceed a short panel's whole
/// content — culling nothing at all.
const CULL_OVERSCAN_FRAC: f32 = 0.5;

/// Floor for the overscan, in logical px. A very short inspector (a docked strip)
/// would otherwise get a slack of almost nothing and pop rows in and out under
/// small scrolls.
const CULL_OVERSCAN_MIN_PX: f32 = 200.0;

/// Off-screen culling state for one section body, alongside its
/// [`SectionBodySpec`].
#[derive(Component, Default)]
pub(super) struct SectionCull {
    /// The body's height in logical px, measured while it still held its rows.
    ///
    /// Pinned onto the body while culled. Without it an emptied body collapses to
    /// its padding, which drags every section below it up the panel and shrinks
    /// the scroll range under the user's thumb — the content would appear to
    /// dissolve as you scrolled. Recording the height and reserving it keeps the
    /// list's geometry byte-identical to the unculled version.
    placeholder_h: f32,
    /// True while the rows have been thrown away for being off screen.
    pub(super) culled: bool,
}

/// What [`cull_offscreen_sections`] decided to do with one section this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CullAction {
    /// Leave it alone.
    Keep,
    /// Throw the rows away and reserve `placeholder_h` in their place.
    Cull,
    /// Release the reservation; the reconciler rebuilds the rows.
    Restore,
    /// Still on screen and holding its rows — record its height for later.
    Measure(f32),
}

/// The culling decision for one section, split out from the ECS plumbing.
///
/// Worth isolating because the two rules that make this safe are both "don't"
/// rules, and a "don't" is exactly what silently stops happening after a
/// refactor: never cull a section whose height was never measured (there would
/// be nothing to reserve, and the list would collapse), and never measure a body
/// that is not currently holding its rows (it would record its padding as the
/// section's height and reserve *that* forever). Neither is observable from a
/// screenshot until the panel is already wrong.
fn cull_action(
    state: &SectionCull,
    filled: bool,
    top: f32,
    height: f32,
    keep_top: f32,
    keep_bot: f32,
) -> CullAction {
    // Half-open overlap against the keep band: a section touching it at all
    // stays built.
    let visible = top < keep_bot && top + height > keep_top;
    if visible {
        if state.culled {
            CullAction::Restore
        } else if filled && (state.placeholder_h - height).abs() > 0.5 {
            CullAction::Measure(height)
        } else {
            CullAction::Keep
        }
    } else if !state.culled && state.placeholder_h > 0.0 {
        CullAction::Cull
    } else {
        CullAction::Keep
    }
}

/// Throw away the rows of open sections that have scrolled out of the inspector's
/// viewport, and rebuild them when they scroll back.
///
/// Reads the *previous* frame's layout (`ComputedNode` / `UiGlobalTransform`),
/// which is why the overscan exists — a frame of lag at scroll speed is well
/// inside the slack.
///
/// Deliberately not built on [`renzora_ember::virtual_scroll`], which the rest of
/// the editor's lists use. That windows a `keyed_list` by measuring one row stride
/// and assuming every item shares it — exact for the asset grid and the hierarchy,
/// and wrong here: a collapsed section is one header, an open one with a native
/// drawer is hundreds of px, and no single stride describes both. Measuring each
/// section's own height sidesteps the assumption instead of fighting it.
pub(super) fn cull_offscreen_sections(
    root: Query<Entity, With<InspectorRoot>>,
    parents: Query<&ChildOf>,
    viewports: Query<(&ComputedNode, &UiGlobalTransform), With<ScrollPosition>>,
    headers: Query<&Section, With<InspectorSectionHeader>>,
    mut bodies: Query<(
        &mut SectionCull,
        &mut Node,
        &ComputedNode,
        &UiGlobalTransform,
        &SectionBodySpec,
    )>,
) {
    let Ok(root) = root.single() else {
        return;
    };
    // Walk up to the enclosing scroll viewport. `scroll_view` puts the content
    // directly under it, but going through the parent chain keeps this working if
    // the panel ever gains an intermediate wrapper.
    let mut e = root;
    let mut viewport = None;
    for _ in 0..8 {
        let Ok(parent) = parents.get(e) else { break };
        let parent = parent.parent();
        if let Ok(v) = viewports.get(parent) {
            viewport = Some(v);
            break;
        }
        e = parent;
    }
    let Some((vp_node, vp_xf)) = viewport else {
        return;
    };

    let inv = vp_node.inverse_scale_factor();
    let vp_h = vp_node.size().y * inv;
    // A zero-height viewport means the panel is in a collapsed tab or a hidden
    // dock leaf. Culling against it would cull *everything*, and the rows would
    // then all rebuild at once the moment the tab came back — the opposite of
    // what this is for. Leave the panel exactly as it is.
    if vp_h <= 0.0 {
        return;
    }
    // `UiGlobalTransform` already carries the scroll offset, so viewport and
    // section rects are directly comparable without consulting `ScrollPosition`.
    let vp_top = vp_xf.translation.y * inv - vp_h * 0.5;
    let overscan = (vp_h * CULL_OVERSCAN_FRAC).max(CULL_OVERSCAN_MIN_PX);
    let keep_top = vp_top - overscan;
    let keep_bot = vp_top + vp_h + overscan;

    for sec in &headers {
        // A collapsed section is already empty; there is nothing to cull, and its
        // zero-height body would measure as a bogus placeholder.
        if !sec.is_open() {
            continue;
        }
        let Ok((mut cull, mut node, computed, xf, spec)) = bodies.get_mut(sec.body()) else {
            continue;
        };
        let h = computed.size().y * inv;
        let top = xf.translation.y * inv - h * 0.5;

        // `Node` is only written on a real transition. Touching it unconditionally
        // would dirty the very thing this exists to avoid: any `DerefMut` on a
        // `Node` re-runs taffy for that subtree, so an unguarded write here would
        // charge a relayout every frame per section to save relayouts.
        match cull_action(&cull, spec.filled, top, h, keep_top, keep_bot) {
            CullAction::Keep => {}
            CullAction::Measure(h) => cull.placeholder_h = h,
            CullAction::Restore => {
                cull.culled = false;
                node.height = Val::Auto;
            }
            CullAction::Cull => {
                cull.culled = true;
                node.height = Val::Px(cull.placeholder_h);
            }
        }
    }
}

#[cfg(test)]
mod cull_tests {
    use super::{cull_action, CullAction, SectionCull};

    // A 600px viewport with half a screen of overscan each side. Spelled out
    // rather than derived from the constants, so retuning the overscan can't
    // quietly move the band these cases are pinned to.
    const KEEP_TOP: f32 = -300.0;
    const KEEP_BOT: f32 = 900.0;

    fn measured(h: f32) -> SectionCull {
        SectionCull { placeholder_h: h, culled: false }
    }

    #[test]
    fn a_section_far_below_the_viewport_is_culled() {
        let s = measured(150.0);
        assert_eq!(
            cull_action(&s, true, 3000.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Cull
        );
    }

    /// The rule that keeps the list from collapsing. A section built this frame
    /// has no recorded height, so emptying it would reserve nothing and drag
    /// everything below it up the panel.
    #[test]
    fn an_unmeasured_section_is_never_culled() {
        let fresh = SectionCull::default();
        assert_eq!(
            cull_action(&fresh, true, 3000.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }

    #[test]
    fn a_section_inside_the_overscan_band_stays_built() {
        let s = measured(150.0);
        // Entirely below the viewport, but within the overscan slack.
        assert_eq!(
            cull_action(&s, true, 800.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }

    /// A section straddling the bottom edge is half on screen; culling it would
    /// blank rows the user is looking at.
    #[test]
    fn a_partially_visible_section_stays_built() {
        let s = measured(400.0);
        assert_eq!(
            cull_action(&s, true, 500.0, 400.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }

    #[test]
    fn a_culled_section_scrolled_back_into_view_restores() {
        let s = SectionCull { placeholder_h: 150.0, culled: true };
        assert_eq!(
            cull_action(&s, false, 100.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Restore
        );
    }

    /// The other "don't" rule. A culled body measures at its reserved height with
    /// no rows in it; re-measuring an unfilled body would let a stale or padding
    /// height overwrite the real one.
    #[test]
    fn an_unfilled_body_is_never_measured() {
        let s = measured(150.0);
        assert_eq!(
            cull_action(&s, false, 100.0, 6.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
        // ...but the same body holding its rows is measured.
        assert_eq!(
            cull_action(&s, true, 100.0, 6.0, KEEP_TOP, KEEP_BOT),
            CullAction::Measure(6.0)
        );
    }

    /// Sub-pixel drift must not re-record, or every frame writes `SectionCull`
    /// for every section.
    #[test]
    fn an_unchanged_height_is_not_re_measured() {
        let s = measured(150.0);
        assert_eq!(
            cull_action(&s, true, 100.0, 150.2, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }

    /// Culling must be a fixed point: a culled section reserves its own height, so
    /// its rect does not move, so nothing below it moves either. If this ever
    /// returned `Restore` the panel would thrash between built and empty forever.
    #[test]
    fn culling_does_not_oscillate() {
        let mut s = measured(150.0);
        assert_eq!(
            cull_action(&s, true, 3000.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Cull
        );
        s.culled = true;
        // Same geometry next frame — the reserved height matches what the rows
        // occupied, which is the whole point of recording it.
        assert_eq!(
            cull_action(&s, false, 3000.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }
}
