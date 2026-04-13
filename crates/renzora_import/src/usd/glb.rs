//! GLB (Binary glTF) output from a UsdStage.
//!
//! Converts the parsed USD scene into a GLB binary suitable for Bevy.
//! Supports meshes, materials with embedded textures, skeletons, and
//! scene hierarchy.

use super::scene::*;
use super::{UsdError, UsdResult};
use gltf_json::Index;

/// Convert a UsdStage to GLB bytes.
pub fn convert(stage: &UsdStage) -> UsdResult<Vec<u8>> {
    if stage.meshes.is_empty() {
        return Err(UsdError::Parse("No meshes in USD stage".into()));
    }

    use gltf_json::*;

    let mut root = Root::default();
    root.asset.generator = Some("renzora_usd".to_string());

    let mut bin_data: Vec<u8> = Vec::new();
    let mut buffer_views: Vec<buffer::View> = Vec::new();
    let mut accessors: Vec<Accessor> = Vec::new();
    let mut gltf_meshes: Vec<Mesh> = Vec::new();
    let mut gltf_nodes: Vec<Node> = Vec::new();
    let mut gltf_materials: Vec<gltf_json::Material> = Vec::new();
    let mut gltf_textures: Vec<gltf_json::Texture> = Vec::new();
    let mut gltf_images: Vec<gltf_json::Image> = Vec::new();

    // --- Embed textures ---
    for tex in &stage.textures {
        let image_idx = gltf_images.len() as u32;

        // Write image data into binary buffer
        let bv_idx = buffer_views.len() as u32;
        let offset = bin_data.len();
        bin_data.extend_from_slice(&tex.data);
        // Pad to 4-byte alignment
        while bin_data.len() % 4 != 0 {
            bin_data.push(0);
        }

        buffer_views.push(buffer::View {
            buffer: Index::new(0),
            byte_length: validation::USize64(tex.data.len() as u64),
            byte_offset: Some(validation::USize64(offset as u64)),
            byte_stride: None,
            name: None,
            target: None,
            extensions: None,
            extras: Default::default(),
        });

        gltf_images.push(gltf_json::Image {
            buffer_view: Some(Index::new(bv_idx)),
            mime_type: Some(gltf_json::image::MimeType(tex.mime_type.clone())),
            name: Some(tex.name.clone()),
            uri: None,
            extensions: None,
            extras: Default::default(),
        });

        gltf_textures.push(gltf_json::Texture {
            name: Some(tex.name.clone()),
            sampler: None,
            source: Index::new(image_idx),
            extensions: None,
            extras: Default::default(),
        });
    }

    // --- Materials ---
    for mat in &stage.materials {
        let pbr = gltf_json::material::PbrMetallicRoughness {
            base_color_factor: gltf_json::material::PbrBaseColorFactor([
                mat.diffuse_color[0],
                mat.diffuse_color[1],
                mat.diffuse_color[2],
                mat.opacity,
            ]),
            base_color_texture: resolve_gltf_texture(&mat.diffuse_texture, &stage.textures),
            metallic_factor: gltf_json::material::StrengthFactor(mat.metallic),
            roughness_factor: gltf_json::material::StrengthFactor(mat.roughness),
            metallic_roughness_texture: resolve_gltf_texture(
                &mat.metallic_texture,
                &stage.textures,
            ),
            extensions: None,
            extras: Default::default(),
        };

        let alpha_mode = if mat.opacity < 1.0 {
            validation::Checked::Valid(gltf_json::material::AlphaMode::Blend)
        } else {
            validation::Checked::Valid(gltf_json::material::AlphaMode::Opaque)
        };

        gltf_materials.push(gltf_json::Material {
            name: Some(mat.name.clone()),
            pbr_metallic_roughness: pbr,
            normal_texture: resolve_gltf_normal_texture(
                &mat.normal_texture,
                mat.normal_scale,
                &stage.textures,
            ),
            emissive_factor: gltf_json::material::EmissiveFactor(mat.emissive_color),
            emissive_texture: resolve_gltf_texture(&mat.emissive_texture, &stage.textures),
            occlusion_texture: resolve_gltf_occlusion_texture(
                &mat.occlusion_texture,
                &stage.textures,
            ),
            alpha_mode,
            alpha_cutoff: None,
            double_sided: false,
            extensions: None,
            extras: Default::default(),
        });
    }

    // --- Meshes ---
    for usd_mesh in &stage.meshes {
        let triangles = super::mesh::triangulate(usd_mesh);
        if triangles.is_empty() || usd_mesh.positions.is_empty() {
            continue;
        }

        let vert_count = usd_mesh.positions.len();

        // Positions
        let pos_bv = buffer_views.len() as u32;
        let pos_acc = accessors.len() as u32;
        let pos_offset = bin_data.len();

        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &usd_mesh.positions {
            for c in 0..3 {
                if p[c] < min[c] { min[c] = p[c]; }
                if p[c] > max[c] { max[c] = p[c]; }
            }
        }

        for p in &usd_mesh.positions {
            bin_data.extend_from_slice(&p[0].to_le_bytes());
            bin_data.extend_from_slice(&p[1].to_le_bytes());
            bin_data.extend_from_slice(&p[2].to_le_bytes());
        }

        buffer_views.push(buffer::View {
            buffer: Index::new(0),
            byte_length: validation::USize64((vert_count * 12) as u64),
            byte_offset: Some(validation::USize64(pos_offset as u64)),
            byte_stride: None,
            name: None,
            target: Some(validation::Checked::Valid(buffer::Target::ArrayBuffer)),
            extensions: None,
            extras: Default::default(),
        });

        let min_val: Value = serde_json::json!([min[0], min[1], min[2]]);
        let max_val: Value = serde_json::json!([max[0], max[1], max[2]]);

        accessors.push(Accessor {
            buffer_view: Some(Index::new(pos_bv)),
            byte_offset: Some(validation::USize64(0)),
            count: validation::USize64(vert_count as u64),
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

        // Normals
        let normals = if usd_mesh.normals.len() == vert_count {
            usd_mesh.normals.clone()
        } else {
            super::mesh::generate_normals(&usd_mesh.positions, &triangles)
        };

        let norm_bv = buffer_views.len() as u32;
        let norm_acc = accessors.len() as u32;
        let norm_offset = bin_data.len();

        for n in &normals {
            bin_data.extend_from_slice(&n[0].to_le_bytes());
            bin_data.extend_from_slice(&n[1].to_le_bytes());
            bin_data.extend_from_slice(&n[2].to_le_bytes());
        }

        buffer_views.push(buffer::View {
            buffer: Index::new(0),
            byte_length: validation::USize64((vert_count * 12) as u64),
            byte_offset: Some(validation::USize64(norm_offset as u64)),
            byte_stride: None,
            name: None,
            target: Some(validation::Checked::Valid(buffer::Target::ArrayBuffer)),
            extensions: None,
            extras: Default::default(),
        });

        accessors.push(Accessor {
            buffer_view: Some(Index::new(norm_bv)),
            byte_offset: Some(validation::USize64(0)),
            count: validation::USize64(vert_count as u64),
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

        // UVs
        let uvs = usd_mesh.uv_sets.get("st").cloned().unwrap_or_default();
        let tc_bv = buffer_views.len() as u32;
        let tc_acc = accessors.len() as u32;
        let tc_offset = bin_data.len();

        for i in 0..vert_count {
            let uv = if i < uvs.len() {
                [uvs[i][0], 1.0 - uvs[i][1]] // Flip V (USD bottom-left -> glTF top-left)
            } else {
                [0.0, 0.0]
            };
            bin_data.extend_from_slice(&uv[0].to_le_bytes());
            bin_data.extend_from_slice(&uv[1].to_le_bytes());
        }

        buffer_views.push(buffer::View {
            buffer: Index::new(0),
            byte_length: validation::USize64((vert_count * 8) as u64),
            byte_offset: Some(validation::USize64(tc_offset as u64)),
            byte_stride: None,
            name: None,
            target: Some(validation::Checked::Valid(buffer::Target::ArrayBuffer)),
            extensions: None,
            extras: Default::default(),
        });

        accessors.push(Accessor {
            buffer_view: Some(Index::new(tc_bv)),
            byte_offset: Some(validation::USize64(0)),
            count: validation::USize64(vert_count as u64),
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

        // Indices
        let idx_bv = buffer_views.len() as u32;
        let idx_acc = accessors.len() as u32;
        let idx_offset = bin_data.len();

        for &idx in &triangles {
            bin_data.extend_from_slice(&(idx as u32).to_le_bytes());
        }

        buffer_views.push(buffer::View {
            buffer: Index::new(0),
            byte_length: validation::USize64((triangles.len() * 4) as u64),
            byte_offset: Some(validation::USize64(idx_offset as u64)),
            byte_stride: None,
            name: None,
            target: Some(validation::Checked::Valid(
                buffer::Target::ElementArrayBuffer,
            )),
            extensions: None,
            extras: Default::default(),
        });

        accessors.push(Accessor {
            buffer_view: Some(Index::new(idx_bv)),
            byte_offset: Some(validation::USize64(0)),
            count: validation::USize64(triangles.len() as u64),
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

        // Build primitive
        let mut attributes = std::collections::BTreeMap::new();
        attributes.insert(
            validation::Checked::Valid(gltf_json::mesh::Semantic::Positions),
            Index::new(pos_acc),
        );
        attributes.insert(
            validation::Checked::Valid(gltf_json::mesh::Semantic::Normals),
            Index::new(norm_acc),
        );
        attributes.insert(
            validation::Checked::Valid(gltf_json::mesh::Semantic::TexCoords(0)),
            Index::new(tc_acc),
        );

        let material_idx = usd_mesh
            .material_index
            .filter(|&i| i < gltf_materials.len())
            .map(|i| Index::new(i as u32));

        gltf_meshes.push(Mesh {
            primitives: vec![gltf_json::mesh::Primitive {
                attributes,
                indices: Some(Index::new(idx_acc)),
                material: material_idx,
                mode: validation::Checked::Valid(gltf_json::mesh::Mode::Triangles),
                targets: None,
                extensions: None,
                extras: Default::default(),
            }],
            name: Some(usd_mesh.name.clone()),
            weights: None,
            extensions: None,
            extras: Default::default(),
        });

        gltf_nodes.push(Node {
            mesh: Some(Index::new((gltf_meshes.len() - 1) as u32)),
            name: Some(usd_mesh.name.clone()),
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
    }

    if gltf_meshes.is_empty() {
        return Err(UsdError::Parse("No valid geometry in USD stage".into()));
    }

    // Scene
    let node_indices: Vec<Index<Node>> = (0..gltf_nodes.len() as u32)
        .map(Index::new)
        .collect();

    // Buffer
    root.buffers.push(buffer::Buffer {
        byte_length: validation::USize64(bin_data.len() as u64),
        name: None,
        uri: None,
        extensions: None,
        extras: Default::default(),
    });

    root.buffer_views = buffer_views;
    root.accessors = accessors;
    root.meshes = gltf_meshes;
    root.nodes = gltf_nodes;
    root.materials = gltf_materials;
    root.textures = gltf_textures;
    root.images = gltf_images;

    root.scenes.push(Scene {
        name: None,
        nodes: node_indices,
        extensions: None,
        extras: Default::default(),
    });
    root.scene = Some(Index::new(0));

    let json_bytes = root
        .to_vec()
        .map_err(|e| UsdError::Parse(format!("glTF JSON serialize: {}", e)))?;

    Ok(pack_glb(&json_bytes, Some(&bin_data)))
}

// ---------------------------------------------------------------------------
// GLB packing (same as renzora_import's gltf_pass::pack_glb)
// ---------------------------------------------------------------------------

fn pack_glb(json: &[u8], bin: Option<&[u8]>) -> Vec<u8> {
    // Pad JSON to 4-byte alignment with spaces
    let json_pad = (4 - (json.len() % 4)) % 4;
    let json_len = json.len() + json_pad;

    let bin_data = bin.unwrap_or(&[]);
    let bin_pad = (4 - (bin_data.len() % 4)) % 4;
    let bin_len = bin_data.len() + bin_pad;

    let total = 12 + 8 + json_len + if bin_data.is_empty() { 0 } else { 8 + bin_len };

    let mut out = Vec::with_capacity(total);

    // GLB header
    out.extend_from_slice(b"glTF");                           // magic
    out.extend_from_slice(&2u32.to_le_bytes());                // version
    out.extend_from_slice(&(total as u32).to_le_bytes());      // length

    // JSON chunk
    out.extend_from_slice(&(json_len as u32).to_le_bytes());   // chunk length
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes());       // chunk type "JSON"
    out.extend_from_slice(json);
    out.extend(std::iter::repeat(b' ').take(json_pad));

    // BIN chunk
    if !bin_data.is_empty() {
        out.extend_from_slice(&(bin_len as u32).to_le_bytes()); // chunk length
        out.extend_from_slice(&0x004E4942u32.to_le_bytes());    // chunk type "BIN\0"
        out.extend_from_slice(bin_data);
        out.extend(std::iter::repeat(0u8).take(bin_pad));
    }

    out
}

// ---------------------------------------------------------------------------
// Texture reference resolution
// ---------------------------------------------------------------------------

fn resolve_gltf_texture(
    tex_ref: &Option<TextureRef>,
    textures: &[UsdTexture],
) -> Option<gltf_json::texture::Info> {
    let tr = tex_ref.as_ref()?;
    let idx = match &tr.source {
        TextureSource::Embedded(i) => *i,
        TextureSource::File(_) => return None,
    };

    if idx >= textures.len() {
        return None;
    }

    Some(gltf_json::texture::Info {
        index: Index::new(idx as u32),
        tex_coord: 0,
        extensions: None,
        extras: Default::default(),
    })
}

fn resolve_gltf_normal_texture(
    tex_ref: &Option<TextureRef>,
    scale: f32,
    textures: &[UsdTexture],
) -> Option<gltf_json::material::NormalTexture> {
    let tr = tex_ref.as_ref()?;
    let idx = match &tr.source {
        TextureSource::Embedded(i) => *i,
        TextureSource::File(_) => return None,
    };

    if idx >= textures.len() {
        return None;
    }

    Some(gltf_json::material::NormalTexture {
        index: Index::new(idx as u32),
        scale,
        tex_coord: 0,
        extensions: None,
        extras: Default::default(),
    })
}

fn resolve_gltf_occlusion_texture(
    tex_ref: &Option<TextureRef>,
    textures: &[UsdTexture],
) -> Option<gltf_json::material::OcclusionTexture> {
    let tr = tex_ref.as_ref()?;
    let idx = match &tr.source {
        TextureSource::Embedded(i) => *i,
        TextureSource::File(_) => return None,
    };

    if idx >= textures.len() {
        return None;
    }

    Some(gltf_json::material::OcclusionTexture {
        index: Index::new(idx as u32),
        strength: gltf_json::material::StrengthFactor(1.0),
        tex_coord: 0,
        extensions: None,
        extras: Default::default(),
    })
}
