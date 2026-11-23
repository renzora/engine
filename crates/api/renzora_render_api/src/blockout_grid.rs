//! The built-in blockout grid: the material a primitive wears before anyone
//! has given it one.
//!
//! It is generated, not loaded: one texture baked at startup, so there is no
//! asset to ship, nothing to go missing from a project, and nothing to keep in
//! sync with an exported `.rpak`. A soft two-tone checker ruled into squares by
//! thin dark lines, with a heavier, darker line around every
//! [`GRID_CELLS`]-square section.
//!
//! # Why it looks like this
//!
//! It is the classic level-editor brush grid. The cells give the fine measure
//! (25 cm) and the section line gives the coarse one (a metre), so a wall can be
//! counted off at a glance without reading individual cells. The field passes
//! the shape's tint through whole, so an orange cube is the orange that was
//! picked and the lines are darker shades of it.
//!
//! An earlier version ruled the grid in **bright** lines over a darkened field,
//! with a four-pointed star marking each section's centre in place of a section
//! line. It held up on sunlit floors but darkened every tint to a third of its
//! value and the star read as decoration rather than as a measure, so it was
//! replaced by this.
//!
//! # The grooves
//!
//! Each line is a hairline V-groove in a normal map baked beside the colour
//! ([`build_grid_normal_image`]), cut into a surface that is otherwise dead
//! flat. Together with a fairly glossy finish that is what makes the grid answer
//! the sun: a low light glints off the groove walls that face it and smears a
//! highlight across the faces, instead of lighting the whole surface as one flat
//! card.
//!
//! **Raised tiles are wrong here.** Two versions rounded or bevelled the tiles
//! themselves, one with ambient occlusion in the grout, and both read as toy
//! bricks or moulded plastic: relief across the face competes with the actual
//! shape of the geometry you are blocking out. Keep relief inside the lines.
//! The heavier of the two also lit a sphere's UV seam into a zigzag ridge, which
//! the per-triangle box projection in `renzora_engine::blockout` no longer
//! produces. The mip chain and the anisotropic sampling date from it too: thin
//! rules with no mips crawl as soon as the camera moves.

use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Grid cells per axis across one repeat of the texture.
///
/// The blockout projection maps one repeat to one world unit, so this is also
/// the number of cells per metre: four, i.e. a 25 cm cell, which is fine enough
/// to measure a doorway against and coarse enough not to turn a wall into noise.
/// One repeat is one section, so the section line falls on the wrap seam.
pub const GRID_CELLS: usize = 4;

/// Handles to the generated blockout-grid images, inserted at startup by the
/// engine. Consumers treat the resource as optional so headless/server builds
/// (no `Assets<Image>`) still work.
///
/// Every untextured primitive shares these images; only the tint, which lives
/// in `base_color`/[`MeshColor`](super::MeshColor), differs per shape.
#[derive(Resource, Clone)]
pub struct GridTexture {
    /// The colour multiplier, from [`build_grid_image`].
    pub color: Handle<Image>,
    /// The line grooves, from [`build_grid_normal_image`].
    pub normal: Handle<Image>,
}

// All in texels of the repeating tile.
//
// 256 texels to a 25 cm cell. Linear magnification softens every edge by one
// texel, so up close the resolution *is* the sharpness: at 128 a line edge
// blurred over two millimetres and read as out of focus with the camera near a
// wall. (`Nearest` magnification would be sharper, but wgpu rejects it on a
// sampler with anisotropic filtering; see `bake`.)
const CELL: usize = 256;
const SIZE: usize = GRID_CELLS * CELL;
/// Cell line width. Thin: it is the fine measure, and a heavy one would
/// compete with the section line.
const LINE: f32 = 8.0;
/// Section line width, two and a half cell lines, so it reads as the coarse
/// measure from across a room and survives the first few mips.
const SECTION_LINE: f32 = 20.0;

/// Bake the blockout grid, mip chain included.
///
/// Single source of truth for every consumer (a freshly spawned primitive, its
/// rehydration after a scene load, and the viewport's Textures-off swap) so
/// "this has no texture yet" looks the same however you arrived at it.
pub fn build_grid_image() -> Image {
    // Half a texel: with both widths even and the lines centred on texel
    // corners, every texel lands wholly inside a line or wholly outside it, so
    // a line's edge is as sharp as linear magnification allows: one texel of
    // blend. A one-texel ramp left a half-tone texel on each edge, which
    // magnified into a two-texel soft band.
    const EDGE: f32 = 0.5;

    // Linear multipliers over `base_color`: 1.0 leaves the tint alone. Cells
    // alternate between the tint itself and a slightly darker shade of it, a
    // soft checker that lets you count cells along a receding floor where the
    // lines have merged into the mips. The lines are darker shades again, the
    // section line darker than the cell line so the two measures stay apart.
    const FIELD_VALUE: f32 = 1.0;
    const FIELD_ALT_VALUE: f32 = 0.78;
    const LINE_VALUE: f32 = 0.55;
    const SECTION_VALUE: f32 = 0.22;

    let mut level = Vec::with_capacity(SIZE * SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Distance to the nearest cell boundary in either axis. Boundaries
            // fall on multiples of `CELL`, and `SIZE` is a whole number of
            // cells, so the lines on the wrap seam meet their neighbours across
            // it and form one line of the full width rather than two half ones.
            let d = seam_distance(px, CELL as f32).min(seam_distance(py, CELL as f32));
            let line = ramp(LINE * 0.5 - d, EDGE);
            // The same for the section boundary, which is the wrap seam.
            let ds = seam_distance(px, SIZE as f32).min(seam_distance(py, SIZE as f32));
            let section = ramp(SECTION_LINE * 0.5 - ds, EDGE);

            // `GRID_CELLS` is even, so the checker carries on unbroken across
            // the wrap seam.
            let field = if (x / CELL + y / CELL).is_multiple_of(2) { FIELD_VALUE } else { FIELD_ALT_VALUE };
            let value = field + (LINE_VALUE - field) * line;
            level.push(Vec3::splat(value + (SECTION_VALUE - value) * section));
        }
    }

    bake(level, TextureFormat::Rgba8UnormSrgb, false, |v| {
        let byte = srgb_byte(v.x);
        [byte, byte, byte, 255]
    })
}

/// Bake the line grooves as a tangent-space normal map, mip chain included.
///
/// Every line is a shallow V cut into an otherwise perfectly flat surface: the
/// cell faces have no relief at all, and only the line's own width slopes. Under
/// a low sun the wall facing the light catches a hairline glint and the far wall
/// falls dark, which is what makes the grid respond to the light as you turn.
/// The groove sits *inside* the dark line, so it reads as a scored line on a
/// flat panel and never as a raised tile. A rounded tile bevel was tried here
/// first and read as toy bricks.
///
/// Tangent space here is the box projection's: `+x` is `+u`, `+y` is `+v`
/// (down the image), and `renzora_engine::blockout` writes tangents that agree,
/// rather than relying on a generic tangent generator's handedness.
pub fn build_grid_normal_image() -> Image {
    /// Groove wall slope: depth gained per texel toward the line's centre. 1 is
    /// a 45 degree wall, steep enough to throw a clear glint across the narrow
    /// width it has.
    const SLOPE: f32 = 1.0;

    // Depth of the groove under a texel coordinate along one axis, in texels.
    // The section line is wider than a cell line, so its groove is too.
    fn groove(v: f32) -> f32 {
        let half = if seam_distance(v, SIZE as f32) < CELL as f32 * 0.5 {
            SECTION_LINE * 0.5
        } else {
            LINE * 0.5
        };
        (half - seam_distance(v, CELL as f32)).max(0.0) * SLOPE
    }

    // Surface height: flat at zero, dropping into whichever groove is deeper
    // where two cross.
    fn height(x: f32, y: f32) -> f32 {
        -groove(x).max(groove(y))
    }

    let mut level = Vec::with_capacity(SIZE * SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let dx = (height(px + 1.0, py) - height(px - 1.0, py)) * 0.5;
            let dy = (height(px, py + 1.0) - height(px, py - 1.0)) * 0.5;
            level.push(Vec3::new(-dx, -dy, 1.0).normalize());
        }
    }

    bake(level, TextureFormat::Rgba8Unorm, true, |n| {
        let byte = |c: f32| ((c * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
        [byte(n.x), byte(n.y), byte(n.z), 255]
    })
}

/// Distance to the nearest multiple of `period`: a cell boundary, and at the
/// texture's edge the wrap seam.
fn seam_distance(v: f32, period: f32) -> f32 {
    let m = v.rem_euclid(period);
    m.min(period - m)
}

/// 0..1 over `width` texels of `depth` past the surface.
fn ramp(depth: f32, width: f32) -> f32 {
    (depth / width).clamp(0.0, 1.0)
}

/// Encode level 0 and every mip below it into one image. Downsampling happens
/// on the f32 values, in *linear* space, not on sRGB bytes, so a distant
/// surface averages to the brightness it should rather than to the darker
/// result gamma-space filtering gives.
///
/// `renormalize` is for the normal map: an average of unit vectors is shorter
/// than a unit vector, and a short normal darkens the lighting of every distant
/// tile edge.
fn bake(
    mut level: Vec<Vec3>,
    format: TextureFormat,
    renormalize: bool,
    encode: fn(Vec3) -> [u8; 4],
) -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension};

    let size = SIZE;
    let mut data: Vec<u8> = Vec::with_capacity(size * size * 4 * 4 / 3);
    let mut width = size;
    let mut mip_level_count = 0u32;
    loop {
        for value in &level {
            data.extend_from_slice(&encode(*value));
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
                let mean = (level[y0 * width + x0]
                    + level[y0 * width + x0 + 1]
                    + level[(y0 + 1) * width + x0]
                    + level[(y0 + 1) * width + x0 + 1])
                    * 0.25;
                next.push(if renormalize { mean.normalize_or(Vec3::Z) } else { mean });
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
        format,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = mip_level_count;
    image.data = Some(data);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        // Linear, though `Nearest` would draw the hard texels below with a
        // sharper edge up close: wgpu rejects any non-linear filter on a
        // sampler with anisotropy above 1 ("all filter modes must be linear"),
        // and the editor died with an unrecoverable validation error the first
        // time a blockout shape drew. Grazing angles matter more than the last
        // texel of edge, so anisotropy stays and the edge softens by one texel.
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        // Grazing angles are the whole job here: a blockout floor is mostly
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

    /// The field passes the tint through whole and the cell lines are darker
    /// shades of it, so a shape is the colour that was picked for it.
    #[test]
    fn field_passes_the_tint_and_lines_are_darker() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let cell = size / GRID_CELLS;
        let data = image.data.clone().unwrap();
        let texel = |x: usize, y: usize| data[(y * size + x) * 4];

        let mid = cell + cell / 2;
        let field = texel(mid, mid);
        let alt = texel(mid + cell, mid);
        let line = texel(cell, mid);
        assert_eq!(field, 255, "the field must pass the tint through");
        assert!(alt < field, "the checker's other cell {alt} should be darker");
        assert!(line < alt, "line {line} should be darker than both cells");
    }

    /// The section line is darker and wider than a cell line, or the coarse
    /// measure would not stand out from the fine one.
    #[test]
    fn section_line_is_darker_and_wider_than_a_cell_line() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let cell = size / GRID_CELLS;
        let data = image.data.clone().unwrap();
        let texel = |x: usize, y: usize| data[(y * size + x) * 4];

        let mid = cell + cell / 2;
        let cell_line = texel(cell, mid);
        let section_line = texel(0, mid);
        assert!(
            section_line < cell_line,
            "section line {section_line} should be darker than cell line {cell_line}"
        );

        // Width: texels in a row no lighter than a cell line, near each kind of
        // line. (Not "darker than the field": the checker's darker cells are.)
        let dark_run =
            |from: usize, to: usize| (from..to).filter(|&x| texel(x, mid) <= cell_line).count();
        let section_width = dark_run(0, cell / 2) + dark_run(size - cell / 2, size);
        let cell_width = dark_run(cell / 2, cell + cell / 2);
        assert!(
            section_width > cell_width,
            "section line {section_width} texels should be wider than cell line {cell_width}"
        );
    }

    /// wgpu rejects a sampler with anisotropy above 1 unless every filter is
    /// linear, and the rejection is an unrecoverable validation error: the
    /// editor crashed the first time a blockout shape drew.
    #[test]
    fn the_sampler_is_one_wgpu_accepts() {
        use bevy::image::{ImageFilterMode, ImageSampler};
        let image = build_grid_image();
        let ImageSampler::Descriptor(sampler) = &image.sampler else {
            panic!("the grid names its own sampler");
        };
        if sampler.anisotropy_clamp > 1 {
            assert_eq!(
                (sampler.mag_filter, sampler.min_filter, sampler.mipmap_filter),
                (ImageFilterMode::Linear, ImageFilterMode::Linear, ImageFilterMode::Linear),
                "anisotropic filtering needs every filter linear"
            );
        }
    }

    /// The relief is a groove inside the line and nothing else: the cell face,
    /// right up to the line's edge, faces straight out, and the groove's walls
    /// tilt in toward the line's centre. A tilt on the face itself is the
    /// raised-tile look that read as toy bricks; walls tilting outward would be
    /// a ridge rather than a groove.
    #[test]
    fn the_only_relief_is_a_groove_in_the_line() {
        let image = build_grid_normal_image();
        let size = image.texture_descriptor.size.width as usize;
        assert_eq!(image.texture_descriptor.mip_level_count, size.ilog2() + 1);
        let cell = size / GRID_CELLS;
        let data = image.data.clone().unwrap();
        let normal = |x: usize, y: usize| {
            let i = (y * size + x) * 4;
            (data[i] as i32 - 128, data[i + 1] as i32 - 128, data[i + 2] as i32)
        };

        let mid = cell + cell / 2;
        let half = LINE as usize / 2;
        let is_flat = |n: (i32, i32, i32)| n.0.abs() <= 1 && n.1.abs() <= 1;
        assert!(is_flat(normal(mid, mid)), "cell face tilts: {:?}", normal(mid, mid));
        // Two texels clear of the line on either side: still dead flat.
        assert!(is_flat(normal(cell + half + 2, mid)), "the face slopes into the line");
        assert!(is_flat(normal(cell - half - 3, mid)), "the face slopes into the line");

        // Inside the line, the wall right of centre faces back toward `-u` and
        // the wall left of it toward `+u`; the same for `v` down the image.
        let right_wall = normal(cell + 1, mid);
        let left_wall = normal(cell - 2, mid);
        let lower_wall = normal(mid, cell + 1);
        assert!(right_wall.0 < -10, "right wall should face -u: {right_wall:?}");
        assert!(left_wall.0 > 10, "left wall should face +u: {left_wall:?}");
        assert!(lower_wall.1 < -10, "lower wall should face -v: {lower_wall:?}");
    }

    /// Level 0 holds only the four values, never one between them: an
    /// in-between texel on a line's edge would widen the blend the sampler's
    /// linear magnification already adds, and show up close as a soft band.
    #[test]
    fn edges_are_hard_on_the_top_level() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let data = image.data.clone().unwrap();
        let mut values: Vec<u8> = (0..size * size).map(|i| data[i * 4]).collect();
        values.sort_unstable();
        values.dedup();
        assert_eq!(values.len(), 4, "expected two cells, line and section only: {values:?}");
    }

    /// Every interior cell boundary is the same line; only the section boundary
    /// on the wrap seam is heavier.
    #[test]
    fn interior_cell_lines_are_uniform() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let cell = size / GRID_CELLS;
        let data = image.data.clone().unwrap();
        let texel = |x: usize, y: usize| data[(y * size + x) * 4];

        for n in 1..GRID_CELLS {
            assert_eq!(
                texel(n * cell, cell + cell / 2),
                texel(cell, cell + cell / 2),
                "boundary {n} differs from the others"
            );
        }
    }

    /// Flat means flat: a cell face has to be one even value edge to edge, not
    /// a gradient falling away into the line. The bevelled version this
    /// replaced read as moulded plastic.
    #[test]
    fn cell_faces_are_evenly_lit() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let cell = size / GRID_CELLS;
        let data = image.data.unwrap();
        let expected = data[((cell + cell / 2) * size + cell + cell / 2) * 4];
        // Across the interior of one cell, staying a few texels clear of the
        // lines so the antialiasing ramp isn't in the sample.
        let margin = 8;
        for x in (cell + margin)..(2 * cell - margin) {
            assert_eq!(
                data[((cell + cell / 2) * size + x) * 4],
                expected,
                "cell face should be flat at x={x}"
            );
        }
    }
}
