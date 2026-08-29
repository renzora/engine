//! Font → 3D quad mesh, via a signed distance field.
//!
//! Bevy 0.19 shapes+rasterizes text (parley + swash) into a coverage font atlas
//! and hands back `TextLayoutInfo.glyphs` — each a centre position and a px rect
//! in that atlas. We drive that pipeline for the *layout*, then hand the glyph
//! rects to [`pack_sdf_strip`] (coverage → SDF, packed into one strip) and emit
//! one padded quad per glyph that samples it. The
//! [`SdfTextMaterial`](super::material::SdfTextMaterial) shader keeps the edges
//! crisp at any magnification.
//!
//! A plain function (no ECS) so the mesh-based world-space UI could reuse it; in
//! practice the UI emitter shares only [`pack_sdf_strip`] (it already has bevy_ui's
//! layout) while this drives its own layout for a standalone `Text3d` entity.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::text::{
    ComputedTextBlock, Font, FontAtlasSet, FontCx, FontHinting, FontSize, FontSource, Justify,
    LayoutCx, LetterSpacing, LineBreak, LineHeight, ScaleCx, TextBounds, TextFont, TextLayoutInfo,
    TextPipeline,
};

use super::pack::{glyph_key, pack_sdf_strip};

/// World units per glyph pixel. A `size` of 100 with this factor makes cap
/// height ≈ 1 m — a readable default the entity's `Transform` then tunes.
pub const WORLD_UNITS_PER_PX: f32 = 0.01;

/// Rasterize the coverage source at this multiple of the requested size. The SDF
/// is scale-independent, but a higher-res coverage yields a cleaner distance
/// field, and it keeps short strings inside one atlas texture.
const SUPERSAMPLE: f32 = 2.0;

/// Build a mesh of per-glyph SDF quads for `text`, plus the SDF strip texture the
/// quads sample. `None` if empty, the font isn't loaded yet (retry), or nothing
/// laid out. The mesh is centred on the origin, +Y up, facing +Z, world units.
/// Vertex colours are white — tint via [`SdfTextMaterial::color`].
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
    let rects: Vec<Rect> = info
        .glyphs
        .iter()
        .filter(|g| g.atlas_info.texture == atlas_id)
        .map(|g| g.atlas_info.rect)
        .collect();
    let (strip_handle, lookup) = pack_sdf_strip(images, atlas_id, &rects)?;

    // ── Emit one padded quad per positioned glyph ────────────────────────────
    let block = info.size;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(info.glyphs.len() * 4);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(info.glyphs.len() * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(info.glyphs.len() * 6);

    for g in &info.glyphs {
        if g.atlas_info.texture != atlas_id {
            continue;
        }
        let Some(pg) = lookup.get(&glyph_key(g.atlas_info.rect)) else {
            continue;
        };
        // Quad covers the PADDED region (glyph + SPREAD each side), centred on the
        // glyph centre so the SDF's outside ramp is included.
        let half = Vec2::new(pg.pw, pg.ph) * 0.5 * ppu;
        let cx = (g.position.x - block.x * 0.5) * ppu;
        let cy = -(g.position.y - block.y * 0.5) * ppu;

        let base = positions.len() as u32;
        positions.push([cx - half.x, cy - half.y, 0.0]); // bottom-left
        positions.push([cx + half.x, cy - half.y, 0.0]); // bottom-right
        positions.push([cx + half.x, cy + half.y, 0.0]); // top-right
        positions.push([cx - half.x, cy + half.y, 0.0]); // top-left

        // Strip is y-down: glyph top = v_top, bottom = v_bot. World bottom → bottom.
        uvs.push([pg.u0, pg.v_bot]);
        uvs.push([pg.u1, pg.v_bot]);
        uvs.push([pg.u1, pg.v_top]);
        uvs.push([pg.u0, pg.v_top]);

        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    if positions.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    // White vertex colours: the shared SDF shader multiplies uniform × vertex, so
    // the `VERTEX_COLORS` def is on for both this (white) and the UI emitter.
    let colors = vec![[1.0f32, 1.0, 1.0, 1.0]; positions.len()];
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    Some((mesh, strip_handle))
}
