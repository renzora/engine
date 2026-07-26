//! Font outline → true 3D letter geometry.
//!
//! Mesh mode reads each glyph's actual outline from the font (`ttf-parser`),
//! flattens the béziers to polylines, triangulates the fill — with holes, so the
//! inside of `o`/`e` stays open — via `lyon`, and optionally extrudes it into a
//! solid (front + back faces + side walls). The result is real, lit, extrudable
//! geometry (Godot's `TextMesh`), not a textured card — so it looks correct from
//! every angle and can be given thickness.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use lyon_tessellation::{
    math::point, path::Path, BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex,
    VertexBuffers,
};

use renzora_text_mesh::WORLD_UNITS_PER_PX;

/// Bézier flattening steps. Fixed (not adaptive) — glyphs are small and this
/// runs only on rebuild; 8/12 is smooth enough for typical sizes.
const QUAD_STEPS: usize = 8;
const CUBIC_STEPS: usize = 12;

/// Collects a glyph's outline into flattened contours (in font units), driven by
/// `ttf-parser`'s `OutlineBuilder` callbacks.
#[derive(Default)]
struct ContourBuilder {
    contours: Vec<Vec<Vec2>>,
    current: Vec<Vec2>,
    pos: Vec2,
}

impl ContourBuilder {
    fn flush(&mut self) {
        if self.current.len() >= 2 {
            self.contours.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }
}

impl ttf_parser::OutlineBuilder for ContourBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush();
        self.pos = Vec2::new(x, y);
        self.current.push(self.pos);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pos = Vec2::new(x, y);
        self.current.push(self.pos);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, p1, p2) = (self.pos, Vec2::new(x1, y1), Vec2::new(x, y));
        for i in 1..=QUAD_STEPS {
            let t = i as f32 / QUAD_STEPS as f32;
            let a = p0.lerp(p1, t);
            let b = p1.lerp(p2, t);
            self.current.push(a.lerp(b, t));
        }
        self.pos = p2;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0, p1, p2, p3) = (self.pos, Vec2::new(x1, y1), Vec2::new(x2, y2), Vec2::new(x, y));
        for i in 1..=CUBIC_STEPS {
            let t = i as f32 / CUBIC_STEPS as f32;
            let a = p0.lerp(p1, t);
            let b = p1.lerp(p2, t);
            let c = p2.lerp(p3, t);
            let d = a.lerp(b, t);
            let e = b.lerp(c, t);
            self.current.push(d.lerp(e, t));
        }
        self.pos = p3;
    }
    fn close(&mut self) {
        self.flush();
    }
}

/// Build extruded 3D geometry for `text` from raw font bytes. `size` is the em
/// size in px (matched to flat mode via [`WORLD_UNITS_PER_PX`]); `depth` is the
/// extrusion in world units (0 = a flat filled outline). `None` if the bytes
/// aren't a parseable font or nothing outlines.
pub fn build_outline_mesh(font_bytes: &[u8], text: &str, size: f32, depth: f32) -> Option<Mesh> {
    let face = ttf_parser::Face::parse(font_bytes, 0).ok()?;
    let upem = face.units_per_em() as f32;
    if upem <= 0.0 {
        return None;
    }
    let scale = size / upem * WORLD_UNITS_PER_PX;

    // Lay glyphs left-to-right, accumulating contours in font units at their pen
    // position; scale/centre afterwards.
    let mut contours: Vec<Vec<Vec2>> = Vec::new();
    let mut pen_x = 0.0f32;
    for ch in text.chars() {
        let gid = match face.glyph_index(ch) {
            Some(g) => g,
            None => {
                pen_x += upem * 0.3;
                continue;
            }
        };
        let mut b = ContourBuilder::default();
        if face.outline_glyph(gid, &mut b).is_some() {
            b.flush();
            for c in b.contours {
                contours.push(c.into_iter().map(|p| Vec2::new(p.x + pen_x, p.y)).collect());
            }
        }
        pen_x += face.glyph_hor_advance(gid).unwrap_or(0) as f32;
    }
    if contours.is_empty() {
        return None;
    }

    // Scale to world units and centre (horizontally on the pen span, vertically
    // on roughly half the ascender so the text sits centred on the origin).
    let total_w = pen_x * scale;
    let cy = face.ascender() as f32 * 0.5 * scale;
    for c in &mut contours {
        for p in c.iter_mut() {
            p.x = p.x * scale - total_w * 0.5;
            p.y = p.y * scale - cy;
        }
    }

    // ── Triangulate the fill (holes via non-zero winding) ────────────────────
    let mut builder = Path::builder();
    for c in &contours {
        if c.len() < 2 {
            continue;
        }
        builder.begin(point(c[0].x, c[0].y));
        for p in &c[1..] {
            builder.line_to(point(p.x, p.y));
        }
        builder.end(true);
    }
    let path = builder.build();

    let mut fill: VertexBuffers<Vec2, u32> = VertexBuffers::new();
    let mut tess = FillTessellator::new();
    tess.tessellate_path(
        &path,
        &FillOptions::default().with_fill_rule(FillRule::NonZero),
        &mut BuffersBuilder::new(&mut fill, |v: FillVertex| {
            let p = v.position();
            Vec2::new(p.x, p.y)
        }),
    )
    .ok()?;
    if fill.vertices.is_empty() {
        return None;
    }

    // ── Assemble the mesh: front face, (extruded) back face + side walls ──────
    let d = depth.max(0.0);
    let front_z = d * 0.5; // centre the slab on z=0
    let back_z = -d * 0.5;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Front face (+Z).
    let front_base = 0u32;
    for v in &fill.vertices {
        positions.push([v.x, v.y, front_z]);
        normals.push([0.0, 0.0, 1.0]);
    }
    for i in &fill.indices {
        indices.push(front_base + i);
    }

    if d > 0.0 {
        // Back face (−Z), reversed winding so it faces outward.
        let back_base = positions.len() as u32;
        for v in &fill.vertices {
            positions.push([v.x, v.y, back_z]);
            normals.push([0.0, 0.0, -1.0]);
        }
        for tri in fill.indices.chunks_exact(3) {
            indices.push(back_base + tri[0]);
            indices.push(back_base + tri[2]);
            indices.push(back_base + tri[1]);
        }

        // Side walls: one quad per contour edge, front rim → back rim.
        for c in &contours {
            let n = c.len();
            for i in 0..n {
                let a = c[i];
                let b = c[(i + 1) % n];
                let edge = (b - a).normalize_or_zero();
                // Outward-ish normal (perpendicular to the edge in the plane).
                let nrm = [edge.y, -edge.x, 0.0];
                let base = positions.len() as u32;
                positions.push([a.x, a.y, front_z]);
                positions.push([b.x, b.y, front_z]);
                positions.push([b.x, b.y, back_z]);
                positions.push([a.x, a.y, back_z]);
                for _ in 0..4 {
                    normals.push(nrm);
                }
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }

    let uvs = vec![[0.0f32, 0.0]; positions.len()];
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}
