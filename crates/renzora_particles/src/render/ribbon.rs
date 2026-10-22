//! Ribbon meshes: [`Facing::Ribbon`](crate::asset::Facing::Ribbon) strips.
//!
//! A strip is a run of points, two vertices each, joined by two triangles per
//! segment. Every vertex carries its point, its side (`corner.x` of -0.5 or
//! +0.5), how far along the strip it is (`corner.y`, -0.5 at the head to +0.5
//! at the tail), the strip's direction there as the axis, and the width there
//! as `size.x`. The vertex shader turns the width across the direction toward
//! the camera; nothing here knows where the camera is, so one mesh is right in
//! every view, the same rule the quads follow.
//!
//! `corner.y` running the length is what makes every shading mode work on a
//! ribbon unchanged: a texture's v runs down the trail, and `ProceduralFlame`,
//! which draws its base at -0.5 and its tip at +0.5, licks back along it from
//! the particle.
//!
//! ## Tubes
//!
//! A [`RibbonShape::Tube`](crate::asset::RibbonShape::Tube) writes a ring of
//! `segments + 1` vertices per point instead of two, `corner.x` running from
//! -0.5 to 0.5 once round (the last vertex repeats the first's place, so the
//! texture's u closes without a seam in the UVs). The vertex shader's tube
//! facing pushes each one out round the strip's direction by half the width;
//! the ring's frame is turned toward the camera there, so no frame has to be
//! carried from point to point, and the GPU's ribbon fetch, which sees one
//! vertex at a time, builds the very same ring. In 2D, which has no tube
//! facing, the same vertices are pushed straight across the strip and draw
//! it flat, which is what a tube looks like side on.

use bevy::math::{Affine3A, Vec3, Vec4};

use super::{frame, particle_seed, MeshScratch};
use crate::asset::{RenderSettings, RibbonKind, RibbonShape};
use crate::sim::ParticleBuffer;

/// One point of a strip, in mesh space.
#[derive(Clone, Copy, Debug)]
struct StripPoint {
    position: Vec3,
    /// The width at the head; the taper is applied when the strip is built.
    width: f32,
    color: Vec4,
    life: f32,
}

/// What the strips of one emitter came to: the bounds, or nothing drawn.
pub(super) struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
    pub any: bool,
}

/// Write every strip of an emitter into `scratch`.
pub(super) fn write(
    scratch: &mut MeshScratch,
    particles: &ParticleBuffer,
    render: &RenderSettings,
    to_mesh: Affine3A,
) -> Bounds {
    let mut bounds = Bounds { min: Vec3::splat(f32::MAX), max: Vec3::splat(f32::MIN), any: false };
    let mut points: Vec<StripPoint> = Vec::new();
    let life_of = |i: usize| {
        if particles.lifetime[i] > 0.0 {
            (particles.age[i] / particles.lifetime[i]).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    match render.ribbon.kind {
        RibbonKind::Trail => {
            for i in 0..particles.len() {
                points.clear();
                let here = particles.position[i];
                let point = |p: Vec3| StripPoint {
                    position: to_mesh.transform_point3(p),
                    width: particles.size[i].x,
                    color: particles.color[i],
                    life: life_of(i),
                };
                points.push(point(here));
                // The newest kept point is usually where the particle is now
                // (it was kept this step); a zero-length first segment has no
                // direction, so it is skipped.
                for p in particles.trail(i) {
                    if points.len() == 1 && p.distance_squared(here) < 1e-12 {
                        continue;
                    }
                    points.push(point(p));
                }
                let frame = frame(render, particles.age[i], particles.lifetime[i]);
                strip(scratch, &points, render, particle_seed(particles.id[i]), frame, &mut bounds);
            }
        }
        RibbonKind::Connect => {
            let groups = render.ribbon.groups.max(1);
            // Newest first, so the head of each strip is its youngest
            // particle and the tail its oldest, as a wake reads.
            let mut order: Vec<usize> = (0..particles.len()).collect();
            order.sort_by(|a, b| particles.id[*b].cmp(&particles.id[*a]));
            for g in 0..groups {
                points.clear();
                for &i in order.iter().filter(|&&i| particles.id[i] % groups == g) {
                    points.push(StripPoint {
                        position: to_mesh.transform_point3(particles.position[i]),
                        width: particles.size[i].x,
                        color: particles.color[i],
                        life: life_of(i),
                    });
                }
                strip(scratch, &points, render, particle_seed(g), 0.0, &mut bounds);
            }
        }
    }
    bounds
}

/// Append one strip. Fewer than two finite points draw nothing.
fn strip(scratch: &mut MeshScratch, points: &[StripPoint], render: &RenderSettings, seed: f32, frame: f32, bounds: &mut Bounds) {
    let m = points.len();
    if m < 2 || points.iter().any(|p| !p.position.is_finite() || !p.width.is_finite()) {
        return;
    }
    let tail_width = finite(render.ribbon.tail_width);
    let tail_alpha = finite(render.ribbon.tail_alpha);
    let base = scratch.position.len() as u32;
    let across = columns(render);
    let cols = across.len() as u32;
    let mut last_dir = Vec3::Y;
    for (j, p) in points.iter().enumerate() {
        let u = j as f32 / (m - 1) as f32;
        let prev = points[j.saturating_sub(1)].position;
        let next = points[(j + 1).min(m - 1)].position;
        // Central difference: the direction at a point is the chord through
        // its neighbours, so the strip bends smoothly instead of kinking at
        // every point. A repeated point keeps the last direction.
        let dir = next - prev;
        let dir = if dir.length_squared() > 1e-16 { dir } else { last_dir };
        last_dir = dir;
        let width = p.width * (1.0 + (tail_width - 1.0) * u);
        let mut color = p.color;
        color.w *= 1.0 + (tail_alpha - 1.0) * u;
        let params = [width, 0.0, 0.0, frame];
        let extra = [seed, p.life, 0.0, 0.0];
        let y = u - 0.5;
        for &x in &across {
            scratch.position.push(p.position.to_array());
            scratch.corner.push([x, y]);
            scratch.color.push(color.to_array());
            scratch.params.push(params);
            scratch.axis.push(dir.to_array());
            scratch.extra.push(extra);
        }
        let r = 0.5 * width.abs();
        bounds.min = bounds.min.min(p.position - Vec3::splat(r));
        bounds.max = bounds.max.max(p.position + Vec3::splat(r));
    }
    for j in 0..(m as u32 - 1) {
        for c in 0..cols - 1 {
            let a = base + cols * j + c;
            let next = a + cols;
            scratch.indices.extend_from_slice(&[a, a + 1, next + 1, a, next + 1, next]);
        }
    }
    bounds.any = true;
}

/// The `corner.x` of each vertex across one point: the two sides of a flat
/// strip, or once round a tube with the first place repeated at the end.
fn columns(render: &RenderSettings) -> Vec<f32> {
    match render.ribbon.shape {
        RibbonShape::Flat => vec![-0.5, 0.5],
        RibbonShape::Tube => {
            let s = render.ribbon.tube_segments();
            (0..=s).map(|k| k as f32 / s as f32 - 0.5).collect()
        }
    }
}

fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// Vertices and indices one flat strip of `points` points takes. Public for
/// the tests, which hold the mesh to it.
pub fn strip_size(points: usize) -> (usize, usize) {
    tube_size(points, 1)
}

/// Vertices and indices one strip of `points` points takes with `segments`
/// quads across it: 1 for a flat strip, a tube's segments round.
pub fn tube_size(points: usize, segments: usize) -> (usize, usize) {
    if points < 2 {
        (0, 0)
    } else {
        (points * (segments + 1), (points - 1) * segments * 6)
    }
}
