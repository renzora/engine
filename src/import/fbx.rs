//! FBX binary file parser and GLB converter using the `fbxcel-dom` crate.
//!
//! Uses the low-level tree API for maximum compatibility with different FBX versions.
//! Supports skeleton, skin weights, and animation clip extraction.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::glb_builder::{GlbBuilder, JointNodeData, MaterialData, MeshData, SkinData};
use super::fbx_skeleton::{
    self, parse_connections, parse_skeleton, parse_inverse_bind_matrices,
    parse_skin_weights, parse_animations,
    euler_zyx_to_quat, clean_fbx_name,
};
use crate::animator::anim_file::{AnimFile, BoneTrack};

/// Parse a binary FBX file and feed geometry/materials/skin/animations into a GlbBuilder.
/// Returns a list of .anim file paths that were written alongside the source FBX.
pub fn convert_fbx(path: &Path, builder: &mut GlbBuilder) -> Result<Vec<PathBuf>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("Failed to open FBX file: {}", e))?;
    let reader = std::io::BufReader::new(file);

    let any_doc = fbxcel_dom::any::AnyDocument::from_seekable_reader(reader)
        .map_err(|e| format!("Failed to parse FBX: {}", e))?;

    match any_doc {
        fbxcel_dom::any::AnyDocument::V7400(_header, doc) => {
            parse_fbx_v7400(path, &doc, builder)
        }
        _ => Err("Only FBX version 7.4+ (binary) is supported".to_string()),
    }
}

fn parse_fbx_v7400(
    source_path: &Path,
    doc: &fbxcel_dom::v7400::Document,
    builder: &mut GlbBuilder,
) -> Result<Vec<PathBuf>, String> {
    let tree = doc.tree();
    let root = tree.root();

    // Parse connection maps
    let (child_of, prop_of) = parse_connections(&root);

    let objects_node = root.children().find(|n| n.name() == "Objects");
    let Some(objects_node) = objects_node else {
        return Err("No Objects node found in FBX".to_string());
    };

    // ── Parse skeleton ───────────────────────────────────────────────────────
    let bones = parse_skeleton(&objects_node, &child_of);
    let has_skeleton = !bones.is_empty();

    // Build bone_id → joint_index map
    let mut bone_id_to_idx: HashMap<i64, u16> = HashMap::new();
    for (i, bone) in bones.iter().enumerate() {
        bone_id_to_idx.insert(bone.id, i as u16);
    }

    // ── Parse geometry ──────────────────────────────────────────────────────
    let mut mesh_data_list: Vec<(i64, MeshData)> = Vec::new(); // (geometry_id, mesh)

    for child in objects_node.children() {
        match child.name() {
            "Geometry" => {
                let geo_id = child.attributes().get(0)
                    .and_then(fbx_skeleton::get_i64)
                    .unwrap_or(0);
                if let Some(mesh_data) = parse_geometry_node(&child) {
                    mesh_data_list.push((geo_id, mesh_data));
                }
            }
            "Material" => {
                parse_material_node(&child, builder);
            }
            _ => {}
        }
    }

    // ── Parse skin weights & attach to meshes ────────────────────────────────
    if has_skeleton {
        let inv_bind = parse_inverse_bind_matrices(&objects_node);

        for (_, ref mut mesh_data) in mesh_data_list.iter_mut() {
            let vertex_count = mesh_data.positions.len();
            if let Some(weights) = parse_skin_weights(
                &objects_node,
                &child_of,
                &bone_id_to_idx,
                vertex_count,
            ) {
                mesh_data.joints = Some(weights.joint_indices);
                mesh_data.weights = Some(weights.joint_weights);
            }
        }

        // Build skeleton hierarchy for GLB
        // Sort bones: parents before children
        let bone_idx_map: HashMap<i64, usize> = bones.iter().enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();

        let joint_nodes: Vec<JointNodeData> = bones.iter().map(|bone| {
            let parent = bone.parent_id
                .and_then(|pid| bone_idx_map.get(&pid).copied());

            // Convert local rotation from Euler degrees to quaternion
            let rotation = euler_zyx_to_quat(
                bone.local_rotation[0],
                bone.local_rotation[1],
                bone.local_rotation[2],
            );

            JointNodeData {
                name: bone.name.clone(),
                parent,
                translation: bone.local_translation,
                rotation,
                scale: bone.local_scale,
            }
        }).collect();

        // Build inverse bind matrices ordered by bone index
        let ibm: Vec<[[f32; 4]; 4]> = bones.iter().map(|bone| {
            inv_bind.get(&bone.id).copied().unwrap_or_else(|| {
                // Identity matrix as fallback
                [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ]
            })
        }).collect();

        let skin_data = SkinData {
            name: Some("Armature".to_string()),
            joints: joint_nodes,
            inverse_bind_matrices: ibm,
        };
        builder.set_skin(skin_data);
    }

    // Add meshes to builder
    let mut mesh_count = 0;
    for (_, mesh_data) in mesh_data_list {
        builder.add_mesh(mesh_data);
        mesh_count += 1;
    }

    // For skinned FBX we don't error on zero meshes (animation-only export is valid)
    if mesh_count == 0 && !has_skeleton {
        return Err("No mesh geometry found in FBX file".to_string());
    }

    // Apply Z-up to Y-up rotation (only if no skeleton — skeleton bones are already in object-space)
    if !has_skeleton {
        builder.set_root_rotation([
            -std::f32::consts::FRAC_1_SQRT_2,
            0.0,
            0.0,
            std::f32::consts::FRAC_1_SQRT_2,
        ]);
    }

    // ── Parse animations & write .anim files ─────────────────────────────────
    let anim_stacks = parse_animations(&objects_node, &child_of, &prop_of);

    let mut written_anims: Vec<PathBuf> = Vec::new();

    if !anim_stacks.is_empty() {
        let stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("character");
        let parent_dir = source_path.parent().unwrap_or(Path::new("."));

        // Build bone_id → name map for anim track naming
        let bone_id_to_name: HashMap<i64, String> = bones.iter()
            .map(|b| (b.id, b.name.clone()))
            .collect();

        for stack in &anim_stacks {
            let anim_file = build_anim_file(stack, &bone_id_to_name);
            let file_name = format!("{}@{}.anim", stem, stack.name);
            let anim_path = parent_dir.join(&file_name);

            match ron::ser::to_string_pretty(&anim_file, ron::ser::PrettyConfig::default()) {
                Ok(ron_str) => {
                    if let Err(e) = std::fs::write(&anim_path, &ron_str) {
                        log::warn!("Failed to write {}: {}", anim_path.display(), e);
                    } else {
                        log::info!("Wrote animation: {}", anim_path.display());
                        written_anims.push(anim_path);
                    }
                }
                Err(e) => {
                    log::warn!("Failed to serialize anim '{}': {}", stack.name, e);
                }
            }
        }
    }

    Ok(written_anims)
}

/// Convert an AnimStackData into an AnimFile for serialization.
fn build_anim_file(
    stack: &fbx_skeleton::AnimStackData,
    bone_id_to_name: &HashMap<i64, String>,
) -> AnimFile {
    use std::collections::BTreeMap;

    // Group curve nodes by bone_id
    let mut per_bone: BTreeMap<i64, Vec<&fbx_skeleton::AnimCurveNode>> = BTreeMap::new();
    for cn in &stack.curve_nodes {
        per_bone.entry(cn.bone_id).or_default().push(cn);
    }

    let mut tracks: Vec<BoneTrack> = Vec::new();

    for (bone_id, curve_nodes) in per_bone {
        let bone_name = match bone_id_to_name.get(&bone_id) {
            Some(n) => n.clone(),
            None => continue,
        };

        let mut translations: Vec<(f32, [f32; 3])> = Vec::new();
        let mut rotations: Vec<(f32, [f32; 4])> = Vec::new();
        let mut scales: Vec<(f32, [f32; 3])> = Vec::new();

        for cn in curve_nodes {
            match cn.property.as_str() {
                "Lcl Translation" => {
                    // Merge X/Y/Z into vec3 keyframes
                    let merged = merge_xyz_curves(&cn.x_curve, &cn.y_curve, &cn.z_curve);
                    translations = merged.into_iter().map(|(t, x, y, z)| (t, [x, y, z])).collect();
                }
                "Lcl Rotation" => {
                    // Convert Euler degrees to quaternions
                    let merged = merge_xyz_curves(&cn.x_curve, &cn.y_curve, &cn.z_curve);
                    rotations = merged.into_iter()
                        .map(|(t, x, y, z)| (t, euler_zyx_to_quat(x, y, z)))
                        .collect();
                }
                "Lcl Scaling" => {
                    let merged = merge_xyz_curves(&cn.x_curve, &cn.y_curve, &cn.z_curve);
                    scales = merged.into_iter().map(|(t, x, y, z)| (t, [x, y, z])).collect();
                }
                _ => {}
            }
        }

        if translations.is_empty() && rotations.is_empty() && scales.is_empty() {
            continue;
        }

        tracks.push(BoneTrack { bone_name, translations, rotations, scales });
    }

    AnimFile {
        name: stack.name.clone(),
        duration: stack.duration,
        tracks,
    }
}

/// Merge separate X, Y, Z keyframe curves into (time, x, y, z) tuples.
/// Uses linear interpolation to fill in missing values at times present in any channel.
fn merge_xyz_curves(
    x_curve: &[fbx_skeleton::AnimKeyframe],
    y_curve: &[fbx_skeleton::AnimKeyframe],
    z_curve: &[fbx_skeleton::AnimKeyframe],
) -> Vec<(f32, f32, f32, f32)> {
    // Collect all unique times
    let mut times: Vec<f32> = Vec::new();
    for kf in x_curve.iter().chain(y_curve).chain(z_curve) {
        if !times.iter().any(|&t| (t - kf.time).abs() < 1e-6) {
            times.push(kf.time);
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    times.iter().map(|&t| {
        let x = sample_curve(x_curve, t);
        let y = sample_curve(y_curve, t);
        let z = sample_curve(z_curve, t);
        (t, x, y, z)
    }).collect()
}

fn sample_curve(curve: &[fbx_skeleton::AnimKeyframe], time: f32) -> f32 {
    if curve.is_empty() {
        return 0.0;
    }
    if time <= curve[0].time {
        return curve[0].value;
    }
    if time >= curve[curve.len() - 1].time {
        return curve[curve.len() - 1].value;
    }
    // Linear interpolation
    for i in 0..curve.len() - 1 {
        if time >= curve[i].time && time <= curve[i + 1].time {
            let t = (time - curve[i].time) / (curve[i + 1].time - curve[i].time);
            return curve[i].value + t * (curve[i + 1].value - curve[i].value);
        }
    }
    curve.last().map(|k| k.value).unwrap_or(0.0)
}

fn parse_material_node(node: &fbxcel::tree::v7400::NodeHandle, builder: &mut GlbBuilder) {
    use fbx_skeleton::{get_string, get_f64};
    let attrs = node.attributes();
    let name = attrs.get(1).and_then(get_string).map(|s| clean_fbx_name(&s));

    let mut base_color = [0.8f32, 0.8, 0.8, 1.0];

    for prop_child in node.children() {
        if prop_child.name() == "Properties70" {
            for p in prop_child.children() {
                if p.name() == "P" {
                    let p_attrs = p.attributes();
                    if let Some(prop_name) = p_attrs.get(0).and_then(get_string) {
                        if prop_name == "DiffuseColor" && p_attrs.len() >= 7 {
                            let r = get_f64(&p_attrs[4]).unwrap_or(0.8) as f32;
                            let g = get_f64(&p_attrs[5]).unwrap_or(0.8) as f32;
                            let b = get_f64(&p_attrs[6]).unwrap_or(0.8) as f32;
                            base_color = [r, g, b, 1.0];
                        }
                    }
                }
            }
        }
    }

    builder.add_material(MaterialData {
        name,
        base_color,
        metallic: 0.0,
        roughness: 0.8,
        base_color_texture_index: None,
    });
}

fn parse_geometry_node(node: &fbxcel::tree::v7400::NodeHandle) -> Option<MeshData> {
    use fbx_skeleton::{get_string, get_f64_array, get_i32_array};
    let node_attrs = node.attributes();
    let name = node_attrs.get(1).and_then(get_string).map(|s| clean_fbx_name(&s));

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals_raw: Vec<[f32; 3]> = Vec::new();
    let mut uvs_raw: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for child in node.children() {
        match child.name() {
            "Vertices" => {
                if let Some(data) = child.attributes().get(0).and_then(get_f64_array) {
                    for chunk in data.chunks(3) {
                        if chunk.len() == 3 {
                            positions.push([chunk[0] as f32, chunk[1] as f32, chunk[2] as f32]);
                        }
                    }
                }
            }
            "PolygonVertexIndex" => {
                if let Some(data) = child.attributes().get(0).and_then(get_i32_array) {
                    let mut polygon_verts: Vec<i32> = Vec::new();
                    for &idx in &data {
                        let actual_idx = if idx < 0 { !idx } else { idx };
                        polygon_verts.push(actual_idx);
                        if idx < 0 {
                            if polygon_verts.len() >= 3 {
                                for i in 1..polygon_verts.len() - 1 {
                                    indices.push(polygon_verts[0] as u32);
                                    indices.push(polygon_verts[i] as u32);
                                    indices.push(polygon_verts[i + 1] as u32);
                                }
                            }
                            polygon_verts.clear();
                        }
                    }
                }
            }
            "LayerElementNormal" => {
                for sub in child.children() {
                    if sub.name() == "Normals" {
                        if let Some(data) = sub.attributes().get(0).and_then(get_f64_array) {
                            for chunk in data.chunks(3) {
                                if chunk.len() == 3 {
                                    normals_raw.push([chunk[0] as f32, chunk[1] as f32, chunk[2] as f32]);
                                }
                            }
                        }
                    }
                }
            }
            "LayerElementUV" => {
                for sub in child.children() {
                    if sub.name() == "UV" {
                        if let Some(data) = sub.attributes().get(0).and_then(get_f64_array) {
                            for chunk in data.chunks(2) {
                                if chunk.len() == 2 {
                                    uvs_raw.push([chunk[0] as f32, 1.0 - chunk[1] as f32]);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if positions.is_empty() {
        return None;
    }

    let normals = if normals_raw.len() == positions.len() { Some(normals_raw) } else { None };
    let tex_coords = if uvs_raw.len() == positions.len() { Some(uvs_raw) } else { None };

    Some(MeshData {
        name,
        positions,
        normals,
        tex_coords,
        indices: if indices.is_empty() { None } else { Some(indices) },
        material_index: None,
        joints: None,
        weights: None,
    })
}
