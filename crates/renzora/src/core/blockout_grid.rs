//! The built-in blockout grid — the material a primitive wears before anyone
//! has given it one.
//!
//! It is generated, not loaded: one texture baked at startup, so there is no
//! asset to ship, nothing to go missing from a project, and nothing to keep in
//! sync with an exported `.rpak`. A flat field ruled into squares by thin bright
//! lines, with a bright four-pointed star on every fourth intersection.
//!
//! # Why it is this plain
//!
//! It used to be rounded tiles separated by dark grout, with a heavy rule around
//! every [`GRID_CELLS`]-square section and a cross through that section's
//! middle. The rounded tiles and the heavy rule are gone: they drew a coarse
//! second grid over the first, so a wall read as a pattern of large marked
//! panels rather than as a ruled surface, and the eye picked out the decoration
//! instead of the measure.
//!
//! The cross survives, inverted. It marks the same thing the heavy rule did, one
//! section every [`GRID_CELLS`] cells, but it does it by brightening a single
//! intersection rather than by drawing a box round the whole section: a mark you
//! can count off to judge a distance, which is the job, without a second set of
//! edges competing with the geometry's own. It sits *on* an intersection with
//! its arms along the lines, so it thickens the grid where it lands instead of
//! cutting across it, and it tapers to a point so it reads as a mark rather than
//! as a shape.
//!
//! The lines are **brighter** than the field they rule, not darker. A greybox is
//! read at a glance for its proportions, and light rules on a darker face carry
//! at distance and at grazing angles the way dark grout does not: grout closes up
//! into a muddy smear as the surface turns away, where a bright rule thins out
//! and stays legible.
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
//! The texture is a multiplier over the shape's own tint, so the *line* is the
//! part that passes the tint through untouched and the field is held below it.
//! That is the only way round it: a multiply cannot brighten, so a line lighter
//! than its surroundings has to be made by darkening the surroundings.

use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Grid cells per axis across one repeat of the texture.
///
/// The blockout projection maps one repeat to one world unit, so this is also
/// the number of cells per metre: four, i.e. a 25 cm cell, which is fine enough
/// to measure a doorway against and coarse enough not to turn a wall into noise.
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
    // All in texels of the repeating tile.
    const CELL: usize = 128;
    const SIZE: usize = GRID_CELLS * CELL;
    /// Line width. Thin on purpose: this is a ruler laid over the surface, and
    /// a heavy rule starts reading as a gap between panels instead.
    const LINE: f32 = 5.0;
    // Edges resolve over about a texel. Not zero — a hard step would alias on
    // the top mip, where the anisotropic sampler is reading close to 1:1 — and
    // no wider than half the line, or the ramps meet in the middle and the line
    // never reaches full value: a soft grey smear instead of a rule.
    const EDGE: f32 = 1.0;

    /// How far each arm of the star reaches, and how wide it is where they
    /// cross. Short of the neighbouring intersection on purpose: a star that
    /// reached one would join up with the next and the marks would read as a
    /// second grid, which is what the heavy section rule did wrong.
    const STAR_ARM: f32 = CELL as f32 * 0.85;
    const STAR_WIDTH: f32 = 12.0;

    // Linear multipliers over `base_color`: 1.0 leaves the tint alone.
    //
    // The ordinary line stops short of 1.0 so the star has somewhere brighter to
    // go. Being both wider and brighter is what lets it read at a glance from
    // across a room, which is the only reason it is there.
    //
    // The field is well below the line, and further below it than a flat swatch
    // of the texture suggests it needs to be. A lit surface is not a swatch: a
    // sunlit face is pushed to the top of the range where everything compresses
    // together, and a grid that reads clearly on the texture itself washes out
    // to nothing on the floor of an actual scene. The contrast has to be sized
    // for the lit case, which is the only one anybody sees.
    const FIELD_VALUE: f32 = 0.30;
    const LINE_VALUE: f32 = 0.85;
    const STAR_VALUE: f32 = 1.0;

    // The middle of the texture is `GRID_CELLS / 2` cells in, and `GRID_CELLS`
    // is even, so it lands exactly on an intersection. That is what puts the
    // star on the grid rather than across it.
    const CENTRE: f32 = SIZE as f32 * 0.5;

    /// One arm of the star: `along` runs down its length, `across` its width.
    ///
    /// The half-width falls to nothing over the arm's reach, which is what
    /// gives the taper. Two of these at right angles make the star, and they
    /// overlap at the centre where both are at full width.
    ///
    /// The falloff is quadratic rather than linear: an arm that narrows in a
    /// straight line is a thin triangle and reads as a smudge on the line it
    /// sits on, where one that holds its width through the middle and then runs
    /// out to a point reads as a mark placed there deliberately.
    fn arm(along: f32, across: f32) -> f32 {
        let t = (along.abs() / STAR_ARM).clamp(0.0, 1.0);
        ramp(STAR_WIDTH * 0.5 * (1.0 - t * t) - across.abs(), EDGE)
    }

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

            let (dx, dy) = (px - CENTRE, py - CENTRE);
            let star = arm(dx, dy).max(arm(dy, dx));

            let value = FIELD_VALUE + (LINE_VALUE - FIELD_VALUE) * line;
            level.push(value + (STAR_VALUE - value) * star);
        }
    }

    bake(level, SIZE)
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

    /// The line is what passes the tint through, and the field is held below
    /// it. A multiply cannot brighten, so this ordering is the only way a rule
    /// ends up lighter than the surface it rules; getting it backwards is the
    /// dark-grout version this replaced.
    #[test]
    fn lines_are_brighter_than_the_field_they_rule() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let cell = size / GRID_CELLS;
        let data = image.data.clone().unwrap();
        let texel = |x: usize, y: usize| data[(y * size + x) * 4];

        // A cell boundary, and the middle of the cell beyond it. Both clear of
        // the star at the texture's centre.
        let line = texel(cell, cell + cell / 2);
        let field = texel(cell + cell / 2, cell + cell / 2);
        assert!(line > field, "line {line} should be brighter than field {field}");
        assert!(field < 220, "the field must be held well below the line");
    }

    /// The star is brighter and wider than the line it lands on, or it would be
    /// invisible from the distance a greybox is judged at. It is also the only
    /// thing brighter than a line, so it is what the tint passes through whole.
    #[test]
    fn the_star_is_brighter_and_wider_than_the_line() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let data = image.data.clone().unwrap();
        let texel = |x: usize, y: usize| data[(y * size + x) * 4];
        let cell = size / GRID_CELLS;

        let centre = size / 2;
        // A plain intersection: two lines crossing, and nothing more.
        let plain = texel(cell, cell);
        assert_eq!(texel(centre, centre), 255, "the star centre passes the tint");
        assert!(
            texel(centre, centre) > plain,
            "the star must outshine a plain intersection"
        );

        // The star is the only thing on the texture brighter than a line, and it
        // is a run of texels rather than a single bright one, or it would vanish
        // into the first mip.
        let brighter_than_a_line =
            |x: usize| (0..size).filter(|&y| texel(x, y) > plain).count();
        assert!(
            brighter_than_a_line(centre) > 40,
            "the star arm covered only {} texels",
            brighter_than_a_line(centre)
        );
        assert_eq!(
            brighter_than_a_line(cell),
            0,
            "a plain line should have nothing brighter sitting on it"
        );
    }

    /// The star's arms must stop short of the neighbouring intersections. Joined
    /// up, they would draw the coarse second grid the heavy section rule used to,
    /// which is the thing this texture was rebuilt to get rid of.
    #[test]
    fn the_stars_do_not_join_up() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let cell = size / GRID_CELLS;
        let data = image.data.unwrap();
        let texel = |x: usize, y: usize| data[(y * size + x) * 4];

        let centre = size / 2;
        // Just short of the next intersection along, and clear of both lines
        // there, so anything left in the sample is the arm overreaching.
        let near_the_next = texel(centre + cell - 10, centre + 10);
        let field = texel(centre + cell / 2, centre + cell / 2);
        assert_eq!(
            near_the_next, field,
            "the arm still had width a whole cell out from the centre"
        );
    }

    /// One line weight and one cell size, everywhere. The old texture drew a
    /// heavy rule around each section and a cross through its middle, which is a
    /// second, coarser grid over the first: the wrap seam and the tile centre
    /// used to be marked, and now they are ordinary.
    #[test]
    fn the_grid_is_uniform() {
        let image = build_grid_image();
        let size = image.texture_descriptor.size.width as usize;
        let cell = size / GRID_CELLS;
        let data = image.data.clone().unwrap();
        let texel = |x: usize, y: usize| data[(y * size + x) * 4];

        // The centre of the texture is the middle of a cell, not a cross.
        assert_eq!(texel(size / 2 + cell / 2, size / 2 + cell / 2), texel(cell / 2, cell / 2));
        // Every cell boundary is the same line: no section is marked out.
        for n in 0..GRID_CELLS {
            assert_eq!(
                texel(n * cell, cell / 2),
                texel(cell, cell / 2),
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
