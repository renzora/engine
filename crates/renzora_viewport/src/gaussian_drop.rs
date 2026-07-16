//! Drag-and-drop gaussian-splat spawning — detects `.ply` / `.gcloud` asset
//! drops on the 3D viewport and spawns an entity carrying a
//! `renzora::GaussianSplat` (resolved into a live cloud by the
//! `renzora_gaussian_splatting` distribution plugin's sync system).

use std::path::PathBuf;

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora::GaussianSplat;
use renzora_editor_framework::EditorSelection;

pub(crate) const GAUSSIAN_EXTENSIONS: &[&str] = &["ply", "gcloud", "sog", "ssog"];

/// Commit a splat-cloud drop at the given viewport-space pointer. 3D-only
/// (`native_drop::classify` never routes it in 2D view). `screen_pos` /
/// `vp_rect` are in window logical pixels.
pub(crate) fn commit_gaussian_drop(
    world: &mut World,
    screen_pos: Vec2,
    vp_rect: Rect,
    path: PathBuf,
) {
    // Store a project-relative path so the entity saves/loads portably and the
    // runtime VFS can resolve it (matches models / particles / audio).
    let rel = world
        .get_resource::<CurrentProject>()
        .and_then(|p| p.make_relative(&path))
        .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"));

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Gaussian Splat".to_string());

    let pos = crate::particle_drop::compute_ground_position(world, screen_pos, vp_rect)
        .unwrap_or(Vec3::ZERO);

    let entity = world
        .spawn((
            Name::new(name),
            Transform::from_translation(pos),
            Visibility::default(),
            GaussianSplat {
                source: rel,
                ..default()
            },
        ))
        .id();

    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(entity));
    }
}
