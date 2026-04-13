//! FBX importer backed by the `ufbx` crate.
//!
//! Replaces the previous hand-rolled binary/ASCII/legacy parsers. ufbx
//! supports every FBX version from 3.0 through 7.7 (binary + ASCII) and
//! normalizes quirks across exporters (Maya / 3ds Max / Blender / Mixamo /
//! MotionBuilder). It bakes PreRotation / GeometricTransform into the usable
//! local transforms and exposes skin clusters with ready-to-use inverse bind
//! matrices, so we can build a skinned GLB straight from its output.

use std::path::Path;

use renzora_core::{AnimClip, BoneTrack, write_anim_file};

use crate::anim_extract::AnimExtractResult;
use crate::convert::{ImportError, ImportResult};
use crate::obj::{build_glb, build_skinned_glb};
use crate::settings::ImportSettings;

// ─── Public API ────────────────────────────────────────────────────────────

/// Convert an FBX file to a GLB, preserving skeleton + skin weights when
/// present. Any FBX version (3.0 – 7.7), binary or ASCII, is accepted.
pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
    let scene = load_scene(path, settings)?;
    let scene_ref: &ufbx::Scene = &scene;

    log::info!(
        "[import] {}: ufbx loaded FBX version {}, {} meshes, {} skin clusters",
        file_name,
        scene_ref.metadata.version,
        scene_ref.meshes.len(),
        scene_ref.skin_clusters.len(),
    );

    let mut all_positions: Vec<f32> = Vec::new();
    let mut all_normals: Vec<f32> = Vec::new();
    let mut all_texcoords: Vec<f32> = Vec::new();
    let mut all_indices: Vec<u32> = Vec::new();
    let mut all_joints: Vec<[u16; 4]> = Vec::new();
    let mut all_weights: Vec<[f32; 4]> = Vec::new();

    // Build the joint list once up-front so the same joint indices are valid
    // across every mesh. We include every bone node that appears as a cluster
    // target; this keeps the skeleton minimal to what actually drives skin.
    let joints = collect_joints(scene_ref);
    let has_skin = !joints.is_empty();
    // element_id → joint index. Same key space as the parent-walk lookup.
    let eid_to_joint: std::collections::HashMap<u32, usize> = joints
        .iter()
        .enumerate()
        .map(|(i, j)| (j.element_id, i))
        .collect();

    let mut warnings: Vec<String> = Vec::new();

    for mesh in scene_ref.meshes.iter() {
        let vertex_count = mesh.num_vertices;
        if vertex_count == 0 {
            continue;
        }
        let base_vertex = (all_positions.len() / 3) as u32;

        // Positions: mesh.vertices is one Vec3 per FBX-vertex.
        for v in mesh.vertices.iter() {
            all_positions.push(v.x as f32);
            all_positions.push(v.y as f32);
            all_positions.push(v.z as f32);
        }

        // Normals per vertex via the first mesh-corner that references this
        // vertex. `vertex_first_index[v] == u32::MAX` means the vertex has no
        // corner — shouldn't happen for a valid skinned mesh but we guard it.
        let mut normals = vec![0.0f32; vertex_count * 3];
        if mesh.vertex_normal.exists {
            for v in 0..vertex_count {
                let first = mesh.vertex_first_index[v];
                if first == u32::MAX {
                    continue;
                }
                let n = &mesh.vertex_normal[first as usize];
                normals[v * 3] = n.x as f32;
                normals[v * 3 + 1] = n.y as f32;
                normals[v * 3 + 2] = n.z as f32;
            }
        }
        all_normals.extend_from_slice(&normals);

        let mut uvs = vec![0.0f32; vertex_count * 2];
        if mesh.vertex_uv.exists {
            for v in 0..vertex_count {
                let first = mesh.vertex_first_index[v];
                if first == u32::MAX {
                    continue;
                }
                let uv = &mesh.vertex_uv[first as usize];
                uvs[v * 2] = uv.x as f32;
                uvs[v * 2 + 1] = if settings.flip_uvs {
                    1.0 - uv.y as f32
                } else {
                    uv.y as f32
                };
            }
        }
        all_texcoords.extend_from_slice(&uvs);

        // Indices: triangulate each face using the ufbx helper. It produces
        // mesh-corner indices; we remap them to per-vertex indices.
        let mut tri_scratch: Vec<u32> = Vec::new();
        for face_idx in 0..mesh.num_faces {
            let face = mesh.faces[face_idx];
            if face.num_indices < 3 {
                continue;
            }
            tri_scratch.clear();
            tri_scratch.resize((face.num_indices as usize - 2) * 3, 0);
            let produced = ufbx::triangulate_face_vec(&mut tri_scratch, mesh, face);
            for i in 0..produced as usize * 3 {
                let corner = tri_scratch[i] as usize;
                let vi = mesh.vertex_indices[corner];
                all_indices.push(vi + base_vertex);
            }
        }

        // Skin: look at the first skin deformer on this mesh (Mixamo output has
        // exactly one). Collapse its per-vertex top-4 influences into our
        // shared joint index space.
        let mut mesh_joints = vec![[0u16; 4]; vertex_count];
        let mut mesh_weights = vec![[0.0f32; 4]; vertex_count];
        if let Some(skin) = mesh.skin_deformers.into_iter().next() {
            for v in 0..vertex_count {
                let sv = skin.vertices[v];
                let start = sv.weight_begin as usize;
                let n = sv.num_weights as usize;
                let mut infl: Vec<(u16, f32)> = (0..n)
                    .filter_map(|k| {
                        let w = skin.weights[start + k];
                        let clusters: &[ufbx::Ref<ufbx::SkinCluster>] = &skin.clusters;
                        let cluster = clusters.get(w.cluster_index as usize)?;
                        let bone = cluster.bone_node.as_ref()?;
                        let bone_eid = bone.element.element_id;
                        eid_to_joint
                            .get(&bone_eid)
                            .map(|&ji| (ji as u16, w.weight as f32))
                    })
                    .collect();
                infl.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                });
                let top = &infl[..infl.len().min(4)];
                let mut js = [0u16; 4];
                let mut ws = [0.0f32; 4];
                for (i, (j, w)) in top.iter().enumerate() {
                    js[i] = *j;
                    ws[i] = *w;
                }
                let sum: f32 = ws.iter().sum();
                if sum > 0.0 {
                    for w in &mut ws {
                        *w /= sum;
                    }
                }
                mesh_joints[v] = js;
                mesh_weights[v] = ws;
            }
        } else if has_skin {
            warnings.push(format!(
                "mesh '{}' has no skin deformer but scene has a skeleton",
                mesh.element.name.as_ref()
            ));
        }
        all_joints.extend_from_slice(&mesh_joints);
        all_weights.extend_from_slice(&mesh_weights);
    }

    if all_positions.is_empty() {
        return Err(ImportError::ParseError(
            "no geometry found in FBX file".into(),
        ));
    }

    let glb_bytes = if has_skin {
        log::info!(
            "[import] {}: building skinned GLB with {} joints, {} vertices",
            file_name,
            joints.len(),
            all_positions.len() / 3,
        );
        let joint_structs: Vec<crate::obj::SkinJoint> = joints
            .iter()
            .map(|j| crate::obj::SkinJoint {
                name: j.name.clone(),
                parent: j.parent,
                translation: j.translation,
                rotation: j.rotation,
                scale: j.scale,
                inverse_bind_matrix: j.inverse_bind_matrix,
            })
            .collect();
        build_skinned_glb(
            &all_positions,
            &all_normals,
            &all_texcoords,
            &all_indices,
            &all_joints,
            &all_weights,
            &joint_structs,
        )?
    } else {
        build_glb(&all_positions, &all_normals, &all_texcoords, &all_indices)?
    };

    log::info!(
        "[import] {}: GLB output {} bytes ({} vertices, {} triangles)",
        file_name,
        glb_bytes.len(),
        all_positions.len() / 3,
        all_indices.len() / 3,
    );

    Ok(ImportResult { glb_bytes, warnings })
}

/// Extract every animation stack in an FBX file to a directory of `.anim` files.
pub fn extract_animations(
    path: &Path,
    output_dir: &Path,
) -> Result<AnimExtractResult, String> {
    let settings = ImportSettings::default();
    let scene = load_scene(path, &settings).map_err(|e| format!("{}", e))?;
    let scene_ref: &ufbx::Scene = &scene;

    let mut result = AnimExtractResult {
        written_files: Vec::new(),
        warnings: Vec::new(),
    };

    if scene_ref.anim_stacks.is_empty() {
        result.warnings.push("no animation stacks found".into());
        return Ok(result);
    }

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("failed to create animations directory: {}", e))?;

    // Sample each stack at a fixed rate. 30 Hz matches the Mixamo default and
    // is dense enough for most gameplay. If the stack has very few keys we
    // still get at least the endpoints.
    let sample_rate: f64 = 30.0;

    // Mixamo and several other tools emit every stack with the same internal
    // name ("mixamo.com"), which makes multiple imports collide. Prefer the
    // source filename stem as the clip name, falling back to the stack name
    // only when we can't read the path, and suffixing when there are several
    // stacks in one file.
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("clip")
        .to_string();
    let stack_count = scene_ref.anim_stacks.len();

    for (stack_i, stack_ref) in (&scene_ref.anim_stacks).into_iter().enumerate() {
        let clip_name = if stack_count == 1 {
            file_stem.clone()
        } else {
            let inner = stack_ref.element.name.as_ref();
            if inner.is_empty() {
                format!("{}_{}", file_stem, stack_i)
            } else {
                format!("{}_{}", file_stem, inner)
            }
        };

        let duration_f = (stack_ref.time_end - stack_ref.time_begin).max(0.0);
        let n_samples = ((duration_f * sample_rate).ceil() as usize + 1).max(2);
        let dt = if n_samples > 1 {
            duration_f / (n_samples as f64 - 1.0)
        } else {
            0.0
        };

        // Evaluate each bone node at each sample time.
        let anim_ref: &ufbx::Anim = &stack_ref.anim;
        let mut tracks: Vec<BoneTrack> = Vec::new();

        for node in &scene_ref.nodes {
            // Only emit tracks for bones — avoids cluttering the clip with
            // meshes, cameras, etc.
            if node.bone.is_none() {
                continue;
            }
            let name = node.element.name.as_ref();
            if name.is_empty() {
                continue;
            }

            let mut track = BoneTrack {
                bone_name: name.to_string(),
                translations: Vec::new(),
                rotations: Vec::new(),
                scales: Vec::new(),
            };

            for i in 0..n_samples {
                let t = stack_ref.time_begin + dt * i as f64;
                let tr = ufbx::evaluate_transform(anim_ref, node, t);
                let rel_t = t - stack_ref.time_begin;
                track.translations.push((
                    rel_t as f32,
                    [tr.translation.x as f32, tr.translation.y as f32, tr.translation.z as f32],
                ));
                track.rotations.push((
                    rel_t as f32,
                    [tr.rotation.x as f32, tr.rotation.y as f32, tr.rotation.z as f32, tr.rotation.w as f32],
                ));
                track.scales.push((
                    rel_t as f32,
                    [tr.scale.x as f32, tr.scale.y as f32, tr.scale.z as f32],
                ));
            }

            tracks.push(track);
        }

        if tracks.is_empty() {
            result.warnings.push(format!(
                "{}: animation stack has no bone tracks",
                clip_name
            ));
            continue;
        }

        let clip = AnimClip {
            name: clip_name.clone(),
            duration: duration_f as f32,
            tracks,
        };

        let safe_name: String = clip_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let file_path = output_dir.join(format!("{}.anim", safe_name));
        match write_anim_file(&clip, &file_path) {
            Ok(()) => {
                log::info!(
                    "[import] wrote animation '{}' ({} tracks, {:.2}s) → {}",
                    clip_name,
                    clip.tracks.len(),
                    clip.duration,
                    file_path.display()
                );
                result.written_files.push(file_path.display().to_string());
            }
            Err(e) => {
                result
                    .warnings
                    .push(format!("{}: failed to write .anim: {}", clip_name, e));
            }
        }
    }

    Ok(result)
}

// ─── Internals ─────────────────────────────────────────────────────────────

struct JointOut {
    /// `Element::element_id` of this node — used to key cluster-to-joint lookups.
    element_id: u32,
    name: String,
    parent: Option<usize>,
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    inverse_bind_matrix: [f32; 16],
}

fn collect_joints(scene: &ufbx::Scene) -> Vec<JointOut> {
    // A joint is a node that ufbx has tagged with `bone = Some(_)`. Clusters
    // often reference the mesh node itself as a "bind pose" anchor, so we
    // deliberately do NOT flag cluster targets — that would sweep the mesh
    // into the joint list and corrupt the parent chain.
    //
    // Everything here is keyed by `Element::element_id` (the scene-wide unique
    // ID carried on every ufbx element) rather than by the node's position in
    // `scene.nodes`. The two are not equivalent: element_id is a sparse
    // identifier into `scene.elements`, while the nodes list just happens to
    // hold references. Using element_id keeps cluster-target lookups and
    // parent-walk lookups in the same key space.
    let mut eid_is_joint: std::collections::HashSet<u32> =
        std::collections::HashSet::new();
    for node in &scene.nodes {
        if node.bone.is_some() {
            eid_is_joint.insert(node.element.element_id);
        }
    }

    let mut eid_to_joint_idx: std::collections::HashMap<u32, usize> =
        std::collections::HashMap::new();
    let mut joints: Vec<JointOut> = Vec::new();
    for node in &scene.nodes {
        let eid = node.element.element_id;
        if !eid_is_joint.contains(&eid) {
            continue;
        }
        let name = (&*node.element.name).to_string();
        let t = node.local_transform.translation;
        let r = node.local_transform.rotation;
        let s = node.local_transform.scale;
        let mut ibm = identity_mat4();
        for cluster in &scene.skin_clusters {
            if let Some(bone) = cluster.bone_node.as_ref() {
                if bone.element.element_id == eid {
                    ibm = matrix_to_gltf(&cluster.geometry_to_bone);
                    break;
                }
            }
        }
        eid_to_joint_idx.insert(eid, joints.len());
        joints.push(JointOut {
            element_id: eid,
            name,
            parent: None,
            translation: [t.x as f32, t.y as f32, t.z as f32],
            rotation: [r.x as f32, r.y as f32, r.z as f32, r.w as f32],
            scale: [s.x as f32, s.y as f32, s.z as f32],
            inverse_bind_matrix: ibm,
        });
    }

    // Link parents — walk up each joint's original parent chain until we hit
    // another joint. Non-joint helper nodes between bones are skipped.
    // We need to find each joint's Node again via element_id; build a lookup.
    let mut eid_to_node: std::collections::HashMap<u32, &ufbx::Node> =
        std::collections::HashMap::new();
    for node in &scene.nodes {
        eid_to_node.insert(node.element.element_id, node);
    }

    for joint_i in 0..joints.len() {
        let eid = joints[joint_i].element_id;
        let node = match eid_to_node.get(&eid) {
            Some(n) => *n,
            None => continue,
        };
        let mut walker: Option<&ufbx::Node> =
            node.parent.as_ref().map(|p| -> &ufbx::Node { p });
        while let Some(parent) = walker {
            let pid = parent.element.element_id;
            if let Some(&pji) = eid_to_joint_idx.get(&pid) {
                joints[joint_i].parent = Some(pji);
                break;
            }
            walker = parent.parent.as_ref().map(|p| -> &ufbx::Node { p });
        }
    }

    joints
}

fn identity_mat4() -> [f32; 16] {
    let mut m = [0.0f32; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

/// Convert ufbx's 3×4 affine matrix into a GLTF 4×4 column-major mat4.
fn matrix_to_gltf(m: &ufbx::Matrix) -> [f32; 16] {
    // GLTF column-major. ufbx stores mXY where X=row, Y=column, and
    // m03/m13/m23 are the translation column.
    [
        m.m00 as f32, m.m10 as f32, m.m20 as f32, 0.0,
        m.m01 as f32, m.m11 as f32, m.m21 as f32, 0.0,
        m.m02 as f32, m.m12 as f32, m.m22 as f32, 0.0,
        m.m03 as f32, m.m13 as f32, m.m23 as f32, 1.0,
    ]
}

fn load_scene(path: &Path, settings: &ImportSettings) -> Result<ufbx::SceneRoot, ImportError> {
    let mut opts = ufbx::LoadOpts::default();
    // Normalize everything to a right-handed, Y-up, meters coordinate system
    // so downstream code doesn't have to guess. ufbx applies unit scaling and
    // axis conversion to both meshes and bone transforms consistently.
    opts.target_axes = ufbx::CoordinateAxes::right_handed_y_up();
    opts.target_unit_meters = settings.scale as ufbx::Real;
    opts.space_conversion = ufbx::SpaceConversion::ModifyGeometry;
    opts.generate_missing_normals = settings.generate_normals;

    let path_str = path
        .to_str()
        .ok_or_else(|| ImportError::ParseError("non-utf8 FBX path".into()))?;
    ufbx::load_file(path_str, opts).map_err(|e| {
        ImportError::ParseError(format!("ufbx load failed: {}", &*e.description))
    })
}
