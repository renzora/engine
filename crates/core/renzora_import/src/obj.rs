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

    let (models, materials_result) = tobj::load_obj(path, &load_options)
        .map_err(|e| ImportError::ParseError(format!("OBJ parse error: {}", e)))?;

    let mut warnings = Vec::new();
    let mtl_materials = match materials_result {
        Ok(m) => m,
        Err(e) => {
            warnings.push(format!("MTL parse: {} (materials skipped)", e));
            Vec::new()
        }
    };

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

    // Walk MTL materials: copy referenced texture files into
    // `extracted_textures` (so they land in `<model_dir>/textures/`), build
    // the GLB's MaterialBundle, and emit plain PbrMaterialExtracted records.
    let (material_bundle, extracted_textures, extracted_materials) =
        if settings.extract_textures || settings.extract_materials {
            extract_obj_materials(path, &mtl_materials, settings, &mut warnings)
        } else {
            (
                MaterialBundle::default(),
                Vec::new(),
                Vec::new(),
            )
        };

    let glb_bytes = build_glb(
        &all_positions,
        &all_normals,
        &all_texcoords,
        &all_indices,
        &material_bundle,
    )?;

    Ok(ImportResult {
        glb_bytes,
        warnings,
        extracted_textures,
        extracted_materials,
    })
}

/// Read every MTL-referenced texture file relative to the OBJ, sniff the
/// format, and build a [`MaterialBundle`] + [`ExtractedPbrMaterial`] list.
/// Missing files surface as warnings; the material entry is still emitted
/// without that particular map.
fn extract_obj_materials(
    obj_path: &Path,
    mtl_materials: &[tobj::Material],
    settings: &ImportSettings,
    warnings: &mut Vec<String>,
) -> (
    MaterialBundle,
    Vec<crate::convert::ExtractedTexture>,
    Vec<crate::convert::ExtractedPbrMaterial>,
) {
    use crate::convert::{ExtractedPbrMaterial, ExtractedTexture};

    let mut bundle = MaterialBundle::default();
    let mut extracted_textures: Vec<ExtractedTexture> = Vec::new();
    let mut extracted_materials: Vec<ExtractedPbrMaterial> = Vec::new();
    // MTL texture path (relative to .obj) → index in `extracted_textures`.
    let mut tex_paths: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let obj_dir = obj_path.parent().unwrap_or(Path::new("."));

    // Helper that either finds an already-loaded texture or reads + sniffs a
    // new one. Returns the index into bundle.textures / extracted_textures.
    let mut load_texture = |rel_path: &str,
                            bundle: &mut MaterialBundle,
                            extracted_textures: &mut Vec<ExtractedTexture>,
                            tex_paths: &mut std::collections::HashMap<String, usize>,
                            used_names: &mut std::collections::HashSet<String>,
                            warnings: &mut Vec<String>|
     -> Option<usize> {
        if let Some(&i) = tex_paths.get(rel_path) {
            return Some(i);
        }
        let abs = obj_dir.join(rel_path);
        let data = match std::fs::read(&abs) {
            Ok(d) => d,
            Err(e) => {
                warnings.push(format!("texture '{}': {}", rel_path, e));
                return None;
            }
        };
        let extension_hint = std::path::Path::new(rel_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let extension = if !extension_hint.is_empty() {
            extension_hint.to_lowercase()
        } else {
            sniff_image_ext(&data).to_string()
        };
        let stem = std::path::Path::new(rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("texture")
            .to_string();
        let base = sanitize_name(&stem);
        let mut name = base.clone();
        let mut n = 1;
        while used_names.contains(&name) {
            n += 1;
            name = format!("{}_{}", base, n);
        }
        used_names.insert(name.clone());

        let uri = format!("textures/{}.{}", name, extension);
        let idx = bundle.textures.len();
        bundle.textures.push(TextureRef { uri });
        extracted_textures.push(ExtractedTexture {
            name,
            extension,
            data,
        });
        tex_paths.insert(rel_path.to_string(), idx);
        Some(idx)
    };

    for mat in mtl_materials {
        let base_color = if let Some(d) = mat.diffuse {
            [d[0], d[1], d[2], mat.dissolve.unwrap_or(1.0)]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        let base_tex = mat.diffuse_texture.as_ref().and_then(|p| {
            if !settings.extract_textures {
                return None;
            }
            load_texture(
                p,
                &mut bundle,
                &mut extracted_textures,
                &mut tex_paths,
                &mut used_names,
                warnings,
            )
        });
        let normal_tex = mat.normal_texture.as_ref().and_then(|p| {
            if !settings.extract_textures {
                return None;
            }
            load_texture(
                p,
                &mut bundle,
                &mut extracted_textures,
                &mut tex_paths,
                &mut used_names,
                warnings,
            )
        });

        // Crude roughness/metallic fallback: OBJ/MTL is pre-PBR. Map shininess
        // into roughness (lower shininess → rougher) and leave metallic at 0.
        let roughness = mat
            .shininess
            .map(|s| (1.0 - (s / 1000.0)).clamp(0.05, 1.0))
            .unwrap_or(0.8);

        if settings.extract_materials {
            bundle.materials.push(PbrMaterialDef {
                name: mat.name.clone(),
                base_color,
                base_color_texture: base_tex,
                normal_texture: normal_tex,
                metallic: 0.0,
                roughness,
            });
            let lookup = |idx: Option<usize>| -> Option<String> {
                idx.and_then(|i| bundle.textures.get(i).map(|t| t.uri.clone()))
            };
            extracted_materials.push(ExtractedPbrMaterial {
                name: mat.name.clone(),
                base_color,
                metallic: 0.0,
                roughness,
                base_color_texture: lookup(base_tex),
                normal_texture: lookup(normal_tex),
            });
        }
    }

    (bundle, extracted_textures, extracted_materials)
}

fn sanitize_name(input: &str) -> String {
    if input.is_empty() {
        return "texture".into();
    }
    input
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn sniff_image_ext(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) { "png" }
    else if data.starts_with(&[0xFF, 0xD8, 0xFF]) { "jpg" }
    else if data.starts_with(b"DDS ") { "dds" }
    else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") { "gif" }
    else if data.starts_with(b"BM") { "bmp" }
    else if data.starts_with(&[0x52, 0x49, 0x46, 0x46]) && data.get(8..12) == Some(b"WEBP") { "webp" }
    else { "bin" }
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
    materials: &MaterialBundle,
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

    emit_material_bundle(&mut root, materials);
    let primitive_material = if materials.materials.is_empty() {
        None
    } else {
        Some(Index::new(0))
    };

    root.meshes.push(Mesh {
        primitives: vec![mesh::Primitive {
            attributes,
            indices: Some(Index::new(3)),
            material: primitive_material,
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

/// A skeleton joint as consumed by [`build_skinned_glb`]. `parent` is an index
/// into the same slice of joints, or `None` for skeleton roots.
#[derive(Debug, Clone)]
pub(crate) struct SkinJoint {
    pub name: String,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub inverse_bind_matrix: [f32; 16],
}

/// A material bundle consumed by the GLB builders. The builder emits one
/// GLTF material entry per `PbrMaterialDef` and one image+texture per
/// `TextureRef`; the mesh primitive references material 0 when the bundle
/// is non-empty.
#[derive(Debug, Clone, Default)]
pub(crate) struct MaterialBundle {
    pub materials: Vec<PbrMaterialDef>,
    pub textures: Vec<TextureRef>,
}

#[derive(Debug, Clone)]
pub(crate) struct PbrMaterialDef {
    pub name: String,
    pub base_color: [f32; 4],
    pub base_color_texture: Option<usize>,
    pub normal_texture: Option<usize>,
    pub metallic: f32,
    pub roughness: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct TextureRef {
    /// Asset-relative URI, e.g. `"textures/diffuse.png"`. The GLB stores this
    /// as the image's `uri`; Bevy resolves it relative to the GLB file.
    pub uri: String,
}

/// Build a GLB that contains a skinned mesh. `joint_indices` and `weights` must
/// be the same length as the vertex count implied by `positions`. `joints` is
/// the skeleton in flat order — children refer to parents via their index.
/// IBM list is parallel to `joints`.
pub(crate) fn build_skinned_glb(
    positions: &[f32],
    normals: &[f32],
    texcoords: &[f32],
    indices: &[u32],
    joint_indices: &[[u16; 4]],
    weights: &[[f32; 4]],
    joints: &[SkinJoint],
    materials: &MaterialBundle,
) -> Result<Vec<u8>, ImportError> {
    let vertex_count = positions.len() / 3;
    if joint_indices.len() != vertex_count || weights.len() != vertex_count {
        return Err(ImportError::ConversionError(format!(
            "skin attribute length mismatch: {} vertices, {} joint_indices, {} weights",
            vertex_count,
            joint_indices.len(),
            weights.len()
        )));
    }

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

    // JOINTS_0 as u16x4 (8 bytes per vertex).
    let mut ji_bytes = Vec::with_capacity(vertex_count * 8);
    for ji in joint_indices {
        for &j in ji {
            ji_bytes.extend_from_slice(&j.to_le_bytes());
        }
    }
    // WEIGHTS_0 as f32x4 (16 bytes per vertex).
    let mut w_bytes = Vec::with_capacity(vertex_count * 16);
    for w in weights {
        for &v in w {
            w_bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    // Inverse bind matrices — one mat4 per joint (64 bytes each).
    let mut ibm_bytes = Vec::with_capacity(joints.len() * 64);
    for j in joints {
        for &v in &j.inverse_bind_matrix {
            ibm_bytes.extend_from_slice(&v.to_le_bytes());
        }
    }

    // Pad index buffer to 4-byte alignment (it already is u32; ji_bytes to 4; others fine).
    // Order: pos, norm, tc, indices, joints, weights, ibm.
    let pos_offset = 0usize;
    let norm_offset = pos_offset + pos_bytes.len();
    let tc_offset = norm_offset + norm_bytes.len();
    let idx_offset = tc_offset + tc_bytes.len();
    let ji_offset = idx_offset + idx_bytes.len();
    let w_offset = ji_offset + ji_bytes.len();
    let ibm_offset = w_offset + w_bytes.len();
    let total_len = ibm_offset + ibm_bytes.len();

    let mut bin = Vec::with_capacity(total_len);
    bin.extend_from_slice(&pos_bytes);
    bin.extend_from_slice(&norm_bytes);
    bin.extend_from_slice(&tc_bytes);
    bin.extend_from_slice(&idx_bytes);
    bin.extend_from_slice(&ji_bytes);
    bin.extend_from_slice(&w_bytes);
    bin.extend_from_slice(&ibm_bytes);

    use gltf_json::*;

    let mut root = Root::default();
    root.asset.generator = Some("renzora_import".to_string());

    root.buffers.push(Buffer {
        byte_length: validation::USize64(bin.len() as u64),
        name: None,
        uri: None,
        extensions: None,
        extras: Default::default(),
    });

    // 0: positions, 1: normals, 2: texcoords, 3: indices,
    // 4: joints, 5: weights, 6: IBMs.
    let views = [
        (pos_offset, pos_bytes.len(), Some(buffer::Target::ArrayBuffer)),
        (norm_offset, norm_bytes.len(), Some(buffer::Target::ArrayBuffer)),
        (tc_offset, tc_bytes.len(), Some(buffer::Target::ArrayBuffer)),
        (idx_offset, idx_bytes.len(), Some(buffer::Target::ElementArrayBuffer)),
        (ji_offset, ji_bytes.len(), Some(buffer::Target::ArrayBuffer)),
        (w_offset, w_bytes.len(), Some(buffer::Target::ArrayBuffer)),
        (ibm_offset, ibm_bytes.len(), None),
    ];
    for (off, len, target) in views {
        root.buffer_views.push(buffer::View {
            buffer: Index::new(0),
            byte_length: validation::USize64(len as u64),
            byte_offset: Some(validation::USize64(off as u64)),
            byte_stride: None,
            name: None,
            target: target.map(validation::Checked::Valid),
            extensions: None,
            extras: Default::default(),
        });
    }

    let min_val: Value = serde_json::json!([min[0], min[1], min[2]]);
    let max_val: Value = serde_json::json!([max[0], max[1], max[2]]);

    // Accessors:
    // 0 positions, 1 normals, 2 texcoords, 3 indices,
    // 4 joints (u16 vec4), 5 weights (f32 vec4), 6 IBMs (f32 mat4).
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
    root.accessors.push(Accessor {
        buffer_view: Some(Index::new(4)),
        byte_offset: Some(validation::USize64(0)),
        count: validation::USize64(vertex_count as u64),
        component_type: validation::Checked::Valid(accessor::GenericComponentType(
            accessor::ComponentType::U16,
        )),
        type_: validation::Checked::Valid(accessor::Type::Vec4),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    });
    root.accessors.push(Accessor {
        buffer_view: Some(Index::new(5)),
        byte_offset: Some(validation::USize64(0)),
        count: validation::USize64(vertex_count as u64),
        component_type: validation::Checked::Valid(accessor::GenericComponentType(
            accessor::ComponentType::F32,
        )),
        type_: validation::Checked::Valid(accessor::Type::Vec4),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    });
    root.accessors.push(Accessor {
        buffer_view: Some(Index::new(6)),
        byte_offset: Some(validation::USize64(0)),
        count: validation::USize64(joints.len() as u64),
        component_type: validation::Checked::Valid(accessor::GenericComponentType(
            accessor::ComponentType::F32,
        )),
        type_: validation::Checked::Valid(accessor::Type::Mat4),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
        extensions: None,
        extras: Default::default(),
    });

    // Mesh primitive with JOINTS_0 / WEIGHTS_0 attributes.
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
    attributes.insert(
        validation::Checked::Valid(mesh::Semantic::Joints(0)),
        Index::new(4),
    );
    attributes.insert(
        validation::Checked::Valid(mesh::Semantic::Weights(0)),
        Index::new(5),
    );

    // Emit GLTF materials/images/textures/samplers from the bundle. The mesh
    // primitive uses material 0 when the bundle is non-empty.
    emit_material_bundle(&mut root, materials);
    let primitive_material = if materials.materials.is_empty() {
        None
    } else {
        Some(Index::new(0))
    };

    root.meshes.push(Mesh {
        primitives: vec![mesh::Primitive {
            attributes,
            indices: Some(Index::new(3)),
            material: primitive_material,
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

    // Emit joint nodes. Node 0 is the mesh; joint nodes start at index 1.
    let mesh_node_idx = 0usize;
    let joint_base = 1usize;

    // First: push placeholder for the mesh node (fill after joints).
    root.nodes.push(Node {
        mesh: None,
        name: Some("Mesh".to_string()),
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

    // Build children lists for each joint first.
    let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); joints.len()];
    let mut root_joints: Vec<usize> = Vec::new();
    for (i, j) in joints.iter().enumerate() {
        match j.parent {
            Some(p) => children_of[p].push(i),
            None => root_joints.push(i),
        }
    }

    for (i, j) in joints.iter().enumerate() {
        let children = if children_of[i].is_empty() {
            None
        } else {
            Some(
                children_of[i]
                    .iter()
                    .map(|&c| Index::new((joint_base + c) as u32))
                    .collect(),
            )
        };
        root.nodes.push(Node {
            mesh: None,
            name: Some(j.name.clone()),
            camera: None,
            children,
            skin: None,
            matrix: None,
            rotation: Some(scene::UnitQuaternion(j.rotation)),
            scale: Some(j.scale),
            translation: Some(j.translation),
            weights: None,
            extensions: None,
            extras: Default::default(),
        });
    }

    // Skin: joints list + IBM accessor.
    let skin_joints: Vec<Index<Node>> = (0..joints.len())
        .map(|i| Index::new((joint_base + i) as u32))
        .collect();
    root.skins.push(Skin {
        inverse_bind_matrices: Some(Index::new(6)),
        joints: skin_joints,
        skeleton: root_joints
            .first()
            .map(|&i| Index::new((joint_base + i) as u32)),
        name: None,
        extensions: None,
        extras: Default::default(),
    });

    // Fill in the mesh node with mesh + skin references and parent the
    // skeleton root(s) under it. This makes the mesh node the single scene
    // root so Bevy spawns one grouped entity with the skeleton as children —
    // instead of mesh and skeleton appearing as separate siblings.
    root.nodes[mesh_node_idx].mesh = Some(Index::new(0));
    root.nodes[mesh_node_idx].skin = Some(Index::new(0));
    if !root_joints.is_empty() {
        root.nodes[mesh_node_idx].children = Some(
            root_joints
                .iter()
                .map(|&r| Index::new((joint_base + r) as u32))
                .collect(),
        );
    }

    root.scenes.push(Scene {
        name: None,
        nodes: vec![Index::new(mesh_node_idx as u32)],
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

/// Push GLTF entries (image / sampler / texture / material) from the bundle.
/// Images use external URIs (relative to the GLB); the caller writes the
/// actual bytes to disk separately. One default sampler is shared by all
/// textures.
fn emit_material_bundle(root: &mut gltf_json::Root, bundle: &MaterialBundle) {
    if bundle.materials.is_empty() && bundle.textures.is_empty() {
        return;
    }

    use gltf_json::*;

    // One linear/repeat sampler shared across all textures.
    if !bundle.textures.is_empty() {
        let mut sampler = texture::Sampler::default();
        sampler.mag_filter = Some(validation::Checked::Valid(texture::MagFilter::Linear));
        sampler.min_filter = Some(validation::Checked::Valid(texture::MinFilter::LinearMipmapLinear));
        sampler.wrap_s = validation::Checked::Valid(texture::WrappingMode::Repeat);
        sampler.wrap_t = validation::Checked::Valid(texture::WrappingMode::Repeat);
        root.samplers.push(sampler);
    }
    let sampler_idx = if bundle.textures.is_empty() {
        None
    } else {
        Some(Index::new(0))
    };

    for (i, tex) in bundle.textures.iter().enumerate() {
        root.images.push(Image {
            buffer_view: None,
            mime_type: None,
            name: None,
            uri: Some(tex.uri.clone()),
            extensions: None,
            extras: Default::default(),
        });
        root.textures.push(Texture {
            name: None,
            sampler: sampler_idx,
            source: Index::new(i as u32),
            extensions: None,
            extras: Default::default(),
        });
    }

    for mat in &bundle.materials {
        let base_tex = mat.base_color_texture.map(|i| texture::Info {
            index: Index::new(i as u32),
            tex_coord: 0,
            extensions: None,
            extras: Default::default(),
        });
        let normal_tex = mat.normal_texture.map(|i| material::NormalTexture {
            index: Index::new(i as u32),
            scale: 1.0,
            tex_coord: 0,
            extensions: None,
            extras: Default::default(),
        });
        let mut m = Material::default();
        m.alpha_mode = validation::Checked::Valid(material::AlphaMode::Opaque);
        m.pbr_metallic_roughness.base_color_factor =
            material::PbrBaseColorFactor(mat.base_color);
        m.pbr_metallic_roughness.base_color_texture = base_tex;
        m.pbr_metallic_roughness.metallic_factor = material::StrengthFactor(mat.metallic);
        m.pbr_metallic_roughness.roughness_factor = material::StrengthFactor(mat.roughness);
        m.normal_texture = normal_tex;
        let _ = &mat.name; // name is behind the `names` feature; skip safely.
        root.materials.push(m);
    }
}
