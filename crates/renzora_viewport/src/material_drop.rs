//! Drag-and-drop material application — when the user drags a `.material`
//! from the asset browser onto a mesh in the viewport, apply it to that mesh.
//!
//! On pointer release over the viewport, raycasts against scene meshes under
//! the pointer and inserts/updates `MaterialRef` on the nearest hit entity.

use std::path::PathBuf;

use bevy::ecs::system::SystemState;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;

use renzora::core::{CurrentProject, EditorCamera};
use renzora_shader::material::material_ref::MaterialRef;
use renzora_shader::material::resolver::MaterialResolved;

use crate::ViewportState;

pub(crate) const MATERIAL_EXTENSIONS: &[&str] = &["material"];

/// Commit a material drop — raycast for the mesh under `screen_pos` and apply the
/// `.material`. Shared by the egui drop check and the native bevy_ui drop
/// (`native_drop::commit_viewport_drop`). `screen_pos` / `vp_rect` are in window
/// logical pixels.
pub(crate) fn commit_material_drop(
    world: &mut World,
    screen_pos: Vec2,
    vp_rect: Rect,
    path: PathBuf,
) {
    let Some(entity) = pick_mesh_under_pointer(world, screen_pos, vp_rect) else {
        info!("[material_drop] No mesh under pointer — ignoring drop");
        return;
    };
    apply_material_to_entity(world, entity, path);
}

/// Raycast against scene meshes to find the closest entity under the given
/// viewport-space pointer position (window logical pixels).
fn pick_mesh_under_pointer(
    world: &mut World,
    screen_pos: Vec2,
    viewport_rect: Rect,
) -> Option<Entity> {
    let mut state: SystemState<(
        MeshRayCast,
        Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
        Res<ViewportState>,
    )> = SystemState::new(world);
    let (mut ray_cast, camera_q, vp_state) = state.get_mut(world);

    let (camera, camera_transform) = camera_q.iter().next()?;

    let vp_x = screen_pos.x - viewport_rect.min.x;
    let vp_y = screen_pos.y - viewport_rect.min.y;
    let render_x = vp_x / viewport_rect.width() * vp_state.current_size.x as f32;
    let render_y = vp_y / viewport_rect.height() * vp_state.current_size.y as f32;

    let ray = camera
        .viewport_to_world(camera_transform, Vec2::new(render_x, render_y))
        .ok()?;

    let hits = ray_cast.cast_ray(
        ray,
        &MeshRayCastSettings {
            early_exit_test: &|_| false,
            ..MeshRayCastSettings::default()
        },
    );

    hits.iter().next().map(|(entity, _)| *entity)
}

/// Insert or update `MaterialRef` on `entity` with the given `.material` path.
/// Also removes `MaterialResolved` so the resolver re-processes the entity
/// and picks up the new material on the next frame.
fn apply_material_to_entity(world: &mut World, entity: Entity, abs_path: PathBuf) {
    if world.get_entity(entity).is_err() {
        warn!(
            "[material_drop] Target entity {:?} no longer exists",
            entity
        );
        return;
    }

    let asset_path = if let Some(project) = world.get_resource::<CurrentProject>() {
        project.make_asset_relative(&abs_path)
    } else {
        abs_path.to_string_lossy().to_string()
    };

    world.entity_mut(entity).remove::<MaterialResolved>();
    if let Some(mut mr) = world.get_mut::<MaterialRef>(entity) {
        mr.0 = asset_path.clone();
    } else {
        world
            .entity_mut(entity)
            .insert(MaterialRef(asset_path.clone()));
    }

    info!(
        "[material_drop] Applied '{}' to entity {:?}",
        asset_path, entity
    );
}
