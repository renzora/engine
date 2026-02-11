//! GLB binary builder that accumulates meshes, materials, and nodes
//! and produces a valid GLB file (JSON chunk + BIN chunk).

use gltf_json as json;
use json::validation::Checked::Valid;
use std::mem;

/// A single mesh to be added to the GLB
pub struct MeshData {
    pub name: Option<String>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Option<Vec<[f32; 3]>>,
    pub tex_coords: Option<Vec<[f32; 2]>>,
    pub indices: Option<Vec<u32>>,
    pub material_index: Option<usize>,
}

/// A material to be added to the GLB
pub struct MaterialData {
    pub name: Option<String>,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub base_color_texture_index: Option<usize>,
}

/// A texture (embedded image data) to be added to the GLB
pub struct TextureData {
    pub name: Option<String>,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// A node in the scene hierarchy
pub struct NodeData {
    pub name: Option<String>,
    pub mesh_index: Option<usize>,
    pub children: Vec<usize>,
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
}

/// Builder that accumulates geometry/materials and produces a GLB binary
pub struct GlbBuilder {
    meshes: Vec<MeshData>,
    materials: Vec<MaterialData>,
    textures: Vec<TextureData>,
    nodes: Vec<NodeData>,
    root_scale: Option<f32>,
    root_rotation: Option<[f32; 4]>,
}

impl GlbBuilder {
    pub fn new() -> Self {
        Self {
            meshes: Vec::new(),
            materials: Vec::new(),
            textures: Vec::new(),
            nodes: Vec::new(),
            root_scale: None,
            root_rotation: None,
        }
    }

    pub fn add_mesh(&mut self, mesh: MeshData) -> usize {
        let idx = self.meshes.len();
        self.meshes.push(mesh);
        idx
    }

    pub fn add_material(&mut self, material: MaterialData) -> usize {
        let idx = self.materials.len();
        self.materials.push(material);
        idx
    }

    pub fn add_texture(&mut self, texture: TextureData) -> usize {
        let idx = self.textures.len();
        self.textures.push(texture);
        idx
    }

    pub fn add_node(&mut self, node: NodeData) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }

    /// Set a uniform scale on the root transform
    pub fn set_root_scale(&mut self, scale: f32) {
        self.root_scale = Some(scale);
    }

    /// Set a rotation on the root transform (quaternion [x, y, z, w])
    pub fn set_root_rotation(&mut self, rotation: [f32; 4]) {
        self.root_rotation = Some(rotation);
    }

    /// Build a valid GLB binary from accumulated data
    pub fn build(self) -> Vec<u8> {
        let mut bin_data: Vec<u8> = Vec::new();
        let mut accessors: Vec<json::Accessor> = Vec::new();
        let mut buffer_views: Vec<json::buffer::View> = Vec::new();
        let mut json_meshes: Vec<json::Mesh> = Vec::new();
        let mut json_materials: Vec<json::Material> = Vec::new();
        let mut json_textures: Vec<json::Texture> = Vec::new();
        let mut json_images: Vec<json::Image> = Vec::new();
        let mut json_nodes: Vec<json::Node> = Vec::new();

        // Build textures/images first (materials reference them)
        for tex in &self.textures {
            let image_index = json_images.len();

            // Pad to 4-byte alignment
            while bin_data.len() % 4 != 0 {
                bin_data.push(0);
            }

            let offset = bin_data.len();
            bin_data.extend_from_slice(&tex.data);
            let length = tex.data.len();

            let bv_index = buffer_views.len();
            buffer_views.push(json::buffer::View {
                buffer: json::Index::new(0),
                byte_length: json::validation::USize64(length as u64),
                byte_offset: Some(json::validation::USize64(offset as u64)),
                byte_stride: None,
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
                target: None,
            });

            json_images.push(json::Image {
                buffer_view: Some(json::Index::new(bv_index as u32)),
                mime_type: Some(json::image::MimeType(tex.mime_type.clone())),
                name: tex.name.clone(),
                uri: None,
                extensions: Default::default(),
                extras: Default::default(),
            });

            json_textures.push(json::Texture {
                name: tex.name.clone(),
                sampler: None,
                source: json::Index::new(image_index as u32),
                extensions: Default::default(),
                extras: Default::default(),
            });
        }

        // Build materials
        for mat in &self.materials {
            let base_color_texture = mat.base_color_texture_index.map(|idx| json::texture::Info {
                index: json::Index::new(idx as u32),
                tex_coord: 0,
                extensions: Default::default(),
                extras: Default::default(),
            });

            json_materials.push(json::Material {
                name: mat.name.clone(),
                pbr_metallic_roughness: json::material::PbrMetallicRoughness {
                    base_color_factor: json::material::PbrBaseColorFactor(mat.base_color),
                    base_color_texture,
                    metallic_factor: json::material::StrengthFactor(mat.metallic),
                    metallic_roughness_texture: None,
                    roughness_factor: json::material::StrengthFactor(mat.roughness),
                    extensions: Default::default(),
                    extras: Default::default(),
                },
                alpha_cutoff: None,
                alpha_mode: Valid(json::material::AlphaMode::Opaque),
                double_sided: false,
                normal_texture: None,
                occlusion_texture: None,
                emissive_texture: None,
                emissive_factor: json::material::EmissiveFactor([0.0, 0.0, 0.0]),
                extensions: Default::default(),
                extras: Default::default(),
            });
        }

        // Build meshes (each MeshData becomes a glTF mesh with one primitive)
        for mesh in &self.meshes {
            let primitive = build_primitive(
                mesh,
                &mut bin_data,
                &mut accessors,
                &mut buffer_views,
            );
            json_meshes.push(json::Mesh {
                name: mesh.name.clone(),
                primitives: vec![primitive],
                weights: None,
                extensions: Default::default(),
                extras: Default::default(),
            });
        }

        // Build nodes
        if self.nodes.is_empty() {
            // Auto-generate one node per mesh
            for (i, mesh) in self.meshes.iter().enumerate() {
                json_nodes.push(json::Node {
                    name: mesh.name.clone(),
                    mesh: Some(json::Index::new(i as u32)),
                    camera: None,
                    children: None,
                    extensions: Default::default(),
                    extras: Default::default(),
                    matrix: None,
                    rotation: None,
                    scale: None,
                    translation: None,
                    skin: None,
                    weights: None,
                });
            }
        } else {
            for node in &self.nodes {
                json_nodes.push(json::Node {
                    name: node.name.clone(),
                    mesh: node.mesh_index.map(|i| json::Index::new(i as u32)),
                    camera: None,
                    children: if node.children.is_empty() {
                        None
                    } else {
                        Some(node.children.iter().map(|&i| json::Index::new(i as u32)).collect())
                    },
                    extensions: Default::default(),
                    extras: Default::default(),
                    matrix: None,
                    rotation: node.rotation.map(json::scene::UnitQuaternion),
                    scale: node.scale.map(|s| [s[0], s[1], s[2]]),
                    translation: node.translation,
                    skin: None,
                    weights: None,
                });
            }
        }

        // Wrap everything in a root node if we need scale/rotation
        let scene_nodes: Vec<json::Index<json::Node>>;
        if self.root_scale.is_some() || self.root_rotation.is_some() {
            let child_indices: Vec<usize> = (0..json_nodes.len()).collect();
            let root_idx = json_nodes.len();
            json_nodes.push(json::Node {
                name: Some("Root".to_string()),
                mesh: None,
                camera: None,
                children: Some(child_indices.iter().map(|&i| json::Index::new(i as u32)).collect()),
                extensions: Default::default(),
                extras: Default::default(),
                matrix: None,
                rotation: self.root_rotation.map(json::scene::UnitQuaternion),
                scale: self.root_scale.map(|s| [s, s, s]),
                translation: None,
                skin: None,
                weights: None,
            });
            scene_nodes = vec![json::Index::new(root_idx as u32)];
        } else {
            scene_nodes = (0..json_nodes.len())
                .map(|i| json::Index::new(i as u32))
                .collect();
        }

        // Pad bin_data to 4-byte alignment
        while bin_data.len() % 4 != 0 {
            bin_data.push(0);
        }

        let buffer = json::Buffer {
            byte_length: json::validation::USize64(bin_data.len() as u64),
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            uri: None,
        };

        let scene = json::Scene {
            name: Some("Scene".to_string()),
            nodes: scene_nodes,
            extensions: Default::default(),
            extras: Default::default(),
        };

        let root = json::Root {
            accessors,
            buffers: vec![buffer],
            buffer_views,
            meshes: json_meshes,
            nodes: json_nodes,
            scenes: vec![scene],
            scene: Some(json::Index::new(0)),
            materials: json_materials,
            textures: json_textures,
            images: json_images,
            asset: json::Asset {
                version: "2.0".to_string(),
                generator: Some("Renzora Engine Import".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        // Serialize JSON
        let json_string = json::serialize::to_string(&root).expect("Failed to serialize glTF JSON");
        let mut json_bytes = json_string.into_bytes();
        // Pad JSON to 4-byte alignment with spaces
        while json_bytes.len() % 4 != 0 {
            json_bytes.push(b' ');
        }

        // Build GLB
        let total_length = 12 + 8 + json_bytes.len() + 8 + bin_data.len();
        let mut glb = Vec::with_capacity(total_length);

        // GLB header
        glb.extend_from_slice(&0x46546C67u32.to_le_bytes()); // magic "glTF"
        glb.extend_from_slice(&2u32.to_le_bytes()); // version
        glb.extend_from_slice(&(total_length as u32).to_le_bytes()); // total length

        // JSON chunk
        glb.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
        glb.extend_from_slice(&json_bytes);

        // BIN chunk
        glb.extend_from_slice(&(bin_data.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
        glb.extend_from_slice(&bin_data);

        glb
    }
}

/// Build a single primitive from mesh data, appending to the binary buffer
fn build_primitive(
    mesh: &MeshData,
    bin_data: &mut Vec<u8>,
    accessors: &mut Vec<json::Accessor>,
    buffer_views: &mut Vec<json::buffer::View>,
) -> json::mesh::Primitive {
    let mut attributes = std::collections::BTreeMap::new();

    // Positions (required)
    let pos_accessor = write_vec3_accessor(
        &mesh.positions,
        bin_data,
        accessors,
        buffer_views,
        true, // compute min/max
    );
    attributes.insert(
        Valid(json::mesh::Semantic::Positions),
        json::Index::new(pos_accessor),
    );

    // Normals (optional)
    if let Some(ref normals) = mesh.normals {
        let norm_accessor = write_vec3_accessor(
            normals,
            bin_data,
            accessors,
            buffer_views,
            false,
        );
        attributes.insert(
            Valid(json::mesh::Semantic::Normals),
            json::Index::new(norm_accessor),
        );
    }

    // Tex coords (optional)
    if let Some(ref tex_coords) = mesh.tex_coords {
        let uv_accessor = write_vec2_accessor(
            tex_coords,
            bin_data,
            accessors,
            buffer_views,
        );
        attributes.insert(
            Valid(json::mesh::Semantic::TexCoords(0)),
            json::Index::new(uv_accessor),
        );
    }

    // Indices (optional)
    let indices = mesh.indices.as_ref().map(|idx| {
        let accessor_idx = write_scalar_u32_accessor(
            idx,
            bin_data,
            accessors,
            buffer_views,
        );
        json::Index::new(accessor_idx)
    });

    let material = mesh.material_index.map(|i| json::Index::new(i as u32));

    json::mesh::Primitive {
        attributes,
        extensions: Default::default(),
        extras: Default::default(),
        indices,
        material,
        mode: Valid(json::mesh::Mode::Triangles),
        targets: None,
    }
}

fn write_vec3_accessor(
    data: &[[f32; 3]],
    bin_data: &mut Vec<u8>,
    accessors: &mut Vec<json::Accessor>,
    buffer_views: &mut Vec<json::buffer::View>,
    compute_bounds: bool,
) -> u32 {
    // Pad to 4-byte alignment
    while bin_data.len() % 4 != 0 {
        bin_data.push(0);
    }

    let offset = bin_data.len();
    let byte_length = data.len() * mem::size_of::<[f32; 3]>();

    for v in data {
        for f in v {
            bin_data.extend_from_slice(&f.to_le_bytes());
        }
    }

    let bv_index = buffer_views.len() as u32;
    buffer_views.push(json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: json::validation::USize64(byte_length as u64),
        byte_offset: Some(json::validation::USize64(offset as u64)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ArrayBuffer)),
    });

    let (min, max) = if compute_bounds && !data.is_empty() {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for v in data {
            for i in 0..3 {
                min[i] = min[i].min(v[i]);
                max[i] = max[i].max(v[i]);
            }
        }
        (
            Some(json::Value::from(vec![
                json::Value::from(min[0]),
                json::Value::from(min[1]),
                json::Value::from(min[2]),
            ])),
            Some(json::Value::from(vec![
                json::Value::from(max[0]),
                json::Value::from(max[1]),
                json::Value::from(max[2]),
            ])),
        )
    } else {
        (None, None)
    };

    let acc_index = accessors.len() as u32;
    accessors.push(json::Accessor {
        buffer_view: Some(json::Index::new(bv_index)),
        byte_offset: None,
        count: json::validation::USize64(data.len() as u64),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Vec3),
        min,
        max,
        name: None,
        normalized: false,
        sparse: None,
    });

    acc_index
}

fn write_vec2_accessor(
    data: &[[f32; 2]],
    bin_data: &mut Vec<u8>,
    accessors: &mut Vec<json::Accessor>,
    buffer_views: &mut Vec<json::buffer::View>,
) -> u32 {
    while bin_data.len() % 4 != 0 {
        bin_data.push(0);
    }

    let offset = bin_data.len();
    let byte_length = data.len() * mem::size_of::<[f32; 2]>();

    for v in data {
        for f in v {
            bin_data.extend_from_slice(&f.to_le_bytes());
        }
    }

    let bv_index = buffer_views.len() as u32;
    buffer_views.push(json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: json::validation::USize64(byte_length as u64),
        byte_offset: Some(json::validation::USize64(offset as u64)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ArrayBuffer)),
    });

    let acc_index = accessors.len() as u32;
    accessors.push(json::Accessor {
        buffer_view: Some(json::Index::new(bv_index)),
        byte_offset: None,
        count: json::validation::USize64(data.len() as u64),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::F32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Vec2),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
    });

    acc_index
}

fn write_scalar_u32_accessor(
    data: &[u32],
    bin_data: &mut Vec<u8>,
    accessors: &mut Vec<json::Accessor>,
    buffer_views: &mut Vec<json::buffer::View>,
) -> u32 {
    while bin_data.len() % 4 != 0 {
        bin_data.push(0);
    }

    let offset = bin_data.len();
    let byte_length = data.len() * mem::size_of::<u32>();

    for &v in data {
        bin_data.extend_from_slice(&v.to_le_bytes());
    }

    let bv_index = buffer_views.len() as u32;
    buffer_views.push(json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: json::validation::USize64(byte_length as u64),
        byte_offset: Some(json::validation::USize64(offset as u64)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        name: None,
        target: Some(Valid(json::buffer::Target::ElementArrayBuffer)),
    });

    let acc_index = accessors.len() as u32;
    accessors.push(json::Accessor {
        buffer_view: Some(json::Index::new(bv_index)),
        byte_offset: None,
        count: json::validation::USize64(data.len() as u64),
        component_type: Valid(json::accessor::GenericComponentType(
            json::accessor::ComponentType::U32,
        )),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(json::accessor::Type::Scalar),
        min: None,
        max: None,
        name: None,
        normalized: false,
        sparse: None,
    });

    acc_index
}
