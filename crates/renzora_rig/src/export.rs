//! Putting a model's clips into its `.glb`, for taking a rigged and animated
//! character out of the engine (into a DCC tool, another engine, a viewer).
//!
//! Inside the engine a model's clips live beside it as `.anim` files, keyed
//! by bone name; a `.glb` keeps its animations inside, keyed by node. So the
//! clips are written in as glTF animations, each track aimed at the node of
//! the bone it names. Names are matched loosely (case and punctuation
//! ignored), as the engine binds them: an imported `mixamorig:Hips` is
//! `mixamorig_hips` once it has been through the scene, and both mean the
//! same bone.
//!
//! The keys arrive already sampled (the editor bakes eased and stepped keys
//! to dense samples first), so every channel is written `LINEAR` and plays
//! the same in any viewer whatever interpolation the clip used.

use std::collections::HashMap;

use bevy::math::{Quat, Vec3};
use serde_json::{json, Value};

use crate::glb::{Glb, GlbError};

/// One bone's keys in a clip.
#[derive(Clone, Debug, Default)]
pub struct ExportTrack {
    pub bone: String,
    pub translations: Vec<(f32, Vec3)>,
    pub rotations: Vec<(f32, Quat)>,
    pub scales: Vec<(f32, Vec3)>,
}

/// A clip to write.
#[derive(Clone, Debug, Default)]
pub struct ExportClip {
    pub name: String,
    pub tracks: Vec<ExportTrack>,
}

/// A bone name as the engine compares them.
fn loose(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect()
}

/// Add `clips` to `glb` as animations. Tracks whose bone has no node are
/// left out. Returns how many tracks were written.
pub fn add_clips(glb: &mut Glb, clips: &[ExportClip]) -> Result<usize, GlbError> {
    let nodes: HashMap<String, usize> = glb.nodes().iter().enumerate().filter_map(|(i, n)| Some((loose(n.get("name")?.as_str()?), i))).collect();
    let mut written = 0;
    let mut animations: Vec<Value> = glb.json.get("animations").and_then(Value::as_array).cloned().unwrap_or_default();
    for clip in clips {
        let mut samplers = Vec::new();
        let mut channels = Vec::new();
        for track in &clip.tracks {
            let Some(&node) = nodes.get(&loose(&track.bone)) else { continue };
            let mut channel = |glb: &mut Glb, path: &str, times: Vec<f32>, values: Vec<f32>, kind: &str| {
                if times.is_empty() {
                    return;
                }
                let (lo, hi) = times.iter().fold((f32::MAX, f32::MIN), |(a, b), t| (a.min(*t), b.max(*t)));
                let input = glb.add_floats(&times, "SCALAR", Some((vec![lo], vec![hi])));
                let output = glb.add_floats(&values, kind, None);
                samplers.push(json!({ "input": input, "output": output, "interpolation": "LINEAR" }));
                channels.push(json!({ "sampler": samplers.len() - 1, "target": { "node": node, "path": path } }));
            };
            channel(
                glb,
                "translation",
                track.translations.iter().map(|k| k.0).collect(),
                track.translations.iter().flat_map(|k| k.1.to_array()).collect(),
                "VEC3",
            );
            channel(
                glb,
                "rotation",
                track.rotations.iter().map(|k| k.0).collect(),
                track.rotations.iter().flat_map(|k| k.1.normalize().to_array()).collect(),
                "VEC4",
            );
            channel(glb, "scale", track.scales.iter().map(|k| k.0).collect(), track.scales.iter().flat_map(|k| k.1.to_array()).collect(), "VEC3");
            written += 1;
        }
        if !channels.is_empty() {
            animations.push(json!({ "name": clip.name, "samplers": samplers, "channels": channels }));
        }
    }
    glb.json["animations"] = json!(animations);
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{self, ChainSpec};
    use crate::glb::Geometry;

    #[test]
    fn clips_go_in_aimed_at_their_bones() {
        let positions = [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(1.0, 1.0, 0.0)];
        let g = Geometry { name: "quad", positions: &positions, normals: &[], uvs: &[], indices: &[0, 1, 2, 1, 3, 2], base_color: [1.0; 4] };
        let mut glb = Glb::from_geometry(&g).expect("written");
        chain::build(&mut glb, &ChainSpec { start: Vec3::ZERO, end: Vec3::Y, segments: 2 }).expect("rigged");
        let clip = ExportClip {
            name: "Bend".into(),
            tracks: vec![
                ExportTrack { bone: "Chain_2".into(), rotations: vec![(0.0, Quat::IDENTITY), (1.0, Quat::from_rotation_z(0.5))], ..default_track() },
                ExportTrack { bone: "no_such_bone".into(), translations: vec![(0.0, Vec3::ZERO)], ..default_track() },
            ],
        };
        assert_eq!(add_clips(&mut glb, &[clip]).expect("added"), 1);
        let reread = Glb::parse(&glb.to_bytes()).expect("still a glb");
        let anim = &reread.json["animations"][0];
        assert_eq!(anim["name"], "Bend");
        let target = anim["channels"][0]["target"]["node"].as_u64().expect("node") as usize;
        assert_eq!(reread.node_name(target), "chain_2");
        assert_eq!(anim["channels"][0]["target"]["path"], "rotation");
    }

    fn default_track() -> ExportTrack {
        ExportTrack::default()
    }
}
