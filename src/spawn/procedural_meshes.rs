//! Procedural mesh generators for shapes not provided by Bevy natively

use bevy::prelude::*;
use bevy::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::PI;

/// Ensure all triangles have correct CCW winding relative to their vertex normals.
/// For each triangle, compute the geometric face normal via cross product, compare
/// to the stored vertex normal, and swap winding if they disagree.
fn ensure_correct_winding(positions: &[[f32; 3]], normals: &[[f32; 3]], indices: &mut [u32]) {
    for tri in indices.chunks_mut(3) {
        let a = Vec3::from(positions[tri[0] as usize]);
        let b = Vec3::from(positions[tri[1] as usize]);
        let c = Vec3::from(positions[tri[2] as usize]);
        let face_normal = (b - a).cross(c - a);
        let vertex_normal = Vec3::from(normals[tri[0] as usize]);
        if face_normal.dot(vertex_normal) < 0.0 {
            tri.swap(1, 2);
        }
    }
}

/// Helper: add a quad face with 4 corners and a normal.
/// Corners should roughly go around the face; winding is auto-corrected.
fn add_quad(
    positions: &mut Vec<[f32; 3]>,
    normals_out: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    corners: [[f32; 3]; 4],
    normal: [f32; 3],
) {
    let base = positions.len() as u32;
    positions.extend_from_slice(&corners);
    normals_out.extend_from_slice(&[normal; 4]);
    uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Helper: add a triangle face with 3 corners and a normal.
fn add_tri(
    positions: &mut Vec<[f32; 3]>,
    normals_out: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    corners: [[f32; 3]; 3],
    normal: [f32; 3],
    tri_uvs: [[f32; 2]; 3],
) {
    let base = positions.len() as u32;
    positions.extend_from_slice(&corners);
    normals_out.extend_from_slice(&[normal; 3]);
    uvs.extend_from_slice(&tri_uvs);
    indices.extend_from_slice(&[base, base + 1, base + 2]);
}

fn build_mesh(
    mut positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    mut indices: Vec<u32>,
    center_offset: Option<[f32; 3]>,
) -> Mesh {
    // Fix any incorrect winding
    ensure_correct_winding(&positions, &normals, &mut indices);

    // Apply centering offset
    if let Some(off) = center_offset {
        for p in positions.iter_mut() {
            p[0] -= off[0];
            p[1] -= off[1];
            p[2] -= off[2];
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create a wedge/ramp mesh (right-triangle cross-section, 1x1x1)
pub fn create_wedge_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Bottom face (Y=0), normal -Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
        [0.0, -1.0, 0.0]);

    // Back face (X=0, vertical), normal -X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0]],
        [-1.0, 0.0, 0.0]);

    // Slope face (hypotenuse)
    let slope_n = Vec3::new(1.0, 1.0, 0.0).normalize();
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0]],
        [slope_n.x, slope_n.y, slope_n.z]);

    // Front triangle (Z=0), normal -Z
    add_tri(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        [0.0, 0.0, -1.0],
        [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);

    // Back triangle (Z=1), normal +Z
    add_tri(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0]],
        [0.0, 0.0, 1.0],
        [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0]]);

    build_mesh(positions, normals, uvs, indices, Some([0.5, 0.5, 0.5]))
}

/// Create a staircase mesh with configurable step count
pub fn create_stairs_mesh(steps: u32) -> Mesh {
    let steps = steps.max(2);
    let step_h = 1.0 / steps as f32;
    let step_d = 1.0 / steps as f32;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for i in 0..steps {
        let yb = i as f32 * step_h;
        let yt = (i + 1) as f32 * step_h;
        let zf = i as f32 * step_d;
        let zb = (i + 1) as f32 * step_d;

        // Top face of step, normal +Y
        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[0.0, yt, zf], [1.0, yt, zf], [1.0, yt, zb], [0.0, yt, zb]],
            [0.0, 1.0, 0.0]);

        // Front face of step (riser), normal -Z
        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[0.0, yb, zf], [1.0, yb, zf], [1.0, yt, zf], [0.0, yt, zf]],
            [0.0, 0.0, -1.0]);
    }

    // Bottom face, normal -Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
        [0.0, -1.0, 0.0]);

    // Left side (X=0), normal -X — stepped profile
    for i in 0..steps {
        let yb = i as f32 * step_h;
        let yt = (i + 1) as f32 * step_h;
        let zf = i as f32 * step_d;
        let zb = (i + 1) as f32 * step_d;

        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[0.0, yb, zf], [0.0, yb, zb], [0.0, yt, zb], [0.0, yt, zf]],
            [-1.0, 0.0, 0.0]);
    }

    // Right side (X=1), normal +X — stepped profile
    for i in 0..steps {
        let yb = i as f32 * step_h;
        let yt = (i + 1) as f32 * step_h;
        let zf = i as f32 * step_d;
        let zb = (i + 1) as f32 * step_d;

        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[1.0, yb, zf], [1.0, yt, zf], [1.0, yt, zb], [1.0, yb, zb]],
            [1.0, 0.0, 0.0]);
    }

    // Back face (Z=1), normal +Z
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0]],
        [0.0, 0.0, 1.0]);

    build_mesh(positions, normals, uvs, indices, Some([0.5, 0.5, 0.5]))
}

/// Create a half-torus arch mesh
pub fn create_arch_mesh(segments: u32) -> Mesh {
    let segments = segments.max(8);
    let tube_radius = 0.15;
    let arch_radius = 0.5;
    let tube_segments = 8u32;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for i in 0..=segments {
        let angle = PI * i as f32 / segments as f32;
        let center = Vec3::new(arch_radius * angle.cos(), arch_radius * angle.sin(), 0.0);

        for j in 0..=tube_segments {
            let tube_angle = 2.0 * PI * j as f32 / tube_segments as f32;
            let normal = Vec3::new(
                angle.cos() * tube_angle.cos(),
                angle.sin() * tube_angle.cos(),
                tube_angle.sin(),
            );

            let pos = center + normal * tube_radius;
            positions.push([pos.x, pos.y, pos.z]);
            normals.push([normal.x, normal.y, normal.z]);
            uvs.push([i as f32 / segments as f32, j as f32 / tube_segments as f32]);
        }
    }

    for i in 0..segments {
        for j in 0..tube_segments {
            let a = i * (tube_segments + 1) + j;
            let b = a + tube_segments + 1;
            indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
        }
    }

    // Fix winding and build
    ensure_correct_winding(&positions, &normals, &mut indices);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create a half-cylinder mesh (cut lengthwise)
pub fn create_half_cylinder_mesh(segments: u32) -> Mesh {
    let segments = segments.max(8);
    let radius = 0.5;
    let height = 1.0;
    let half_h = height / 2.0;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Curved surface (half circle, 0 to PI)
    for i in 0..=segments {
        let angle = PI * i as f32 / segments as f32;
        let x = radius * angle.cos();
        let z = radius * angle.sin();
        let nx = angle.cos();
        let nz = angle.sin();

        positions.push([x, -half_h, z]);
        normals.push([nx, 0.0, nz]);
        uvs.push([i as f32 / segments as f32, 0.0]);

        positions.push([x, half_h, z]);
        normals.push([nx, 0.0, nz]);
        uvs.push([i as f32 / segments as f32, 1.0]);
    }

    for i in 0..segments {
        let a = i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
    }

    // Flat face (the cut plane, facing -Z since half-cylinder is the +Z side)
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[radius, -half_h, 0.0], [-radius, -half_h, 0.0],
         [-radius, half_h, 0.0], [radius, half_h, 0.0]],
        [0.0, 0.0, -1.0]);

    // Top cap (half circle at Y=+half_h), normal +Y
    let center_top = positions.len() as u32;
    positions.push([0.0, half_h, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);
    for i in 0..=segments {
        let angle = PI * i as f32 / segments as f32;
        positions.push([radius * angle.cos(), half_h, radius * angle.sin()]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([(angle.cos() + 1.0) / 2.0, (angle.sin() + 1.0) / 2.0]);
    }
    for i in 0..segments {
        indices.extend_from_slice(&[center_top, center_top + 1 + i, center_top + 2 + i]);
    }

    // Bottom cap (half circle at Y=-half_h), normal -Y
    let center_bot = positions.len() as u32;
    positions.push([0.0, -half_h, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 0.5]);
    for i in 0..=segments {
        let angle = PI * i as f32 / segments as f32;
        positions.push([radius * angle.cos(), -half_h, radius * angle.sin()]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([(angle.cos() + 1.0) / 2.0, (angle.sin() + 1.0) / 2.0]);
    }
    for i in 0..segments {
        indices.extend_from_slice(&[center_bot, center_bot + 2 + i, center_bot + 1 + i]);
    }

    ensure_correct_winding(&positions, &normals, &mut indices);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create a quarter-pipe (quarter cylinder) mesh
pub fn create_quarter_pipe_mesh(segments: u32) -> Mesh {
    let segments = segments.max(8);
    let radius = 1.0;
    let width = 1.0;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Curved surface (quarter circle from Y-axis to Z-axis)
    for i in 0..=segments {
        let angle = (PI / 2.0) * i as f32 / segments as f32;
        let y = radius * angle.cos();
        let z = radius * angle.sin();
        let ny = angle.cos();
        let nz = angle.sin();

        positions.push([0.0, y, z]);
        normals.push([0.0, ny, nz]);
        uvs.push([0.0, i as f32 / segments as f32]);

        positions.push([width, y, z]);
        normals.push([0.0, ny, nz]);
        uvs.push([1.0, i as f32 / segments as f32]);
    }

    for i in 0..segments {
        let a = i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
    }

    // Bottom face (Y=0), normal -Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [width, 0.0, 0.0], [width, 0.0, radius], [0.0, 0.0, radius]],
        [0.0, -1.0, 0.0]);

    // Back face (Z=0), normal -Z
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [width, 0.0, 0.0], [width, radius, 0.0], [0.0, radius, 0.0]],
        [0.0, 0.0, -1.0]);

    // Left side (X=0) — quarter-circle fan, normal -X
    let base = positions.len() as u32;
    positions.push([0.0, 0.0, 0.0]);
    normals.push([-1.0, 0.0, 0.0]);
    uvs.push([0.0, 0.0]);
    for i in 0..=segments {
        let angle = (PI / 2.0) * i as f32 / segments as f32;
        positions.push([0.0, radius * angle.cos(), radius * angle.sin()]);
        normals.push([-1.0, 0.0, 0.0]);
        uvs.push([angle.sin(), angle.cos()]);
    }
    for i in 0..segments {
        indices.extend_from_slice(&[base, base + 1 + i, base + 2 + i]);
    }

    // Right side (X=width) — quarter-circle fan, normal +X
    let base = positions.len() as u32;
    positions.push([width, 0.0, 0.0]);
    normals.push([1.0, 0.0, 0.0]);
    uvs.push([0.0, 0.0]);
    for i in 0..=segments {
        let angle = (PI / 2.0) * i as f32 / segments as f32;
        positions.push([width, radius * angle.cos(), radius * angle.sin()]);
        normals.push([1.0, 0.0, 0.0]);
        uvs.push([angle.sin(), angle.cos()]);
    }
    for i in 0..segments {
        indices.extend_from_slice(&[base, base + 1 + i, base + 2 + i]);
    }

    build_mesh(positions, normals, uvs, indices, Some([width / 2.0, 0.5, 0.5]))
}

/// Create an L-shaped corner piece mesh
pub fn create_corner_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let w = 0.3_f32; // wall thickness
    let d = 0.3_f32; // depth (z)

    // L-profile vertices (2D, XY plane):
    //  (0,0) -> (1,0) -> (1,w) -> (w,w) -> (w,1) -> (0,1)
    let profile = [
        [0.0, 0.0], [1.0, 0.0], [1.0, w], [w, w], [w, 1.0], [0.0, 1.0],
    ];

    // Front face (Z=d), normal +Z
    let base = positions.len() as u32;
    for p in &profile {
        positions.push([p[0], p[1], d]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([p[0], p[1]]);
    }
    // Fan triangulation
    indices.extend_from_slice(&[base, base + 1, base + 2]);
    indices.extend_from_slice(&[base, base + 2, base + 3]);
    indices.extend_from_slice(&[base, base + 3, base + 4]);
    indices.extend_from_slice(&[base, base + 4, base + 5]);

    // Back face (Z=0), normal -Z (reversed winding)
    let base = positions.len() as u32;
    for p in &profile {
        positions.push([p[0], p[1], 0.0]);
        normals.push([0.0, 0.0, -1.0]);
        uvs.push([p[0], p[1]]);
    }
    indices.extend_from_slice(&[base, base + 2, base + 1]);
    indices.extend_from_slice(&[base, base + 3, base + 2]);
    indices.extend_from_slice(&[base, base + 4, base + 3]);
    indices.extend_from_slice(&[base, base + 5, base + 4]);

    // Side faces (extrusion edges)
    let edge_count = profile.len();
    for i in 0..edge_count {
        let next = (i + 1) % edge_count;
        let p0 = profile[i];
        let p1 = profile[next];

        let dx = p1[0] - p0[0];
        let dy = p1[1] - p0[1];
        let len = (dx * dx + dy * dy).sqrt();
        let nx = dy / len;
        let ny = -dx / len;

        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[p0[0], p0[1], 0.0], [p1[0], p1[1], 0.0],
             [p1[0], p1[1], d],   [p0[0], p0[1], d]],
            [nx, ny, 0.0]);
    }

    build_mesh(positions, normals, uvs, indices, Some([0.5, 0.5, d / 2.0]))
}

/// Create a triangular prism mesh
pub fn create_prism_mesh() -> Mesh {
    let h = 1.0_f32;
    let half = 0.5_f32;
    let sqrt3_4 = (3.0_f32).sqrt() / 4.0;
    let tri = [
        [-half, 0.0, -sqrt3_4],
        [half, 0.0, -sqrt3_4],
        [0.0, 0.0, sqrt3_4],
    ];

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Bottom face (Y=0), normal -Y
    add_tri(&mut positions, &mut normals, &mut uvs, &mut indices,
        [tri[0], tri[2], tri[1]],
        [0.0, -1.0, 0.0],
        [[tri[0][0] + 0.5, tri[0][2] + 0.5], [tri[2][0] + 0.5, tri[2][2] + 0.5], [tri[1][0] + 0.5, tri[1][2] + 0.5]]);

    // Top face (Y=h), normal +Y
    add_tri(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[tri[0][0], h, tri[0][2]], [tri[1][0], h, tri[1][2]], [tri[2][0], h, tri[2][2]]],
        [0.0, 1.0, 0.0],
        [[tri[0][0] + 0.5, tri[0][2] + 0.5], [tri[1][0] + 0.5, tri[1][2] + 0.5], [tri[2][0] + 0.5, tri[2][2] + 0.5]]);

    // Side faces
    for i in 0..3 {
        let next = (i + 1) % 3;
        let p0 = tri[i];
        let p1 = tri[next];

        let dx = p1[0] - p0[0];
        let dz = p1[2] - p0[2];
        let len = (dx * dx + dz * dz).sqrt();
        let nx = dz / len;
        let nz = -dx / len;

        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[p0[0], 0.0, p0[2]], [p1[0], 0.0, p1[2]],
             [p1[0], h, p1[2]],   [p0[0], h, p0[2]]],
            [nx, 0.0, nz]);
    }

    build_mesh(positions, normals, uvs, indices, Some([0.0, h / 2.0, 0.0]))
}

/// Create a 4-sided pyramid mesh
pub fn create_pyramid_mesh() -> Mesh {
    let half = 0.5_f32;
    let h = 1.0_f32;
    let apex = [0.0, h, 0.0];
    let base_verts = [
        [-half, 0.0, -half],
        [half, 0.0, -half],
        [half, 0.0, half],
        [-half, 0.0, half],
    ];

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Base face, normal -Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [base_verts[0], base_verts[3], base_verts[2], base_verts[1]],
        [0.0, -1.0, 0.0]);

    // Side faces — compute proper normals
    for i in 0..4 {
        let next = (i + 1) % 4;
        let v0 = Vec3::from(base_verts[i]);
        let v1 = Vec3::from(base_verts[next]);
        let va = Vec3::from(apex);
        let edge = v1 - v0;
        let to_apex = va - v0;
        let n = edge.cross(to_apex).normalize();

        add_tri(&mut positions, &mut normals, &mut uvs, &mut indices,
            [base_verts[i], base_verts[next], apex],
            [n.x, n.y, n.z],
            [[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]]);
    }

    build_mesh(positions, normals, uvs, indices, Some([0.0, h / 2.0, 0.0]))
}

/// Create a hollow cylinder (pipe) mesh
pub fn create_pipe_mesh(segments: u32) -> Mesh {
    let segments = segments.max(12);
    let outer_r = 0.5;
    let inner_r = 0.35;
    let half_h = 0.5;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Outer surface — normals point outward
    for i in 0..=segments {
        let angle = 2.0 * PI * i as f32 / segments as f32;
        let (c, s) = (angle.cos(), angle.sin());
        let u = i as f32 / segments as f32;

        positions.push([outer_r * c, -half_h, outer_r * s]);
        normals.push([c, 0.0, s]);
        uvs.push([u, 0.0]);

        positions.push([outer_r * c, half_h, outer_r * s]);
        normals.push([c, 0.0, s]);
        uvs.push([u, 1.0]);
    }
    for i in 0..segments {
        let a = i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
    }

    // Inner surface — normals point inward
    let inner_base = positions.len() as u32;
    for i in 0..=segments {
        let angle = 2.0 * PI * i as f32 / segments as f32;
        let (c, s) = (angle.cos(), angle.sin());
        let u = i as f32 / segments as f32;

        positions.push([inner_r * c, -half_h, inner_r * s]);
        normals.push([-c, 0.0, -s]);
        uvs.push([u, 0.0]);

        positions.push([inner_r * c, half_h, inner_r * s]);
        normals.push([-c, 0.0, -s]);
        uvs.push([u, 1.0]);
    }
    for i in 0..segments {
        let a = inner_base + i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
    }

    // Top and bottom ring caps
    for (y, ny) in [(-half_h, -1.0_f32), (half_h, 1.0_f32)] {
        let ring_base = positions.len() as u32;
        for i in 0..=segments {
            let angle = 2.0 * PI * i as f32 / segments as f32;
            let (c, s) = (angle.cos(), angle.sin());

            positions.push([outer_r * c, y, outer_r * s]);
            normals.push([0.0, ny, 0.0]);
            uvs.push([(c + 1.0) / 2.0, (s + 1.0) / 2.0]);

            positions.push([inner_r * c, y, inner_r * s]);
            normals.push([0.0, ny, 0.0]);
            uvs.push([(inner_r / outer_r * c + 1.0) / 2.0, (inner_r / outer_r * s + 1.0) / 2.0]);
        }

        for i in 0..segments {
            let a = ring_base + i * 2;
            let b = a + 2;
            if ny > 0.0 {
                indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
            } else {
                indices.extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
            }
        }
    }

    ensure_correct_winding(&positions, &normals, &mut indices);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create a flat ring/washer mesh
pub fn create_ring_mesh(segments: u32) -> Mesh {
    let segments = segments.max(12);
    let outer_r = 0.5;
    let inner_r = 0.3;
    let half_t = 0.05;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Top and bottom faces
    for (y, ny) in [(-half_t, -1.0_f32), (half_t, 1.0_f32)] {
        let ring_base = positions.len() as u32;
        for i in 0..=segments {
            let angle = 2.0 * PI * i as f32 / segments as f32;
            let (c, s) = (angle.cos(), angle.sin());

            positions.push([outer_r * c, y, outer_r * s]);
            normals.push([0.0, ny, 0.0]);
            uvs.push([(c + 1.0) / 2.0, (s + 1.0) / 2.0]);

            positions.push([inner_r * c, y, inner_r * s]);
            normals.push([0.0, ny, 0.0]);
            uvs.push([(inner_r / outer_r * c + 1.0) / 2.0, (inner_r / outer_r * s + 1.0) / 2.0]);
        }

        for i in 0..segments {
            let a = ring_base + i * 2;
            let b = a + 2;
            if ny > 0.0 {
                indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
            } else {
                indices.extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
            }
        }
    }

    // Outer edge
    let outer_base = positions.len() as u32;
    for i in 0..=segments {
        let angle = 2.0 * PI * i as f32 / segments as f32;
        let (c, s) = (angle.cos(), angle.sin());

        positions.push([outer_r * c, -half_t, outer_r * s]);
        normals.push([c, 0.0, s]);
        uvs.push([i as f32 / segments as f32, 0.0]);

        positions.push([outer_r * c, half_t, outer_r * s]);
        normals.push([c, 0.0, s]);
        uvs.push([i as f32 / segments as f32, 1.0]);
    }
    for i in 0..segments {
        let a = outer_base + i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
    }

    // Inner edge
    let inner_base = positions.len() as u32;
    for i in 0..=segments {
        let angle = 2.0 * PI * i as f32 / segments as f32;
        let (c, s) = (angle.cos(), angle.sin());

        positions.push([inner_r * c, -half_t, inner_r * s]);
        normals.push([-c, 0.0, -s]);
        uvs.push([i as f32 / segments as f32, 0.0]);

        positions.push([inner_r * c, half_t, inner_r * s]);
        normals.push([-c, 0.0, -s]);
        uvs.push([i as f32 / segments as f32, 1.0]);
    }
    for i in 0..segments {
        let a = inner_base + i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
    }

    ensure_correct_winding(&positions, &normals, &mut indices);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create an elongated wedge/ramp mesh (2.0 wide x 0.5 tall x 1.0 deep)
pub fn create_ramp_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let w = 2.0_f32;
    let h = 0.5_f32;
    let d = 1.0_f32;

    // Bottom face (Y=0), normal -Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [w, 0.0, 0.0], [w, 0.0, d], [0.0, 0.0, d]],
        [0.0, -1.0, 0.0]);

    // Back face (X=0, vertical), normal -X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [0.0, 0.0, d], [0.0, h, d], [0.0, h, 0.0]],
        [-1.0, 0.0, 0.0]);

    // Slope face (hypotenuse from X=w,Y=0 up to X=0,Y=h)
    let slope_n = Vec3::new(h, w, 0.0).normalize();
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[w, 0.0, 0.0], [w, 0.0, d], [0.0, h, d], [0.0, h, 0.0]],
        [slope_n.x, slope_n.y, slope_n.z]);

    // Front triangle (Z=0), normal -Z
    add_tri(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [w, 0.0, 0.0], [0.0, h, 0.0]],
        [0.0, 0.0, -1.0],
        [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);

    // Back triangle (Z=d), normal +Z
    add_tri(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, d], [0.0, h, d], [w, 0.0, d]],
        [0.0, 0.0, 1.0],
        [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0]]);

    build_mesh(positions, normals, uvs, indices, Some([1.0, 0.25, 0.5]))
}

/// Create a hemisphere (top half of a sphere) mesh
pub fn create_hemisphere_mesh(segments: u32) -> Mesh {
    let segments = segments.max(8);
    let radius = 0.5;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Generate vertices for the hemisphere surface (theta from 0 at top to PI/2 at equator)
    for lat in 0..=segments {
        let theta = (PI / 2.0) * lat as f32 / segments as f32;
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=segments {
            let phi = 2.0 * PI * lon as f32 / segments as f32;
            let x = radius * sin_theta * phi.cos();
            let z = radius * sin_theta * phi.sin();
            let y = radius * cos_theta;

            let nx = sin_theta * phi.cos();
            let ny = cos_theta;
            let nz = sin_theta * phi.sin();

            positions.push([x, y, z]);
            normals.push([nx, ny, nz]);
            uvs.push([lon as f32 / segments as f32, lat as f32 / segments as f32]);
        }
    }

    let ring_size = segments + 1;
    for lat in 0..segments {
        for lon in 0..segments {
            let a = lat * ring_size + lon;
            let b = a + ring_size;
            indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
        }
    }

    // Bottom cap disc (flat circle at Y=0), normal -Y
    let center_idx = positions.len() as u32;
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for i in 0..=segments {
        let phi = 2.0 * PI * i as f32 / segments as f32;
        let x = radius * phi.cos();
        let z = radius * phi.sin();
        positions.push([x, 0.0, z]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([(phi.cos() + 1.0) / 2.0, (phi.sin() + 1.0) / 2.0]);
    }

    for i in 0..segments {
        indices.extend_from_slice(&[center_idx, center_idx + 2 + i, center_idx + 1 + i]);
    }

    ensure_correct_winding(&positions, &normals, &mut indices);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create a curved wall mesh (90-degree arc, outer radius 1.0, inner radius 0.9, height 1.0)
pub fn create_curved_wall_mesh(segments: u32) -> Mesh {
    let segments = segments.max(8);
    let outer_r = 1.0_f32;
    let inner_r = 0.9_f32;
    let height = 1.0_f32;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Outer surface (normals point outward)
    for i in 0..=segments {
        let angle = (PI / 2.0) * i as f32 / segments as f32;
        let c = angle.cos();
        let s = angle.sin();
        let u = i as f32 / segments as f32;

        positions.push([outer_r * c, 0.0, outer_r * s]);
        normals.push([c, 0.0, s]);
        uvs.push([u, 0.0]);

        positions.push([outer_r * c, height, outer_r * s]);
        normals.push([c, 0.0, s]);
        uvs.push([u, 1.0]);
    }
    for i in 0..segments {
        let a = i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
    }

    // Inner surface (normals point inward)
    let inner_base = positions.len() as u32;
    for i in 0..=segments {
        let angle = (PI / 2.0) * i as f32 / segments as f32;
        let c = angle.cos();
        let s = angle.sin();
        let u = i as f32 / segments as f32;

        positions.push([inner_r * c, 0.0, inner_r * s]);
        normals.push([-c, 0.0, -s]);
        uvs.push([u, 0.0]);

        positions.push([inner_r * c, height, inner_r * s]);
        normals.push([-c, 0.0, -s]);
        uvs.push([u, 1.0]);
    }
    for i in 0..segments {
        let a = inner_base + i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
    }

    // Top cap ring (Y=height), normal +Y
    let top_base = positions.len() as u32;
    for i in 0..=segments {
        let angle = (PI / 2.0) * i as f32 / segments as f32;
        let c = angle.cos();
        let s = angle.sin();

        positions.push([outer_r * c, height, outer_r * s]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([outer_r * c, outer_r * s]);

        positions.push([inner_r * c, height, inner_r * s]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([inner_r * c, inner_r * s]);
    }
    for i in 0..segments {
        let a = top_base + i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
    }

    // Bottom cap ring (Y=0), normal -Y
    let bot_base = positions.len() as u32;
    for i in 0..=segments {
        let angle = (PI / 2.0) * i as f32 / segments as f32;
        let c = angle.cos();
        let s = angle.sin();

        positions.push([outer_r * c, 0.0, outer_r * s]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([outer_r * c, outer_r * s]);

        positions.push([inner_r * c, 0.0, inner_r * s]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([inner_r * c, inner_r * s]);
    }
    for i in 0..segments {
        let a = bot_base + i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
    }

    // End cap at angle=0 (flat face), normal -Z
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[outer_r, 0.0, 0.0], [inner_r, 0.0, 0.0],
         [inner_r, height, 0.0], [outer_r, height, 0.0]],
        [0.0, 0.0, -1.0]);

    // End cap at angle=PI/2 (flat face), normal -X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, inner_r], [0.0, 0.0, outer_r],
         [0.0, height, outer_r], [0.0, height, inner_r]],
        [-1.0, 0.0, 0.0]);

    ensure_correct_winding(&positions, &normals, &mut indices);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create a wall with a rectangular door opening
/// Wall: 1.5 wide x 2.0 tall x 0.1 deep
/// Door opening: 0.6 wide x 1.4 tall, centered horizontally at bottom
pub fn create_doorway_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let w = 1.5_f32;    // wall width
    let h = 2.0_f32;    // wall height
    let d = 0.1_f32;    // wall depth
    let dw = 0.6_f32;   // door width
    let dh = 1.4_f32;   // door height
    let dl = (w - dw) / 2.0; // door left edge
    let dr = dl + dw;        // door right edge

    // Front face (Z=d), normal +Z
    // Left strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, d], [dl, 0.0, d], [dl, h, d], [0.0, h, d]],
        [0.0, 0.0, 1.0]);
    // Right strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[dr, 0.0, d], [w, 0.0, d], [w, h, d], [dr, h, d]],
        [0.0, 0.0, 1.0]);
    // Top strip (above door)
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[dl, dh, d], [dr, dh, d], [dr, h, d], [dl, h, d]],
        [0.0, 0.0, 1.0]);

    // Back face (Z=0), normal -Z
    // Left strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[dl, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, h, 0.0], [dl, h, 0.0]],
        [0.0, 0.0, -1.0]);
    // Right strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[w, 0.0, 0.0], [dr, 0.0, 0.0], [dr, h, 0.0], [w, h, 0.0]],
        [0.0, 0.0, -1.0]);
    // Top strip (above door)
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[dr, dh, 0.0], [dl, dh, 0.0], [dl, h, 0.0], [dr, h, 0.0]],
        [0.0, 0.0, -1.0]);

    // Top face (Y=h), normal +Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, h, 0.0], [w, h, 0.0], [w, h, d], [0.0, h, d]],
        [0.0, 1.0, 0.0]);

    // Bottom face left (Y=0), normal -Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [dl, 0.0, 0.0], [dl, 0.0, d], [0.0, 0.0, d]],
        [0.0, -1.0, 0.0]);
    // Bottom face right
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[dr, 0.0, 0.0], [w, 0.0, 0.0], [w, 0.0, d], [dr, 0.0, d]],
        [0.0, -1.0, 0.0]);

    // Left side (X=0), normal -X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [0.0, 0.0, d], [0.0, h, d], [0.0, h, 0.0]],
        [-1.0, 0.0, 0.0]);

    // Right side (X=w), normal +X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[w, 0.0, d], [w, 0.0, 0.0], [w, h, 0.0], [w, h, d]],
        [1.0, 0.0, 0.0]);

    // Door opening inner faces
    // Left jamb (X=dl), normal -X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[dl, 0.0, d], [dl, 0.0, 0.0], [dl, dh, 0.0], [dl, dh, d]],
        [-1.0, 0.0, 0.0]);
    // Right jamb (X=dr), normal +X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[dr, 0.0, 0.0], [dr, 0.0, d], [dr, dh, d], [dr, dh, 0.0]],
        [1.0, 0.0, 0.0]);
    // Top of door opening (Y=dh), normal -Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[dl, dh, d], [dl, dh, 0.0], [dr, dh, 0.0], [dr, dh, d]],
        [0.0, -1.0, 0.0]);

    build_mesh(positions, normals, uvs, indices, Some([w / 2.0, h / 2.0, d / 2.0]))
}

/// Create a wall with a rectangular window opening
/// Wall: 1.5 wide x 2.0 tall x 0.1 deep
/// Window: 0.6 wide x 0.5 tall, centered at height 1.1
pub fn create_window_wall_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let w = 1.5_f32;     // wall width
    let h = 2.0_f32;     // wall height
    let d = 0.1_f32;     // wall depth
    let ww = 0.6_f32;    // window width
    let wh = 0.5_f32;    // window height
    let wc = 1.1_f32;    // window center height
    let wl = (w - ww) / 2.0;  // window left
    let wr = wl + ww;         // window right
    let wb = wc - wh / 2.0;   // window bottom
    let wt = wc + wh / 2.0;   // window top

    // Front face (Z=d), normal +Z
    // Left strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, d], [wl, 0.0, d], [wl, h, d], [0.0, h, d]],
        [0.0, 0.0, 1.0]);
    // Right strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wr, 0.0, d], [w, 0.0, d], [w, h, d], [wr, h, d]],
        [0.0, 0.0, 1.0]);
    // Bottom strip (below window, between left/right strips)
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wl, 0.0, d], [wr, 0.0, d], [wr, wb, d], [wl, wb, d]],
        [0.0, 0.0, 1.0]);
    // Top strip (above window, between left/right strips)
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wl, wt, d], [wr, wt, d], [wr, h, d], [wl, h, d]],
        [0.0, 0.0, 1.0]);

    // Back face (Z=0), normal -Z
    // Left strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wl, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, h, 0.0], [wl, h, 0.0]],
        [0.0, 0.0, -1.0]);
    // Right strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[w, 0.0, 0.0], [wr, 0.0, 0.0], [wr, h, 0.0], [w, h, 0.0]],
        [0.0, 0.0, -1.0]);
    // Bottom strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wr, 0.0, 0.0], [wl, 0.0, 0.0], [wl, wb, 0.0], [wr, wb, 0.0]],
        [0.0, 0.0, -1.0]);
    // Top strip
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wr, wt, 0.0], [wl, wt, 0.0], [wl, h, 0.0], [wr, h, 0.0]],
        [0.0, 0.0, -1.0]);

    // Top face (Y=h), normal +Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, h, 0.0], [w, h, 0.0], [w, h, d], [0.0, h, d]],
        [0.0, 1.0, 0.0]);

    // Bottom face (Y=0), normal -Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [w, 0.0, 0.0], [w, 0.0, d], [0.0, 0.0, d]],
        [0.0, -1.0, 0.0]);

    // Left side (X=0), normal -X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[0.0, 0.0, 0.0], [0.0, 0.0, d], [0.0, h, d], [0.0, h, 0.0]],
        [-1.0, 0.0, 0.0]);

    // Right side (X=w), normal +X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[w, 0.0, d], [w, 0.0, 0.0], [w, h, 0.0], [w, h, d]],
        [1.0, 0.0, 0.0]);

    // Window opening inner faces
    // Left sill (X=wl), normal -X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wl, wb, d], [wl, wb, 0.0], [wl, wt, 0.0], [wl, wt, d]],
        [-1.0, 0.0, 0.0]);
    // Right sill (X=wr), normal +X
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wr, wb, 0.0], [wr, wb, d], [wr, wt, d], [wr, wt, 0.0]],
        [1.0, 0.0, 0.0]);
    // Bottom sill (Y=wb), normal -Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wl, wb, d], [wl, wb, 0.0], [wr, wb, 0.0], [wr, wb, d]],
        [0.0, -1.0, 0.0]);
    // Top sill (Y=wt), normal +Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[wl, wt, 0.0], [wl, wt, d], [wr, wt, d], [wr, wt, 0.0]],
        [0.0, 1.0, 0.0]);

    build_mesh(positions, normals, uvs, indices, Some([w / 2.0, h / 2.0, d / 2.0]))
}

/// Create an L-shaped floor block mesh
/// Profile: full bottom 1.0x1.0 plus vertical arm 0.3 wide x 1.0 tall on left
/// Extruded 0.3 along Z
pub fn create_l_shape_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let d = 0.3_f32; // extrusion depth

    // L-profile vertices (2D, XY plane):
    // Bottom-left (0,0) -> right (1,0) -> step up (1,0.3) -> inner corner (0.3,0.3) -> up (0.3,1) -> top-left (0,1)
    let profile = [
        [0.0, 0.0], [1.0, 0.0], [1.0, 0.3], [0.3, 0.3], [0.3, 1.0], [0.0, 1.0],
    ];

    // Front face (Z=d), normal +Z
    let base = positions.len() as u32;
    for p in &profile {
        positions.push([p[0], p[1], d]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([p[0], p[1]]);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2]);
    indices.extend_from_slice(&[base, base + 2, base + 3]);
    indices.extend_from_slice(&[base, base + 3, base + 4]);
    indices.extend_from_slice(&[base, base + 4, base + 5]);

    // Back face (Z=0), normal -Z
    let base = positions.len() as u32;
    for p in &profile {
        positions.push([p[0], p[1], 0.0]);
        normals.push([0.0, 0.0, -1.0]);
        uvs.push([p[0], p[1]]);
    }
    indices.extend_from_slice(&[base, base + 2, base + 1]);
    indices.extend_from_slice(&[base, base + 3, base + 2]);
    indices.extend_from_slice(&[base, base + 4, base + 3]);
    indices.extend_from_slice(&[base, base + 5, base + 4]);

    // Side faces (extrusion edges)
    let edge_count = profile.len();
    for i in 0..edge_count {
        let next = (i + 1) % edge_count;
        let p0 = profile[i];
        let p1 = profile[next];

        let dx = p1[0] - p0[0];
        let dy = p1[1] - p0[1];
        let len = (dx * dx + dy * dy).sqrt();
        let nx = dy / len;
        let ny = -dx / len;

        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[p0[0], p0[1], 0.0], [p1[0], p1[1], 0.0],
             [p1[0], p1[1], d],   [p0[0], p0[1], d]],
            [nx, ny, 0.0]);
    }

    build_mesh(positions, normals, uvs, indices, Some([0.5, 0.5, d / 2.0]))
}

/// Create a T-shaped junction mesh
/// Profile: horizontal bar 1.0 wide x 0.3 tall at top, vertical stem 0.3 wide going down 0.7 from center
/// Extruded 0.3 along Z
pub fn create_t_shape_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let d = 0.3_f32;

    // T-profile (XY plane):
    // Stem bottom-left, going clockwise
    let stem_left = (1.0 - 0.3) / 2.0;  // 0.35
    let stem_right = stem_left + 0.3;    // 0.65
    let bar_bottom = 0.7_f32;            // where the horizontal bar starts

    let profile = [
        [stem_left, 0.0],   // 0: stem bottom-left
        [stem_right, 0.0],  // 1: stem bottom-right
        [stem_right, bar_bottom], // 2: stem meets bar right
        [1.0, bar_bottom],  // 3: bar bottom-right
        [1.0, 1.0],         // 4: bar top-right
        [0.0, 1.0],         // 5: bar top-left
        [0.0, bar_bottom],  // 6: bar bottom-left
        [stem_left, bar_bottom], // 7: stem meets bar left
    ];

    // Front face (Z=d), normal +Z
    let base = positions.len() as u32;
    for p in &profile {
        positions.push([p[0], p[1], d]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([p[0], p[1]]);
    }
    // Triangulate: stem rectangle + bar rectangle
    // Stem: 0,1,2,7
    indices.extend_from_slice(&[base, base + 1, base + 2]);
    indices.extend_from_slice(&[base, base + 2, base + 7]);
    // Bar: 6,3,4,5
    indices.extend_from_slice(&[base + 6, base + 3, base + 4]);
    indices.extend_from_slice(&[base + 6, base + 4, base + 5]);

    // Back face (Z=0), normal -Z
    let base = positions.len() as u32;
    for p in &profile {
        positions.push([p[0], p[1], 0.0]);
        normals.push([0.0, 0.0, -1.0]);
        uvs.push([p[0], p[1]]);
    }
    indices.extend_from_slice(&[base, base + 2, base + 1]);
    indices.extend_from_slice(&[base, base + 7, base + 2]);
    indices.extend_from_slice(&[base + 6, base + 4, base + 3]);
    indices.extend_from_slice(&[base + 6, base + 5, base + 4]);

    // Side faces
    let edge_count = profile.len();
    for i in 0..edge_count {
        let next = (i + 1) % edge_count;
        let p0 = profile[i];
        let p1 = profile[next];

        let dx = p1[0] - p0[0];
        let dy = p1[1] - p0[1];
        let len = (dx * dx + dy * dy).sqrt();
        let nx = dy / len;
        let ny = -dx / len;

        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[p0[0], p0[1], 0.0], [p1[0], p1[1], 0.0],
             [p1[0], p1[1], d],   [p0[0], p0[1], d]],
            [nx, ny, 0.0]);
    }

    build_mesh(positions, normals, uvs, indices, Some([0.5, 0.5, d / 2.0]))
}

/// Create a plus/cross shaped block mesh
/// Profile: two 0.3-wide bars crossing at center, each 1.0 long
/// Extruded 0.3 along Z
pub fn create_cross_shape_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let d = 0.3_f32;
    let arm = 0.35_f32; // (1.0 - 0.3) / 2.0

    // Cross profile (XY plane), 12 vertices going clockwise from bottom-left of bottom arm:
    let profile = [
        [arm, 0.0],        // 0
        [arm + 0.3, 0.0],  // 1
        [arm + 0.3, arm],  // 2
        [1.0, arm],        // 3
        [1.0, arm + 0.3],  // 4
        [arm + 0.3, arm + 0.3], // 5
        [arm + 0.3, 1.0],  // 6
        [arm, 1.0],        // 7
        [arm, arm + 0.3],  // 8
        [0.0, arm + 0.3],  // 9
        [0.0, arm],        // 10
        [arm, arm],         // 11
    ];

    // Front face (Z=d), normal +Z
    let base = positions.len() as u32;
    for p in &profile {
        positions.push([p[0], p[1], d]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([p[0], p[1]]);
    }
    // Triangulate cross as 5 pieces:
    // Bottom arm: 0,1,2,11
    indices.extend_from_slice(&[base, base + 1, base + 2]);
    indices.extend_from_slice(&[base, base + 2, base + 11]);
    // Right arm: 2,3,4,5
    indices.extend_from_slice(&[base + 2, base + 3, base + 4]);
    indices.extend_from_slice(&[base + 2, base + 4, base + 5]);
    // Top arm: 5,6,7,8
    indices.extend_from_slice(&[base + 8, base + 5, base + 6]);
    indices.extend_from_slice(&[base + 8, base + 6, base + 7]);
    // Left arm: 8,9,10,11
    indices.extend_from_slice(&[base + 11, base + 8, base + 9]);
    indices.extend_from_slice(&[base + 11, base + 9, base + 10]);
    // Center: 11,2,5,8
    indices.extend_from_slice(&[base + 11, base + 2, base + 5]);
    indices.extend_from_slice(&[base + 11, base + 5, base + 8]);

    // Back face (Z=0), normal -Z
    let base = positions.len() as u32;
    for p in &profile {
        positions.push([p[0], p[1], 0.0]);
        normals.push([0.0, 0.0, -1.0]);
        uvs.push([p[0], p[1]]);
    }
    indices.extend_from_slice(&[base, base + 2, base + 1]);
    indices.extend_from_slice(&[base, base + 11, base + 2]);
    indices.extend_from_slice(&[base + 2, base + 4, base + 3]);
    indices.extend_from_slice(&[base + 2, base + 5, base + 4]);
    indices.extend_from_slice(&[base + 8, base + 6, base + 5]);
    indices.extend_from_slice(&[base + 8, base + 7, base + 6]);
    indices.extend_from_slice(&[base + 11, base + 9, base + 8]);
    indices.extend_from_slice(&[base + 11, base + 10, base + 9]);
    indices.extend_from_slice(&[base + 11, base + 5, base + 2]);
    indices.extend_from_slice(&[base + 11, base + 8, base + 5]);

    // Side faces
    let edge_count = profile.len();
    for i in 0..edge_count {
        let next = (i + 1) % edge_count;
        let p0 = profile[i];
        let p1 = profile[next];

        let dx = p1[0] - p0[0];
        let dy = p1[1] - p0[1];
        let len = (dx * dx + dy * dy).sqrt();
        let nx = dy / len;
        let ny = -dx / len;

        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[p0[0], p0[1], 0.0], [p1[0], p1[1], 0.0],
             [p1[0], p1[1], d],   [p0[0], p0[1], d]],
            [nx, ny, 0.0]);
    }

    build_mesh(positions, normals, uvs, indices, Some([0.5, 0.5, d / 2.0]))
}

/// Create a truncated cone (funnel) mesh
/// Top radius 0.5, bottom radius 0.2, height 1.0
pub fn create_funnel_mesh(segments: u32) -> Mesh {
    let segments = segments.max(12);
    let top_r = 0.5_f32;
    let bot_r = 0.2_f32;
    let height = 1.0_f32;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Side surface
    let slope_len = ((top_r - bot_r) * (top_r - bot_r) + height * height).sqrt();
    let ny_side = (top_r - bot_r) / slope_len;
    let nr_side = height / slope_len;

    for i in 0..=segments {
        let angle = 2.0 * PI * i as f32 / segments as f32;
        let c = angle.cos();
        let s = angle.sin();
        let u = i as f32 / segments as f32;

        // Bottom vertex
        positions.push([bot_r * c, 0.0, bot_r * s]);
        normals.push([nr_side * c, ny_side, nr_side * s]);
        uvs.push([u, 0.0]);

        // Top vertex
        positions.push([top_r * c, height, top_r * s]);
        normals.push([nr_side * c, ny_side, nr_side * s]);
        uvs.push([u, 1.0]);
    }

    for i in 0..segments {
        let a = i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
    }

    // Top cap disc (Y=height), normal +Y
    let top_center = positions.len() as u32;
    positions.push([0.0, height, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for i in 0..=segments {
        let angle = 2.0 * PI * i as f32 / segments as f32;
        positions.push([top_r * angle.cos(), height, top_r * angle.sin()]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([(angle.cos() + 1.0) / 2.0, (angle.sin() + 1.0) / 2.0]);
    }
    for i in 0..segments {
        indices.extend_from_slice(&[top_center, top_center + 1 + i, top_center + 2 + i]);
    }

    // Bottom cap disc (Y=0), normal -Y
    let bot_center = positions.len() as u32;
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for i in 0..=segments {
        let angle = 2.0 * PI * i as f32 / segments as f32;
        positions.push([bot_r * angle.cos(), 0.0, bot_r * angle.sin()]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([(angle.cos() + 1.0) / 2.0, (angle.sin() + 1.0) / 2.0]);
    }
    for i in 0..segments {
        indices.extend_from_slice(&[bot_center, bot_center + 2 + i, bot_center + 1 + i]);
    }

    ensure_correct_winding(&positions, &normals, &mut indices);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create a half-pipe trough/channel mesh (opening faces up)
/// Outer radius 0.5, length 1.0
pub fn create_gutter_mesh(segments: u32) -> Mesh {
    let segments = segments.max(8);
    let radius = 0.5_f32;
    let length = 1.0_f32;
    let half_l = length / 2.0;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    // Curved outer surface (bottom half of cylinder, PI to 2*PI i.e. below Y=0)
    // We go from angle PI to 2*PI so the curved part is below, opening at top
    for i in 0..=segments {
        let angle = PI + PI * i as f32 / segments as f32;
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        let nx = angle.cos();
        let ny = angle.sin();

        positions.push([x, y, -half_l]);
        normals.push([nx, ny, 0.0]);
        uvs.push([i as f32 / segments as f32, 0.0]);

        positions.push([x, y, half_l]);
        normals.push([nx, ny, 0.0]);
        uvs.push([i as f32 / segments as f32, 1.0]);
    }
    for i in 0..segments {
        let a = i * 2;
        let b = a + 2;
        indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
    }

    // Flat top face (Y=0 plane, from x=-radius to x=+radius), normal +Y
    add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
        [[-radius, 0.0, -half_l], [radius, 0.0, -half_l],
         [radius, 0.0, half_l], [-radius, 0.0, half_l]],
        [0.0, 1.0, 0.0]);

    // Front end cap (Z=-half_l), half-circle, normal -Z
    let front_center = positions.len() as u32;
    positions.push([0.0, 0.0, -half_l]);
    normals.push([0.0, 0.0, -1.0]);
    uvs.push([0.5, 0.5]);

    for i in 0..=segments {
        let angle = PI + PI * i as f32 / segments as f32;
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        positions.push([x, y, -half_l]);
        normals.push([0.0, 0.0, -1.0]);
        uvs.push([(angle.cos() + 1.0) / 2.0, (angle.sin() + 1.0) / 2.0]);
    }
    for i in 0..segments {
        indices.extend_from_slice(&[front_center, front_center + 2 + i, front_center + 1 + i]);
    }

    // Back end cap (Z=half_l), half-circle, normal +Z
    let back_center = positions.len() as u32;
    positions.push([0.0, 0.0, half_l]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push([0.5, 0.5]);

    for i in 0..=segments {
        let angle = PI + PI * i as f32 / segments as f32;
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        positions.push([x, y, half_l]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([(angle.cos() + 1.0) / 2.0, (angle.sin() + 1.0) / 2.0]);
    }
    for i in 0..segments {
        indices.extend_from_slice(&[back_center, back_center + 1 + i, back_center + 2 + i]);
    }

    ensure_correct_winding(&positions, &normals, &mut indices);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Create a spiral staircase mesh
/// Inner radius 0.1, outer radius 0.5, 360 degrees total rotation, height 2.0
pub fn create_spiral_stairs_mesh(steps: u32) -> Mesh {
    let steps = steps.max(4);
    let inner_r = 0.1_f32;
    let outer_r = 0.5_f32;
    let total_height = 2.0_f32;
    let step_height = total_height / steps as f32;
    let angle_per_step = 2.0 * PI / steps as f32;

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for i in 0..steps {
        let a0 = angle_per_step * i as f32;
        let a1 = angle_per_step * (i + 1) as f32;
        let y_bot = step_height * i as f32;
        let y_top = step_height * (i + 1) as f32;

        let c0 = a0.cos();
        let s0 = a0.sin();
        let c1 = a1.cos();
        let s1 = a1.sin();

        // Four corners on top face of step (sector at y_top)
        let inner0 = [inner_r * c0, y_top, inner_r * s0];
        let inner1 = [inner_r * c1, y_top, inner_r * s1];
        let outer0 = [outer_r * c0, y_top, outer_r * s0];
        let outer1 = [outer_r * c1, y_top, outer_r * s1];

        // Bottom corners at y_bot
        let inner0_b = [inner_r * c0, y_bot, inner_r * s0];
        let inner1_b = [inner_r * c1, y_bot, inner_r * s1];
        let outer0_b = [outer_r * c0, y_bot, outer_r * s0];
        let outer1_b = [outer_r * c1, y_bot, outer_r * s1];

        // Top face (Y=y_top), normal +Y
        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [inner0, outer0, outer1, inner1],
            [0.0, 1.0, 0.0]);

        // Bottom face (Y=y_bot), normal -Y
        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [inner1_b, outer1_b, outer0_b, inner0_b],
            [0.0, -1.0, 0.0]);

        // Front face (riser at angle a0), normal pointing backward in angle direction
        let riser_n = Vec3::new(-s0, 0.0, c0).normalize();
        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [inner0_b, outer0_b, outer0, inner0],
            [-riser_n.x, riser_n.y, -riser_n.z]);

        // Outer face (curved outer edge), normal pointing outward radially
        let mid_angle = (a0 + a1) / 2.0;
        let outer_n = [mid_angle.cos(), 0.0, mid_angle.sin()];
        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [outer0_b, outer1_b, outer1, outer0],
            outer_n);

        // Inner face (curved inner edge), normal pointing inward
        let inner_n = [-mid_angle.cos(), 0.0, -mid_angle.sin()];
        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [inner1_b, inner0_b, inner0, inner1],
            inner_n);

        // Back face (at angle a1), only needed if there is a gap (last step connects to first)
        let back_n = Vec3::new(s1, 0.0, -c1).normalize();
        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [outer1_b, inner1_b, inner1, outer1],
            [back_n.x, back_n.y, back_n.z]);
    }

    build_mesh(positions, normals, uvs, indices, Some([0.0, total_height / 2.0, 0.0]))
}

/// Create an octagonal pillar/column mesh
/// 8-sided prism, radius 0.25, height 2.0
pub fn create_pillar_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    let radius = 0.25_f32;
    let height = 2.0_f32;
    let sides = 8u32;

    // Precompute octagon vertices
    let mut ring = Vec::new();
    for i in 0..sides {
        let angle = 2.0 * PI * i as f32 / sides as f32;
        ring.push([radius * angle.cos(), radius * angle.sin()]);
    }

    // Side faces (8 quads)
    for i in 0..sides {
        let next = (i + 1) % sides;
        let p0 = ring[i as usize];
        let p1 = ring[next as usize];

        // Outward normal for this face
        let mid_angle = 2.0 * PI * (i as f32 + 0.5) / sides as f32;
        let nx = mid_angle.cos();
        let nz = mid_angle.sin();

        add_quad(&mut positions, &mut normals, &mut uvs, &mut indices,
            [[p0[0], 0.0, p0[1]], [p1[0], 0.0, p1[1]],
             [p1[0], height, p1[1]], [p0[0], height, p0[1]]],
            [nx, 0.0, nz]);
    }

    // Top cap (Y=height), normal +Y — triangle fan
    let top_center = positions.len() as u32;
    positions.push([0.0, height, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for i in 0..sides {
        let p = ring[i as usize];
        positions.push([p[0], height, p[1]]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([(p[0] / radius + 1.0) / 2.0, (p[1] / radius + 1.0) / 2.0]);
    }
    for i in 0..sides {
        let next = (i + 1) % sides;
        indices.extend_from_slice(&[top_center, top_center + 1 + i, top_center + 1 + next]);
    }

    // Bottom cap (Y=0), normal -Y — triangle fan
    let bot_center = positions.len() as u32;
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, -1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for i in 0..sides {
        let p = ring[i as usize];
        positions.push([p[0], 0.0, p[1]]);
        normals.push([0.0, -1.0, 0.0]);
        uvs.push([(p[0] / radius + 1.0) / 2.0, (p[1] / radius + 1.0) / 2.0]);
    }
    for i in 0..sides {
        let next = (i + 1) % sides;
        indices.extend_from_slice(&[bot_center, bot_center + 1 + next, bot_center + 1 + i]);
    }

    build_mesh(positions, normals, uvs, indices, Some([0.0, 1.0, 0.0]))
}
