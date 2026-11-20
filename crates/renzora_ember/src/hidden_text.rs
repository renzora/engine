//! Keeps text inside a hidden (`Display::None`) subtree from being laid out
//! again every frame when the UI is scaled.
//!
//! Bevy's `measure_text_system` measures a text node again whenever its
//! window's scale factor differs from the node's
//! `ComputedNode::inverse_scale_factor`. Layout never reaches a node under a
//! `Display::None` ancestor, so it resets that node's `ComputedNode` to the
//! default, whose scale is 1.0. At a UI scale of 100% the default happens to
//! match and nothing shows; at any other scale every hidden menu item, dropdown
//! option and submenu row fails the check on every frame.
//!
//! Ember hides all of its popups that way, so at 125% several hundred text
//! nodes were measured and laid out each frame (about 15 ms of a frame), and
//! each pass built new Parley layouts. Parley drops a system font it has not
//! seen in its last 128 layouts and reloads it under a new id; the glyph atlases
//! are keyed by that id and never freed, so the editor made a new 512x512 atlas
//! roughly every frame until the GPU ran out of memory.
//!
//! The fix is to give a hidden text node its window's scale back before
//! measurement. It is limited to nodes with a zero size, which is what an
//! unreached node is reset to, so a visible node keeps whatever layout wrote.
//! A visible node mid scale change does not need this check anyway: the
//! changed render target already makes Bevy measure it again. The write
//! bypasses change detection so it does not itself make `text_system` lay the
//! node out; layout's next reset still does, but only on the frames it walks
//! the whole tree, and that reuses the existing measure rather than building
//! a new one.

use bevy::prelude::*;
use bevy::ui::{ComputedUiRenderTargetInfo, UiSystems};

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        match_hidden_text_scale
            .after(UiSystems::Propagate)
            .before(UiSystems::Content),
    );
}

/// Set a hidden text node's scale to its window's, so Bevy does not measure it
/// again every frame. See the module docs.
fn match_hidden_text_scale(mut text: Query<(&mut ComputedNode, &ComputedUiRenderTargetInfo), With<Text>>) {
    for (mut node, target) in &mut text {
        let scale = target.scale_factor();
        if scale <= 0.0 || node.size != Vec2::ZERO {
            continue;
        }
        if (scale - node.inverse_scale_factor.recip()).abs() > 1e-5 {
            node.bypass_change_detection().inverse_scale_factor = scale.recip();
        }
    }
}
