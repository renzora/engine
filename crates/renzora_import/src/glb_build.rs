//! The shared glTF/GLB writer.
//!
//! Every format converter — FBX, OBJ, USD, Collada, Alembic, STL, PLY — ends by
//! handing its geometry and materials to the builders here, and the result goes
//! on to `gltf_pass::finish_converted_glb`. Nothing in this module knows or
//! cares which format the data came from.
//!
//! It used to live inside `obj.rs`, which made it look like OBJ-specific code
//! and hid the fact that a change here lands in every importer at once.

use std::collections::HashMap;

use crate::convert::ImportError;
use crate::gltf_pass::pack_glb;

/// One primitive's worth of triangles and the material it wears.
///
/// A GLTF primitive carries exactly one material, so a source file with many
/// materials has to become many primitives over a shared vertex buffer. Every
/// group here indexes the same positions/normals/texcoords; only the triangle
/// list and the material differ.
#[derive(Debug, Clone, Default)]
pub(crate) struct MaterialGroup {
    /// Index into [`MaterialBundle::materials`], or `None` for triangles the
    /// source left unassigned.
    pub material: Option<usize>,
    pub indices: Vec<u32>,
}

/// One group's geometry after compaction: its own vertices, and indices
/// numbered from zero within them.
struct CompactGroup {
    material: Option<usize>,
    /// Where this group's vertices start in the merged vertex buffer, and how
    /// many there are.
    vertex_start: usize,
    vertex_count: usize,
    /// Group-local triangle list — glTF indices are relative to the accessor,
    /// so these count from 0 regardless of `vertex_start`.
    indices: Vec<u32>,
    /// POSITION accessor `min`/`max`, which the spec requires and which Bevy
    /// uses for the mesh AABB. Per-group, so culling is per-group too.
    min: [f32; 3],
    max: [f32; 3],
}

/// Rebuild the vertex buffer so each material group owns a private, contiguous
/// range of it, and renumber each group's indices into that range.
///
/// **This is what keeps a multi-material model loadable.** A glTF primitive may
/// legally point at an accessor another primitive also uses, but nothing
/// downstream shares the result: Bevy builds one `Mesh` asset per primitive and
/// reads that primitive's attribute accessors in full. Point 132 primitives at
/// one 2.1-million-vertex accessor and you get 132 copies of all 2.1 million
/// vertices — 8.8 GB for a scene whose triangles fit in 140 MB, and an
/// immediate `Caught rendering error: Out of Memory`. Compacting per group
/// brings the total back to roughly the original vertex count, with only the
/// duplication that material seams genuinely require.
fn compact_groups(
    positions: &[f32],
    normals: &[f32],
    texcoords: &[f32],
    joints: &[[u16; 4]],
    weights: &[[f32; 4]],
    groups: &[MaterialGroup],
) -> (Vertices, Vec<CompactGroup>) {
    let source_vertex_count = positions.len() / 3;
    let skinned = !joints.is_empty();

    let mut out = Vertices::default();
    let mut compacted = Vec::with_capacity(groups.len());
    // Source vertex → its index within the group currently being built.
    // `u32::MAX` means "not yet emitted for this group"; the stamp array avoids
    // clearing a two-million-entry map once per material.
    let mut remap = vec![u32::MAX; source_vertex_count];
    let mut stamp = vec![u32::MAX; source_vertex_count];

    for (group_id, group) in groups.iter().enumerate() {
        if group.indices.is_empty() {
            continue;
        }
        let group_id = group_id as u32;
        let vertex_start = out.positions.len() / 3;
        let mut indices = Vec::with_capacity(group.indices.len());
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];

        for &source in &group.indices {
            let source = source as usize;
            if source >= source_vertex_count {
                continue;
            }
            if stamp[source] != group_id {
                stamp[source] = group_id;
                remap[source] = (out.positions.len() / 3 - vertex_start) as u32;

                for c in 0..3 {
                    let v = positions[source * 3 + c];
                    out.positions.push(v);
                    if v < min[c] {
                        min[c] = v;
                    }
                    if v > max[c] {
                        max[c] = v;
                    }
                    out.normals.push(normals.get(source * 3 + c).copied().unwrap_or(0.0));
                }
                for c in 0..2 {
                    out.texcoords
                        .push(texcoords.get(source * 2 + c).copied().unwrap_or(0.0));
                }
                if skinned {
                    out.joints.push(joints.get(source).copied().unwrap_or([0; 4]));
                    out.weights
                        .push(weights.get(source).copied().unwrap_or([0.0; 4]));
                }
            }
            indices.push(remap[source]);
        }

        if indices.is_empty() {
            continue;
        }
        compacted.push(CompactGroup {
            material: group.material,
            vertex_start,
            vertex_count: out.positions.len() / 3 - vertex_start,
            indices,
            min,
            max,
        });
    }

    (out, compacted)
}

/// The merged vertex buffer [`compact_groups`] produces.
#[derive(Default)]
struct Vertices {
    positions: Vec<f32>,
    normals: Vec<f32>,
    texcoords: Vec<f32>,
    joints: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
}

/// Lay every group's triangles out back-to-back in one buffer, returning the
/// bytes and each group's `(byte_offset, index_count)` within them.
///
/// Groups share a single index buffer view; each gets its own accessor into it.
fn pack_group_indices(groups: &[CompactGroup]) -> (Vec<u8>, Vec<(usize, usize)>) {
    let total: usize = groups.iter().map(|g| g.indices.len()).sum();
    let mut bytes = Vec::with_capacity(total * 4);
    let mut spans = Vec::with_capacity(groups.len());
    for group in groups {
        spans.push((bytes.len(), group.indices.len()));
        bytes.extend_from_slice(&cast_u32_to_bytes(&group.indices));
    }
    (bytes, spans)
}

/// Which buffer view holds each vertex attribute, so the two builders can
/// share the accessor emitter despite laying their binary chunks out
/// differently.
struct VertexViews {
    positions: u32,
    normals: u32,
    texcoords: u32,
    indices: u32,
    /// Skinning attributes, when the mesh has them.
    skin: Option<(u32, u32)>,
}

impl VertexViews {
    fn unskinned() -> Self {
        Self {
            positions: 0,
            normals: 1,
            texcoords: 2,
            indices: 3,
            skin: None,
        }
    }
}

/// Emit one set of attribute accessors per group and return the primitives
/// that read them.
///
/// Each group's accessors are offset into the shared buffer views so they cover
/// only that group's slice — this is the half of [`compact_groups`] that makes
/// the saving real. Pointing every primitive at one full-length accessor is
/// valid glTF and catastrophic in practice; see that function for the numbers.
fn push_group_accessors(
    root: &mut gltf_json::Root,
    groups: &[CompactGroup],
    idx_spans: &[(usize, usize)],
    views: VertexViews,
) -> Vec<gltf_json::mesh::Primitive> {
    use gltf_json::*;

    let mut primitives = Vec::with_capacity(groups.len());
    for (group, (idx_offset, idx_count)) in groups.iter().zip(idx_spans) {
        let mut vertex_accessor = |view: u32,
                                   type_: accessor::Type,
                                   component: accessor::ComponentType,
                                   stride: usize,
                                   bounds: Option<([f32; 3], [f32; 3])>|
         -> Index<Accessor> {
            let index = root.accessors.len() as u32;
            root.accessors.push(Accessor {
                buffer_view: Some(Index::new(view)),
                byte_offset: Some(validation::USize64(
                    (group.vertex_start * stride) as u64,
                )),
                count: validation::USize64(group.vertex_count as u64),
                component_type: validation::Checked::Valid(accessor::GenericComponentType(
                    component,
                )),
                type_: validation::Checked::Valid(type_),
                min: bounds.map(|(min, _)| serde_json::json!([min[0], min[1], min[2]])),
                max: bounds.map(|(_, max)| serde_json::json!([max[0], max[1], max[2]])),
                name: None,
                normalized: false,
                sparse: None,
                extensions: None,
                extras: Default::default(),
            });
            Index::new(index)
        };

        let mut attributes = std::collections::BTreeMap::new();
        attributes.insert(
            validation::Checked::Valid(mesh::Semantic::Positions),
            vertex_accessor(
                views.positions,
                accessor::Type::Vec3,
                accessor::ComponentType::F32,
                12,
                Some((group.min, group.max)),
            ),
        );
        attributes.insert(
            validation::Checked::Valid(mesh::Semantic::Normals),
            vertex_accessor(
                views.normals,
                accessor::Type::Vec3,
                accessor::ComponentType::F32,
                12,
                None,
            ),
        );
        attributes.insert(
            validation::Checked::Valid(mesh::Semantic::TexCoords(0)),
            vertex_accessor(
                views.texcoords,
                accessor::Type::Vec2,
                accessor::ComponentType::F32,
                8,
                None,
            ),
        );
        if let Some((joints_view, weights_view)) = views.skin {
            attributes.insert(
                validation::Checked::Valid(mesh::Semantic::Joints(0)),
                vertex_accessor(
                    joints_view,
                    accessor::Type::Vec4,
                    accessor::ComponentType::U16,
                    8,
                    None,
                ),
            );
            attributes.insert(
                validation::Checked::Valid(mesh::Semantic::Weights(0)),
                vertex_accessor(
                    weights_view,
                    accessor::Type::Vec4,
                    accessor::ComponentType::F32,
                    16,
                    None,
                ),
            );
        }

        let index_accessor = root.accessors.len() as u32;
        root.accessors.push(Accessor {
            buffer_view: Some(Index::new(views.indices)),
            byte_offset: Some(validation::USize64(*idx_offset as u64)),
            count: validation::USize64(*idx_count as u64),
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

        primitives.push(mesh::Primitive {
            attributes,
            indices: Some(Index::new(index_accessor)),
            // Filled in by `finish_group_primitives`, once the material list
            // the index has to be valid against exists.
            material: None,
            mode: validation::Checked::Valid(mesh::Mode::Triangles),
            targets: None,
            extensions: None,
            extras: Default::default(),
        });
    }
    primitives
}

/// Attach each primitive's material.
///
/// Out-of-range indices are dropped rather than written through, since a
/// dangling material index fails validation on load. The catch-all group for
/// faces the source never assigned lands here too, as a primitive with no
/// material.
fn finish_group_primitives(
    mut primitives: Vec<gltf_json::mesh::Primitive>,
    groups: &[CompactGroup],
    material_count: usize,
) -> Vec<gltf_json::mesh::Primitive> {
    for (primitive, group) in primitives.iter_mut().zip(groups) {
        primitive.material = group
            .material
            .filter(|m| *m < material_count)
            .map(|m| gltf_json::Index::new(m as u32));
    }
    primitives
}

/// Build a GLB from flat arrays of positions, normals, texcoords, and indices.
pub(crate) fn build_glb(
    positions: &[f32],
    normals: &[f32],
    texcoords: &[f32],
    indices: &[u32],
    materials: &MaterialBundle,
) -> Result<Vec<u8>, ImportError> {
    let group = MaterialGroup {
        material: (!materials.materials.is_empty()).then_some(0),
        indices: indices.to_vec(),
    };
    build_glb_grouped(positions, normals, texcoords, &[group], materials)
}

/// Build a GLB whose mesh has one primitive per [`MaterialGroup`], each owning
/// its own slice of the vertex buffer — see [`compact_groups`] for why they
/// must not share one.
pub(crate) fn build_glb_grouped(
    positions: &[f32],
    normals: &[f32],
    texcoords: &[f32],
    groups: &[MaterialGroup],
    materials: &MaterialBundle,
) -> Result<Vec<u8>, ImportError> {
    let (vertices, groups) = compact_groups(positions, normals, texcoords, &[], &[], groups);

    let pos_bytes = cast_f32_to_bytes(&vertices.positions);
    let norm_bytes = cast_f32_to_bytes(&vertices.normals);
    let tc_bytes = cast_f32_to_bytes(&vertices.texcoords);
    let (idx_bytes, idx_spans) = pack_group_indices(&groups);

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

    // Per-group accessors: each primitive reads only its own vertices.
    let primitives = push_group_accessors(&mut root, &groups, &idx_spans, VertexViews::unskinned());

    emit_material_bundle(&mut root, materials);

    let primitives = finish_group_primitives(primitives, &groups, materials.materials.len());
    if primitives.is_empty() {
        return Err(ImportError::ConversionError(
            "no triangles to write".to_string(),
        ));
    }

    root.meshes.push(Mesh {
        primitives,
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

    pack_embedded_images(&mut root, materials, &mut bin);
    let json_bytes = serialize_root(&root, materials)?;

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
    /// Separate roughness / metallic maps, as the PBR-MTL extension writes
    /// them. glTF only has the combined `metallicRoughnessTexture`, so these
    /// ride in the vendor extension alongside opacity and specular.
    pub roughness_texture: Option<usize>,
    pub metallic_texture: Option<usize>,
    /// How transparency is rendered. Converters set this; it is written
    /// straight out as glTF `alphaMode` (+ `alphaCutoff` for `Mask`).
    pub alpha: AlphaKind,
    /// Render back faces too. Foliage, fabric and glass need it, and glTF has
    /// a field for it — this used never to be written at all, which is why
    /// every transcoded model came out single-sided.
    pub double_sided: bool,
    /// Extended PBR channels (clearcoat, transmission, ior, anisotropy) read
    /// from modern FBX PBR materials. Texture URIs are model-relative, resolved
    /// at extraction time. Default for legacy Phong / OBJ.
    pub advanced: renzora::core::PbrAdvanced,
}

/// glTF's three transparency modes. Kept as its own type rather than a bool so
/// `Mask` is representable: a cutout is not "blended a bit", and rendering
/// foliage as either opaque or blended is wrong in different ways.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum AlphaKind {
    #[default]
    Opaque,
    /// Alpha-tested at the given cutoff.
    Mask(f32),
    Blend,
}

#[derive(Debug, Clone)]
pub(crate) struct TextureRef {
    /// How the intermediate GLB refers to this image: an **absolute path** to
    /// the file the converter located on disk. `gltf_pass::extract_glb_textures`
    /// rewrites it to the final `textures/<name>.<ext>` once it has processed
    /// the file.
    ///
    /// Doubles as the key advanced-PBR slots use to find their texture index —
    /// `PbrAdvanced` carries texture *URIs* rather than indices, so the writer
    /// looks them up by this string.
    pub uri: String,
    /// Image bytes, when the source file embedded the image instead of
    /// referencing it. Packed into the GLB's BIN chunk as a `bufferView`-backed
    /// image, which is exactly where a glTF source would have put it — so the
    /// shared pass handles both origins through one code path.
    pub embedded: Option<Vec<u8>>,
}

/// Build a GLB that contains a skinned mesh. `joint_indices` and `weights` must
/// be the same length as the vertex count implied by `positions`. `joints` is
/// the skeleton in flat order — children refer to parents via their index.
/// IBM list is parallel to `joints`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_skinned_glb(
    positions: &[f32],
    normals: &[f32],
    texcoords: &[f32],
    groups: &[MaterialGroup],
    joint_indices: &[[u16; 4]],
    weights: &[[f32; 4]],
    joints: &[SkinJoint],
    materials: &MaterialBundle,
) -> Result<Vec<u8>, ImportError> {
    let source_vertex_count = positions.len() / 3;
    if joint_indices.len() != source_vertex_count || weights.len() != source_vertex_count {
        return Err(ImportError::ConversionError(format!(
            "skin attribute length mismatch: {} vertices, {} joint_indices, {} weights",
            source_vertex_count,
            joint_indices.len(),
            weights.len()
        )));
    }

    let (vertices, groups) = compact_groups(
        positions,
        normals,
        texcoords,
        joint_indices,
        weights,
        groups,
    );

    let pos_bytes = cast_f32_to_bytes(&vertices.positions);
    let norm_bytes = cast_f32_to_bytes(&vertices.normals);
    let tc_bytes = cast_f32_to_bytes(&vertices.texcoords);
    let (idx_bytes, idx_spans) = pack_group_indices(&groups);

    // JOINTS_0 as u16x4 (8 bytes per vertex).
    let mut ji_bytes = Vec::with_capacity(vertices.joints.len() * 8);
    for ji in &vertices.joints {
        for &j in ji {
            ji_bytes.extend_from_slice(&j.to_le_bytes());
        }
    }
    // WEIGHTS_0 as f32x4 (16 bytes per vertex).
    let mut w_bytes = Vec::with_capacity(vertices.weights.len() * 16);
    for w in &vertices.weights {
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

    // Inverse bind matrices — one mat4 per joint. Emitted before the
    // per-group attribute accessors so its index is known up front for the
    // skin below.
    let ibm_accessor = root.accessors.len() as u32;
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

    // Per-group accessors: each primitive reads only its own vertices.
    let primitives = push_group_accessors(
        &mut root,
        &groups,
        &idx_spans,
        VertexViews {
            positions: 0,
            normals: 1,
            texcoords: 2,
            indices: 3,
            skin: Some((4, 5)),
        },
    );

    // Emit GLTF materials/images/textures/samplers from the bundle; each group
    // becomes a primitive wearing its own material.
    emit_material_bundle(&mut root, materials);

    let primitives = finish_group_primitives(primitives, &groups, materials.materials.len());
    if primitives.is_empty() {
        return Err(ImportError::ConversionError(
            "no triangles to write".to_string(),
        ));
    }

    root.meshes.push(Mesh {
        primitives,
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
        inverse_bind_matrices: Some(Index::new(ibm_accessor)),
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

    pack_embedded_images(&mut root, materials, &mut bin);
    let json_bytes = serialize_root(&root, materials)?;

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
/// Vendor extension carrying the two legacy-Phong channels glTF has no place
/// for. FBX materials routinely bind a separate opacity or specular map, and
/// the `.material` graph has pins for both — without somewhere to put them in
/// the intermediate GLB they'd be dropped the moment the importer started
/// reading materials back out of it rather than building them directly.
pub(crate) const RENZORA_LEGACY_EXT: &str = "RENZORA_materials_legacy";

/// Every extension [`material_json`] may emit, for `extensionsUsed`.
const EMITTED_EXTENSIONS: &[&str] = &[
    "KHR_materials_clearcoat",
    "KHR_materials_transmission",
    "KHR_materials_volume",
    "KHR_materials_ior",
    "KHR_materials_specular",
    "KHR_materials_anisotropy",
    "KHR_materials_unlit",
    RENZORA_LEGACY_EXT,
];

/// Build the complete glTF JSON for one material.
///
/// `gltf_json`'s typed `Material` only covers the metal-rough core, which is
/// why the GLB these converters produce used to drop the material name,
/// emissive, occlusion, alpha mode and every `KHR_materials_*` channel. That
/// was survivable while each converter also handed its materials to the caller
/// directly — and became the thing to fix once the GLB became the single
/// source of truth that `gltf_pass::extract_glb_materials` reads.
///
/// Kept deliberately in step with that reader: a key written here that it
/// doesn't look for is dead weight, and one it looks for that isn't written
/// here is a silently lost channel.
fn material_json(mat: &PbrMaterialDef, uri_index: &HashMap<&str, usize>) -> serde_json::Value {
    use serde_json::{json, Map, Value};

    // Main slots already carry texture indices; the advanced ones carry URIs,
    // so they resolve back through the bundle's texture list.
    let info = |idx: Option<usize>| -> Option<Value> { idx.map(|i| json!({ "index": i })) };
    let info_uri = |uri: &Option<String>| -> Option<Value> {
        uri.as_deref()
            .and_then(|u| uri_index.get(u))
            .map(|i| json!({ "index": i }))
    };

    let mut pbr = Map::new();
    pbr.insert("baseColorFactor".into(), json!(mat.base_color));
    pbr.insert("metallicFactor".into(), json!(mat.metallic));
    pbr.insert("roughnessFactor".into(), json!(mat.roughness));
    if let Some(t) = info(mat.base_color_texture) {
        pbr.insert("baseColorTexture".into(), t);
    }

    let mut out = Map::new();
    out.insert("name".into(), json!(mat.name));
    out.insert("pbrMetallicRoughness".into(), Value::Object(pbr));
    out.insert("emissiveFactor".into(), json!(mat.emissive));
    match mat.alpha {
        AlphaKind::Opaque => {
            out.insert("alphaMode".into(), json!("OPAQUE"));
        }
        AlphaKind::Mask(cutoff) => {
            out.insert("alphaMode".into(), json!("MASK"));
            out.insert("alphaCutoff".into(), json!(cutoff));
        }
        AlphaKind::Blend => {
            out.insert("alphaMode".into(), json!("BLEND"));
        }
    }
    if mat.double_sided {
        out.insert("doubleSided".into(), json!(true));
    }
    if let Some(t) = info(mat.normal_texture) {
        out.insert("normalTexture".into(), t);
    }
    if let Some(t) = info(mat.emissive_texture) {
        out.insert("emissiveTexture".into(), t);
    }
    if let Some(t) = info(mat.occlusion_texture) {
        out.insert("occlusionTexture".into(), t);
    }

    // ── Extensions ──────────────────────────────────────────────────────
    let adv = &mat.advanced;
    let mut exts = Map::new();
    fn put(exts: &mut Map<String, Value>, name: &str, block: Map<String, Value>) {
        if !block.is_empty() {
            exts.insert(name.into(), Value::Object(block));
        }
    }

    let mut clearcoat = Map::new();
    if adv.clearcoat != 0.0 || adv.clearcoat_texture.is_some() {
        clearcoat.insert("clearcoatFactor".into(), json!(adv.clearcoat));
        clearcoat.insert(
            "clearcoatRoughnessFactor".into(),
            json!(adv.clearcoat_roughness),
        );
        for (key, uri) in [
            ("clearcoatTexture", &adv.clearcoat_texture),
            ("clearcoatRoughnessTexture", &adv.clearcoat_roughness_texture),
            ("clearcoatNormalTexture", &adv.clearcoat_normal_texture),
        ] {
            if let Some(t) = info_uri(uri) {
                clearcoat.insert(key.into(), t);
            }
        }
    }
    put(&mut exts, "KHR_materials_clearcoat", clearcoat);

    let mut transmission = Map::new();
    if adv.specular_transmission != 0.0 || adv.transmission_texture.is_some() {
        transmission.insert(
            "transmissionFactor".into(),
            json!(adv.specular_transmission),
        );
        if let Some(t) = info_uri(&adv.transmission_texture) {
            transmission.insert("transmissionTexture".into(), t);
        }
    }
    put(&mut exts, "KHR_materials_transmission", transmission);

    let mut volume = Map::new();
    if adv.thickness != 0.0 || adv.thickness_texture.is_some() {
        volume.insert("thicknessFactor".into(), json!(adv.thickness));
        volume.insert("attenuationDistance".into(), json!(adv.attenuation_distance));
        volume.insert("attenuationColor".into(), json!(adv.attenuation_color));
        if let Some(t) = info_uri(&adv.thickness_texture) {
            volume.insert("thicknessTexture".into(), t);
        }
    }
    put(&mut exts, "KHR_materials_volume", volume);

    if adv.ior != 1.5 {
        let mut ior = Map::new();
        ior.insert("ior".into(), json!(adv.ior));
        put(&mut exts, "KHR_materials_ior", ior);
    }

    if adv.reflectance != 0.5 {
        // The reader halves `specularFactor`, so a Bevy reflectance of 0.5 is
        // the glTF default of 1.0. Undo that here or the value drifts on every
        // round trip.
        let mut specular = Map::new();
        specular.insert("specularFactor".into(), json!(adv.reflectance * 2.0));
        put(&mut exts, "KHR_materials_specular", specular);
    }

    let mut anisotropy = Map::new();
    if adv.anisotropy_strength != 0.0 || adv.anisotropy_texture.is_some() {
        anisotropy.insert("anisotropyStrength".into(), json!(adv.anisotropy_strength));
        anisotropy.insert("anisotropyRotation".into(), json!(adv.anisotropy_rotation));
        if let Some(t) = info_uri(&adv.anisotropy_texture) {
            anisotropy.insert("anisotropyTexture".into(), t);
        }
    }
    put(&mut exts, "KHR_materials_anisotropy", anisotropy);

    if adv.unlit {
        exts.insert("KHR_materials_unlit".into(), json!({}));
    }

    let mut legacy = Map::new();
    for (key, idx) in [
        ("opacityTexture", mat.opacity_texture),
        ("specularTexture", mat.specular_texture),
        ("roughnessTexture", mat.roughness_texture),
        ("metallicTexture", mat.metallic_texture),
    ] {
        if let Some(t) = info(idx) {
            legacy.insert(key.into(), t);
        }
    }
    put(&mut exts, RENZORA_LEGACY_EXT, legacy);

    if !exts.is_empty() {
        out.insert("extensions".into(), Value::Object(exts));
    }
    Value::Object(out)
}

/// Append every embedded image to the GLB's binary chunk and point its image
/// entry at the resulting `bufferView`.
///
/// Called once the geometry buffer is complete, since the offsets depend on
/// where it ends. Keeps each image 4-byte aligned so the chunk stays valid for
/// any accessor that might follow.
fn pack_embedded_images(root: &mut gltf_json::Root, bundle: &MaterialBundle, bin: &mut Vec<u8>) {
    use gltf_json::*;

    for (i, tex) in bundle.textures.iter().enumerate() {
        let Some(bytes) = tex.embedded.as_ref() else {
            continue;
        };
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let offset = bin.len();
        bin.extend_from_slice(bytes);

        let view = root.buffer_views.len();
        root.buffer_views.push(buffer::View {
            buffer: Index::new(0),
            byte_length: validation::USize64(bytes.len() as u64),
            byte_offset: Some(validation::USize64(offset as u64)),
            byte_stride: None,
            name: None,
            target: None,
            extensions: None,
            extras: Default::default(),
        });
        if let Some(image) = root.images.get_mut(i) {
            image.buffer_view = Some(Index::new(view as u32));
        }
    }

    // The buffer's declared length has to cover what we just appended.
    if let Some(buffer) = root.buffers.first_mut() {
        buffer.byte_length = validation::USize64(bin.len() as u64);
    }
}

/// Serialize a GLTF root, swapping in complete material definitions.
///
/// `emit_material_bundle` pushes placeholder materials so indices line up while
/// the typed tree is built; the real definitions go in here, where the full
/// JSON is available. See [`material_json`] for why the typed API isn't enough.
fn serialize_root(
    root: &gltf_json::Root,
    bundle: &MaterialBundle,
) -> Result<Vec<u8>, ImportError> {
    let mut value = serde_json::to_value(root)
        .map_err(|e| ImportError::ConversionError(format!("GLTF JSON serialize: {}", e)))?;

    if !bundle.materials.is_empty() {
        let uri_index: HashMap<&str, usize> = bundle
            .textures
            .iter()
            .enumerate()
            .map(|(i, t)| (t.uri.as_str(), i))
            .collect();

        let materials: Vec<serde_json::Value> = bundle
            .materials
            .iter()
            .map(|m| material_json(m, &uri_index))
            .collect();
        value["materials"] = serde_json::Value::Array(materials);

        // Declare what we emitted. `extensionsUsed` is advisory — unlike
        // `extensionsRequired`, a loader that doesn't know an entry just
        // ignores it — so listing everything is safe and keeps the file valid.
        let used: Vec<serde_json::Value> = EMITTED_EXTENSIONS
            .iter()
            .map(|e| serde_json::Value::String((*e).to_string()))
            .collect();
        value["extensionsUsed"] = serde_json::Value::Array(used);
    }

    serde_json::to_vec(&value)
        .map_err(|e| ImportError::ConversionError(format!("GLTF JSON serialize: {}", e)))
}

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
        // An embedded image needs its bytes in the BIN chunk; `pack_embedded_
        // images` appends them and fills in the bufferView index, because only
        // the builders know where the chunk ends.
        root.images.push(Image {
            buffer_view: None,
            mime_type: None,
            name: None,
            uri: tex.embedded.is_none().then(|| tex.uri.clone()),
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cast_empty_slices() {
        assert!(cast_f32_to_bytes(&[]).is_empty());
        assert!(cast_u32_to_bytes(&[]).is_empty());
    }

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
                roughness_texture: None,
                metallic_texture: None,
                alpha: AlphaKind::Opaque,
                double_sided: false,
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

    fn plain_material(name: &str) -> PbrMaterialDef {
        PbrMaterialDef {
            name: name.into(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            base_color_texture: None,
            normal_texture: None,
            metallic: 0.0,
            roughness: 0.5,
            emissive: [0.0, 0.0, 0.0],
            emissive_texture: None,
            occlusion_texture: None,
            opacity_texture: None,
            specular_texture: None,
            roughness_texture: None,
            metallic_texture: None,
            alpha: AlphaKind::Opaque,
            double_sided: false,
            advanced: renzora::core::PbrAdvanced::default(),
        }
    }

    #[test]
    fn grouped_glb_emits_one_primitive_per_material() {
        // Two triangles over a shared 6-vertex buffer, one per material.
        let positions: Vec<f32> = (0..6)
            .flat_map(|i| [i as f32, 0.0, 0.0])
            .collect();
        let normals: Vec<f32> = (0..6).flat_map(|_| [0.0f32, 0.0, 1.0]).collect();
        let texcoords: Vec<f32> = (0..6).flat_map(|_| [0.0f32, 0.0]).collect();

        let bundle = MaterialBundle {
            materials: vec![plain_material("a"), plain_material("b")],
            textures: Vec::new(),
        };
        let groups = vec![
            MaterialGroup {
                material: Some(0),
                indices: vec![0, 1, 2],
            },
            MaterialGroup {
                material: Some(1),
                indices: vec![3, 4, 5],
            },
        ];

        let glb =
            build_glb_grouped(&positions, &normals, &texcoords, &groups, &bundle).unwrap();
        let json_len = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let json: serde_json::Value =
            serde_json::from_slice(&glb[20..20 + json_len]).expect("valid GLTF JSON");

        let primitives = json["meshes"][0]["primitives"].as_array().unwrap();
        assert_eq!(primitives.len(), 2);
        assert_eq!(primitives[0]["material"], 0);
        assert_eq!(primitives[1]["material"], 1);

        // Each primitive must get its OWN vertex accessor covering only its own
        // vertices. Pointing several primitives at one full-length accessor is
        // valid glTF, but Bevy builds a Mesh per primitive and reads the whole
        // accessor each time — so sharing means N copies of every vertex in the
        // model. On a 2-million-vertex scene with 132 materials that was 8.8 GB
        // and an out-of-memory crash.
        let accessors = json["accessors"].as_array().unwrap();
        let pos = |p: &serde_json::Value| -> usize {
            p["attributes"]["POSITION"].as_u64().unwrap() as usize
        };
        assert_ne!(pos(&primitives[0]), pos(&primitives[1]));
        assert_eq!(accessors[pos(&primitives[0])]["count"], 3);
        assert_eq!(accessors[pos(&primitives[1])]["count"], 3);

        // Disjoint slices of the shared buffer view: three VEC3 f32 = 36 bytes.
        assert_eq!(
            accessors[pos(&primitives[0])]["byteOffset"].as_u64().unwrap_or(0),
            0
        );
        assert_eq!(accessors[pos(&primitives[1])]["byteOffset"], 36);

        // Indices are group-local, so both start at zero and the second group's
        // triangle no longer references vertices 3..5.
        assert_ne!(primitives[0]["indices"], primitives[1]["indices"]);
        let second_indices = &accessors[primitives[1]["indices"].as_u64().unwrap() as usize];
        assert_eq!(second_indices["byteOffset"], 12);
        assert_eq!(second_indices["count"], 3);
    }

    #[test]
    fn compaction_keeps_the_total_vertex_count_near_the_original() {
        // The saving only exists if groups don't each carry the whole buffer.
        // Two disjoint triangles over a 6-vertex mesh must stay 6 vertices
        // total, not 12.
        let positions: Vec<f32> = (0..6).flat_map(|i| [i as f32, 0.0, 0.0]).collect();
        let normals: Vec<f32> = (0..6).flat_map(|_| [0.0f32, 0.0, 1.0]).collect();
        let texcoords: Vec<f32> = (0..6).flat_map(|_| [0.0f32, 0.0]).collect();
        let groups = vec![
            MaterialGroup {
                material: Some(0),
                indices: vec![0, 1, 2],
            },
            MaterialGroup {
                material: Some(1),
                indices: vec![3, 4, 5],
            },
        ];

        let (vertices, compacted) =
            compact_groups(&positions, &normals, &texcoords, &[], &[], &groups);
        assert_eq!(vertices.positions.len() / 3, 6);
        assert_eq!(compacted[0].vertex_start, 0);
        assert_eq!(compacted[1].vertex_start, 3);
        assert!(compacted.iter().all(|g| g.vertex_count == 3));
        // Group-local numbering.
        assert_eq!(compacted[1].indices, vec![0, 1, 2]);
    }

    #[test]
    fn compaction_shares_a_vertex_within_a_group_and_splits_it_across_groups() {
        // Vertex 0 is used by both groups, so it appears once in each — that
        // duplication is the real cost of splitting by material, and it's
        // bounded by the seams rather than by the model size.
        let positions: Vec<f32> = (0..4).flat_map(|i| [i as f32, 0.0, 0.0]).collect();
        let normals: Vec<f32> = (0..4).flat_map(|_| [0.0f32, 0.0, 1.0]).collect();
        let texcoords: Vec<f32> = (0..4).flat_map(|_| [0.0f32, 0.0]).collect();
        let groups = vec![
            MaterialGroup {
                material: Some(0),
                indices: vec![0, 1, 2, 0, 2, 1],
            },
            MaterialGroup {
                material: Some(1),
                indices: vec![0, 2, 3],
            },
        ];

        let (vertices, compacted) =
            compact_groups(&positions, &normals, &texcoords, &[], &[], &groups);
        // Group 0 touches vertices 0,1,2 → 3 of them, reused across both its
        // triangles. Group 1 touches 0,2,3 → 3 more.
        assert_eq!(compacted[0].vertex_count, 3);
        assert_eq!(compacted[1].vertex_count, 3);
        assert_eq!(vertices.positions.len() / 3, 6);
        assert_eq!(compacted[0].indices, vec![0, 1, 2, 0, 2, 1]);

        // Bounds are per group, so each primitive culls against its own extent.
        assert_eq!(compacted[0].max[0], 2.0);
        assert_eq!(compacted[1].max[0], 3.0);
    }

    #[test]
    fn grouped_glb_drops_empty_groups_and_dangling_materials() {
        let positions = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let normals = [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        let texcoords = [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0];

        let bundle = MaterialBundle {
            materials: vec![plain_material("only")],
            textures: Vec::new(),
        };
        let groups = vec![
            // A material the source named but never assigned to a face.
            MaterialGroup {
                material: Some(0),
                indices: Vec::new(),
            },
            // Points past the end of the bundle — must not be written through,
            // since a dangling material index fails validation on load.
            MaterialGroup {
                material: Some(9),
                indices: vec![0, 1, 2],
            },
        ];

        let glb =
            build_glb_grouped(&positions, &normals, &texcoords, &groups, &bundle).unwrap();
        let json_len = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let json: serde_json::Value =
            serde_json::from_slice(&glb[20..20 + json_len]).expect("valid GLTF JSON");

        let primitives = json["meshes"][0]["primitives"].as_array().unwrap();
        assert_eq!(primitives.len(), 1);
        assert!(primitives[0].get("material").is_none_or(|m| m.is_null()));
    }
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
}
