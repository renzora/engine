//! Camera-facing ribbon mesh builder.
//!
//! Each strand's world-space centerline is expanded into a thin quad strip whose
//! width axis is turned to face the camera, so a strand reads as hair from any
//! viewing angle instead of vanishing edge-on. Rebuilt every frame from the
//! current (simulated) strand positions, using the active camera's world
//! position — cheap enough for the capped strand count, and it means the ribbon
//! orientation needs no custom shader.

use crate::generate::Strand;
use bevy::mesh::{Indices, Mesh};
use bevy::prelude::*;

/// Fill `mesh` with the ribbon geometry for `strands`, billboarded toward
/// `camera`. Overwrites all attributes (POSITION/NORMAL/UV_0/COLOR) and indices.
pub(crate) fn build_ribbons(strands: &[Strand], camera: Vec3, mesh: &mut Mesh) {
    let vert_estimate = strands.iter().map(|s| s.world.len() * 2).sum();
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vert_estimate);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vert_estimate);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(vert_estimate);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(vert_estimate);
    let mut indices: Vec<u32> = Vec::new();

    for s in strands {
        let n = s.world.len();
        if n < 2 {
            continue;
        }
        let base = positions.len() as u32;

        for i in 0..n {
            let p = s.world[i];
            // Tangent along the strand (forward difference, backward at the tip).
            let tangent = if i + 1 < n {
                s.world[i + 1] - p
            } else {
                p - s.world[i - 1]
            }
            .normalize_or_zero();

            // Width axis = perpendicular to both the strand and the view ray, so
            // the ribbon's flat face turns toward the camera.
            let view = (camera - p).normalize_or_zero();
            let mut side = tangent.cross(view);
            if side.length_squared() < 1e-10 {
                // Strand pointing straight at the camera — pick any perpendicular.
                side = tangent.cross(Vec3::Y);
                if side.length_squared() < 1e-10 {
                    side = tangent.cross(Vec3::X);
                }
            }
            let side = side.normalize_or_zero();
            // Normal faces the camera (≈ view), so PBR lighting catches the strand.
            let normal = side.cross(tangent).normalize_or_zero();

            let t01 = i as f32 / (n - 1) as f32;
            let half_w = s.half_width * (1.0 - 0.85 * t01); // taper to the tip
            let l = p - side * half_w;
            let r = p + side * half_w;

            positions.push(l.to_array());
            positions.push(r.to_array());
            normals.push(normal.to_array());
            normals.push(normal.to_array());
            uvs.push([0.0, t01]);
            uvs.push([1.0, t01]);
            // Slight root→tip darkening + per-strand shade, via vertex color.
            let shade = (0.65 + 0.35 * s.shade) * (1.0 - 0.25 * t01);
            let c = [shade, shade, shade, 1.0];
            colors.push(c);
            colors.push(c);
        }

        // Two triangles per segment between consecutive left/right vertex pairs.
        for i in 0..(n - 1) as u32 {
            let a = base + i * 2; // left  i
            let b = a + 1; // right i
            let c = a + 2; // left  i+1
            let d = a + 3; // right i+1
            indices.extend_from_slice(&[a, b, c, c, b, d]);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}
