//! Mesh processing utilities — triangulation, normal generation, tangent computation.

use crate::scene::UsdMesh;

/// Triangulate a mesh's polygon faces into triangle indices.
/// Returns a flat list of triangle vertex indices.
pub fn triangulate(mesh: &UsdMesh) -> Vec<u32> {
    let mut result = Vec::new();
    let mut idx_offset = 0usize;

    for &count in &mesh.face_vertex_counts {
        let n = count as usize;
        if n < 3 || idx_offset + n > mesh.face_vertex_indices.len() {
            idx_offset += n;
            continue;
        }

        // Fan triangulation from first vertex of each face
        let v0 = mesh.face_vertex_indices[idx_offset];
        for i in 1..n - 1 {
            result.push(v0);
            result.push(mesh.face_vertex_indices[idx_offset + i]);
            result.push(mesh.face_vertex_indices[idx_offset + i + 1]);
        }

        idx_offset += n;
    }

    result
}

/// Generate smooth vertex normals from positions and triangle indices.
pub fn generate_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32; 3]; positions.len()];

    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
            continue;
        }

        let p0 = positions[i0];
        let p1 = positions[i1];
        let p2 = positions[i2];

        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];

        for &idx in &[i0, i1, i2] {
            normals[idx][0] += n[0];
            normals[idx][1] += n[1];
            normals[idx][2] += n[2];
        }
    }

    for n in &mut normals {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 1e-8 {
            n[0] /= len;
            n[1] /= len;
            n[2] /= len;
        } else {
            *n = [0.0, 1.0, 0.0];
        }
    }

    normals
}

/// Generate tangents using MikkTSpace-style algorithm.
/// Requires positions, normals, UVs, and triangle indices.
pub fn generate_tangents(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Vec<[f32; 4]> {
    let vert_count = positions.len();
    let mut tangents = vec![[0.0f32; 3]; vert_count];
    let mut bitangents = vec![[0.0f32; 3]; vert_count];

    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if i0 >= vert_count || i1 >= vert_count || i2 >= vert_count {
            continue;
        }
        if i0 >= uvs.len() || i1 >= uvs.len() || i2 >= uvs.len() {
            continue;
        }

        let p0 = positions[i0];
        let p1 = positions[i1];
        let p2 = positions[i2];
        let uv0 = uvs[i0];
        let uv1 = uvs[i1];
        let uv2 = uvs[i2];

        let dp1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let dp2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let duv1 = [uv1[0] - uv0[0], uv1[1] - uv0[1]];
        let duv2 = [uv2[0] - uv0[0], uv2[1] - uv0[1]];

        let r = duv1[0] * duv2[1] - duv1[1] * duv2[0];
        if r.abs() < 1e-8 {
            continue;
        }
        let r = 1.0 / r;

        let t = [
            (duv2[1] * dp1[0] - duv1[1] * dp2[0]) * r,
            (duv2[1] * dp1[1] - duv1[1] * dp2[1]) * r,
            (duv2[1] * dp1[2] - duv1[1] * dp2[2]) * r,
        ];
        let b = [
            (duv1[0] * dp2[0] - duv2[0] * dp1[0]) * r,
            (duv1[0] * dp2[1] - duv2[0] * dp1[1]) * r,
            (duv1[0] * dp2[2] - duv2[0] * dp1[2]) * r,
        ];

        for &idx in &[i0, i1, i2] {
            tangents[idx][0] += t[0];
            tangents[idx][1] += t[1];
            tangents[idx][2] += t[2];
            bitangents[idx][0] += b[0];
            bitangents[idx][1] += b[1];
            bitangents[idx][2] += b[2];
        }
    }

    // Gram-Schmidt orthonormalize and compute handedness
    let mut result = Vec::with_capacity(vert_count);
    for i in 0..vert_count {
        let n = if i < normals.len() { normals[i] } else { [0.0, 1.0, 0.0] };
        let t = tangents[i];

        // Gram-Schmidt: t' = normalize(t - n * dot(n, t))
        let dot_nt = n[0] * t[0] + n[1] * t[1] + n[2] * t[2];
        let t_orth = [
            t[0] - n[0] * dot_nt,
            t[1] - n[1] * dot_nt,
            t[2] - n[2] * dot_nt,
        ];
        let len = (t_orth[0] * t_orth[0] + t_orth[1] * t_orth[1] + t_orth[2] * t_orth[2]).sqrt();

        let (tx, ty, tz) = if len > 1e-8 {
            (t_orth[0] / len, t_orth[1] / len, t_orth[2] / len)
        } else {
            (1.0, 0.0, 0.0)
        };

        // Handedness: sign of dot(cross(n, t), b)
        let cross = [
            n[1] * t[2] - n[2] * t[1],
            n[2] * t[0] - n[0] * t[2],
            n[0] * t[1] - n[1] * t[0],
        ];
        let b = bitangents[i];
        let w = if cross[0] * b[0] + cross[1] * b[1] + cross[2] * b[2] < 0.0 {
            -1.0
        } else {
            1.0
        };

        result.push([tx, ty, tz, w]);
    }

    result
}
