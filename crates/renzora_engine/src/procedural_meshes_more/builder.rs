//! The one mesh builder every generator in this directory draws with.
//!
//! The older generators in [`procedural_meshes`](crate::procedural_meshes)
//! each push raw vertex arrays and stamp a `0..1` UV square on every quad, so a
//! 2 m wall face and a 5 cm trim face wear the same texture once each. That is
//! invisible under the blockout grid (which re-projects UVs anyway) and very
//! visible the moment a real tiling material goes on. Everything here goes
//! through [`MeshBuilder`] instead, which gives every surface UVs measured in
//! metres, so a tiling material reads at one scale across the whole library.
//!
//! The builder also owns the two invariants the old helpers left to each
//! caller: a triangle is wound to agree with its vertex normals (decided per
//! triangle from the geometry, so no generator has to get its corner order
//! right), and a triangle with no area is never emitted at all. The second
//! matters more than it looks: ramps and fans that start at zero height are
//! easiest to write as ordinary quads, and the zero-height half of such a quad
//! is a sliver a rasteriser draws as a stray line.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::f32::consts::TAU;

/// Twice-area squared below which a triangle is treated as degenerate.
///
/// Compared against `|ab x ac|^2`, so this is a triangle of roughly
/// `5e-8 m^2`: far below anything a generator here means to draw, far above
/// float noise on a collapsed quad.
const DEGENERATE: f32 = 1e-14;

/// A point on an extruded or swept cross-section.
///
/// `n` is `Some` for a point on a smooth curve (its analytic outward normal in
/// profile space) and `None` for a hard corner. An edge between two smooth
/// points is shaded smoothly; any edge touching a hard corner is shaded flat
/// with its own face normal, which is what keeps a rounded corner meeting a
/// flat side from bending the flat side's shading.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProfilePt {
    pub p: Vec2,
    pub n: Option<Vec2>,
}

impl ProfilePt {
    pub(crate) fn hard(x: f32, y: f32) -> Self {
        Self { p: Vec2::new(x, y), n: None }
    }

    pub(crate) fn smooth(x: f32, y: f32, n: Vec2) -> Self {
        Self { p: Vec2::new(x, y), n: Some(n.normalize()) }
    }
}

/// A point on a lathe profile: `p` is `(radius, height)` and `n` the outward
/// normal in that same plane.
///
/// A crease is two consecutive points at the same position with different
/// normals; the zero-length band between them is skipped, so the bands either
/// side get their own vertices.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LathePt {
    pub p: Vec2,
    pub n: Vec2,
}

impl LathePt {
    pub(crate) fn new(r: f32, y: f32, n: Vec2) -> Self {
        Self { p: Vec2::new(r, y), n: n.normalize() }
    }
}

/// One station along a sweep path. `side` and `up` span the cross-section
/// plane; they must be unit length and perpendicular.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub origin: Vec3,
    pub side: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

impl Frame {
    fn place(&self, p: Vec2) -> Vec3 {
        self.origin + self.side * p.x + self.up * p.y
    }

    fn orient(&self, n: Vec2) -> Vec3 {
        (self.side * n.x + self.up * n.y).normalize()
    }
}

#[derive(Default)]
pub(crate) struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl MeshBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Number of vertices so far: the index the next vertex will get.
    fn mark(&self) -> usize {
        self.positions.len()
    }

    fn vertex(&mut self, p: Vec3, n: Vec3, uv: Vec2) -> u32 {
        let i = self.positions.len() as u32;
        self.positions.push(p.into());
        self.normals.push(n.normalize_or(Vec3::Y).into());
        self.uvs.push(uv.into());
        i
    }

    /// One triangle, wound so its geometric normal agrees with its vertex
    /// normals, or nothing if it has no area.
    fn tri(&mut self, a: u32, b: u32, c: u32) {
        let pa = Vec3::from(self.positions[a as usize]);
        let pb = Vec3::from(self.positions[b as usize]);
        let pc = Vec3::from(self.positions[c as usize]);
        let face = (pb - pa).cross(pc - pa);
        if face.length_squared() < DEGENERATE {
            return;
        }
        let shading = Vec3::from(self.normals[a as usize])
            + Vec3::from(self.normals[b as usize])
            + Vec3::from(self.normals[c as usize]);
        if face.dot(shading) < 0.0 {
            self.indices.extend_from_slice(&[a, c, b]);
        } else {
            self.indices.extend_from_slice(&[a, b, c]);
        }
    }

    fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }

    /// A flat polygon (convex or not, no holes), shaded with one normal that
    /// points to the same side as `outward`.
    ///
    /// The normal is computed from the corners (Newell's method), so `outward`
    /// only has to be on the right side, not exact: a face centroid minus the
    /// solid's centre is enough for anything convex.
    pub(crate) fn face(&mut self, pts: &[Vec3], outward: Vec3) {
        // A section may repeat a point to put a crease in its sweep (see
        // `ProfilePt`); as a cap outline that is a zero-length edge, which
        // ear clipping would only have to step around.
        let mut pts: Vec<Vec3> = pts.to_vec();
        pts.dedup_by(|a, b| a.distance_squared(*b) < 1e-12);
        while pts.len() > 1 && pts[0].distance_squared(pts[pts.len() - 1]) < 1e-12 {
            pts.pop();
        }
        let pts = pts.as_slice();
        if pts.len() < 3 {
            return;
        }
        let mut n = Vec3::ZERO;
        for (i, c) in pts.iter().enumerate() {
            let d = pts[(i + 1) % pts.len()];
            n += Vec3::new(
                (c.y - d.y) * (c.z + d.z),
                (c.z - d.z) * (c.x + d.x),
                (c.x - d.x) * (c.y + d.y),
            );
        }
        if n.length_squared() < DEGENERATE {
            return;
        }
        let mut n = n.normalize();
        if n.dot(outward) < 0.0 {
            n = -n;
        }
        let (u, v) = plane_axes(n);
        let base = self.mark() as u32;
        let flat: Vec<Vec2> = pts.iter().map(|p| Vec2::new(p.dot(u), p.dot(v))).collect();
        for (p, uv) in pts.iter().zip(&flat) {
            self.vertex(*p, n, *uv);
        }
        for [a, b, c] in ear_clip(&flat) {
            self.tri(base + a, base + b, base + c);
        }
    }

    /// An axis-aligned box. The workhorse of every prop assembled from
    /// planks and posts.
    pub(crate) fn cuboid(&mut self, min: Vec3, max: Vec3) {
        let c = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        let (a, b) = (min, max);
        self.face(&[c(b.x, a.y, a.z), c(b.x, b.y, a.z), c(b.x, b.y, b.z), c(b.x, a.y, b.z)], Vec3::X);
        self.face(&[c(a.x, a.y, a.z), c(a.x, a.y, b.z), c(a.x, b.y, b.z), c(a.x, b.y, a.z)], -Vec3::X);
        self.face(&[c(a.x, b.y, a.z), c(a.x, b.y, b.z), c(b.x, b.y, b.z), c(b.x, b.y, a.z)], Vec3::Y);
        self.face(&[c(a.x, a.y, a.z), c(b.x, a.y, a.z), c(b.x, a.y, b.z), c(a.x, a.y, b.z)], -Vec3::Y);
        self.face(&[c(a.x, a.y, b.z), c(b.x, a.y, b.z), c(b.x, b.y, b.z), c(a.x, b.y, b.z)], Vec3::Z);
        self.face(&[c(a.x, a.y, a.z), c(a.x, b.y, a.z), c(b.x, b.y, a.z), c(b.x, a.y, a.z)], -Vec3::Z);
    }

    /// Faces of a closed polyhedron given as corner loops, each shaded flat and
    /// oriented away from the vertex centroid. Only valid for shapes that are
    /// star-shaped about that centroid, which every convex solid is.
    pub(crate) fn polyhedron(&mut self, faces: &[Vec<Vec3>]) {
        let all: Vec<Vec3> = faces.iter().flatten().copied().collect();
        let centre = all.iter().copied().sum::<Vec3>() / all.len().max(1) as f32;
        for f in faces {
            let fc = f.iter().copied().sum::<Vec3>() / f.len() as f32;
            self.face(f, fc - centre);
        }
    }

    /// Sweep a closed cross-section along `frames`.
    ///
    /// UVs run around the section in `u` (its perimeter, in metres) and along
    /// the path in `v`, where `v` is each vertex's own travelled distance. On a
    /// bend that is what keeps the inside of the curve from wearing the same
    /// number of tiles as the much longer outside.
    ///
    /// `invert` turns the normals inward, for the bore of a hollow pipe.
    /// `caps` closes both ends flat, which needs a section with no holes.
    pub(crate) fn sweep(&mut self, profile: &[ProfilePt], frames: &[Frame], invert: bool, caps: bool) {
        let count = profile.len();
        if count < 3 || frames.len() < 2 {
            return;
        }
        let ccw = signed_area(&profile.iter().map(|p| p.p).collect::<Vec<_>>()) > 0.0;
        let sign = if invert { -1.0 } else { 1.0 };

        // Travelled distance of each profile vertex along the path.
        let travel: Vec<Vec<f32>> = (0..count)
            .map(|j| {
                let mut acc = 0.0;
                let mut out = vec![0.0];
                for w in frames.windows(2) {
                    acc += w[0].place(profile[j].p).distance(w[1].place(profile[j].p));
                    out.push(acc);
                }
                out
            })
            .collect();

        let mut around = 0.0;
        for j in 0..count {
            let k = (j + 1) % count;
            let (a, b) = (profile[j], profile[k]);
            let e = b.p - a.p;
            let len = e.length();
            if len < 1e-7 {
                continue;
            }
            let flat = if ccw { Vec2::new(e.y, -e.x) } else { Vec2::new(-e.y, e.x) } / len;
            let (na, nb) = match (a.n, b.n) {
                (Some(na), Some(nb)) => (na, nb),
                _ => (flat, flat),
            };
            let base = self.mark() as u32;
            for (i, f) in frames.iter().enumerate() {
                self.vertex(f.place(a.p), f.orient(na) * sign, Vec2::new(around, travel[j][i]));
                self.vertex(f.place(b.p), f.orient(nb) * sign, Vec2::new(around + len, travel[k][i]));
            }
            for i in 0..frames.len() as u32 - 1 {
                let (s0, s1) = (base + i * 2, base + i * 2 + 2);
                self.quad(s0, s0 + 1, s1 + 1, s1);
            }
            around += len;
        }

        if caps {
            let first = frames[0];
            let last = frames[frames.len() - 1];
            let start: Vec<Vec3> = profile.iter().map(|p| first.place(p.p)).collect();
            let end: Vec<Vec3> = profile.iter().map(|p| last.place(p.p)).collect();
            self.face(&start, -first.forward);
            self.face(&end, last.forward);
        }
    }

    /// Straight extrusion of a closed section from `from` to `to`, capped.
    /// `side` and `up` say how the section's x and y map into the world.
    pub(crate) fn extrude(&mut self, profile: &[ProfilePt], from: Vec3, to: Vec3, side: Vec3, up: Vec3) {
        let forward = (to - from).normalize();
        let frames = [
            Frame { origin: from, side, up, forward },
            Frame { origin: to, side, up, forward },
        ];
        self.sweep(profile, &frames, false, true);
    }

    /// Flat band between two closed loops with matching vertex counts: the end
    /// of a hollow pipe, the face of a frame.
    pub(crate) fn ring(&mut self, outer: &[Vec3], inner: &[Vec3], outward: Vec3) {
        debug_assert_eq!(outer.len(), inner.len());
        let mut n = Vec3::ZERO;
        for (i, c) in outer.iter().enumerate() {
            let d = outer[(i + 1) % outer.len()];
            n += (*c).cross(d);
        }
        let mut n = n.normalize_or(outward.normalize_or(Vec3::Y));
        if n.dot(outward) < 0.0 {
            n = -n;
        }
        let (u, v) = plane_axes(n);
        let uv = |p: Vec3| Vec2::new(p.dot(u), p.dot(v));
        let base = self.mark() as u32;
        for (o, i) in outer.iter().zip(inner) {
            self.vertex(*o, n, uv(*o));
            self.vertex(*i, n, uv(*i));
        }
        let count = outer.len() as u32;
        for j in 0..count {
            let k = (j + 1) % count;
            self.quad(base + j * 2, base + k * 2, base + k * 2 + 1, base + j * 2 + 1);
        }
    }

    /// Surface of revolution about +Y, smooth around the axis.
    ///
    /// UVs are `(arc length around, distance along the profile)`, both in
    /// metres, so they agree wherever two smooth bands meet. A band lying flat
    /// (a cap) is mapped straight down onto XZ instead, which is the grid it
    /// would wear as part of a box.
    pub(crate) fn lathe(&mut self, profile: &[LathePt], segments: u32) {
        let segments = segments.max(3);
        let mut along = 0.0;
        for w in profile.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = a.p.distance(b.p);
            if len < 1e-7 || (a.p.x < 1e-6 && b.p.x < 1e-6) {
                along += len;
                continue;
            }
            let flat = a.n.x.abs() < 1e-6 && b.n.x.abs() < 1e-6 && (a.p.y - b.p.y).abs() < 1e-6;
            let base = self.mark() as u32;
            for i in 0..=segments {
                let t = TAU * i as f32 / segments as f32;
                let (s, c) = t.sin_cos();
                for (pt, dist) in [(a, along), (b, along + len)] {
                    let p = Vec3::new(pt.p.x * c, pt.p.y, pt.p.x * s);
                    let n = Vec3::new(pt.n.x * c, pt.n.y, pt.n.x * s);
                    let uv = if flat { Vec2::new(p.x, p.z) } else { Vec2::new(t * pt.p.x, dist) };
                    self.vertex(p, n, uv);
                }
            }
            for i in 0..segments {
                let s0 = base + i * 2;
                let s1 = s0 + 2;
                self.quad(s0, s1, s1 + 1, s0 + 1);
            }
            along += len;
        }
    }

    /// A grid surface with smooth normals: `f(i, j)` gives position, normal
    /// and UV for row `i` of `rows + 1` and column `j` of `cols + 1`.
    pub(crate) fn grid(&mut self, rows: u32, cols: u32, f: impl Fn(u32, u32) -> (Vec3, Vec3, Vec2)) {
        let base = self.mark() as u32;
        for i in 0..=rows {
            for j in 0..=cols {
                let (p, n, uv) = f(i, j);
                self.vertex(p, n, uv);
            }
        }
        let stride = cols + 1;
        for i in 0..rows {
            for j in 0..cols {
                let a = base + i * stride + j;
                self.quad(a, a + 1, a + stride + 1, a + stride);
            }
        }
    }

    /// Shift everything so the bounding box is centred on the origin, the
    /// pivot convention every older generator follows (a shape lands centred
    /// where it is dropped and scales about its middle).
    pub(crate) fn center_bounds(&mut self) {
        let (min, max) = self.bounds();
        let mid = (min + max) * 0.5;
        for p in &mut self.positions {
            *p = (Vec3::from(*p) - mid).into();
        }
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        self.positions.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| {
            (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))
        })
    }

    /// Drop vertices no triangle uses (the far end of a skipped sliver), so a
    /// mesh carries nothing a GPU would transform for no reason.
    fn compact(&mut self) {
        let mut remap = vec![u32::MAX; self.positions.len()];
        let mut next = 0u32;
        let (mut p, mut n, mut uv) = (Vec::new(), Vec::new(), Vec::new());
        for idx in &mut self.indices {
            let old = *idx as usize;
            if remap[old] == u32::MAX {
                remap[old] = next;
                next += 1;
                p.push(self.positions[old]);
                n.push(self.normals[old]);
                uv.push(self.uvs[old]);
            }
            *idx = remap[old];
        }
        self.positions = p;
        self.normals = n;
        self.uvs = uv;
    }

    pub(crate) fn build(mut self) -> Mesh {
        self.compact();
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

/// Turn a hard-cornered `(radius, height)` polygon into lathe points, each
/// edge with its own outward normal. The polygon must be counter-clockwise.
pub(crate) fn hard_lathe(pts: &[Vec2], closed: bool) -> Vec<LathePt> {
    let mut out = Vec::new();
    let edges = if closed { pts.len() } else { pts.len() - 1 };
    for i in 0..edges {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let e = b - a;
        let n = Vec2::new(e.y, -e.x);
        out.push(LathePt::new(a.x, a.y, n));
        out.push(LathePt::new(b.x, b.y, n));
    }
    out
}

/// Frames along a polyline with known tangents, carried by parallel transport
/// so the section does not spin about the path.
///
/// For a `closed` path the first point is appended again at the end, and the
/// twist transport leaves over a full loop is spread evenly along it, so the
/// last frame lands exactly on the first and the seam does not show.
pub(crate) fn transported_frames(points: &[Vec3], tangents: &[Vec3], up_hint: Vec3, closed: bool) -> Vec<Frame> {
    let mut pts = points.to_vec();
    let mut tans: Vec<Vec3> = tangents.iter().map(|t| t.normalize()).collect();
    if closed {
        pts.push(points[0]);
        tans.push(tans[0]);
    }
    let first = tans[0];
    let mut up = (up_hint - first * up_hint.dot(first)).normalize_or(first.any_orthonormal_vector());
    let mut ups = vec![up];
    for w in tans.windows(2) {
        up = (Quat::from_rotation_arc(w[0], w[1]) * up).normalize();
        ups.push(up);
    }
    if closed {
        let last = ups[ups.len() - 1];
        let twist = last.cross(ups[0]).dot(first).atan2(last.dot(ups[0]));
        let steps = (ups.len() - 1) as f32;
        for (i, u) in ups.iter_mut().enumerate() {
            *u = Quat::from_axis_angle(tans[i], twist * i as f32 / steps) * *u;
        }
    }
    let mut frames: Vec<Frame> = pts
        .iter()
        .zip(&tans)
        .zip(&ups)
        .map(|((p, t), u)| {
            let up = (*u - *t * u.dot(*t)).normalize();
            Frame { origin: *p, side: up.cross(*t).normalize(), up, forward: *t }
        })
        .collect();
    if closed {
        // Bit-identical, not merely close: the closing ring's vertices are
        // separate from the opening ring's, and any difference at all is a
        // crack along the seam.
        frames[pts.len() - 1] = frames[0];
    }
    frames
}

/// `count` points of a circle of radius `r` as a smooth section.
pub(crate) fn circle_profile(r: f32, count: u32) -> Vec<ProfilePt> {
    (0..count)
        .map(|i| {
            let t = TAU * i as f32 / count as f32;
            let (s, c) = t.sin_cos();
            ProfilePt::smooth(r * c, r * s, Vec2::new(c, s))
        })
        .collect()
}

/// A face's texture axes: `u` horizontal along the face, `v` up it, so a wall
/// reads the same way round whichever way it faces.
fn plane_axes(n: Vec3) -> (Vec3, Vec3) {
    let u = if n.y.abs() < 0.999 { Vec3::Y.cross(n).normalize() } else { Vec3::X };
    (u, n.cross(u))
}

fn signed_area(pts: &[Vec2]) -> f32 {
    let mut a = 0.0;
    for (i, p) in pts.iter().enumerate() {
        a += p.perp_dot(pts[(i + 1) % pts.len()]);
    }
    a * 0.5
}

/// Triangulate a simple polygon by ear clipping.
///
/// The polygons here are a few dozen points, so the quadratic search is
/// nothing; what matters is that a concave outline (an archway, a star, a
/// battlement) gets triangles that stay inside it, which a fan does not.
pub(crate) fn ear_clip(pts: &[Vec2]) -> Vec<[u32; 3]> {
    let n = pts.len();
    let mut out = Vec::new();
    if n < 3 {
        return out;
    }
    let mut ring: Vec<usize> = (0..n).collect();
    if signed_area(pts) < 0.0 {
        ring.reverse();
    }
    let cross = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - b);
    while ring.len() > 3 {
        let m = ring.len();
        let ear = (0..m).find(|&i| {
            let (ia, ib, ic) = (ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(a, b, c) <= 1e-9 {
                return false;
            }
            !ring.iter().any(|&j| {
                j != ia && j != ib && j != ic && {
                    let p = pts[j];
                    p != a && p != b && p != c && in_triangle(p, a, b, c)
                }
            })
        });
        match ear {
            Some(i) => {
                out.push([ring[(i + m - 1) % m] as u32, ring[i] as u32, ring[(i + 1) % m] as u32]);
                ring.remove(i);
            }
            None => {
                // Only straight-through (collinear) corners are left, which add
                // no area; drop one and carry on. Generators avoid them, so
                // this is a guard against an infinite loop, not a code path.
                let flat = (0..m).find(|&i| {
                    let (a, b, c) = (pts[ring[(i + m - 1) % m]], pts[ring[i]], pts[ring[(i + 1) % m]]);
                    cross(a, b, c).abs() <= 1e-9
                });
                match flat {
                    Some(i) => {
                        ring.remove(i);
                    }
                    None => break,
                }
            }
        }
    }
    if ring.len() == 3 {
        out.push([ring[0] as u32, ring[1] as u32, ring[2] as u32]);
    }
    out
}

/// Inside or on the edge of the counter-clockwise triangle `abc`.
fn in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let e = -1e-7;
    (b - a).perp_dot(p - a) >= e && (c - b).perp_dot(p - b) >= e && (a - c).perp_dot(p - c) >= e
}
