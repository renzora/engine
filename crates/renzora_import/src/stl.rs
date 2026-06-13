//! STL → GLB converter.

use std::path::Path;

use crate::convert::{ImportError, ImportResult};
use crate::obj::build_glb;
use crate::settings::{ImportSettings, UpAxis};

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let mut file = std::fs::OpenOptions::new().read(true).open(path)?;

    let mesh = stl_io::read_stl(&mut file)
        .map_err(|e| ImportError::ParseError(format!("STL parse error: {}", e)))?;

    if mesh.faces.is_empty() {
        return Err(ImportError::ParseError(
            "STL file contains no triangles".into(),
        ));
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

    // Per-vertex normals accumulated from face normals. Accumulating
    // un-normalized face normals area-weights the average, which gives smoother
    // results on irregular meshes than a plain sum of unit normals.
    let vertex_count = mesh.vertices.len();
    let mut vert_normals = vec![0.0f32; vertex_count * 3];

    let pos = |vi: usize| {
        [
            positions[vi * 3],
            positions[vi * 3 + 1],
            positions[vi * 3 + 2],
        ]
    };

    for face in &mesh.faces {
        // Use the stored facet normal when meaningful. A large share of
        // real-world STLs (especially ASCII exports and many slicers) write
        // (0,0,0) facet normals and rely on the importer deriving them from the
        // triangle winding (STL is CCW = outward). Fall back to the geometric
        // normal of the already-transformed triangle in that case so flat
        // prints still light correctly.
        let stored = {
            let (nx, mut ny, mut nz) = (face.normal.0[0], face.normal.0[1], face.normal.0[2]);
            if settings.up_axis == UpAxis::ZUp {
                let tmp = ny;
                ny = nz;
                nz = -tmp;
            }
            [nx, ny, nz]
        };
        let normal = if stored[0] * stored[0] + stored[1] * stored[1] + stored[2] * stored[2] > 1e-12
        {
            stored
        } else {
            let a = pos(face.vertices[0]);
            let b = pos(face.vertices[1]);
            let c = pos(face.vertices[2]);
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ]
        };

        for &vi in &face.vertices {
            vert_normals[vi * 3] += normal[0];
            vert_normals[vi * 3 + 1] += normal[1];
            vert_normals[vi * 3 + 2] += normal[2];
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

    // STL carries no materials, but we still emit one neutral "Default"
    // material so the mesh imports with an editable `.material` binding rather
    // than the engine's fallback. The single bundle entry makes `build_glb`
    // reference material 0 on the primitive, and the matching extracted
    // material binds to it by name.
    let mut bundle = crate::obj::MaterialBundle::default();
    let extracted_materials = if settings.extract_materials {
        bundle.materials.push(crate::obj::PbrMaterialDef {
            name: "Default".into(),
            base_color: [0.8, 0.8, 0.8, 1.0],
            base_color_texture: None,
            normal_texture: None,
            metallic: 0.0,
            roughness: 0.7,
            emissive: [0.0, 0.0, 0.0],
            emissive_texture: None,
            occlusion_texture: None,
            opacity_texture: None,
            specular_texture: None,
            alpha_blend: false,
            advanced: renzora::core::PbrAdvanced::default(),
        });
        vec![crate::convert::ExtractedPbrMaterial {
            name: "Default".into(),
            base_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.7,
            emissive: [0.0, 0.0, 0.0],
            base_color_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
            roughness_texture: None,
            metallic_texture: None,
            emissive_texture: None,
            occlusion_texture: None,
            specular_glossiness_texture: None,
            opacity_texture: None,
            specular_texture: None,
            advanced: renzora::core::PbrAdvanced::default(),
            alpha_mode: crate::convert::ExtractedAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }]
    } else {
        Vec::new()
    };

    let glb_bytes = build_glb(&positions, &vert_normals, &texcoords, &indices, &bundle)?;

    Ok(ImportResult {
        glb_bytes,
        warnings,
        extracted_textures: Vec::new(),
        extracted_materials,
    })
}
