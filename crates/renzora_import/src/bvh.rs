#![allow(unused_variables, unused_assignments, dead_code)]

//! BVH (Biovision Hierarchy) → animation clip converter.
//!
//! BVH is an animation-only format containing a skeleton hierarchy
//! and per-frame joint transforms. It has no mesh data.
//!
//! The `convert` function returns an error (no geometry) so the import
//! overlay's fallback path calls `extract_animations_from_bvh` instead.

use std::path::Path;

use crate::anim_extract::AnimExtractResult;
use crate::convert::{ImportError, ImportResult};
use crate::settings::ImportSettings;

use renzora::{AnimClip, BoneTrack};
use renzora::write_anim_file;

/// BVH has no mesh geometry — always fails so the animation fallback kicks in.
pub fn convert(_path: &Path, _settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    Err(ImportError::ParseError(
        "BVH files contain only animation data (no mesh geometry). \
         Animations will be extracted automatically."
            .into(),
    ))
}

/// Extract animations from a BVH file and write `.anim` files.
pub fn extract_animations_from_bvh(
    path: &Path,
    output_dir: &Path,
) -> Result<AnimExtractResult, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read BVH file: {}", e))?;

    let bvh = parse_bvh(&content)?;

    if bvh.joints.is_empty() || bvh.frames.is_empty() {
        return Ok(AnimExtractResult {
            written_files: Vec::new(),
            warnings: vec!["BVH file contains no animation data".into()],
        });
    }

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;

    let clip = bvh_to_clip(&bvh);
    let file_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bvh_clip");
    let out_path = output_dir.join(format!("{}.anim", file_name));

    write_anim_file(&clip, &out_path)
        .map_err(|e| format!("Failed to write animation: {}", e))?;

    Ok(AnimExtractResult {
        written_files: vec![out_path.display().to_string()],
        warnings: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// BVH parser
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct BvhFile {
    joints: Vec<BvhJoint>,
    frame_time: f32,
    frames: Vec<Vec<f32>>,
}

#[derive(Debug)]
struct BvhJoint {
    name: String,
    channels: Vec<BvhChannel>,
    channel_offset: usize,
}

#[derive(Debug, Clone, Copy)]
enum BvhChannel {
    Xposition,
    Yposition,
    Zposition,
    Xrotation,
    Yrotation,
    Zrotation,
}

fn parse_bvh(content: &str) -> Result<BvhFile, String> {
    let mut lines = content.lines().peekable();
    let mut joints: Vec<BvhJoint> = Vec::new();
    let mut channel_offset = 0usize;

    // Skip until HIERARCHY
    while let Some(line) = lines.next() {
        if line.trim().eq_ignore_ascii_case("HIERARCHY") {
            break;
        }
    }

    // Parse joint hierarchy
    let mut depth = 0;
    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case("MOTION") {
            break;
        }

        if trimmed.starts_with("ROOT") || trimmed.starts_with("JOINT") {
            let name = trimmed
                .split_whitespace()
                .nth(1)
                .unwrap_or("Joint")
                .to_string();

            // Look for CHANNELS line
            let mut channels = Vec::new();
            let mut found_channels = false;

            while let Some(inner) = lines.peek() {
                let inner_trimmed = inner.trim();
                if inner_trimmed == "{" {
                    lines.next();
                    depth += 1;
                    continue;
                }
                if inner_trimmed.starts_with("OFFSET") {
                    lines.next();
                    continue;
                }
                if inner_trimmed.starts_with("CHANNELS") {
                    lines.next();
                    channels = parse_channels(inner_trimmed);
                    found_channels = true;
                    break;
                }
                break;
            }

            if found_channels {
                joints.push(BvhJoint {
                    name,
                    channels: channels.clone(),
                    channel_offset,
                });
                channel_offset += channels.len();
            }
        } else if trimmed.starts_with("End Site") {
            // Skip end site block
            while let Some(inner) = lines.next() {
                let inner_trimmed = inner.trim();
                if inner_trimmed == "{" {
                    depth += 1;
                } else if inner_trimmed == "}" {
                    depth -= 1;
                    break;
                }
            }
        } else if trimmed == "}" {
            depth -= 1;
        }
    }

    // Parse motion data
    let mut frame_count = 0usize;
    let mut frame_time = 1.0 / 30.0f32;
    let mut frames: Vec<Vec<f32>> = Vec::new();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.starts_with("Frames:") {
            frame_count = trimmed
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        } else if trimmed.starts_with("Frame Time:") {
            frame_time = trimmed
                .split(':')
                .nth(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1.0 / 30.0);
        } else if !trimmed.is_empty() {
            // Frame data line
            let values: Vec<f32> = trimmed
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if !values.is_empty() {
                frames.push(values);
            }
        }
    }

    Ok(BvhFile {
        joints,
        frame_time,
        frames,
    })
}

fn parse_channels(line: &str) -> Vec<BvhChannel> {
    let mut channels = Vec::new();
    // Format: "CHANNELS 6 Xposition Yposition Zposition Xrotation Yrotation Zrotation"
    for word in line.split_whitespace().skip(2) {
        match word.to_lowercase().as_str() {
            "xposition" => channels.push(BvhChannel::Xposition),
            "yposition" => channels.push(BvhChannel::Yposition),
            "zposition" => channels.push(BvhChannel::Zposition),
            "xrotation" => channels.push(BvhChannel::Xrotation),
            "yrotation" => channels.push(BvhChannel::Yrotation),
            "zrotation" => channels.push(BvhChannel::Zrotation),
            _ => {}
        }
    }
    channels
}

// ---------------------------------------------------------------------------
// BVH → AnimClip conversion
// ---------------------------------------------------------------------------

fn bvh_to_clip(bvh: &BvhFile) -> AnimClip {
    let duration = bvh.frames.len() as f32 * bvh.frame_time;
    let mut tracks = Vec::new();

    for joint in &bvh.joints {
        let mut translations: Vec<(f32, [f32; 3])> = Vec::new();
        let mut rotations: Vec<(f32, [f32; 4])> = Vec::new();

        for (frame_idx, frame) in bvh.frames.iter().enumerate() {
            let time = frame_idx as f32 * bvh.frame_time;

            let mut tx = 0.0f32;
            let mut ty = 0.0f32;
            let mut tz = 0.0f32;
            let mut rx = 0.0f32;
            let mut ry = 0.0f32;
            let mut rz = 0.0f32;
            let mut has_translation = false;
            let mut has_rotation = false;

            for (ch_idx, channel) in joint.channels.iter().enumerate() {
                let data_idx = joint.channel_offset + ch_idx;
                let value = frame.get(data_idx).copied().unwrap_or(0.0);

                match channel {
                    BvhChannel::Xposition => { tx = value; has_translation = true; }
                    BvhChannel::Yposition => { ty = value; has_translation = true; }
                    BvhChannel::Zposition => { tz = value; has_translation = true; }
                    BvhChannel::Xrotation => { rx = value; has_rotation = true; }
                    BvhChannel::Yrotation => { ry = value; has_rotation = true; }
                    BvhChannel::Zrotation => { rz = value; has_rotation = true; }
                }
            }

            if has_translation {
                translations.push((time, [tx, ty, tz]));
            }

            if has_rotation {
                let quat = euler_to_quat(
                    rx.to_radians(),
                    ry.to_radians(),
                    rz.to_radians(),
                );
                rotations.push((time, quat));
            }
        }

        tracks.push(BoneTrack {
            bone_name: joint.name.clone(),
            translations,
            rotations,
            scales: Vec::new(),
        });
    }

    AnimClip {
        name: "bvh_clip".to_string(),
        duration,
        tracks,
    }
}

/// Convert ZYX Euler angles (radians) to quaternion [x, y, z, w].
fn euler_to_quat(rx: f32, ry: f32, rz: f32) -> [f32; 4] {
    let (sx, cx) = (rx * 0.5).sin_cos();
    let (sy, cy) = (ry * 0.5).sin_cos();
    let (sz, cz) = (rz * 0.5).sin_cos();

    // ZYX order (BVH convention)
    let w = cx * cy * cz + sx * sy * sz;
    let x = sx * cy * cz - cx * sy * sz;
    let y = cx * sy * cz + sx * cy * sz;
    let z = cx * cy * sz - sx * sy * cz;

    [x, y, z, w]
}
