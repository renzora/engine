//! Shape drag-and-drop — handles dragging shapes from the shape library panel
//! onto the viewport with surface raycast placement.
//!
//! Follows the legacy pattern: a persistent `ShapeDragState` resource with fields
//! mutated directly by UI code and polled by regular Bevy systems.

use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::egui;

use renzora::core::{EditorCamera, ShapeRegistry};
use renzora_undo::{self, SpawnShapeCmd, UndoContext};
use renzora_ui::shape_drag::{
    PendingShapeDrop, ShapeDragPreview, ShapeDragPreviewState, ShapeDragState,
};

use crate::ViewportState;

// ── Viewport UI: drop detection + ground position tracking ─────────────────

/// Called from the viewport panel's `ui()` method every frame.
///
/// Updates `drag_ground_position` while dragging, and on pointer release
/// sets `pending_drop` so the spawn system picks it up next frame.
pub fn check_viewport_shape_drop(ui: &mut egui::Ui, world: &World, viewport_rect: egui::Rect) {
    let Some(drag_state) = world.get_resource::<ShapeDragState>() else {
        return;
    };
    if drag_state.dragging_shape.is_none() {
        return;
    }

    let pointer_pos = ui.ctx().pointer_hover_pos();
    let in_viewport = pointer_pos.map_or(false, |p| viewport_rect.contains(p));

    // On release: set pending_drop or cancel
    if ui.ctx().input(|i| i.pointer.any_released()) {
        if in_viewport {
            let shape_id = drag_state.dragging_shape.unwrap();
            let drop_pos = drag_state
                .drag_surface_position
                .or(drag_state.drag_ground_position)
                .unwrap_or(Vec3::ZERO);
            let normal = drag_state.drag_surface_normal;

            if let Some(cmds) = world.get_resource::<renzora_editor::EditorCommands>() {
                cmds.push(move |world: &mut World| {
                    let state = world.resource_mut::<ShapeDragState>();
                    state.into_inner().pending_drop = Some(PendingShapeDrop {
                        shape_id,
                        position: drop_pos,
                        normal,
                    });
                });
            }
        }
        // Clear dragging in both cases (drop or cancel)
        if let Some(cmds) = world.get_resource::<renzora_editor::EditorCommands>() {
            cmds.push(|world: &mut World| {
                let mut state = world.resource_mut::<ShapeDragState>();
                state.dragging_shape = None;
                state.drag_ground_position = None;
                state.drag_surface_position = None;
                state.drag_surface_normal = Vec3::ZERO;
            });
        }
    }
}

// ── Ground position tracking system ────────────────────────────────────────

/// System that updates `drag_ground_position` every frame while a shape is
/// being dragged over the viewport.
pub fn shape_drag_ground_tracking(
    mut drag_state: ResMut<ShapeDragState>,
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
) {
    if drag_state.dragging_shape.is_none() {
        drag_state.drag_ground_position = None;
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(cursor_pos): Option<Vec2> = window.cursor_position() else {
        drag_state.drag_ground_position = None;
        return;
    };

    // Check if cursor is over the viewport
    let vp_min = viewport.screen_position;
    let vp_max = vp_min + viewport.screen_size;
    if cursor_pos.x < vp_min.x
        || cursor_pos.y < vp_min.y
        || cursor_pos.x > vp_max.x
        || cursor_pos.y > vp_max.y
    {
        drag_state.drag_ground_position = None;
        return;
    }

    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        drag_state.drag_ground_position = None;
        return;
    };

    // Convert screen position to viewport-local render coordinates
    let viewport_pos = Vec2::new(
        (cursor_pos.x - vp_min.x) / viewport.screen_size.x * viewport.current_size.x as f32,
        (cursor_pos.y - vp_min.y) / viewport.screen_size.y * viewport.current_size.y as f32,
    );

    let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_pos) else {
        drag_state.drag_ground_position = None;
        return;
    };

    // Ground plane intersection (Y=0)
    if ray.direction.y.abs() > 1e-6 {
        let t = -ray.origin.y / ray.direction.y;
        if t > 0.0 && t < 1000.0 {
            let hit = ray.origin + ray.direction * t;
            drag_state.drag_ground_position = Some(Vec3::new(hit.x, 0.0, hit.z));
        } else {
            drag_state.drag_ground_position = None;
        }
    } else {
        drag_state.drag_ground_position = None;
    }
}

// ── Surface raycast system ─────────────────────────────────────────────────

/// System that raycasts against scene meshes during shape drag operations.
/// Stores the hit point and normal in [`ShapeDragState`] so shapes can be
/// placed on the sides of existing meshes.
pub fn shape_drag_raycast_system(
    mut drag_state: ResMut<ShapeDragState>,
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut mesh_ray_cast: MeshRayCast,
    preview_query: Query<Entity, With<ShapeDragPreview>>,
    parent_query: Query<&ChildOf>,
) {
    // Only run while dragging and cursor is over viewport
    if drag_state.dragging_shape.is_none() || drag_state.drag_ground_position.is_none() {
        drag_state.drag_surface_position = None;
        drag_state.drag_surface_normal = Vec3::ZERO;
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(cursor_pos): Option<Vec2> = window.cursor_position() else {
        drag_state.drag_surface_position = None;
        return;
    };

    let vp_min = viewport.screen_position;

    let Some((camera, camera_transform)) = camera_query.iter().next() else {
        drag_state.drag_surface_position = None;
        return;
    };

    let viewport_pos = Vec2::new(
        (cursor_pos.x - vp_min.x) / viewport.screen_size.x * viewport.current_size.x as f32,
        (cursor_pos.y - vp_min.y) / viewport.screen_size.y * viewport.current_size.y as f32,
    );

    let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_pos) else {
        drag_state.drag_surface_position = None;
        return;
    };

    // Disable early exit so we can skip preview entities
    let hits = mesh_ray_cast.cast_ray(
        ray,
        &MeshRayCastSettings {
            early_exit_test: &|_| false,
            ..MeshRayCastSettings::default()
        },
    );

    // Find closest hit that isn't a preview entity
    for (hit_entity, hit) in hits.iter() {
        if preview_query.contains(*hit_entity) {
            continue;
        }
        if is_descendant_of_preview(*hit_entity, &parent_query, &preview_query) {
            continue;
        }

        let normal = hit.normal.normalize_or_zero();
        let surface_normal = if normal == Vec3::ZERO { Vec3::Y } else { normal };

        drag_state.drag_surface_position = Some(hit.point);
        drag_state.drag_surface_normal = surface_normal;
        return;
    }

    // No mesh hit
    drag_state.drag_surface_position = None;
    drag_state.drag_surface_normal = Vec3::ZERO;
}

fn is_descendant_of_preview(
    entity: Entity,
    parent_query: &Query<&ChildOf>,
    preview_roots: &Query<Entity, With<ShapeDragPreview>>,
) -> bool {
    let mut current = entity;
    for _ in 0..16 {
        if let Ok(child_of) = parent_query.get(current) {
            let parent = child_of.0;
            if preview_roots.contains(parent) {
                return true;
            }
            current = parent;
        } else {
            break;
        }
    }
    false
}

// ── Drag preview (solid mesh follows cursor) ───────────────────────────────

/// System that spawns/updates/despawns the preview mesh during shape drags.
pub fn update_shape_drag_preview(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    drag_state: Res<ShapeDragState>,
    registry: Res<ShapeRegistry>,
    mut preview_state: ResMut<ShapeDragPreviewState>,
    mut transform_query: Query<&mut Transform, With<ShapeDragPreview>>,
    visibility_query: Query<&Visibility, With<ShapeDragPreview>>,
) {
    let is_dragging = drag_state.dragging_shape.is_some();
    let shape_id = drag_state.dragging_shape;

    match (is_dragging, preview_state.preview_entity) {
        // Drag active, no preview yet — spawn when cursor enters viewport
        (true, None) => {
            let Some(ground_pos) = drag_state.drag_ground_position else {
                return;
            };
            let shape_id = shape_id.unwrap();
            let Some(entry) = registry.get(shape_id) else {
                return;
            };

            let mesh = (entry.create_mesh)(&mut meshes);
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.7, 0.6),
                ..default()
            });

            let effective_pos = drag_state.drag_surface_position.unwrap_or(ground_pos);
            let normal = if drag_state.drag_surface_normal != Vec3::ZERO {
                drag_state.drag_surface_normal
            } else {
                Vec3::Y
            };
            let spawn_pos = effective_pos + normal * 0.5;

            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_translation(spawn_pos),
                    Visibility::default(),
                    ShapeDragPreview,
                ))
                .id();

            preview_state.preview_entity = Some(entity);
            preview_state.preview_shape_id = Some(shape_id);
        }
        // Shape changed mid-drag — respawn
        (true, Some(entity)) if preview_state.preview_shape_id != shape_id => {
            commands.entity(entity).despawn();

            let shape_id = shape_id.unwrap();
            let Some(entry) = registry.get(shape_id) else {
                preview_state.preview_entity = None;
                preview_state.preview_shape_id = None;
                return;
            };

            let mesh = (entry.create_mesh)(&mut meshes);
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.7, 0.6),
                ..default()
            });

            let pos = drag_state.drag_ground_position.unwrap_or(Vec3::ZERO);
            let effective_pos = drag_state.drag_surface_position.unwrap_or(pos);
            let normal = if drag_state.drag_surface_normal != Vec3::ZERO {
                drag_state.drag_surface_normal
            } else {
                Vec3::Y
            };
            let spawn_pos = effective_pos + normal * 0.5;

            let new_entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_translation(spawn_pos),
                    Visibility::default(),
                    ShapeDragPreview,
                ))
                .id();

            preview_state.preview_entity = Some(new_entity);
            preview_state.preview_shape_id = Some(shape_id);
        }
        // Still dragging same shape — update position or hide
        (true, Some(entity)) => {
            if let Some(ground_pos) = drag_state.drag_ground_position {
                if let Ok(mut tf) = transform_query.get_mut(entity) {
                    let effective_pos =
                        drag_state.drag_surface_position.unwrap_or(ground_pos);
                    let normal = if drag_state.drag_surface_normal != Vec3::ZERO {
                        drag_state.drag_surface_normal
                    } else {
                        Vec3::Y
                    };
                    tf.translation = effective_pos + normal * 0.5;
                }
                if let Ok(vis) = visibility_query.get(entity) {
                    if *vis == Visibility::Hidden {
                        commands.entity(entity).insert(Visibility::default());
                    }
                }
            } else {
                // Cursor left viewport — hide
                commands.entity(entity).insert(Visibility::Hidden);
            }
        }
        // Drag ended — cleanup preview
        (false, Some(entity)) => {
            commands.entity(entity).despawn();
            preview_state.preview_entity = None;
            preview_state.preview_shape_id = None;
        }
        // Idle
        (false, None) => {}
    }
}

// ── Spawn system: polls pending_drop ───────────────────────────────────────

/// System that spawns a shape entity when `pending_drop` is set.
/// Runs every frame, consumes the pending drop.
pub fn handle_shape_spawn(world: &mut World) {
    let drop = {
        let Some(mut state) = world.get_resource_mut::<ShapeDragState>() else { return };
        let Some(d) = state.pending_drop.take() else { return };
        d
    };
    let Some((shape_id, name, default_color)) = world.get_resource::<ShapeRegistry>()
        .and_then(|r| r.get(drop.shape_id))
        .map(|e| (e.id.to_string(), e.name.to_string(), e.default_color))
    else {
        warn!("Shape '{}' not found in registry", drop.shape_id);
        return;
    };
    let normal = if drop.normal != Vec3::ZERO { drop.normal } else { Vec3::Y };
    let position = drop.position + normal * 0.5;

    renzora_undo::execute(world, UndoContext::Scene, Box::new(SpawnShapeCmd {
        entity: Entity::PLACEHOLDER,
        shape_id, name, position, color: default_color,
    }));
}
