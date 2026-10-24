//! Clip auto-discovery — build an [`AnimatorComponent`] from the `.anim` files
//! sitting in an `animations/` folder next to a model.
//!
//! The import pipeline extracts a model's animations into
//! `<model_dir>/animations/*.anim`; this is the single place that maps that
//! on-disk convention back to clip slots. Used by the viewport when a model is
//! dropped into the scene and by the animation editor's "Scan for clips"
//! action.

use std::collections::HashMap;
use std::path::Path;

use crate::component::{AnimClipSlot, AnimatorComponent};

/// Look for `.anim` files in an `animations/` directory next to the model and
/// build an [`AnimatorComponent`] from them. `asset_path` is the
/// project-relative model path (e.g. `models/Man.glb`); `project_root` is the
/// absolute project directory. Returns `None` if no `.anim` files are found.
pub fn discover_animation_clips(
    asset_path: &str,
    project_root: &Path,
) -> Option<AnimatorComponent> {
    // Model is e.g. "models/Man.glb" → look in "models/animations/"
    let model_dir = Path::new(asset_path).parent().unwrap_or(Path::new(""));
    let anim_dir_abs = project_root.join(model_dir).join("animations");

    if !anim_dir_abs.is_dir() {
        return None;
    }

    let mut clips = Vec::new();
    let all: Vec<_> = std::fs::read_dir(&anim_dir_abs).ok()?.filter_map(|e| e.ok()).collect();
    let mut entries: Vec<_> = all.iter().filter(|e| e.path().extension().is_some_and(|ext| ext == "anim")).collect();
    entries.sort_by_key(|e| e.file_name());
    let model_stem = Path::new(asset_path).file_stem().and_then(|s| s.to_str());
    let skeletons = clip_skeletons(&all, model_dir, model_stem);

    for entry in entries {
        let file_path = entry.path();
        let stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("clip")
            .to_string();

        // Asset-relative path: e.g. "models/animations/HumanArmature_Man_Idle.anim"
        let anim_asset_path = model_dir
            .join("animations")
            .join(entry.file_name())
            .to_string_lossy()
            .replace('\\', "/");

        let source_skeleton = entry.file_name().to_str().and_then(|n| skeletons.get(n)).cloned();
        clips.push(AnimClipSlot {
            name: stem,
            path: anim_asset_path,
            looping: true,
            speed: 1.0,
            blend_in: None,
            blend_out: None,
            source_skeleton,
        });
    }

    if clips.is_empty() {
        return None;
    }

    let default_clip = clips
        .iter()
        .find(|c| c.name.to_lowercase().contains("idle"))
        .or(clips.first())
        .map(|c| c.name.clone());

    Some(AnimatorComponent {
        clips,
        default_clip,
        blend_duration: 0.2,
        state_machine: None,
        layers: Vec::new(),
        ..Default::default()
    })
}

/// Which `.skel` in the folder claims each clip: `clip file name ->
/// asset-relative .skel path`, from each skeleton's [`renzora::SkeletonRestPose::clips`].
///
/// The importer writes one `.skel` per model it extracted clips from, naming
/// those clips, so a clip from an animation-only download carries its own
/// rig's rest pose to whichever character it is played on. A clip nobody
/// claims (hand-made in the timeline, or imported before skeletons were
/// written) gets none and binds by name as it always did. A skeleton that
/// does not parse is skipped rather than failing discovery.
///
/// The model's own skeleton (`<model>.skel`) is left out: its clips were keyed
/// on this very rig, where retargeting is the identity, and a retargeted slot
/// plays a derived copy of its clip, which the timeline's live edits (written
/// into the loaded clip) would then never reach.
fn clip_skeletons(entries: &[std::fs::DirEntry], model_dir: &Path, model_stem: Option<&str>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut skels: Vec<_> = entries
        .iter()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "skel"))
        .filter(|e| model_stem.is_none_or(|m| e.path().file_stem().and_then(|s| s.to_str()) != Some(m)))
        .collect();
    skels.sort_by_key(|e| e.file_name());
    for skel in skels {
        let Ok(text) = std::fs::read_to_string(skel.path()) else { continue };
        let Ok(pose) = ron::from_str::<renzora::SkeletonRestPose>(&text) else { continue };
        let asset = model_dir.join("animations").join(skel.file_name()).to_string_lossy().replace('\\', "/");
        for clip in pose.clips {
            out.entry(clip).or_insert_with(|| asset.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clip claimed by another model's `.skel` gets it as its source
    /// skeleton; the model's own clips, and clips nobody claims, bind by
    /// name, as before.
    #[test]
    fn slots_point_at_the_skeleton_that_claims_them() {
        let root = std::env::temp_dir().join(format!("renzora_discovery_{}", std::process::id()));
        let anims = root.join("models").join("animations");
        std::fs::create_dir_all(&anims).unwrap();
        let clip = crate::extract::create_empty_clip("walk", 1.0);
        for name in ["Walk.anim", "Mine.anim", "Handmade.anim"] {
            renzora::write_anim_file(&clip, &anims.join(name)).unwrap();
        }
        let theirs = renzora::SkeletonRestPose { bones: vec![], clips: vec!["Walk.anim".into()] };
        renzora::write_skeleton_file(&theirs, &anims.join("Walking.skel")).unwrap();
        let own = renzora::SkeletonRestPose { bones: vec![], clips: vec!["Mine.anim".into()] };
        renzora::write_skeleton_file(&own, &anims.join("Hero.skel")).unwrap();

        let animator = discover_animation_clips("models/Hero.glb", &root).unwrap();
        let _ = std::fs::remove_dir_all(&root);
        let slot = |n: &str| animator.clips.iter().find(|c| c.name == n).unwrap().source_skeleton.clone();
        assert_eq!(slot("Walk").as_deref(), Some("models/animations/Walking.skel"));
        assert_eq!(slot("Mine"), None);
        assert_eq!(slot("Handmade"), None);
    }
}
