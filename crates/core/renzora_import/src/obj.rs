//! OBJ (Wavefront) → GLB converter.

use std::path::Path;

use crate::convert::{ImportError, ImportResult};
use crate::gltf_pass::pack_glb;
use crate::settings::{ImportSettings, UpAxis};

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let load_options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };

    let (models, _materials) = tobj::load_obj(path, &load_options)
        .map_err(|e| ImportError::ParseError(format!("OBJ parse error: {}", e)))?;

    let mut warnings = Vec::new();

    if models.is_empty() {
        return Err(ImportError::ParseError("OBJ file contains no meshes".into()));
    }

    let mut all_positions: Vec<f32> = Vec::new();
    let mut all_normals: Vec<f32> = Vec::new();
    let mut all_texcoords: Vec<f32> = Vec::new();
    let mut all_indices: Vec<u32> = Vec::new();

    for model in &models {
        let mesh = &model.mesh;
        let vertex_count = mesh.positions.len() / 3;

        if vertex_count == 0 {
            warnings.push(format!("mesh '{}' has no vertices, skipping", model.name));
            continue;
        }

        let base_vertex = (all_positions.len() / 3) as u32;

        for i in 0..vertex_count {
            let (x, mut y, mut z) = (
                mesh.positions[i * 3] * settings.scale,
                mesh.positions[i * 3 + 1] * settings.scale,
                mesh.positions[i * 3 + 2] * settings.scale,
            );

            if settings.up_axis == UpAxis::ZUp {
                let tmp = y;
                y = z;
                z = -tmp;
            }

            all_positions.extend_from_slice(&[x, y, z]);
        }

        let has_normals = mesh.normals.len() == vertex_count * 3;
        if has_normals {
            for i in 0..vertex_count {
                let (nx, mut ny, mut nz) = (
                    mesh.normals[i * 3],
                    mesh.normals[i * 3 + 1],
                    mesh.normals[i * 3 + 2],
                );

                if settings.up_axis == UpAxis::ZUp {
                    let tmp = ny;
                    ny = nz;
                    nz = -tmp;
                }

                all_normals.extend_from_slice(&[nx, ny, nz]);
            }
        } else if settings.generate_normals {
            let normals = generate_flat_normals(
                &all_positions[base_vertex as usize * 3..],
                &mesh.indices,
                vertex_count,
            );
            all_normals.extend_from_slice(&normals);
        } else {
            all_normals.extend(std::iter::repeat(0.0f32).take(vertex_count * 3));
        }

        let has_texcoords = mesh.texcoords.len() == vertex_count * 2;
        if has_texcoords {
            for i in 0..vertex_count {
                let u = mesh.texcoords[i * 2];
                let v = if settings.flip_uvs {
                    1.0 - mesh.texcoords[i * 2 + 1]
                } else {
                    mesh.texcoords[i * 2 + 1]
                };
                all_texcoords.extend_from_slice(&[u, v]);
            }
        } else {
            all_texcoords.extend(std::iter::repeat(0.0f32).take(vertex_count * 2));
        }

        for &idx in &mesh.indices {
            all_indices.push(idx + base_vertex);
        }
    }

    if all_positions.is_empty() {
        return Err(ImportError::ParseError("no valid geometry found in OBJ".into()));
    }

    let glb_bytes = build_glb(&all_positions, &all_normals, &all_texcoords, &all_indices)?;

    Ok(ImportResult {
        glb_bytes,
        warnings,
    })
}

fn generate_flat_normals(positions: &[f32], indices: &[u32], vertex_count: usize) -> Vec<f32> {
    let mut normals = vec![0.0f32; vertex_count * 3];

    for tri in indices.chunks(3) {
        if tri.len() < 3 { break; }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);

        let p0 = [positions[i0 * 3], positions[i0 * 3 + 1], positions[i0 * 3 + 2]];
        let p1 = [positions[i1 * 3], positions[i1 * 3 + 1], positions[i1 * 3 + 2]];
        let p2 = [positions[i2 * 3], positions[i2 * 3 + 1], positions[i2 * 3 + 2]];

        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];

        for &idx in &[i0, i1, i2] {
            normals[idx * 3] += n[0];
            normals[idx * 3 + 1] += n[1];
            normals[idx * 3 + 2] += n[2];
        }
    }

    for i in 0..vertex_count {
        let (x, y, z) = (normals[i * 3], normals[i * 3 + 1], normals[i * 3 + 2]);
        let len = (x * x + y * y + z * z).sqrt();
        if len > 1e-8 {
            normals[i * 3] /= len;
            normals[i * 3 + 1] /= len;
            normals[i * 3 + 2] /= len;
        } else {
            normals[i * 3 + 1] = 1.0;
        }
    }

    normals
}

/// Build a GLB from flat arrays of positions, normals, texcoords, and indices.
pub(crate) fn build_glb(
    positions: &[f32],
    normals: &[f32],
    texcoords: &[f32],
    indices: &[u32],
) -> Result<Vec<u8>, ImportError> {
    let vertex_count = positions.len() / 3;
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for i in 0..vertex_count {
        for c in 0..3 {
            let v = positions[i * 3 + c];
            if v < min[c] { min[c] = v; }
            if v > max[c] { max[c] = v; }
        }
    }

    let pos_bytes = cast_f32_to_bytes(positions);
    let norm_bytes = cast_f32_to_bytes(normals);
    let tc_bytes = cast_f32_to_bytes(texcoords);
    let idx_bytes = cast_u32_to_bytes(indices);

    let pos_offset = 0usize;
    let norm_offset = pos_bytes.len();
    let tc_offset = norm_offset + norm_bytes.len();
    let idx_offset = tc_offset + tc_bytes.len();

    let mut bin = Vec::with_capacity(idx_offset + idx_bytes.len());
    bin.extend_from_slice(&pos_bytes);
    bin.extend_from_slice(&norm_bytes);
    bin.extend_from_slice(&tc_bytes);
    bin.extend_from_slice(&idx_bytes);

    use gltf_json::*;

    let mut root = Root::default();
    root.asset.generator = Some("renzora_import".to_string());

    // Buffer
    root.buffers.push(Buffer {
        byte_length: validation::USize64(bin.len() as u64),
        name: None,
        uri: None,
        extensions: None,
        extras: Default::default(),
    });

    // Buffer views
    root.buffer_views.push(buffer::View {
        buffer: Index::new(0),
        byte_length: validation::USize64(pos_bytes.len() as u64),
        byte_offset: Some(validation::USize64(pos_offset as u64)),
        byte_stride: None,
        name: None,
        target: Some(validation::Checked::Valid(buffer::Target::ArrayBuffer)),
        extensions: None,
        extras: Default::default(),
    });
    root.buffer_views.push(buffer::View {
        buffer: Index::new(0),
        byte_length: validation::USize64(norm_bytes.len() as u64),
        byte_offset: Some(validation::USize64(norm_offset as u64)),
        byte_stride: None,
        name: None,
        target: Some(validation::Checked::Valid(buffer::Target::ArrayBuffer)),
        extensions: None,
        extras: Default::default(),
    });
    root.buffer_views.push(buffer::View {
        buffer: Index::new(0),
        byte_length: validation::USize64(tc_bytes.len() as u64),
        byte_offset: Some(validation::USize64(tc_offset as u64)),
        byte_stride: None,
        name: None,
        target: Some(validation::Checked::Valid(buffer::Target::ArrayBuffer)),
        extensions: None,
        extras: Default::default(),
    });
    root.buffer_views.push(buffer::View {
        buffer: Index::new(0),
        byte_length: validation::USize64(idx_bytes.len() as u64),
        byte_offset: Some(validation::USize64(idx_offset as u64)),
        byte_stride: None,
        name: None,
        target: Some(validation::Checked::Valid(buffer::Target::ElementArrayBuffer)),
        extensions: None,
        extras: Default::default(),
    });

    let min_val: Value = serde_json::json!([min[0], min[1], min[2]]);
    let max_val: Value = serde_json::json!([max[0], max[1], max[2]]);

    // Accessors
    root.accessors.push(Accessor {
        buffer_view: Some(Index::new(0)),
        byte_offset: Some(validation::USize64(0)),
        count: validation::USize64(vertex_count as u64),
        component_type: validation::Checked::Valid(accessor::GenericComponentType(
            accessor::ComponentType::F32,
        )),
        type_: validation::Checked::Valid(accessor::Type::Vec3),
        min: Some(min_val),
        max: Some(max_val),
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    });
    root.accessors.push(Accessor {
        buffer_view: Some(Index::new(1)),
        byte_offset: Some(validation::USize64(0)),
        count: validation::USize64(vertex_count as u64),
        component_type: validation::Checked::Valid(accessor::GenericComponentType(
            accessor::ComponentType::F32,
        )),
        type_: validation::Checked::Valid(accessor::Type::Vec3),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    });
    root.accessors.push(Accessor {
        buffer_view: Some(Index::new(2)),
        byte_offset: Some(validation::USize64(0)),
        count: validation::USize64(vertex_count as u64),
        component_type: validation::Checked::Valid(accessor::GenericComponentType(
            accessor::ComponentType::F32,
        )),
        type_: validation::Checked::Valid(accessor::Type::Vec2),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    });
    root.accessors.push(Accessor {
        buffer_view: Some(Index::new(3)),
        byte_offset: Some(validation::USize64(0)),
        count: validation::USize64(indices.len() as u64),
        component_type: validation::Checked::Valid(accessor::GenericComponentType(
            accessor::ComponentType::U32,
        )),
        type_: validation::Checked::Valid(accessor::Type::Scalar),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    });

    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert(
        validation::Checked::Valid(mesh::Semantic::Positions),
        Index::new(0),
    );
    attributes.insert(
        validation::Checked::Valid(mesh::Semantic::Normals),
        Index::new(1),
    );
    attributes.insert(
        validation::Checked::Valid(mesh::Semantic::TexCoords(0)),
        Index::new(2),
    );

    root.meshes.push(Mesh {
        primitives: vec![mesh::Primitive {
            attributes,
            indices: Some(Index::new(3)),
            material: None,
            mode: validation::Checked::Valid(mesh::Mode::Triangles),
            targets: None,
            extensions: None,
            extras: Default::default(),
        }],
        name: None,
        weights: None,
        extensions: None,
        extras: Default::default(),
    });

    root.nodes.push(Node {
        mesh: Some(Index::new(0)),
        name: None,
        camera: None,
        children: None,
        skin: None,
        matrix: None,
        rotation: None,
        scale: None,
        translation: None,
        weights: None,
        extensions: None,
        extras: Default::default(),
    });

    root.scenes.push(Scene {
        name: None,
        nodes: vec![Index::new(0)],
        extensions: None,
        extras: Default::default(),
    });

    root.scene = Some(Index::new(0));

    let json_bytes = root
        .to_vec()
        .map_err(|e| ImportError::ConversionError(format!("GLTF JSON serialize: {}", e)))?;

    Ok(pack_glb(&json_bytes, Some(&bin)))
}

fn cast_f32_to_bytes(data: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 4);
    for &v in data {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn cast_u32_to_bytes(data: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 4);
    for &v in data {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}
