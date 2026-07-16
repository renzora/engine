//! Drag-and-drop particle-effect spawning — detects `.particle` asset drops on
//! the viewport and spawns an entity carrying a `HanabiEffect` (resolved into a
//! live `bevy_hanabi` effect by `renzora_hanabi::sync_hanabi_effects`).

use std::path::PathBuf;

use bevy::prelude::*;

use renzora::core::viewport_types::{ViewportSettings, ViewportView};
use renzora::core::{CurrentProject, EditorCamera, EditorCamera2d, Node2d};
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

    // In 2D view the drop lands on the 2D plane through the ortho camera;
    // hanabi renders the effect through the `Transparent2d` phase of the same
    // `Camera2d`, so the entity needs no extra render wiring — just a `Node2d`
    // marker so the 2D picker and hierarchy treat it as a 2D node.
    let is_2d = world
        .get_resource::<ViewportSettings>()
        .is_some_and(|s| s.viewport_view == ViewportView::Two);

    let (pos, node_2d) = if is_2d {
        let p = compute_2d_position(world, screen_pos, vp_rect).unwrap_or(Vec2::ZERO);
        (p.extend(0.0), true)
    } else {
        (
            compute_ground_position(world, screen_pos, vp_rect).unwrap_or(Vec3::ZERO),
            false,
        )
    };

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
    if node_2d {
        world.entity_mut(entity).insert(Node2d);
    }

    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(entity));
    }
}

/// Project the drop point through the 2D editor camera to world space.
/// `screen_pos` / `viewport_rect` are in window logical pixels (same convention
/// as `sprite_drop::commit_sprite_drop`).
fn compute_2d_position(world: &mut World, screen_pos: Vec2, viewport_rect: Rect) -> Option<Vec2> {
    let mut q =
        world.query_filtered::<(&GlobalTransform, &Camera), With<EditorCamera2d>>();
    let (cam_gt, camera) = q.iter(world).next()?;
    let cam_gt = *cam_gt;
    let camera = camera.clone();

    let vp_state = world.get_resource::<ViewportState>()?;
    let image_size = vp_state.current_size.as_vec2();
    if image_size.x <= 0.0 || image_size.y <= 0.0 || viewport_rect.width() <= 0.0 {
        return None;
    }
    let render_pos = Vec2::new(
        (screen_pos.x - viewport_rect.min.x) * image_size.x / viewport_rect.width(),
        (screen_pos.y - viewport_rect.min.y) * image_size.y / viewport_rect.height(),
    );
    camera.viewport_to_world_2d(&cam_gt, render_pos).ok()
}

/// Raycast the editor camera onto the Y=0 plane. `screen_pos` / `viewport_rect`
/// are in window logical pixels. Shared with the gaussian-splat drop.
pub(crate) fn compute_ground_position(
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
