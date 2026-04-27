//! Drag-and-drop scene-instance spawning — detects `.ron` asset drops on the
//! viewport and spawns a `SceneInstance` entity expanded from that file.

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_editor::{EditorCommands, EditorSelection};
use renzora_ui::asset_drag::AssetDragPayload;
use renzora_ui::{DocumentTabState, Toasts};
use renzora::core::{CurrentProject, EditorCamera};

use crate::ViewportState;

const SCENE_EXTENSIONS: &[&str] = &["ron"];

/// Called from the viewport panel's `ui()` method (read-only `&World`).
///
/// When a `.ron` asset is released over the viewport, queues a deferred
/// command to create a `SceneInstance` entity at the drop position.
pub fn check_viewport_scene_drop(ui: &mut egui::Ui, world: &World, viewport_rect: egui::Rect) {
    let Some(payload) = world.get_resource::<AssetDragPayload>() else {
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(SCENE_EXTENSIONS) {
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
            // Reject dropping a scene into itself.
            let host_abs = world
                .get_resource::<CurrentProject>()
                .and_then(|p| {
                    world.get_resource::<DocumentTabState>()
                        .and_then(|t| t.tabs.get(t.active_tab).and_then(|tab| tab.scene_path.clone()))
                        .map(|rel| p.resolve_path(&rel))
                });
            if let (Some(host_abs), Some(project_root)) = (
                host_abs,
                world.get_resource::<CurrentProject>().map(|p| p.path.clone()),
            ) {
                let mut cache = world
                    .remove_resource::<renzora_engine::scene_io::SceneReferenceCache>()
                    .unwrap_or_default();
                let cycle = renzora_engine::scene_io::would_create_reference_cycle(
                    &mut cache, &project_root, &host_abs, &path,
                );
                world.insert_resource(cache);
                if cycle {
                    if let Some(mut toasts) = world.get_resource_mut::<Toasts>() {
                        toasts.warning("You cannot add a scene to itself");
                    }
                    return;
                }
            }
            let pos = compute_ground_position(world, screen_pos, vp_rect).unwrap_or(Vec3::ZERO);
            let transform = Transform::from_translation(pos);
            if let Some(entity) = renzora_engine::scene_io::spawn_scene_instance(
                world,
                &path,
                None,
                transform,
            ) {
                if let Some(sel) = world.get_resource::<EditorSelection>() {
                    sel.set(Some(entity));
                }
            }
        });
    }
}

/// Raycast the editor camera onto the Y=0 plane.
fn compute_ground_position(
    world: &mut World,
    screen_pos: egui::Pos2,
    viewport_rect: egui::Rect,
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
