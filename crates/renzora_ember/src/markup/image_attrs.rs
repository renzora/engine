//! Parsing for the `<image>` attributes that control *how* a texture is drawn,
//! as opposed to which one.
//!
//! Markup used to insert every image as `NodeImageMode::Auto` with the asset
//! server's default (linear) sampler, which is the right default for a photo
//! and the wrong one for the two things game UI is mostly made of:
//!
//! - **A panel, frame or slot** drawn from a small texture with a decorated
//!   border. Stretching it to a useful size smears the border along with the
//!   middle. Nine-slicing holds the corners at their authored size and stretches
//!   or tiles only the middle, which is what the art was drawn for.
//! - **Pixel art**, which a linear sampler turns to mush the moment it is drawn
//!   at anything but 1:1.
//!
//! Both are opt-in per node rather than inferred. The editor's own interface is
//! built from this same markup, so a project-wide "this game is pixel art"
//! switch would resample the editor's icons too.

use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::math::Vec2;
use bevy::sprite::{BorderRect, SliceScaleMode, TextureSlicer};
use bevy::ui::widget::NodeImageMode;

/// Parse an `image_mode="..."` value.
///
/// | Form | Meaning |
/// |---|---|
/// | `auto` | Draw at the texture's own size (the default) |
/// | `stretch` | Stretch to the node's box |
/// | `sliced(8)` | Nine-slice, 8px border on all four sides |
/// | `sliced(8, 12)` | Nine-slice, 8px left/right, 12px top/bottom |
/// | `sliced(l, r, t, b)` | Nine-slice with each side given |
/// | `tiled(1.0)` | Tile both axes, repeating above the given stretch ratio |
/// | `tiled(x, y)` | Tile with separate horizontal and vertical ratios |
///
/// A `sliced(...)` border is measured in **source texture pixels**, not screen
/// pixels, so it does not change when the node is resized — that is the whole
/// point of the mode.
///
/// Returns `None` for anything unrecognised so the caller can warn and fall
/// back rather than silently drawing the wrong thing.
pub fn parse_image_mode(value: &str) -> Option<NodeImageMode> {
    let v = value.trim();
    match v {
        "auto" => return Some(NodeImageMode::Auto),
        "stretch" => return Some(NodeImageMode::Stretch),
        _ => {}
    }

    let (name, args) = split_call(v)?;
    match name {
        "sliced" | "slice" => {
            // `BorderRect` is two corner insets, not four scalars: `min_inset`
            // is (left, top) and `max_inset` is (right, bottom).
            let border = match args.as_slice() {
                [all] => BorderRect::all(*all),
                [x, y] => BorderRect {
                    min_inset: Vec2::new(*x, *y),
                    max_inset: Vec2::new(*x, *y),
                },
                [l, r, t, b] => BorderRect {
                    min_inset: Vec2::new(*l, *t),
                    max_inset: Vec2::new(*r, *b),
                },
                _ => return None,
            };
            Some(NodeImageMode::Sliced(TextureSlicer {
                border,
                // Stretching the middle is the conventional nine-slice and what
                // a gradient-filled panel wants; a caller needing a tiled middle
                // is served by `tiled(..)`.
                center_scale_mode: SliceScaleMode::Stretch,
                sides_scale_mode: SliceScaleMode::Stretch,
                // Corners at their authored size. Letting them grow is what
                // makes a nine-sliced pixel-art frame look wrong.
                max_corner_scale: 1.0,
            }))
        }
        "tiled" | "tile" => {
            let (x, y) = match args.as_slice() {
                [both] => (*both, *both),
                [x, y] => (*x, *y),
                _ => return None,
            };
            Some(NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: x.max(y).max(f32::EPSILON),
            })
        }
        _ => None,
    }
}

/// Split `name(a, b, c)` into its name and its numeric arguments.
fn split_call(v: &str) -> Option<(&str, Vec<f32>)> {
    let open = v.find('(')?;
    let close = v.rfind(')')?;
    if close < open {
        return None;
    }
    let name = v[..open].trim();
    let body = &v[open + 1..close];
    let args: Option<Vec<f32>> = body
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<f32>().ok())
        .collect();
    Some((name, args?))
}

/// Is this attribute value truthy? Used for flags like `pixelated`.
///
/// The markup grammar requires every attribute to be `key="value"`, so a flag
/// is written `pixelated="true"` — a bare `pixelated` fails to parse the whole
/// tag. An empty `pixelated=""` counts as true, since writing the flag at all
/// is the intent.
pub fn attr_is_true(value: &str) -> bool {
    matches!(value.trim(), "" | "true" | "1" | "yes" | "on")
}

/// The sampler a `pixelated` image loads with: nearest-neighbour in both
/// directions, so a texel stays a hard square at any scale.
///
/// Mipmaps are left linear because a UI image is drawn at or above 1:1 in
/// practice, and the nearest mip filter is the one that makes a minified image
/// crawl as the scale factor changes.
pub fn pixelated_sampler() -> ImageSampler {
    let mut d = ImageSamplerDescriptor::nearest();
    d.mipmap_filter = bevy::image::ImageFilterMode::Linear;
    ImageSampler::Descriptor(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_modes_parse() {
        assert!(matches!(parse_image_mode("auto"), Some(NodeImageMode::Auto)));
        assert!(matches!(
            parse_image_mode("  stretch "),
            Some(NodeImageMode::Stretch)
        ));
    }

    #[test]
    fn one_argument_slices_all_four_sides_equally() {
        let Some(NodeImageMode::Sliced(s)) = parse_image_mode("sliced(8)") else {
            panic!("expected a sliced mode");
        };
        assert_eq!(s.border.min_inset, Vec2::splat(8.0));
        assert_eq!(s.border.max_inset, Vec2::splat(8.0));
    }

    /// The two-argument form is horizontal-then-vertical, matching the
    /// `padding="10px 20px"` convention the rest of the markup already uses.
    #[test]
    fn two_arguments_are_horizontal_then_vertical() {
        let Some(NodeImageMode::Sliced(s)) = parse_image_mode("sliced(8, 12)") else {
            panic!("expected a sliced mode");
        };
        // x is left/right, y is top/bottom.
        assert_eq!(s.border.min_inset, Vec2::new(8.0, 12.0));
        assert_eq!(s.border.max_inset, Vec2::new(8.0, 12.0));
    }

    #[test]
    fn four_arguments_are_left_right_top_bottom() {
        let Some(NodeImageMode::Sliced(s)) = parse_image_mode("sliced(1, 2, 3, 4)") else {
            panic!("expected a sliced mode");
        };
        // left=1, right=2, top=3, bottom=4 → min_inset=(l,t), max_inset=(r,b).
        assert_eq!(s.border.min_inset, Vec2::new(1.0, 3.0));
        assert_eq!(s.border.max_inset, Vec2::new(2.0, 4.0));
    }

    /// Corners must not scale, or a nine-sliced pixel-art frame grows fat
    /// corners as the panel gets bigger — the exact artefact the mode exists
    /// to avoid.
    #[test]
    fn corners_never_scale() {
        let Some(NodeImageMode::Sliced(s)) = parse_image_mode("sliced(8)") else {
            panic!("expected a sliced mode");
        };
        assert_eq!(s.max_corner_scale, 1.0);
    }

    #[test]
    fn tiled_accepts_one_or_two_ratios() {
        assert!(matches!(
            parse_image_mode("tiled(1.0)"),
            Some(NodeImageMode::Tiled { .. })
        ));
        assert!(matches!(
            parse_image_mode("tiled(1.0, 2.0)"),
            Some(NodeImageMode::Tiled { .. })
        ));
    }

    /// Unrecognised input returns `None` rather than a default, so the caller
    /// can say so — a typo that silently drew `Auto` would look like the
    /// attribute was ignored, which is the hardest kind of bug to spot.
    #[test]
    fn nonsense_is_rejected_rather_than_defaulted() {
        assert!(parse_image_mode("sliced").is_none());
        assert!(parse_image_mode("sliced()").is_none());
        assert!(parse_image_mode("sliced(a)").is_none());
        assert!(parse_image_mode("sliced(1, 2, 3)").is_none());
        assert!(parse_image_mode("wobble(2)").is_none());
        assert!(parse_image_mode("").is_none());
    }

    #[test]
    fn presence_flags_accept_the_bare_and_explicit_forms() {
        assert!(attr_is_true(""));
        assert!(attr_is_true("true"));
        assert!(attr_is_true("1"));
        assert!(!attr_is_true("false"));
        assert!(!attr_is_true("0"));
    }
}
