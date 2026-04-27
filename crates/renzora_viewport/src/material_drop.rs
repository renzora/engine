//! Drag-and-drop material application — when the user drags a `.material`
//! from the asset browser onto a mesh in the viewport, apply it to that mesh.
//!
//! On pointer release over the viewport, raycasts against scene meshes under
//! the pointer and inserts/updates `MaterialRef` on the nearest hit entity.

use std::path::PathBuf;

use bevy::ecs::system::SystemState;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;
use bevy_egui::egui;

use renzora::core::{CurrentProject, EditorCamera};
use renzora_editor::EditorCommands;
use renzora_shader::material::material_ref::MaterialRef;
use renzora_shader::material::resolver::MaterialResolved;
use renzora_ui::asset_drag::AssetDragPayload;

use crate::ViewportState;

const MATERIAL_EXTENSIONS: &[&str] = &["material"];

/// Called from the viewport panel's `ui()` method. On release of a
/// `.material` drag payload over the viewport, queue a deferred command
/// that raycasts for the mesh under the pointer and applies the material.
pub fn check_viewport_material_drop(
    ui: &mut egui::Ui,
    world: &World,
    viewport_rect: egui::Rect,
) {
    let Some(payload) = world.get_resource::<AssetDragPayload>() else {
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(MATERIAL_EXTENSIONS) {
        return;
    }

    let pointer_pos = ui.ctx().pointer_latest_pos();
    let pointer_in_viewport = pointer_pos.map_or(false, |p| viewport_rect.contains(p));
    if !pointer_in_viewport {
        return;
    }

    let pointer_released = !ui.ctx().input(|i| i.pointer.any_down());
    if !pointer_released {
        return;
    }

    let path = payload.path.clone();
    let screen_pos = pointer_pos.unwrap_or(viewport_rect.center());
    let vp_rect = viewport_rect;

    if let Some(commands) = world.get_resource::<EditorCommands>() {
        commands.push(move |world: &mut World| {
            let Some(entity) = pick_mesh_under_pointer(world, screen_pos, vp_rect) else {
                info!("[material_drop] No mesh under pointer — ignoring drop");
                return;
            };
            apply_material_to_entity(world, entity, path);
        });
    }
}

/// Raycast against scene meshes to find the closest entity under the given
/// screen-space pointer position.
fn pick_mesh_under_pointer(
    world: &mut World,
    screen_pos: egui::Pos2,
    viewport_rect: egui::Rect,
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
        warn!("[material_drop] Target entity {:?} no longer exists", entity);
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
        world.entity_mut(entity).insert(MaterialRef(asset_path.clone()));
    }

    info!(
        "[material_drop] Applied '{}' to entity {:?}",
        asset_path, entity
    );
}
