//! Extract animations from GLB data and write `.anim` RON files.
//!
//! After a model is imported and converted to GLB, this module parses the GLB
//! binary to find animation clips and writes each one as a `.anim` file.

use std::collections::HashMap;
use std::path::Path;

use renzora::write_anim_file;
use renzora::{AnimClip, BoneTrack, KeyCurve, KeyMode, MorphTrack, RestBone, SkeletonRestPose};

/// Result of animation extraction for a single GLB.
#[derive(Debug)]
pub struct AnimExtractResult {
    /// Paths to the `.anim` files that were written.
    pub written_files: Vec<String>,
    /// Warnings encountered during extraction.
    pub warnings: Vec<String>,
}

/// Extract all animations from GLB bytes and write `.anim` files to `output_dir`.
///
/// Each animation in the GLB becomes a separate `.anim` file named after the
/// animation (or `clip_0`, `clip_1`, etc. if unnamed).
pub fn extract_animations_from_glb(
    glb_bytes: &[u8],
    output_dir: &Path,
) -> Result<AnimExtractResult, String> {
    let glb =
        gltf::Gltf::from_slice(glb_bytes).map_err(|e| format!("Failed to parse GLB: {}", e))?;

    let blob = glb.blob.as_deref();

    // Build a map of buffer index → data
    let buffers = load_buffers(&glb, blob)?;

    // Build node index → node name map for resolving bone/target names
    let node_names: HashMap<usize, String> = glb
        .nodes()
        .map(|node| {
            let name = node
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| format!("node_{}", node.index()));
            (node.index(), name)
        })
        .collect();

    let mut result = AnimExtractResult {
        written_files: Vec::new(),
        warnings: Vec::new(),
    };

    if glb.animations().len() == 0 {
        return Ok(result);
    }

    // Ensure output directory exists
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create animations directory: {}", e))?;

    for anim in glb.animations() {
        let clip_name = anim
            .name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("clip_{}", anim.index()));

        // Group channels by target node
        let mut bone_tracks: HashMap<usize, BoneTrack> = HashMap::new();
        let mut morph_tracks: Vec<MorphTrack> = Vec::new();
        let mut duration: f32 = 0.0;

        for channel in anim.channels() {
            let target = channel.target();
            let node_idx = target.node().index();
            let bone_name = node_names
                .get(&node_idx)
                .cloned()
                .unwrap_or_else(|| format!("node_{}", node_idx));

            let reader =
                channel.reader(|buffer| buffers.get(&buffer.index()).map(|v| v.as_slice()));

            let timestamps: Vec<f32> = match reader.read_inputs() {
                Some(iter) => iter.collect(),
                None => {
                    result.warnings.push(format!(
                        "{}: missing timestamps for bone '{}'",
                        clip_name, bone_name
                    ));
                    continue;
                }
            };

            if let Some(&last) = timestamps.last() {
                duration = duration.max(last);
            }

            // Blend shape weights go to morph tracks and must not leave an
            // empty bone track behind for the mesh node.
            let outputs = match reader.read_outputs() {
                Some(gltf::animation::util::ReadOutputs::MorphTargetWeights(weights)) => {
                    let weights: Vec<f32> = weights.into_f32().collect();
                    let names = target.node().mesh().map(|m| morph_target_names(&m)).unwrap_or_default();
                    let interpolation = channel.sampler().interpolation();
                    match morph_tracks_from_channel(&bone_name, &timestamps, &weights, interpolation, &names) {
                        Some(tracks) => morph_tracks.extend(tracks),
                        None => result.warnings.push(format!(
                            "{}: blend shape weights on '{}' do not divide into keys, skipping",
                            clip_name, bone_name
                        )),
                    }
                    continue;
                }
                other => other,
            };

            let track = bone_tracks.entry(node_idx).or_insert_with(|| BoneTrack::new(bone_name.clone()));

            match outputs {
                Some(gltf::animation::util::ReadOutputs::Translations(translations)) => {
                    track.translations = timestamps
                        .iter()
                        .zip(translations)
                        .map(|(&t, v)| (t, v))
                        .collect();
                }
                Some(gltf::animation::util::ReadOutputs::Rotations(rotations)) => {
                    track.rotations = timestamps
                        .iter()
                        .zip(rotations.into_f32())
                        .map(|(&t, q)| (t, q))
                        .collect();
                }
                Some(gltf::animation::util::ReadOutputs::Scales(scales)) => {
                    track.scales = timestamps
                        .iter()
                        .zip(scales)
                        .map(|(&t, v)| (t, v))
                        .collect();
                }
                // Taken above.
                Some(gltf::animation::util::ReadOutputs::MorphTargetWeights(_)) => {}
                None => {
                    result.warnings.push(format!(
                        "{}: missing output data for bone '{}'",
                        clip_name, bone_name
                    ));
                }
            }
        }

        // Skip clips with no usable tracks. A clip of blend shapes alone (a
        // facial performance) is usable.
        let tracks: Vec<BoneTrack> = bone_tracks.into_values().collect();
        if tracks.is_empty() && morph_tracks.is_empty() {
            result.warnings.push(format!(
                "{}: no animation tracks found, skipping",
                clip_name
            ));
            continue;
        }

        let mut clip = AnimClip {
            name: clip_name.clone(),
            duration,
            tracks,
            morph_tracks,
            ..Default::default()
        };
        crate::anim_decimate::decimate_clip(&mut clip);

        // Sanitize filename
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
                result.written_files.push(file_path.display().to_string());
            }
            Err(e) => {
                result
                    .warnings
                    .push(format!("{}: failed to write .anim file: {}", clip_name, e));
            }
        }
    }

    Ok(result)
}

/// The names a mesh gives its morph targets, from the `targetNames` extra
/// every major exporter writes (glTF itself has no field for them). Empty when
/// the mesh names none; a track then shows as "Shape N".
fn morph_target_names(mesh: &gltf::Mesh) -> Vec<String> {
    let Some(raw) = mesh.extras().as_ref() else { return Vec::new() };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw.get()) else { return Vec::new() };
    value
        .get("targetNames")
        .and_then(|n| n.as_array())
        .map(|names| names.iter().map(|n| n.as_str().unwrap_or_default().to_string()).collect())
        .unwrap_or_default()
}

/// One glTF `weights` channel as one morph track per target.
///
/// glTF packs every target's weight into each key (`keys x targets` values,
/// three times that for a cubic spline, whose keys are in-tangent, value,
/// out-tangent). The tangents carry over exactly: a glTF cubic key's tangents
/// are slopes in weight per second, which is what [`KeyMode::Broken`] reads.
/// A step channel holds, which is [`KeyMode::Constant`].
///
/// A target the clip never moves off zero gets no track: zero is a blend
/// shape's rest weight, and a face with fifty shapes would otherwise put fifty
/// rows of nothing in the dopesheet. Returns `None` when the value count does
/// not divide into the keys.
pub(crate) fn morph_tracks_from_channel(
    node: &str,
    times: &[f32],
    values: &[f32],
    interpolation: gltf::animation::Interpolation,
    names: &[String],
) -> Option<Vec<MorphTrack>> {
    use gltf::animation::Interpolation;
    let cubic = interpolation == Interpolation::CubicSpline;
    let per_key = if cubic { 3 } else { 1 };
    let n = times.len();
    if n == 0 {
        return Some(Vec::new());
    }
    if values.is_empty() || !values.len().is_multiple_of(n * per_key) {
        return None;
    }
    let count = values.len() / (n * per_key);
    let mut tracks: Vec<MorphTrack> =
        (0..count).map(|i| MorphTrack::new(node, i, names.get(i).cloned().unwrap_or_default())).collect();
    for (k, &t) in times.iter().enumerate() {
        let base = k * count * per_key;
        for (i, track) in tracks.iter_mut().enumerate() {
            let value = if cubic { values[base + count + i] } else { values[base + i] };
            track.keys.push((t, value));
            match interpolation {
                Interpolation::Step => track.curves.push(KeyCurve::new(KeyMode::Constant)),
                Interpolation::CubicSpline => track.curves.push(KeyCurve {
                    mode: KeyMode::Broken,
                    in_tan: [values[base + i], 0.0, 0.0, 0.0],
                    out_tan: [values[base + 2 * count + i], 0.0, 0.0, 0.0],
                }),
                Interpolation::Linear => {}
            }
        }
    }
    tracks.retain(|t| t.keys.iter().any(|(_, w)| *w != 0.0));
    Some(tracks)
}

/// The rest (bind) pose of a GLB's skeleton, as retargeting reads it from a
/// `.skel` file (`renzora_animation::retarget`). `None` when the GLB has no
/// skin, since an unskinned model has no skeleton to retarget from.
///
/// Every joint is recorded, and so is every node above a joint up to the scene
/// root: retargeting measures a bone's rotation in the space of the hips'
/// parent, so an armature node above the hips (an FBX unit scale, a Z-up
/// correction) has to be in the pose, and the target side
/// (`retarget::capture_skeleton`) records the same nodes from the spawned
/// model. Names are spelled as the clips spell them (`node_N` for an unnamed
/// node, as [`extract_animations_from_glb`] names its tracks); the transforms
/// are each node's local TRS, which is what the node's `Transform` holds when
/// the model spawns unposed.
pub fn rest_pose_from_glb(glb_bytes: &[u8]) -> Result<Option<SkeletonRestPose>, String> {
    let glb = gltf::Gltf::from_slice(glb_bytes).map_err(|e| format!("Failed to parse GLB: {}", e))?;
    if glb.skins().len() == 0 {
        return Ok(None);
    }
    let count = glb.nodes().len();
    let mut parent: Vec<Option<usize>> = vec![None; count];
    for node in glb.nodes() {
        for child in node.children() {
            parent[child.index()] = Some(node.index());
        }
    }
    let mut keep = vec![false; count];
    for skin in glb.skins() {
        for joint in skin.joints() {
            let mut at = Some(joint.index());
            let mut depth = 0;
            while let Some(i) = at {
                if keep[i] || depth > count {
                    break;
                }
                keep[i] = true;
                depth += 1;
                at = parent[i];
            }
        }
    }
    let nodes: Vec<gltf::Node> = glb.nodes().collect();
    let name_of = |i: usize| nodes[i].name().map(str::to_string).unwrap_or_else(|| format!("node_{}", i));
    // Parents before children, in the file's own order below each parent.
    let mut bones = Vec::new();
    let mut stack: Vec<usize> = (0..count).filter(|&i| parent[i].is_none()).rev().collect();
    let mut visited = vec![false; count];
    while let Some(i) = stack.pop() {
        if std::mem::replace(&mut visited[i], true) {
            continue;
        }
        if keep[i] {
            let (translation, rotation, scale) = nodes[i].transform().decomposed();
            bones.push(RestBone { name: name_of(i), parent: parent[i].map(name_of), translation, rotation, scale });
        }
        let kids: Vec<usize> = nodes[i].children().map(|c| c.index()).collect();
        stack.extend(kids.into_iter().rev());
    }
    Ok(Some(SkeletonRestPose { bones, ..Default::default() }))
}

/// Write `<output_dir>/<model_name>.skel` for a skinned GLB, claiming the
/// clips just extracted from it (`clip_files`, the paths
/// [`AnimExtractResult::written_files`] lists), so clip discovery can give
/// each of those clips' slots its source skeleton. Returns the path written,
/// or `None` for a model with no skeleton.
pub fn write_rest_skeleton(
    glb_bytes: &[u8],
    output_dir: &Path,
    model_name: &str,
    clip_files: &[String],
) -> Result<Option<std::path::PathBuf>, String> {
    let Some(mut pose) = rest_pose_from_glb(glb_bytes)? else { return Ok(None) };
    pose.clips = clip_files
        .iter()
        .filter_map(|p| Path::new(p).file_name().and_then(|n| n.to_str()).map(str::to_string))
        .collect();
    let path = output_dir.join(format!("{}.skel", model_name));
    renzora::write_skeleton_file(&pose, &path)?;
    Ok(Some(path))
}

/// Load all buffer data referenced by the GLTF document.
fn load_buffers(glb: &gltf::Gltf, blob: Option<&[u8]>) -> Result<HashMap<usize, Vec<u8>>, String> {
    let mut buffers = HashMap::new();
    for buffer in glb.buffers() {
        match buffer.source() {
            gltf::buffer::Source::Bin => {
                if let Some(data) = blob {
                    buffers.insert(buffer.index(), data.to_vec());
                } else {
                    return Err(format!(
                        "GLB references binary chunk but none found (buffer {})",
                        buffer.index()
                    ));
                }
            }
            gltf::buffer::Source::Uri(_uri) => {
                // External URIs shouldn't appear in a packed GLB, but handle gracefully
                return Err(format!(
                    "Buffer {} references external URI; expected embedded GLB data",
                    buffer.index()
                ));
            }
        }
    }
    Ok(buffers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gltf::animation::Interpolation;

    /// A tiny skinned GLB: `Armature > Hips > Spine` with `Hips` and `Spine` as
    /// joints, a `Face` mesh with two named morph targets, and one clip moving
    /// the hips and stepping both shapes.
    fn tiny_glb() -> Vec<u8> {
        let floats: [f32; 21] = [
            0.0, 1.0, // times
            0.0, 1.0, 0.0, 0.0, 2.0, 0.0, // hips translations
            0.0, 0.5, 1.0, 0.5, // weights, key x target
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, // positions
        ];
        let bin: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
        let json = serde_json::json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "byteLength": bin.len() }],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": 8 },
                { "buffer": 0, "byteOffset": 8, "byteLength": 24 },
                { "buffer": 0, "byteOffset": 32, "byteLength": 16 },
                { "buffer": 0, "byteOffset": 48, "byteLength": 36 }
            ],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0.0], "max": [1.0] },
                { "bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC3" },
                { "bufferView": 2, "componentType": 5126, "count": 4, "type": "SCALAR" },
                { "bufferView": 3, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }
            ],
            "meshes": [{
                "primitives": [{ "attributes": { "POSITION": 3 }, "targets": [{ "POSITION": 3 }, { "POSITION": 3 }] }],
                "weights": [0.0, 0.0],
                "extras": { "targetNames": ["smile", "blink"] }
            }],
            "skins": [{ "joints": [1, 2] }],
            "nodes": [
                { "name": "Armature", "children": [1, 3] },
                { "name": "Hips", "translation": [0.0, 1.0, 0.0], "children": [2] },
                { "name": "Spine", "translation": [0.0, 0.5, 0.0], "rotation": [0.0, 0.0, 0.38268343, 0.9238795] },
                { "name": "Face", "mesh": 0 }
            ],
            "scenes": [{ "nodes": [0] }],
            "scene": 0,
            "animations": [{
                "name": "talk",
                "samplers": [
                    { "input": 0, "output": 1, "interpolation": "LINEAR" },
                    { "input": 0, "output": 2, "interpolation": "STEP" }
                ],
                "channels": [
                    { "sampler": 0, "target": { "node": 1, "path": "translation" } },
                    { "sampler": 1, "target": { "node": 3, "path": "weights" } }
                ]
            }]
        });
        crate::gltf_pass::pack_glb(json.to_string().as_bytes(), Some(&bin))
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("renzora_anim_extract_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn gltf_blend_shape_weights_become_morph_tracks() {
        let dir = temp_dir("morph");
        let result = extract_animations_from_glb(&tiny_glb(), &dir).unwrap();
        assert_eq!(result.written_files.len(), 1, "{:?}", result.warnings);
        let text = std::fs::read_to_string(&result.written_files[0]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let clip: AnimClip = ron::from_str(&text).unwrap();

        // The mesh node gets morph tracks, not an empty bone track.
        assert_eq!(clip.tracks.len(), 1);
        assert_eq!(clip.tracks[0].bone_name, "Hips");
        assert_eq!(clip.morph_tracks.len(), 2);
        let smile = &clip.morph_tracks[0];
        assert_eq!((smile.target.as_str(), smile.index, smile.name.as_str()), ("Face", 0, "smile"));
        assert_eq!(smile.keys, vec![(0.0, 0.0), (1.0, 1.0)]);
        // STEP holds each key.
        assert!(smile.curves.iter().all(|c| c.mode == KeyMode::Constant));
        assert_eq!(smile.sample(0.99), Some(0.0));
        let blink = &clip.morph_tracks[1];
        assert_eq!((blink.index, blink.name.as_str()), (1, "blink"));
        assert_eq!(blink.keys, vec![(0.0, 0.5), (1.0, 0.5)]);
    }

    #[test]
    fn cubic_blend_shape_keys_keep_their_tangents() {
        // Two keys, one target: [in, value, out] per key.
        let values = [0.0, 0.0, 2.0, -1.0, 1.0, 0.0];
        let tracks = morph_tracks_from_channel("Face", &[0.0, 1.0], &values, Interpolation::CubicSpline, &[]).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].keys, vec![(0.0, 0.0), (1.0, 1.0)]);
        assert_eq!(tracks[0].curves[0].mode, KeyMode::Broken);
        assert_eq!(tracks[0].curves[0].out_tan[0], 2.0);
        assert_eq!(tracks[0].curves[1].in_tan[0], -1.0);
        // A shape left at zero throughout gets no track; a count that does not
        // divide into the keys is refused.
        assert!(morph_tracks_from_channel("Face", &[0.0, 1.0], &[0.0, 0.0], Interpolation::Linear, &[]).unwrap().is_empty());
        assert!(morph_tracks_from_channel("Face", &[0.0, 1.0], &[0.0, 0.0, 1.0], Interpolation::Linear, &[]).is_none());
    }

    /// The `.skel` holds what `renzora_animation::retarget` reads: every joint
    /// and the armature above it, parents before children, with local TRS, and
    /// the clips it was written beside.
    #[test]
    fn the_rest_skeleton_is_the_bind_pose_retargeting_reads() {
        let dir = temp_dir("skel");
        let clips = vec![dir.join("talk.anim").display().to_string()];
        let path = write_rest_skeleton(&tiny_glb(), &dir, "Hero", &clips).unwrap().unwrap();
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("Hero.skel"));
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let pose: SkeletonRestPose = ron::from_str(&text).unwrap();

        let names: Vec<&str> = pose.bones.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, vec!["Armature", "Hips", "Spine"], "the mesh node is not a bone");
        assert_eq!(pose.bones[0].parent, None);
        assert_eq!(pose.bones[1].parent.as_deref(), Some("Armature"));
        assert_eq!(pose.bones[2].parent.as_deref(), Some("Hips"));
        assert_eq!(pose.bones[1].translation, [0.0, 1.0, 0.0]);
        assert_eq!(pose.bones[2].rotation, [0.0, 0.0, 0.38268343, 0.9238795]);
        assert_eq!(pose.bones[2].scale, [1.0; 3]);
        assert_eq!(pose.clips, vec!["talk.anim".to_string()]);
    }

    #[test]
    fn an_unskinned_model_writes_no_skeleton() {
        let glb = crate::glb_build::build_glb_grouped(
            &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            &[0.0; 6],
            &[crate::glb_build::MaterialGroup { material: None, indices: vec![0, 1, 2] }],
            &Default::default(),
        )
        .unwrap();
        assert!(rest_pose_from_glb(&glb).unwrap().is_none());
    }
}
