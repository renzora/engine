//! Font outline → true 3D letter geometry.
//!
//! Reads each glyph's actual outline from the font (`ttf-parser`), flattens the
//! béziers to polylines, triangulates the fill — with holes, so the inside of
//! `o`/`e` stays open — via `lyon`, and optionally extrudes it into a solid
//! (front + back faces + side walls). Real, lit, extrudable geometry rather than
//! a textured card, so it reads correctly from every angle and can be given
//! thickness.
//!
//! Ported from `renzora_text3d::outline` essentially unchanged. The only edits
//! are the vector type — `bevy::Vec2` became the local [`P2`], since the
//! boundary carries no maths library — and the return value, which is now plain
//! buffers for [`renzora_plugin::ecs::Meshes::write`] instead of a `Mesh`.

use renzora_plugin::prelude::Vec3;

/// Bézier flattening steps. Fixed rather than adaptive — glyphs are small and
/// this runs only on rebuild; 8/12 is smooth enough for typical sizes.
const QUAD_STEPS: usize = 8;
const CUBIC_STEPS: usize = 12;

/// World units per rasterization pixel, matching the engine's text scale so a
/// `size` of 32 means the same here as it did in the original crate.
const WORLD_UNITS_PER_PX: f32 = 0.01;

/// A point in the glyph plane. Local because `sys::Vec3` is plain data with no
/// maths on it and there is no 2D type at all on the boundary.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct P2 {
    pub x: f32,
    pub y: f32,
}

impl P2 {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    fn lerp(self, o: Self, t: f32) -> Self {
        Self::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
    fn normalize_or_zero(self) -> Self {
        let len = (self.x * self.x + self.y * self.y).sqrt();
        if len > 1e-6 {
            Self::new(self.x / len, self.y / len)
        } else {
            Self::default()
        }
    }
}

/// Collects a glyph's outline into flattened contours, in font units, driven by
/// `ttf-parser`'s callbacks.
#[derive(Default)]
struct ContourBuilder {
    contours: Vec<Vec<P2>>,
    current: Vec<P2>,
    pos: P2,
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
        self.pos = P2::new(x, y);
        self.current.push(self.pos);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pos = P2::new(x, y);
        self.current.push(self.pos);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, p1, p2) = (self.pos, P2::new(x1, y1), P2::new(x, y));
        for i in 1..=QUAD_STEPS {
            let t = i as f32 / QUAD_STEPS as f32;
            let a = p0.lerp(p1, t);
            let b = p1.lerp(p2, t);
            self.current.push(a.lerp(b, t));
        }
        self.pos = p2;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (p0, p1, p2, p3) = (
            self.pos,
            P2::new(x1, y1),
            P2::new(x2, y2),
            P2::new(x, y),
        );
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

/// Triangulated glyph geometry, ready for `Meshes::write`.
pub struct Glyphs {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// Build extruded geometry for `text` from raw font bytes.
///
/// `size` is the em size in px; `depth` is the extrusion in world units, with 0
/// giving a flat filled outline. `None` if the bytes are not a parseable font or
/// nothing outlines — a string of spaces, say.
pub fn build_outline_mesh(font_bytes: &[u8], text: &str, size: f32, depth: f32) -> Option<Glyphs> {
    use lyon_tessellation::{
        math::point, path::Path, BuffersBuilder, FillOptions, FillRule, FillTessellator,
        FillVertex, VertexBuffers,
    };

    let face = ttf_parser::Face::parse(font_bytes, 0).ok()?;
    let upem = face.units_per_em() as f32;
    if upem <= 0.0 {
        return None;
    }
    let scale = size / upem * WORLD_UNITS_PER_PX;

    // Lay glyphs left to right, accumulating contours in font units at their pen
    // position; scale and centre afterwards.
    let mut contours: Vec<Vec<P2>> = Vec::new();
    let mut pen_x = 0.0f32;
    for ch in text.chars() {
        let gid = match face.glyph_index(ch) {
            Some(g) => g,
            // No glyph for this character — advance by a nominal space rather
            // than collapsing, so missing glyphs leave a gap instead of
            // silently shortening the line.
            None => {
                pen_x += upem * 0.3;
                continue;
            }
        };
        let mut b = ContourBuilder::default();
        if face.outline_glyph(gid, &mut b).is_some() {
            b.flush();
            for c in b.contours {
                contours.push(c.into_iter().map(|p| P2::new(p.x + pen_x, p.y)).collect());
            }
        }
        pen_x += face.glyph_hor_advance(gid).unwrap_or(0) as f32;
    }
    if contours.is_empty() {
        return None;
    }

    // Scale to world units and centre: horizontally on the pen span, vertically
    // on roughly half the ascender, so the text sits centred on the origin and
    // the entity's transform places it.
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

    let mut fill: VertexBuffers<P2, u32> = VertexBuffers::new();
    let mut tess = FillTessellator::new();
    tess.tessellate_path(
        &path,
        &FillOptions::default().with_fill_rule(FillRule::NonZero),
        &mut BuffersBuilder::new(&mut fill, |v: FillVertex| {
            let p = v.position();
            P2::new(p.x, p.y)
        }),
    )
    .ok()?;
    if fill.vertices.is_empty() {
        return None;
    }

    // ── Assemble: front face, then back face + side walls if extruded ────────
    let d = depth.max(0.0);
    let front_z = d * 0.5; // centre the slab on z = 0
    let back_z = -d * 0.5;

    let mut positions: Vec<Vec3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for v in &fill.vertices {
        positions.push(Vec3 { x: v.x, y: v.y, z: front_z });
        normals.push(Vec3 { x: 0.0, y: 0.0, z: 1.0 });
    }
    indices.extend_from_slice(&fill.indices);

    if d > 0.0 {
        // Back face, reversed winding so it faces outward.
        let back_base = positions.len() as u32;
        for v in &fill.vertices {
            positions.push(Vec3 { x: v.x, y: v.y, z: back_z });
            normals.push(Vec3 { x: 0.0, y: 0.0, z: -1.0 });
        }
        for tri in fill.indices.chunks_exact(3) {
            indices.push(back_base + tri[0]);
            indices.push(back_base + tri[2]);
            indices.push(back_base + tri[1]);
        }

        // Side walls: one quad per contour edge, front rim to back rim.
        for c in &contours {
            let n = c.len();
            for i in 0..n {
                let a = c[i];
                let b = c[(i + 1) % n];
                let edge = b.sub(a).normalize_or_zero();
                // Perpendicular to the edge in the plane — outward for a
                // correctly wound contour.
                let nrm = Vec3 { x: edge.y, y: -edge.x, z: 0.0 };
                let base = positions.len() as u32;
                positions.push(Vec3 { x: a.x, y: a.y, z: front_z });
                positions.push(Vec3 { x: b.x, y: b.y, z: front_z });
                positions.push(Vec3 { x: b.x, y: b.y, z: back_z });
                positions.push(Vec3 { x: a.x, y: a.y, z: back_z });
                for _ in 0..4 {
                    normals.push(nrm);
                }
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }

    let uvs = vec![[0.0f32, 0.0]; positions.len()];
    Some(Glyphs {
        positions,
        normals,
        uvs,
        indices,
    })
}
