//! Clip auto-discovery — build an [`AnimatorComponent`] from the `.anim` files
//! sitting in an `animations/` folder next to a model.
//!
//! The import pipeline extracts a model's animations into
//! `<model_dir>/animations/*.anim`; this is the single place that maps that
//! on-disk convention back to clip slots. Used by the viewport when a model is
//! dropped into the scene and by the animation editor's "Scan for clips"
//! action.

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
    let mut entries: Vec<_> = std::fs::read_dir(&anim_dir_abs)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "anim"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

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

        clips.push(AnimClipSlot {
            name: stem,
            path: anim_asset_path,
            looping: true,
            speed: 1.0,
            blend_in: None,
            blend_out: None,
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
    })
}
