//! The built-in blockout grid — the material a primitive wears before anyone
//! has given it one.
//!
//! It is generated, not loaded: one texture baked at startup, so there is no
//! asset to ship, nothing to go missing from a project, and nothing to keep in
//! sync with an exported `.rpak`. Rounded tiles separated by grout, a heavy rule
//! around each [`GRID_CELLS`]-square section, and a cross through that section's
//! middle.
//!
//! **It is deliberately flat.** An earlier version bevelled the tile edges in a
//! normal map and put ambient occlusion in the grout, and the result read as a
//! wall of moulded plastic tiles rather than as a greyboxing aid: relief that
//! strong competes with the actual shape of the geometry you are blocking out,
//! which is the one thing the material must not do. It also lit the seam of a
//! sphere's UVs into a visible zigzag ridge. What survives from that attempt is
//! the mip chain and the anisotropic sampling — those were fixing a real problem
//! (thin rules with no mips crawl as soon as the camera moves) and cost nothing
//! visually.
//!
//! Cells are pure white so the shape's own tint multiplies straight through; the
//! texture only ever darkens, where a rule or the grout is.

use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Grid cells per axis across one section — the span between two heavy rules,
/// with the cross at its centre. At an object scale of 1 this is also the number
/// of cells across a primitive's face, since their UVs run 0..1.
pub const GRID_CELLS: usize = 4;

/// Handle to the generated blockout-grid image, inserted at startup by the
/// engine. Consumers treat the resource as optional so headless/server builds
/// (no `Assets<Image>`) still work.
///
/// Every untextured primitive shares this one image — only the tint, which lives
/// in `base_color`/[`MeshColor`](super::MeshColor), differs per shape.
#[derive(Resource, Clone)]
pub struct GridTexture(pub Handle<Image>);

/// Bake the blockout grid, mip chain included.
///
/// Single source of truth for every consumer — a freshly spawned primitive, its
/// rehydration after a scene load, and the viewport's Textures-off swap — so
/// "this has no texture yet" looks the same however you arrived at it.
pub fn build_grid_image() -> Image {
    // All in texels of the repeating tile. The tile is one whole section, so
    // the section rule sits on the wrap seam and meets its neighbour to form a
    // single rule of the full width.
    const CELL: usize = 128;
    const SIZE: usize = GRID_CELLS * CELL;
    const GROUT: f32 = 6.0; // gap between neighbouring tiles
    const RADIUS: f32 = 14.0; // tile corner radius — the "slightly rounded"
    const BORDER: f32 = 16.0; // heavy rule around the section
    const CROSS_WIDTH: f32 = 20.0;
    const CROSS_ARM: f32 = 112.0;
    // Edges resolve over about a texel. Not zero — a hard step would alias on
    // the top mip, where the anisotropic sampler is reading close to 1:1 — but
    // nowhere near wide enough to read as a bevel.
    const EDGE: f32 = 1.5;

    // Linear multipliers over `base_color`: 1.0 leaves the tint alone.
    const TILE_VALUE: f32 = 1.0;
    const GROUT_VALUE: f32 = 0.34;
    const RULE_VALUE: f32 = 0.05;

    let mut level = Vec::with_capacity(SIZE * SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);

            // Rounded-square tile, centred in its cell.
            let local_x = (px % CELL as f32) - CELL as f32 * 0.5;
            let local_y = (py % CELL as f32) - CELL as f32 * 0.5;
            let half = CELL as f32 * 0.5 - GROUT * 0.5 - RADIUS;
            let tile = ramp(-(box_sdf(local_x, local_y, half, half) - RADIUS), EDGE);

            // Section rule, on the wrap seam in both axes.
            let seam = seam_distance(px, SIZE as f32).min(seam_distance(py, SIZE as f32));
            let border = ramp(BORDER * 0.5 - seam, EDGE);

            // Cross through the middle of the section: two overlapping bars.
            let (cx, cy) = (px - SIZE as f32 * 0.5, py - SIZE as f32 * 0.5);
            let cross = ramp(
                -box_sdf(cx, cy, CROSS_ARM, CROSS_WIDTH * 0.5)
                    .min(box_sdf(cx, cy, CROSS_WIDTH * 0.5, CROSS_ARM)),
                EDGE,
            );

            let value = GROUT_VALUE + (TILE_VALUE - GROUT_VALUE) * tile;
            let cut = border.max(cross);
            level.push(value + (RULE_VALUE - value) * cut);
        }
    }

    bake(level, SIZE)
}

/// Signed distance to an axis-aligned box centred on the origin. Negative
/// inside, and it keeps its gradient outside, which is what keeps the edge
/// ramp an even width all the way round the corners.
fn box_sdf(px: f32, py: f32, half_x: f32, half_y: f32) -> f32 {
    let qx = px.abs() - half_x;
    let qy = py.abs() - half_y;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0)
}

/// Distance to the nearest multiple of `period` — i.e. to the tile's wrap seam.
fn seam_distance(v: f32, period: f32) -> f32 {
    let m = v.rem_euclid(period);
    m.min(period - m)
}

/// 0..1 over `width` texels of `depth` past the surface.
fn ramp(depth: f32, width: f32) -> f32 {
    (depth / width).clamp(0.0, 1.0)
}

/// Encode level 0 and every mip below it into one image. Downsampling happens
/// on the f32 values — in *linear* space, not on sRGB bytes — so a distant
/// surface averages to the brightness it should rather than to the darker
/// result gamma-space filtering gives.
fn bake(mut level: Vec<f32>, size: usize) -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension};

    let mut data: Vec<u8> = Vec::with_capacity(size * size * 4 * 4 / 3);
    let mut width = size;
    let mut mip_level_count = 0u32;
    loop {
        for value in &level {
            let byte = srgb_byte(*value);
            data.extend_from_slice(&[byte, byte, byte, 255]);
        }
        mip_level_count += 1;
        if width == 1 {
            break;
        }

        let half = width / 2;
        let mut next = Vec::with_capacity(half * half);
        for y in 0..half {
            for x in 0..half {
                let (x0, y0) = (x * 2, y * 2);
                next.push(
                    (level[y0 * width + x0]
                        + level[y0 * width + x0 + 1]
                        + level[(y0 + 1) * width + x0]
                        + level[(y0 + 1) * width + x0 + 1])
                        * 0.25,
                );
            }
        }
        level = next;
        width = half;
    }

    // `Image::new` validates its buffer against level 0 only, so hand it that
    // slice and attach the full chain afterwards. Bevy's default
    // `TextureDataOrder` is mip-major, which is how `data` is laid out.
    let mut image = Image::new(
        Extent3d {
            width: size as u32,
            height: size as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data[..size * size * 4].to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = mip_level_count;
    image.data = Some(data);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        // Grazing angles are the whole job here — a blockout floor is mostly
        // seen edge-on, and that is exactly where trilinear alone turns to mush.
        anisotropy_clamp: 16,
        ..Default::default()
    });
    image
}

/// Encode a linear value to the sRGB byte an `Rgba8UnormSrgb` texture decodes
/// back to that same linear value on sample. The grid's values are authored in
/// linear space because they are *multipliers* over `base_color`, which the
/// shader also works with in linear space.
fn srgb_byte(linear: f32) -> u8 {
    let s = Srgba::from(LinearRgba::new(linear, linear, linear, 1.0));
    (s.red.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Level 0 plus every halving down to 1x1, laid out mip-major. A wrong
    /// `mip_level_count` or a short buffer is a wgpu upload error at runtime,
    /// which is a long way from here.
    #[test]
    fn mip_chain_is_complete_and_correctly_sized() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width;
        assert_eq!(image.texture_descriptor.mip_level_count, size.ilog2() + 1);
        let expected: usize = (0..image.texture_descriptor.mip_level_count)
            .map(|l| {
                let d = (size >> l).max(1) as usize;
                d * d * 4
            })
            .sum();
        assert_eq!(image.data.as_ref().map(Vec::len), Some(expected));
    }

    /// The tint has to survive: cells are pure white so `base_color` comes
    /// through untouched, and only the rules darken it. A regression here shows
    /// up as every primitive spawning muddy, which is what the checkerboard this
    /// replaced used to do.
    #[test]
    fn cell_centres_are_white_and_rules_are_dark() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let cell = size / GRID_CELLS;
        let data = image.data.unwrap();
        let texel = |x: usize, y: usize| data[(y * size + x) * 4];

        // Middle of the first cell — clear of the grout, the section rule and
        // the centre cross.
        assert_eq!(texel(cell / 2, cell / 2), 255);
        // The section rule sits on the wrap seam.
        assert!(texel(0, 0) < 80, "section rule should be near black");
        // The centre of the tile is the middle of the cross.
        assert!(texel(size / 2, size / 2) < 80, "cross should be near black");
    }

    /// Flat means flat: a tile face has to be one even value edge to edge, not
    /// a gradient falling away into the grout. The bevelled version this
    /// replaced read as moulded plastic.
    #[test]
    fn tile_faces_are_evenly_lit() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let cell = size / GRID_CELLS;
        let data = image.data.unwrap();
        // Across the interior of one cell, staying a few texels clear of the
        // grout so the antialiasing ramp isn't in the sample.
        let margin = 12;
        for x in (cell + margin)..(2 * cell - margin) {
            assert_eq!(
                data[((cell + cell / 2) * size + x) * 4],
                255,
                "tile face should be flat white at x={x}"
            );
        }
    }
}
