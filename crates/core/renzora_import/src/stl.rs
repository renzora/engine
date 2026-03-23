//! STL → GLB converter.

use std::path::Path;

use crate::convert::{ImportError, ImportResult};
use crate::obj::build_glb;
use crate::settings::{ImportSettings, UpAxis};

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .open(path)?;

    let mesh = stl_io::read_stl(&mut file)
        .map_err(|e| ImportError::ParseError(format!("STL parse error: {}", e)))?;

    if mesh.faces.is_empty() {
        return Err(ImportError::ParseError("STL file contains no triangles".into()));
    }

    let warnings = Vec::new();

    // Build vertex arrays from indexed mesh
    let mut positions = Vec::with_capacity(mesh.vertices.len() * 3);
    let mut indices: Vec<u32> = Vec::with_capacity(mesh.faces.len() * 3);

    // Add all vertices
    for v in &mesh.vertices {
        let (x, mut y, mut z) = (
            v.0[0] * settings.scale,
            v.0[1] * settings.scale,
            v.0[2] * settings.scale,
        );

        if settings.up_axis == UpAxis::ZUp {
            let tmp = y;
            y = z;
            z = -tmp;
        }

        positions.extend_from_slice(&[x, y, z]);
    }

    // Per-vertex normals accumulated from face normals
    let vertex_count = mesh.vertices.len();
    let mut vert_normals = vec![0.0f32; vertex_count * 3];

    for face in &mesh.faces {
        let (nx, mut ny, mut nz) = (face.normal.0[0], face.normal.0[1], face.normal.0[2]);

        if settings.up_axis == UpAxis::ZUp {
            let tmp = ny;
            ny = nz;
            nz = -tmp;
        }

        for &vi in &face.vertices {
            vert_normals[vi * 3] += nx;
            vert_normals[vi * 3 + 1] += ny;
            vert_normals[vi * 3 + 2] += nz;
            indices.push(vi as u32);
        }
    }

    // Normalize
    for i in 0..vertex_count {
        let (x, y, z) = (
            vert_normals[i * 3],
            vert_normals[i * 3 + 1],
            vert_normals[i * 3 + 2],
        );
        let len = (x * x + y * y + z * z).sqrt();
        if len > 1e-8 {
            vert_normals[i * 3] /= len;
            vert_normals[i * 3 + 1] /= len;
            vert_normals[i * 3 + 2] /= len;
        } else {
            vert_normals[i * 3 + 1] = 1.0;
        }
    }

    // STL has no UVs
    let texcoords = vec![0.0f32; vertex_count * 2];

    let glb_bytes = build_glb(&positions, &vert_normals, &texcoords, &indices)?;

    Ok(ImportResult {
        glb_bytes,
        warnings,
    })
}
