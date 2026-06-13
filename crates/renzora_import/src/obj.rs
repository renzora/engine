#![allow(unused_mut, dead_code, unused_variables)]

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
        return Err(ImportError::ParseError(
            "OBJ file contains no meshes".into(),
        ));
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
            all_normals.extend(std::iter::repeat_n(0.0f32, vertex_count * 3));
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
            all_texcoords.extend(std::iter::repeat_n(0.0f32, vertex_count * 2));
        }

        for &idx in &mesh.indices {
            all_indices.push(idx + base_vertex);
        }
    }

    if all_positions.is_empty() {
        return Err(ImportError::ParseError(
            "no valid geometry found in OBJ".into(),
        ));
    }

    // Walk MTL materials: copy referenced texture files into
    // `extracted_textures` (so they land in `<model_dir>/textures/`), build
    // the GLB's MaterialBundle, and emit plain PbrMaterialExtracted records.
    let (material_bundle, extracted_textures, extracted_materials) =
        if settings.extract_textures || settings.extract_materials {
            extract_obj_materials(path, &mtl_materials, settings, &mut warnings)
        } else {
            (MaterialBundle::default(), Vec::new(), Vec::new())
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
    let mut tex_paths: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
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

        // PBR-MTL extension. The modern MTL spec adds `Pr` (roughness), `Pm`
        // (metallic), `Ps` (sheen), `Pc`/`Pcr` (clearcoat), `Ke` (emissive),
        // `aniso`/`anisor` (anisotropy) and their `map_*` variants. tobj keeps
        // these unrecognized keywords in `unknown_param`. We honor them when
        // present and fall back to the legacy shininess→roughness heuristic
        // otherwise so plain OBJ files still import sensibly.
        let param_f32 = |key: &str| -> Option<f32> {
            mat.unknown_param
                .get(key)
                .and_then(|v| v.split_whitespace().next())
                .and_then(|s| s.parse::<f32>().ok())
        };
        let param_vec3 = |key: &str| -> Option<[f32; 3]> {
            let v = mat.unknown_param.get(key)?;
            let nums: Vec<f32> = v.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            match nums.len() {
                0 => None,
                1 => Some([nums[0]; 3]),
                _ => Some([nums[0], nums[1], nums[2]]),
            }
        };

        let roughness = param_f32("Pr").unwrap_or_else(|| {
            mat.shininess
                .map(|s| (1.0 - (s / 1000.0)).clamp(0.05, 1.0))
                .unwrap_or(0.8)
        });
        let metallic = param_f32("Pm").unwrap_or(0.0);
        let emissive = param_vec3("Ke").unwrap_or([0.0, 0.0, 0.0]);
        let advanced = renzora::core::PbrAdvanced {
            clearcoat: param_f32("Pc").unwrap_or(0.0),
            clearcoat_roughness: param_f32("Pcr").unwrap_or(0.0),
            ior: mat.optical_density.unwrap_or(1.5),
            anisotropy_strength: param_f32("aniso").unwrap_or(0.0),
            anisotropy_rotation: param_f32("anisor").unwrap_or(0.0),
            ..Default::default()
        };
        let alpha_blend = mat.dissolve.map(|d| d < 1.0).unwrap_or(false);

        // Load the separate PBR map images, if any.
        let mut load_param_tex = |key: &str,
                                  bundle: &mut MaterialBundle,
                                  extracted_textures: &mut Vec<ExtractedTexture>|
         -> Option<usize> {
            if !settings.extract_textures {
                return None;
            }
            let p = mat.unknown_param.get(key)?.split_whitespace().last()?;
            load_texture(
                p,
                bundle,
                extracted_textures,
                &mut tex_paths,
                &mut used_names,
                warnings,
            )
        };
        let roughness_map = load_param_tex("map_Pr", &mut bundle, &mut extracted_textures);
        let metallic_map = load_param_tex("map_Pm", &mut bundle, &mut extracted_textures);
        let emissive_map = load_param_tex("map_Ke", &mut bundle, &mut extracted_textures);

        if settings.extract_materials {
            bundle.materials.push(PbrMaterialDef {
                name: mat.name.clone(),
                base_color,
                base_color_texture: base_tex,
                normal_texture: normal_tex,
                metallic,
                roughness,
                emissive,
                emissive_texture: emissive_map,
                occlusion_texture: None,
                opacity_texture: None,
                specular_texture: None,
                alpha_blend,
                advanced: advanced.clone(),
            });
            let lookup = |idx: Option<usize>| -> Option<String> {
                idx.and_then(|i| bundle.textures.get(i).map(|t| t.uri.clone()))
            };
            extracted_materials.push(ExtractedPbrMaterial {
                name: mat.name.clone(),
                base_color,
                metallic,
                roughness,
                emissive,
                base_color_texture: lookup(base_tex),
                normal_texture: lookup(normal_tex),
                metallic_roughness_texture: None,
                roughness_texture: lookup(roughness_map),
                metallic_texture: lookup(metallic_map),
                emissive_texture: lookup(emissive_map),
                occlusion_texture: None,
                specular_glossiness_texture: None,
                opacity_texture: None,
                specular_texture: None,
                advanced,
                alpha_mode: if alpha_blend {
                    crate::convert::ExtractedAlphaMode::Blend
                } else {
                    crate::convert::ExtractedAlphaMode::Opaque
                },
                alpha_cutoff: 0.5,
                double_sided: false,
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
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if data.starts_with(b"DDS ") {
        "dds"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "gif"
    } else if data.starts_with(b"BM") {
        "bmp"
    } else if data.starts_with(&[0x52, 0x49, 0x46, 0x46]) && data.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        "bin"
    }
}

fn generate_flat_normals(positions: &[f32], indices: &[u32], vertex_count: usize) -> Vec<f32> {
    let mut normals = vec![0.0f32; vertex_count * 3];

    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);

        let p0 = [
            positions[i0 * 3],
            positions[i0 * 3 + 1],
            positions[i0 * 3 + 2],
        ];
        let p1 = [
            positions[i1 * 3],
            positions[i1 * 3 + 1],
            positions[i1 * 3 + 2],
        ];
        let p2 = [
            positions[i2 * 3],
            positions[i2 * 3 + 1],
            positions[i2 * 3 + 2],
        ];

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
            if v < min[c] {
                min[c] = v;
            }
            if v > max[c] {
                max[c] = v;
            }
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
        target: Some(validation::Checked::Valid(
            buffer::Target::ElementArrayBuffer,
        )),
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
    /// Emissive factor (RGB linear), multiplied with `emissive_texture` or
    /// used directly when there is none. Defaults to black.
    pub emissive: [f32; 3],
    /// Indices into [`MaterialBundle::textures`] for the extra channels the
    /// FBX importer pulls off legacy Phong materials. The glTF/GLB writer in
    /// this module ignores them — they flow only into the `.material` graph —
    /// so non-FBX callers leave them `None`.
    pub emissive_texture: Option<usize>,
    pub occlusion_texture: Option<usize>,
    pub opacity_texture: Option<usize>,
    pub specular_texture: Option<usize>,
    /// Whether the material renders with alpha blending (legacy FBX
    /// transparency). Drives the graph's `alpha_mode`.
    pub alpha_blend: bool,
    /// Extended PBR channels (clearcoat, transmission, ior, anisotropy) read
    /// from modern FBX PBR materials. Texture URIs are model-relative, resolved
    /// at extraction time. Default for legacy Phong / OBJ.
    pub advanced: renzora::core::PbrAdvanced,
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
            if v < min[c] {
                min[c] = v;
            }
            if v > max[c] {
                max[c] = v;
            }
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
        (
            pos_offset,
            pos_bytes.len(),
            Some(buffer::Target::ArrayBuffer),
        ),
        (
            norm_offset,
            norm_bytes.len(),
            Some(buffer::Target::ArrayBuffer),
        ),
        (tc_offset, tc_bytes.len(), Some(buffer::Target::ArrayBuffer)),
        (
            idx_offset,
            idx_bytes.len(),
            Some(buffer::Target::ElementArrayBuffer),
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ─── byte casting ───────────────────────────────────────────────────

    #[test]
    fn cast_f32_little_endian() {
        let bytes = cast_f32_to_bytes(&[1.0f32, -2.0]);
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..4], &1.0f32.to_le_bytes());
        assert_eq!(&bytes[4..8], &(-2.0f32).to_le_bytes());
    }

    #[test]
    fn cast_u32_little_endian() {
        let bytes = cast_u32_to_bytes(&[1u32, 0x01020304]);
        assert_eq!(bytes.len(), 8);
        assert_eq!(&bytes[0..4], &[1, 0, 0, 0]);
        assert_eq!(&bytes[4..8], &[0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn cast_empty_slices() {
        assert!(cast_f32_to_bytes(&[]).is_empty());
        assert!(cast_u32_to_bytes(&[]).is_empty());
    }

    // ─── name sanitizing ────────────────────────────────────────────────

    #[test]
    fn sanitize_keeps_safe_chars() {
        assert_eq!(sanitize_name("abc_DEF-1.2"), "abc_DEF-1.2");
    }

    #[test]
    fn sanitize_replaces_unsafe_chars() {
        assert_eq!(sanitize_name("a b/c\\d"), "a_b_c_d");
    }

    #[test]
    fn sanitize_empty_falls_back() {
        assert_eq!(sanitize_name(""), "texture");
    }

    // ─── image format sniffing ──────────────────────────────────────────

    #[test]
    fn sniff_known_magic_bytes() {
        assert_eq!(sniff_image_ext(&[0x89, 0x50, 0x4E, 0x47, 0, 0]), "png");
        assert_eq!(sniff_image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpg");
        assert_eq!(sniff_image_ext(b"DDS  abc"), "dds");
        assert_eq!(sniff_image_ext(b"GIF89a..."), "gif");
        assert_eq!(sniff_image_ext(b"BM......"), "bmp");
    }

    #[test]
    fn sniff_webp_needs_riff_and_webp() {
        let mut data = b"RIFF".to_vec();
        data.extend_from_slice(&[0, 0, 0, 0]); // size
        data.extend_from_slice(b"WEBP");
        assert_eq!(sniff_image_ext(&data), "webp");
        // RIFF without WEBP fourcc should not match webp.
        let mut other = b"RIFF".to_vec();
        other.extend_from_slice(&[0, 0, 0, 0]);
        other.extend_from_slice(b"WAVE");
        assert_eq!(sniff_image_ext(&other), "bin");
    }

    #[test]
    fn sniff_unknown_is_bin() {
        assert_eq!(sniff_image_ext(b"hello world"), "bin");
        assert_eq!(sniff_image_ext(&[]), "bin");
    }

    // ─── flat normal generation ─────────────────────────────────────────

    #[test]
    fn flat_normals_single_triangle_in_xy_plane() {
        // Triangle wound CCW in the XY plane → normal +Z.
        let positions = [
            0.0, 0.0, 0.0, // v0
            1.0, 0.0, 0.0, // v1
            0.0, 1.0, 0.0, // v2
        ];
        let indices = [0u32, 1, 2];
        let normals = generate_flat_normals(&positions, &indices, 3);
        assert_eq!(normals.len(), 9);
        for v in 0..3 {
            assert!((normals[v * 3] - 0.0).abs() < 1e-6);
            assert!((normals[v * 3 + 1] - 0.0).abs() < 1e-6);
            assert!((normals[v * 3 + 2] - 1.0).abs() < 1e-6, "vertex {} z", v);
        }
    }

    #[test]
    fn flat_normals_unreferenced_vertex_defaults_up() {
        // A vertex never touched by a triangle gets the +Y fallback.
        let positions = [0.0, 0.0, 0.0]; // single, unreferenced vertex
        let indices: [u32; 0] = [];
        let normals = generate_flat_normals(&positions, &indices, 1);
        assert_eq!(normals, vec![0.0, 1.0, 0.0]);
    }

    // ─── build_glb end-to-end (no GPU, pure bytes) ──────────────────────

    #[test]
    fn build_glb_produces_valid_container() {
        // A single triangle.
        let positions = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let normals = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let texcoords = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let indices = [0u32, 1, 2];

        let glb = build_glb(
            &positions,
            &normals,
            &texcoords,
            &indices,
            &MaterialBundle::default(),
        )
        .expect("build_glb should succeed");

        // GLB magic "glTF", version 2, and length matches buffer.
        assert_eq!(&glb[0..4], b"glTF");
        let version = u32::from_le_bytes([glb[4], glb[5], glb[6], glb[7]]);
        assert_eq!(version, 2);
        let total_len = u32::from_le_bytes([glb[8], glb[9], glb[10], glb[11]]) as usize;
        assert_eq!(total_len, glb.len());

        // The JSON chunk should mention the accessor count for the triangle.
        let json_len = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let json = &glb[20..20 + json_len];
        let text = String::from_utf8_lossy(json);
        assert!(text.contains("\"meshes\""));
        assert!(text.contains("POSITION"));
    }

    #[test]
    fn build_glb_with_material_references_material_zero() {
        let positions = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let normals = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let texcoords = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let indices = [0u32, 1, 2];

        let bundle = MaterialBundle {
            materials: vec![PbrMaterialDef {
                name: "mat".into(),
                base_color: [1.0, 0.0, 0.0, 1.0],
                base_color_texture: None,
                normal_texture: None,
                metallic: 0.0,
                roughness: 0.5,
                emissive: [0.0, 0.0, 0.0],
                emissive_texture: None,
                occlusion_texture: None,
                opacity_texture: None,
                specular_texture: None,
                alpha_blend: false,
                advanced: renzora::core::PbrAdvanced::default(),
            }],
            textures: Vec::new(),
        };

        let glb = build_glb(&positions, &normals, &texcoords, &indices, &bundle).unwrap();
        let json_len = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let text = String::from_utf8_lossy(&glb[20..20 + json_len]);
        assert!(text.contains("\"materials\""));
        // The primitive references material index 0.
        assert!(text.contains("\"material\":0"));
    }
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
        let sampler = texture::Sampler {
            mag_filter: Some(validation::Checked::Valid(texture::MagFilter::Linear)),
            min_filter: Some(validation::Checked::Valid(
                texture::MinFilter::LinearMipmapLinear,
            )),
            wrap_s: validation::Checked::Valid(texture::WrappingMode::Repeat),
            wrap_t: validation::Checked::Valid(texture::WrappingMode::Repeat),
            ..Default::default()
        };
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
        let mut m = Material {
            alpha_mode: validation::Checked::Valid(material::AlphaMode::Opaque),
            ..Default::default()
        };
        m.pbr_metallic_roughness.base_color_factor = material::PbrBaseColorFactor(mat.base_color);
        m.pbr_metallic_roughness.base_color_texture = base_tex;
        m.pbr_metallic_roughness.metallic_factor = material::StrengthFactor(mat.metallic);
        m.pbr_metallic_roughness.roughness_factor = material::StrengthFactor(mat.roughness);
        m.normal_texture = normal_tex;
        let _ = &mat.name; // name is behind the `names` feature; skip safely.
        root.materials.push(m);
    }
}
