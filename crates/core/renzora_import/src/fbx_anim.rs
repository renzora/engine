//! Extract animations directly from FBX files (any version).
//!
//! Parses the FBX binary tree, reads Connections/Objects sections, and
//! reconstructs animation clips as `.anim` files.  Works independently of
//! geometry conversion so animation-only FBX files import correctly.

use std::collections::HashMap;
use std::path::Path;

use renzora_animation::clip::{AnimClip, BoneTrack};
use renzora_animation::extract::write_anim_file;

use crate::anim_extract::AnimExtractResult;
use crate::fbx_legacy::{
    extract_f32_array, extract_i64_array, find_child, get_i64_prop, get_string_prop, FbxNode,
};

/// FBX time ticks per second.
const FBX_TICKS_PER_SECOND: f64 = 46_186_158_000.0;

// ─── Connection model ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Connection {
    src_id: i64,
    dst_id: i64,
    /// Property name for OP connections (e.g. "Lcl Translation", "d|X").
    property: Option<String>,
}

/// Parse the Connections section into a list of connections.
fn parse_connections(nodes: &[FbxNode]) -> Vec<Connection> {
    let mut conns = Vec::new();
    for node in nodes {
        if node.name != "Connections" {
            continue;
        }
        for child in &node.children {
            if child.name != "C" {
                continue;
            }
            // C: "OO"|"OP", src_id, dst_id [, property]
            let src_id = get_i64_prop(child, 1);
            let dst_id = get_i64_prop(child, 2);
            if let (Some(src), Some(dst)) = (src_id, dst_id) {
                let property = get_string_prop(child, 0)
                    .and_then(|conn_type| {
                        if conn_type == "OP" {
                            get_string_prop(child, 3).map(|s| s.to_string())
                        } else {
                            None
                        }
                    });
                conns.push(Connection {
                    src_id: src,
                    dst_id: dst,
                    property,
                });
            }
        }
    }
    conns
}

/// Build lookup: child_id → list of (parent_id, optional property).
fn build_child_to_parent_map(conns: &[Connection]) -> HashMap<i64, Vec<(i64, Option<String>)>> {
    let mut map: HashMap<i64, Vec<(i64, Option<String>)>> = HashMap::new();
    for c in conns {
        map.entry(c.src_id)
            .or_default()
            .push((c.dst_id, c.property.clone()));
    }
    map
}

/// Build lookup: parent_id → list of (child_id, optional property).
fn build_parent_to_children_map(conns: &[Connection]) -> HashMap<i64, Vec<(i64, Option<String>)>> {
    let mut map: HashMap<i64, Vec<(i64, Option<String>)>> = HashMap::new();
    for c in conns {
        map.entry(c.dst_id)
            .or_default()
            .push((c.src_id, c.property.clone()));
    }
    map
}

// ─── Object index ──────────────────────────────────────────────────────────

struct ObjectIndex<'a> {
    /// object_id → (class, name, node)
    objects: HashMap<i64, (&'a str, String, &'a FbxNode)>,
}

impl<'a> ObjectIndex<'a> {
    fn build(nodes: &'a [FbxNode]) -> Self {
        let mut objects = HashMap::new();
        for node in nodes {
            if node.name != "Objects" {
                continue;
            }
            for child in &node.children {
                // Each object: ClassName: id, "TypeName::Name", "SubType" { ... }
                let id = match get_i64_prop(child, 0) {
                    Some(id) => id,
                    None => continue,
                };

                let raw_name = get_string_prop(child, 1).unwrap_or("");
                // Strip "TypeName::" prefix to get the display name
                let name = raw_name
                    .find("::")
                    .map(|i| &raw_name[i + 2..])
                    .unwrap_or(raw_name)
                    .to_string();

                objects.insert(id, (child.name.as_str(), name, child));
            }
        }
        ObjectIndex { objects }
    }

    fn get(&self, id: i64) -> Option<&(&'a str, String, &'a FbxNode)> {
        self.objects.get(&id)
    }
}

// ─── Animation extraction ──────────────────────────────────────────────────

/// Euler (degrees) → quaternion (x, y, z, w).
/// FBX uses intrinsic XYZ rotation order by default.
fn euler_to_quat(x_deg: f32, y_deg: f32, z_deg: f32) -> [f32; 4] {
    let (hx, hy, hz) = (
        (x_deg * std::f32::consts::PI / 180.0) * 0.5,
        (y_deg * std::f32::consts::PI / 180.0) * 0.5,
        (z_deg * std::f32::consts::PI / 180.0) * 0.5,
    );
    let (sx, cx) = hx.sin_cos();
    let (sy, cy) = hy.sin_cos();
    let (sz, cz) = hz.sin_cos();

    // ZYX extrinsic = XYZ intrinsic
    let w = cx * cy * cz + sx * sy * sz;
    let x = sx * cy * cz - cx * sy * sz;
    let y = cx * sy * cz + sx * cy * sz;
    let z = cx * cy * sz - sx * sy * cz;

    [x, y, z, w]
}

/// Extract animations from a parsed FBX node tree and write `.anim` files.
pub fn extract(nodes: &[FbxNode], output_dir: &Path) -> Result<AnimExtractResult, String> {
    let conns = parse_connections(nodes);
    let child_to_parent = build_child_to_parent_map(&conns);
    let parent_to_children = build_parent_to_children_map(&conns);
    let index = ObjectIndex::build(nodes);

    let mut result = AnimExtractResult {
        written_files: Vec::new(),
        warnings: Vec::new(),
    };

    // Find all AnimationStack objects (top-level animation takes)
    let stacks: Vec<(i64, &str, String, &FbxNode)> = index
        .objects
        .iter()
        .filter(|(_, (class, _, _))| *class == "AnimationStack")
        .map(|(&id, (class, name, node))| (id, *class, name.clone(), *node))
        .collect();

    if stacks.is_empty() {
        return Ok(result);
    }

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("failed to create animations directory: {}", e))?;

    for (stack_id, _, stack_name, _stack_node) in &stacks {
        let clip_name = if stack_name.is_empty() {
            format!("clip_{}", stack_id)
        } else {
            stack_name.clone()
        };

        // Collect all AnimationCurveNode IDs under this stack (via layers)
        let mut curve_node_ids: Vec<i64> = Vec::new();
        if let Some(layer_ids) = parent_to_children.get(stack_id) {
            for &(layer_id, _) in layer_ids {
                if let Some((class, _, _)) = index.get(layer_id) {
                    if *class == "AnimationLayer" {
                        if let Some(cn_ids) = parent_to_children.get(&layer_id) {
                            for &(cn_id, _) in cn_ids {
                                if let Some((cn_class, _, _)) = index.get(cn_id) {
                                    if *cn_class == "AnimationCurveNode" {
                                        curve_node_ids.push(cn_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if curve_node_ids.is_empty() {
            result.warnings.push(format!(
                "{}: no animation curve nodes found",
                clip_name
            ));
            continue;
        }

        // For each AnimationCurveNode, find:
        // 1. The target Model (bone) and property type (T/R/S)
        // 2. The child AnimationCurve(s) with axis (d|X, d|Y, d|Z)
        //
        // bone_id → (bone_name, { "T"|"R"|"S" → { "X"|"Y"|"Z" → curve_data } })
        struct CurveData {
            times: Vec<f32>,
            values: Vec<f32>,
        }

        let mut bone_curves: HashMap<
            i64,
            (String, HashMap<String, HashMap<String, CurveData>>),
        > = HashMap::new();

        let mut duration: f32 = 0.0;

        for cn_id in &curve_node_ids {
            // Find target bone: CurveNode → Model (via OP connection with property name)
            let parents = match child_to_parent.get(cn_id) {
                Some(p) => p,
                None => continue,
            };

            let mut target_bone_id: Option<i64> = None;
            let mut prop_type: Option<String> = None; // "T", "R", or "S"

            for (parent_id, prop) in parents {
                if let Some((class, _, _)) = index.get(*parent_id) {
                    if *class == "Model" {
                        target_bone_id = Some(*parent_id);
                        prop_type = prop.as_ref().and_then(|p| match p.as_str() {
                            "Lcl Translation" => Some("T".to_string()),
                            "Lcl Rotation" => Some("R".to_string()),
                            "Lcl Scaling" => Some("S".to_string()),
                            _ => None,
                        });
                        break;
                    }
                }
            }

            let bone_id = match target_bone_id {
                Some(id) => id,
                None => continue,
            };
            let prop_key = match prop_type {
                Some(p) => p,
                None => continue,
            };

            let bone_name = index
                .get(bone_id)
                .map(|(_, name, _)| name.clone())
                .unwrap_or_else(|| format!("bone_{}", bone_id));

            // Find child AnimationCurve objects
            let children = match parent_to_children.get(cn_id) {
                Some(c) => c,
                None => continue,
            };

            for &(curve_id, ref axis_prop) in children {
                if let Some((class, _, curve_node)) = index.get(curve_id) {
                    if *class != "AnimationCurve" {
                        continue;
                    }

                    let axis = match axis_prop.as_deref() {
                        Some("d|X") => "X",
                        Some("d|Y") => "Y",
                        Some("d|Z") => "Z",
                        _ => continue,
                    };

                    // Extract KeyTime (i64 array) and KeyValueFloat (f32 array)
                    let key_times = find_child(curve_node, "KeyTime")
                        .map(extract_i64_array)
                        .unwrap_or_default();
                    let key_values = find_child(curve_node, "KeyValueFloat")
                        .map(extract_f32_array)
                        .unwrap_or_default();

                    let count = key_times.len().min(key_values.len());
                    if count == 0 {
                        continue;
                    }

                    let times: Vec<f32> = key_times[..count]
                        .iter()
                        .map(|&t| (t as f64 / FBX_TICKS_PER_SECOND) as f32)
                        .collect();

                    if let Some(&last) = times.last() {
                        duration = duration.max(last);
                    }

                    let entry = bone_curves
                        .entry(bone_id)
                        .or_insert_with(|| (bone_name.clone(), HashMap::new()));
                    let axis_map = entry.1.entry(prop_key.clone()).or_default();
                    axis_map.insert(axis.to_string(), CurveData {
                        times,
                        values: key_values[..count].to_vec(),
                    });
                }
            }
        }

        // Build BoneTracks from collected curve data
        let mut tracks: Vec<BoneTrack> = Vec::new();

        for (_bone_id, (bone_name, prop_map)) in &bone_curves {
            let mut track = BoneTrack {
                bone_name: bone_name.clone(),
                translations: Vec::new(),
                rotations: Vec::new(),
                scales: Vec::new(),
            };

            // Translations
            if let Some(axes) = prop_map.get("T") {
                let x = axes.get("X");
                let y = axes.get("Y");
                let z = axes.get("Z");
                if let Some(ref_axis) = x.or(y).or(z) {
                    for (i, &t) in ref_axis.times.iter().enumerate() {
                        let vx = x.and_then(|a| a.values.get(i).copied()).unwrap_or(0.0);
                        let vy = y.and_then(|a| a.values.get(i).copied()).unwrap_or(0.0);
                        let vz = z.and_then(|a| a.values.get(i).copied()).unwrap_or(0.0);
                        track.translations.push((t, [vx, vy, vz]));
                    }
                }
            }

            // Rotations (Euler degrees → quaternion)
            if let Some(axes) = prop_map.get("R") {
                let x = axes.get("X");
                let y = axes.get("Y");
                let z = axes.get("Z");
                if let Some(ref_axis) = x.or(y).or(z) {
                    for (i, &t) in ref_axis.times.iter().enumerate() {
                        let rx = x.and_then(|a| a.values.get(i).copied()).unwrap_or(0.0);
                        let ry = y.and_then(|a| a.values.get(i).copied()).unwrap_or(0.0);
                        let rz = z.and_then(|a| a.values.get(i).copied()).unwrap_or(0.0);
                        track.rotations.push((t, euler_to_quat(rx, ry, rz)));
                    }
                }
            }

            // Scales
            if let Some(axes) = prop_map.get("S") {
                let x = axes.get("X");
                let y = axes.get("Y");
                let z = axes.get("Z");
                if let Some(ref_axis) = x.or(y).or(z) {
                    for (i, &t) in ref_axis.times.iter().enumerate() {
                        let sx = x.and_then(|a| a.values.get(i).copied()).unwrap_or(1.0);
                        let sy = y.and_then(|a| a.values.get(i).copied()).unwrap_or(1.0);
                        let sz = z.and_then(|a| a.values.get(i).copied()).unwrap_or(1.0);
                        track.scales.push((t, [sx, sy, sz]));
                    }
                }
            }

            if !track.translations.is_empty()
                || !track.rotations.is_empty()
                || !track.scales.is_empty()
            {
                tracks.push(track);
            }
        }

        if tracks.is_empty() {
            result.warnings.push(format!(
                "{}: animation stack has no usable tracks",
                clip_name
            ));
            continue;
        }

        let clip = AnimClip {
            name: clip_name.clone(),
            duration,
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
                    duration,
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

// ─── Public API ────────────────────────────────────────────────────────────

/// Extract animations from an FBX file (any binary version) and write `.anim` files.
pub fn extract_animations_from_fbx(
    path: &Path,
    output_dir: &Path,
) -> Result<AnimExtractResult, String> {
    let data =
        std::fs::read(path).map_err(|e| format!("failed to read FBX file: {}", e))?;

    let (_version, nodes) = crate::fbx_legacy::parse_document(&data)
        .map_err(|e| format!("FBX parse error: {}", e))?;

    extract(&nodes, output_dir)
}
