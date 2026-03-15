//! Extract animations from GLB data and write `.anim` RON files.
//!
//! After a model is imported and converted to GLB, this module parses the GLB
//! binary to find animation clips and writes each one as a `.anim` file.

use std::collections::HashMap;
use std::path::Path;

use renzora_animation::clip::{AnimClip, BoneTrack};
use renzora_animation::extract::write_anim_file;

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
    let glb = gltf::Gltf::from_slice(glb_bytes)
        .map_err(|e| format!("Failed to parse GLB: {}", e))?;

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
        let mut duration: f32 = 0.0;

        for channel in anim.channels() {
            let target = channel.target();
            let node_idx = target.node().index();
            let bone_name = node_names
                .get(&node_idx)
                .cloned()
                .unwrap_or_else(|| format!("node_{}", node_idx));

            let reader = channel.reader(|buffer| {
                buffers.get(&buffer.index()).map(|v| v.as_slice())
            });

            let timestamps: Vec<f32> = match reader.read_inputs() {
                Some(iter) => iter.collect(),
                None => {
                    result
                        .warnings
                        .push(format!("{}: missing timestamps for bone '{}'", clip_name, bone_name));
                    continue;
                }
            };

            if let Some(&last) = timestamps.last() {
                duration = duration.max(last);
            }

            let track = bone_tracks.entry(node_idx).or_insert_with(|| BoneTrack {
                bone_name: bone_name.clone(),
                translations: Vec::new(),
                rotations: Vec::new(),
                scales: Vec::new(),
            });

            match reader.read_outputs() {
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
                Some(gltf::animation::util::ReadOutputs::MorphTargetWeights(_)) => {
                    result
                        .warnings
                        .push(format!("{}: morph target weights not supported, skipping", clip_name));
                }
                None => {
                    result
                        .warnings
                        .push(format!("{}: missing output data for bone '{}'", clip_name, bone_name));
                }
            }
        }

        // Skip clips with no usable tracks
        let tracks: Vec<BoneTrack> = bone_tracks.into_values().collect();
        if tracks.is_empty() {
            result
                .warnings
                .push(format!("{}: no animation tracks found, skipping", clip_name));
            continue;
        }

        let clip = AnimClip {
            name: clip_name.clone(),
            duration,
            tracks,
        };

        // Sanitize filename
        let safe_name: String = clip_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect();
        let file_path = output_dir.join(format!("{}.anim", safe_name));

        match write_anim_file(&clip, &file_path) {
            Ok(()) => {
                result
                    .written_files
                    .push(file_path.display().to_string());
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

/// Load all buffer data referenced by the GLTF document.
fn load_buffers(
    glb: &gltf::Gltf,
    blob: Option<&[u8]>,
) -> Result<HashMap<usize, Vec<u8>>, String> {
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
