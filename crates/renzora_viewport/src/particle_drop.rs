//! Drag-and-drop particle-effect spawning — detects `.particle` asset drops on
//! the viewport and spawns an entity carrying a `HanabiEffect` (resolved into a
//! live `bevy_hanabi` effect by `renzora_hanabi::sync_hanabi_effects`).

use std::path::PathBuf;

use bevy::prelude::*;

use renzora::core::{CurrentProject, EditorCamera};
use renzora_editor_framework::EditorSelection;
use renzora_hanabi::{EffectSource, HanabiEffect};

use crate::ViewportState;

pub(crate) const PARTICLE_EXTENSIONS: &[&str] = &["particle"];

/// Commit a `.particle` drop at the given viewport-space pointer. Shared by the
/// native bevy_ui drop (`native_drop::commit_viewport_drop`). `screen_pos` /
/// `vp_rect` are in window logical pixels.
pub(crate) fn commit_particle_drop(
    world: &mut World,
    screen_pos: Vec2,
    vp_rect: Rect,
    path: PathBuf,
) {
    // Store a project-relative path so the entity saves/loads portably and the
    // runtime VFS can resolve it (matches how the inspector references effects).
    let rel = world
        .get_resource::<CurrentProject>()
        .and_then(|p| p.make_relative(&path))
        .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"));

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Particle Effect".to_string());

    let pos = compute_ground_position(world, screen_pos, vp_rect).unwrap_or(Vec3::ZERO);

    let entity = world
        .spawn((
            Name::new(name),
            Transform::from_translation(pos),
            Visibility::default(),
            HanabiEffect {
                source: EffectSource::Asset { path: rel },
                ..default()
            },
        ))
        .id();

    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(entity));
    }
}

/// Raycast the editor camera onto the Y=0 plane. `screen_pos` / `viewport_rect`
/// are in window logical pixels.
fn compute_ground_position(
    world: &mut World,
    screen_pos: Vec2,
    viewport_rect: Rect,
) -> Option<Vec3> {
    let mut q = world.query_filtered::<(&GlobalTransform, &Camera), With<EditorCamera>>();
    let (camera_transform, camera) = q.iter(world).next()?;
    let camera_transform = *camera_transform;
    let camera = camera.clone();

    let vp_state = world.get_resource::<ViewportState>()?;
    let vp_x = screen_pos.x - viewport_rect.min.x;
    let vp_y = screen_pos.y - viewport_rect.min.y;
    let render_x = vp_x / viewport_rect.width() * vp_state.current_size.x as f32;
    let render_y = vp_y / viewport_rect.height() * vp_state.current_size.y as f32;

    let ray = camera
        .viewport_to_world(&camera_transform, Vec2::new(render_x, render_y))
        .ok()?;

    if ray.direction.y.abs() < 1e-6 {
        return Some(Vec3::new(ray.origin.x, 0.0, ray.origin.z));
    }
    let t = -ray.origin.y / ray.direction.y;
    if t < 0.0 {
        return Some(Vec3::ZERO);
    }
    let hit = ray.origin + ray.direction * t;
    Some(Vec3::new(hit.x, 0.0, hit.z))
}
