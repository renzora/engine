//! Pack a set of coverage glyphs into one SDF strip texture.
//!
//! Given the px rects of some already-rasterized glyphs in Bevy's coverage font
//! atlas, convert each to a signed distance field ([`super::sdf`]) and pack the
//! unique ones side-by-side into a single linear-filtered R8 strip. Returns the
//! strip plus a per-glyph UV/size lookup. Callers then emit their own quads in
//! their own coordinate space — 3D text centres on the origin in world units; the
//! world-space UI emitter places them in panel-local space — so the *layout*
//! stays with the caller and only the crisp-edge machinery is shared here.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::sdf::{coverage_to_sdf, SPREAD};

/// One glyph's placement in the packed strip: normalized UV rect + padded px
/// size (the quad covers the padded region so the SDF's outside ramp is visible).
#[derive(Clone, Copy)]
pub struct PackedGlyph {
    pub u0: f32,
    pub u1: f32,
    pub v_top: f32,
    pub v_bot: f32,
    /// Padded width/height in px — the quad's full extents at 1 world-unit/px.
    pub pw: f32,
    pub ph: f32,
}

/// Stable per-rasterized-glyph key: its integer px rect in Bevy's coverage atlas.
pub fn glyph_key(r: Rect) -> [i32; 4] {
    [r.min.x as i32, r.min.y as i32, r.max.x as i32, r.max.y as i32]
}

/// A unique glyph's SDF bytes + padded size, mid-pack (before it gets an `sx`).
struct GlyphSdf {
    data: Vec<u8>,
    pw: usize,
    ph: usize,
    sx: usize,
}

/// Convert each unique glyph in `rects` (px rects in coverage atlas `atlas_id`)
/// to an SDF and pack them into ONE linear-filtered R8 strip. Returns the strip
/// handle plus a [`glyph_key`] → [`PackedGlyph`] lookup. `None` if the atlas isn't
/// resident yet or nothing packs.
///
/// The immutable atlas read and the mutable strip insert don't overlap (the read
/// is scoped), so one `&mut Assets<Image>` serves both.
pub fn pack_sdf_strip(
    images: &mut Assets<Image>,
    atlas_id: AssetId<Image>,
    rects: &[Rect],
) -> Option<(Handle<Image>, HashMap<[i32; 4], PackedGlyph>)> {
    let pad = SPREAD as usize;
    let gap = 2usize;
    let mut glyphs: HashMap<[i32; 4], GlyphSdf> = HashMap::default();

    // ── Coverage → SDF (atlas borrow released at block end) ──────────────────
    {
        let atlas = images.get(atlas_id)?;
        let atlas_w = atlas.width() as usize;
        let data = atlas.data.as_ref()?;
        for &r in rects {
            let key = glyph_key(r);
            if glyphs.contains_key(&key) {
                continue;
            }
            let gw = (r.max.x - r.min.x) as usize;
            let gh = (r.max.y - r.min.y) as usize;
            if gw == 0 || gh == 0 {
                continue;
            }
            let (pw, ph) = (gw + 2 * pad, gh + 2 * pad);
            // Padded coverage grid (border = outside); alpha ≥ 128 == inside.
            let mut inside = vec![false; pw * ph];
            let (rx, ry) = (r.min.x as usize, r.min.y as usize);
            for y in 0..gh {
                for x in 0..gw {
                    let a = data[((ry + y) * atlas_w + (rx + x)) * 4 + 3];
                    if a >= 128 {
                        inside[(y + pad) * pw + (x + pad)] = true;
                    }
                }
            }
            let sdf = coverage_to_sdf(&inside, pw, ph);
            glyphs.insert(key, GlyphSdf { data: sdf, pw, ph, sx: 0 });
        }
    }
    if glyphs.is_empty() {
        return None;
    }

    // ── Pack the unique glyphs into a single row ─────────────────────────────
    let strip_h = glyphs.values().map(|g| g.ph).max().unwrap_or(1);
    // Stable order so packing is deterministic across frames.
    let mut keys: Vec<[i32; 4]> = glyphs.keys().copied().collect();
    keys.sort_unstable();
    let mut cursor = 0usize;
    for k in &keys {
        let g = glyphs.get_mut(k).unwrap();
        g.sx = cursor;
        cursor += g.pw + gap;
    }
    let strip_w = cursor.max(1);

    let mut strip = vec![0u8; strip_w * strip_h];
    for k in &keys {
        let g = &glyphs[k];
        for y in 0..g.ph {
            let dst = y * strip_w + g.sx;
            let src = y * g.pw;
            strip[dst..dst + g.pw].copy_from_slice(&g.data[src..src + g.pw]);
        }
    }

    let mut strip_img = Image::new(
        Extent3d {
            width: strip_w as u32,
            height: strip_h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        strip,
        // R8Unorm: single-channel LINEAR distance (no sRGB decode on distances).
        TextureFormat::R8Unorm,
        RenderAssetUsages::default(),
    );
    // An SDF's distance MUST interpolate between texels — point sampling shows the
    // field's texels as blocks (jagged edges), defeating the SDF entirely.
    strip_img.sampler = bevy::image::ImageSampler::linear();
    let (sw, sh) = (strip_w as f32, strip_h as f32);
    let handle = images.add(strip_img);

    // Every glyph packs at row 0, so v_top = 0 and v_bot = its own height / strip.
    let lookup = keys
        .iter()
        .map(|k| {
            let g = &glyphs[k];
            (
                *k,
                PackedGlyph {
                    u0: g.sx as f32 / sw,
                    u1: (g.sx + g.pw) as f32 / sw,
                    v_top: 0.0,
                    v_bot: g.ph as f32 / sh,
                    pw: g.pw as f32,
                    ph: g.ph as f32,
                },
            )
        })
        .collect();

    Some((handle, lookup))
}
