//! Font → 3D quad mesh, via a signed distance field.
//!
//! Bevy 0.19 shapes+rasterizes text (parley + swash) into a coverage font atlas
//! and hands back `TextLayoutInfo.glyphs` — each a centre position and a px rect
//! in that atlas. We drive that pipeline for the *layout*, then convert each
//! glyph's coverage into an SDF (see [`crate::sdf`]), pack the glyphs of this
//! string into one SDF strip texture, and emit one padded quad per glyph that
//! samples it. The [`SdfTextMaterial`](crate::material::SdfTextMaterial) shader
//! then keeps the edges crisp at any magnification.
//!
//! A plain function (no ECS) so the mesh-based world-space UI can reuse it.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::{
    ComputedTextBlock, Font, FontAtlasSet, FontCx, FontHinting, FontSize, FontSource, Justify,
    LayoutCx, LetterSpacing, LineBreak, LineHeight, ScaleCx, TextBounds, TextFont, TextLayoutInfo,
    TextPipeline,
};

use crate::sdf::{coverage_to_sdf, SPREAD};

/// World units per glyph pixel. A `size` of 100 with this factor makes cap
/// height ≈ 1 m — a readable default the entity's `Transform` then tunes.
pub const WORLD_UNITS_PER_PX: f32 = 0.01;

/// Rasterize the coverage source at this multiple of the requested size. The SDF
/// is scale-independent, but a higher-res coverage yields a cleaner distance
/// field, and it keeps short strings inside one atlas texture.
const SUPERSAMPLE: f32 = 2.0;

/// One deduplicated glyph's SDF, ready to pack into the strip.
struct GlyphSdf {
    /// SDF bytes (R8), `pw * ph`.
    data: Vec<u8>,
    /// Padded dimensions (glyph size + 2·SPREAD).
    pw: usize,
    ph: usize,
    /// Where it landed in the strip (px min corner).
    sx: usize,
    sy: usize,
}

/// Build a mesh of per-glyph SDF quads for `text`, plus the SDF strip texture the
/// quads sample. `None` if empty, the font isn't loaded yet (retry), or nothing
/// laid out. The mesh is centred on the origin, +Y up, facing +Z, world units.
#[allow(clippy::too_many_arguments)]
pub fn build_text_mesh(
    pipeline: &mut TextPipeline,
    fonts: &Assets<Font>,
    atlas_set: &mut FontAtlasSet,
    images: &mut Assets<Image>,
    font_cx: &mut FontCx,
    layout_cx: &mut LayoutCx,
    scale_cx: &mut ScaleCx,
    rem: f32,
    font: FontSource,
    text: &str,
    size_px: f32,
) -> Option<(Mesh, Handle<Image>)> {
    if text.trim().is_empty() || size_px <= 0.0 {
        return None;
    }

    let raster_px = size_px * SUPERSAMPLE;
    let ppu = WORLD_UNITS_PER_PX / SUPERSAMPLE;

    // ── Layout (Bevy's parley pipeline) ──────────────────────────────────────
    let text_font = TextFont {
        font,
        font_size: FontSize::Px(raster_px),
        ..default()
    };
    let mut computed = ComputedTextBlock::default();
    let spans = std::iter::once((
        Entity::PLACEHOLDER,
        0usize,
        text,
        &text_font,
        Color::WHITE,
        LineHeight::default(),
        LetterSpacing::default(),
    ));
    pipeline
        .update_buffer(
            fonts,
            spans,
            LineBreak::NoWrap,
            Justify::Left,
            TextBounds::UNBOUNDED,
            1.0,
            &mut computed,
            font_cx,
            layout_cx,
            Vec2::splat(1000.0),
            rem,
        )
        .ok()?;
    let mut info = TextLayoutInfo::default();
    pipeline
        .update_text_layout_info(
            &mut info,
            atlas_set,
            images,
            &mut computed,
            scale_cx,
            TextBounds::UNBOUNDED,
            Justify::Left,
            FontHinting::default(),
        )
        .ok()?;
    if info.glyphs.is_empty() {
        return None;
    }

    // v1: source glyphs from ONE coverage atlas (true for modest strings).
    let atlas_id = info.glyphs[0].atlas_info.texture;
    let pad = SPREAD as usize;

    // ── Coverage → SDF, deduplicated by atlas rect ───────────────────────────
    // A glyph key is its px rect in Bevy's atlas — stable per rasterized glyph.
    let key_of = |r: Rect| -> [i32; 4] {
        [r.min.x as i32, r.min.y as i32, r.max.x as i32, r.max.y as i32]
    };
    let mut glyphs: HashMap<[i32; 4], GlyphSdf> = HashMap::default();
    {
        let atlas = images.get(atlas_id)?;
        let atlas_w = atlas.width() as usize;
        let data = atlas.data.as_ref()?;
        for g in &info.glyphs {
            if g.atlas_info.texture != atlas_id {
                continue;
            }
            let key = key_of(g.atlas_info.rect);
            if glyphs.contains_key(&key) {
                continue;
            }
            let r = g.atlas_info.rect;
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
            glyphs.insert(key, GlyphSdf { data: sdf, pw, ph, sx: 0, sy: 0 });
        }
    }
    if glyphs.is_empty() {
        return None;
    }

    // ── Pack the unique glyphs into a single-row strip ───────────────────────
    let gap = 2usize;
    let strip_h = glyphs.values().map(|g| g.ph).max().unwrap_or(1);
    let mut cursor = 0usize;
    // Stable order so packing is deterministic.
    let mut keys: Vec<[i32; 4]> = glyphs.keys().copied().collect();
    keys.sort_unstable();
    for k in &keys {
        let g = glyphs.get_mut(k).unwrap();
        g.sx = cursor;
        g.sy = 0;
        cursor += g.pw + gap;
    }
    let strip_w = cursor.max(1);

    let mut strip = vec![0u8; strip_w * strip_h];
    for k in &keys {
        let g = &glyphs[k];
        for y in 0..g.ph {
            let dst = (g.sy + y) * strip_w + g.sx;
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
    // The whole point of an SDF is that the distance interpolates *between*
    // texels — so it MUST be linear-filtered. Point sampling shows the field's
    // texels as blocks (the jagged edges), defeating the SDF entirely.
    strip_img.sampler = bevy::image::ImageSampler::linear();
    let strip_size = Vec2::new(strip_w as f32, strip_h as f32);
    let strip_handle = images.add(strip_img);

    // ── Emit one padded quad per positioned glyph ────────────────────────────
    let block = info.size;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(info.glyphs.len() * 4);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(info.glyphs.len() * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(info.glyphs.len() * 6);

    for g in &info.glyphs {
        if g.atlas_info.texture != atlas_id {
            continue;
        }
        let Some(gs) = glyphs.get(&key_of(g.atlas_info.rect)) else {
            continue;
        };
        // Quad covers the PADDED region (glyph + SPREAD each side), centred on
        // the glyph centre so the SDF's outside ramp is included.
        let half = Vec2::new(gs.pw as f32, gs.ph as f32) * 0.5 * ppu;
        let cx = (g.position.x - block.x * 0.5) * ppu;
        let cy = -(g.position.y - block.y * 0.5) * ppu;

        let base = positions.len() as u32;
        positions.push([cx - half.x, cy - half.y, 0.0]); // bottom-left
        positions.push([cx + half.x, cy - half.y, 0.0]); // bottom-right
        positions.push([cx + half.x, cy + half.y, 0.0]); // top-right
        positions.push([cx - half.x, cy + half.y, 0.0]); // top-left

        // Strip is y-down: glyph top = sy, bottom = sy+ph. World bottom → bottom.
        let u0 = gs.sx as f32 / strip_size.x;
        let u1 = (gs.sx + gs.pw) as f32 / strip_size.x;
        let v_top = gs.sy as f32 / strip_size.y;
        let v_bot = (gs.sy + gs.ph) as f32 / strip_size.y;
        uvs.push([u0, v_bot]);
        uvs.push([u1, v_bot]);
        uvs.push([u1, v_top]);
        uvs.push([u0, v_top]);

        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    if positions.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));

    Some((mesh, strip_handle))
}
