//! Export with Clips: the model on the stage written out as one `.glb` with
//! its skeleton, skin and every clip on its animator, for use outside the
//! engine (`renzora_rig::export`).
//!
//! Clips are baked at 30 samples a second from the same curves the engine
//! plays (`BoneTrack::channel_spline`), so eased, stepped and hand-tangent
//! keys come out looking as they do here in any viewer, which would otherwise
//! read only the key values and join them with straight lines. Muted tracks
//! are left out, as they are when the clip plays.

use bevy::prelude::*;

use renzora::core::AnimClip;
use renzora_rig::export::{self, ExportClip, ExportTrack};
use renzora_rig::Glb;

use super::{model_file, subject, tell};

/// Samples per second a clip is baked at.
const BAKE_RATE: f32 = 30.0;

pub(crate) fn export_model(world: &mut World) {
    let Some(root) = subject(world) else { return };
    let Some((abs, rel)) = model_file(world, root) else {
        tell(world, false, "This entity has no model file to export");
        return;
    };
    let Some(project) = world.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone()) else { return };
    let slots: Vec<(String, String)> = world
        .get::<renzora_animation::AnimatorComponent>(root)
        .map(|a| a.clips.iter().map(|c| (c.name.clone(), c.path.clone())).collect())
        .unwrap_or_default();
    let mut clips = Vec::new();
    for (name, path) in &slots {
        let file = project.join(path);
        match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|s| ron::from_str::<AnimClip>(&s).map_err(|e| e.to_string())) {
            Ok(clip) => clips.push(bake(name, &clip)),
            Err(e) => tell(world, false, format!("Left out clip {name} ({path}): {e}")),
        }
    }
    let result = std::fs::read(&abs).map_err(|e| e.to_string()).and_then(|bytes| {
        let mut glb = Glb::parse(&bytes).map_err(|e| e.to_string())?;
        let tracks = export::add_clips(&mut glb, &clips).map_err(|e| e.to_string())?;
        let stem = abs.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "model".into());
        let dir = project.join("exports");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let out = dir.join(format!("{stem}.glb"));
        std::fs::write(&out, glb.to_bytes()).map_err(|e| e.to_string())?;
        Ok((out, tracks))
    });
    match result {
        Ok((out, tracks)) => tell(world, true, format!("Exported {rel} with {} clips ({tracks} tracks) to {}", clips.len(), out.display())),
        Err(e) => tell(world, false, format!("Could not export {rel}: {e}")),
    }
}

/// One clip sampled evenly from start to end, every channel that has keys.
fn bake(name: &str, clip: &AnimClip) -> ExportClip {
    let duration = clip.duration.max(0.0);
    let steps = ((duration * BAKE_RATE).ceil() as usize).max(1);
    let times: Vec<f32> = (0..=steps).map(|i| (i as f32 / steps as f32) * duration).collect();
    let tracks = clip
        .tracks
        .iter()
        .filter(|t| !t.muted)
        .map(|t| {
            let (pos, rot, scale) = (t.channel_spline(0), t.channel_spline(1), t.channel_spline(2));
            ExportTrack {
                bone: t.bone_name.clone(),
                translations: if t.translations.is_empty() { Vec::new() } else { times.iter().map(|&s| (s, pos.sample_vec3(s))).collect() },
                rotations: if t.rotations.is_empty() { Vec::new() } else { times.iter().map(|&s| (s, rot.sample_quat(s))).collect() },
                scales: if t.scales.is_empty() { Vec::new() } else { times.iter().map(|&s| (s, scale.sample_vec3(s))).collect() },
            }
        })
        .collect();
    ExportClip { name: name.to_string(), tracks }
}
