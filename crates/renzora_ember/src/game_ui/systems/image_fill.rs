//! Drives [`UiImageFill`]'s `ImageNode.rect` and `Node` extent from its `value`.
//!
//! The companion to [`super::apply_bar_fill`], for bars drawn from a texture
//! instead of a flat colour. See [`UiImageFill`] for why a textured bar has to
//! crop rather than resize.

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;

use crate::game_ui::components::{ProgressDirection, UiImageFill};

/// The sub-rectangle of a `tex`-sized texture that is visible at `frac`,
/// growing from the edge `direction` names.
///
/// Shared with the markup `fill=` attribute, which crops the same way when the
/// node it is applied to carries an `ImageNode` — the two paths differ in where
/// the fraction comes from, never in what a fraction means.
pub fn crop_rect(direction: ProgressDirection, frac: f32, tex: Vec2) -> Rect {
    // A `Rect` is min/max corners with y down, matching image coordinates.
    match direction {
        ProgressDirection::LeftToRight => Rect::new(0.0, 0.0, tex.x * frac, tex.y),
        ProgressDirection::RightToLeft => Rect::new(tex.x * (1.0 - frac), 0.0, tex.x, tex.y),
        ProgressDirection::TopToBottom => Rect::new(0.0, 0.0, tex.x, tex.y * frac),
        ProgressDirection::BottomToTop => Rect::new(0.0, tex.y * (1.0 - frac), tex.x, tex.y),
    }
}

/// Write one fraction to an image node as a crop plus a matching on-screen
/// extent, skipping writes that would change nothing.
///
/// Both halves have to move together, which is the reason this is one function
/// rather than two: the rect alone is enough only while the node is sized
/// `auto`, and the extent alone just squashes the picture.
pub fn fill_geometry(
    direction: ProgressDirection,
    frac: f32,
    tex: Vec2,
    extent_px: f32,
) -> (Rect, Val) {
    // `extent_px == 0` means draw the source at 1:1, which is what pixel art
    // wants and the only scale that never resamples. The same `frac` applies to
    // the rect and the extent alike, so the crop stays exact at any scale.
    let full_extent = if extent_px > 0.0 {
        extent_px
    } else if is_horizontal(direction) {
        tex.x
    } else {
        tex.y
    };
    (crop_rect(direction, frac, tex), Val::Px(full_extent * frac))
}

/// Does this direction fill along x?
pub fn is_horizontal(direction: ProgressDirection) -> bool {
    matches!(
        direction,
        ProgressDirection::LeftToRight | ProgressDirection::RightToLeft
    )
}

/// Write a computed crop and extent onto components already borrowed from a
/// query, skipping writes that would change nothing.
///
/// The guards matter: both go through `Mut`, so an unguarded assignment marks
/// `ImageNode` and `Node` changed every frame and re-runs UI extraction and
/// layout for a bar that is sitting still.
pub fn write_fill(
    image_node: &mut Mut<ImageNode>,
    node: &mut Mut<Node>,
    direction: ProgressDirection,
    rect: Rect,
    extent: Val,
) {
    if image_node.rect != Some(rect) {
        image_node.rect = Some(rect);
    }
    if is_horizontal(direction) {
        if node.width != extent {
            node.width = extent;
        }
    } else if node.height != extent {
        node.height = extent;
    }
}

/// Apply [`UiImageFill::fraction`] to the entity's `ImageNode` and `Node`.
///
/// # Why this runs every frame rather than on `Changed<UiImageFill>`
///
/// The crop is computed from the source texture's dimensions, which are not
/// known until the image finishes loading — several frames after the entity is
/// spawned, and long after the last write to `UiImageFill`. A change-filtered
/// system would compute nothing on the frames it ran and never run again on the
/// frame the size arrived, leaving the bar full until the player next took
/// damage. Running unfiltered and comparing before writing costs one asset
/// lookup per bar per frame, and a screen holds a handful of bars.
///
/// The comparisons are what keep this cheap for everything downstream: both
/// writes go through `Mut`, so an unguarded assignment would mark `ImageNode`
/// and `Node` changed every frame and re-run UI extraction and layout for a bar
/// that is sitting still.
pub fn apply_image_fill(
    images: Res<Assets<Image>>,
    mut query: Query<(&UiImageFill, &mut ImageNode, &mut Node)>,
) {
    for (fill, mut image_node, mut node) in &mut query {
        // Texture not resident yet — no dimensions to crop against. Retried
        // next frame, which is the whole reason this system is unfiltered.
        let Some(texture) = images.get(&image_node.image) else {
            continue;
        };
        let tex = texture.size().as_vec2();
        if tex.x <= 0.0 || tex.y <= 0.0 {
            continue;
        }

        let (rect, extent) =
            fill_geometry(fill.direction, fill.fraction(), tex, fill.extent_px);
        write_fill(&mut image_node, &mut node, fill.direction, rect, extent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::crop_rect as crop_for;

    #[test]
    fn left_to_right_keeps_the_left_edge_anchored() {
        let r = crop_for(ProgressDirection::LeftToRight, 0.25, Vec2::new(320.0, 40.0));
        assert_eq!(r.min, Vec2::ZERO);
        assert_eq!(r.max, Vec2::new(80.0, 40.0));
    }

    /// A right-to-left drain has to keep its *right* edge pinned and eat into
    /// the left, which is the case an author gets wrong by symmetry.
    #[test]
    fn right_to_left_keeps_the_right_edge_anchored() {
        let r = crop_for(ProgressDirection::RightToLeft, 0.25, Vec2::new(320.0, 40.0));
        assert_eq!(r.min, Vec2::new(240.0, 0.0));
        assert_eq!(r.max, Vec2::new(320.0, 40.0));
    }

    #[test]
    fn bottom_to_top_grows_upward_from_the_bottom_edge() {
        let r = crop_for(ProgressDirection::BottomToTop, 0.25, Vec2::new(40.0, 200.0));
        assert_eq!(r.min, Vec2::new(0.0, 150.0));
        assert_eq!(r.max, Vec2::new(40.0, 200.0));
    }

    /// Full and empty are the two the eye checks first: full must show the
    /// whole texture, empty must show none of it rather than a stray pixel.
    #[test]
    fn full_shows_the_whole_texture_and_empty_shows_none() {
        let tex = Vec2::new(320.0, 40.0);
        assert_eq!(
            crop_for(ProgressDirection::LeftToRight, 1.0, tex).size(),
            tex
        );
        assert_eq!(
            crop_for(ProgressDirection::LeftToRight, 0.0, tex).size(),
            Vec2::new(0.0, 40.0)
        );
    }

    #[test]
    fn fraction_clamps_and_survives_a_zero_max() {
        let f = UiImageFill {
            value: 5.0,
            max: 2.0,
            ..default()
        };
        assert_eq!(f.fraction(), 1.0);
        let f = UiImageFill {
            value: -1.0,
            max: 2.0,
            ..default()
        };
        assert_eq!(f.fraction(), 0.0);
        let f = UiImageFill {
            value: 1.0,
            max: 0.0,
            ..default()
        };
        assert_eq!(f.fraction(), 1.0);
    }
}
