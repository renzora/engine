//! Shape drag-and-drop — handles dragging shapes from the shape library panel
//! onto the viewport with surface raycast placement.
//!
//! Follows the legacy pattern: a persistent `ShapeDragState` resource with fields
//! mutated directly by UI code and polled by regular Bevy systems.

use bevy::camera::primitives::MeshAabb;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{SnapSettings, ViewportSettings};
use renzora::core::{EditorCamera, ShapeRegistry};
use renzora_ui::shape_drag::{
    PendingShapeDrop, ShapeDragPreview, ShapeDragPreviewState, ShapeDragState,
};
use renzora_undo::{self, SpawnShapeCmd, UndoContext};

use crate::ViewportState;

// ── Placement resolution ───────────────────────────────────────────────────

/// Where a dragged shape would land right now: the surface hit (falling back to
/// the ground plane) pushed half a unit along the surface normal so the shape
/// rests on top instead of intersecting, then rounded onto the transform-snap
/// grid. Returns `None` while the cursor is outside the viewport.
fn placement_position(
    drag_state: &ShapeDragState,
    snap: &SnapSettings,
    min_offset: Vec3,
) -> Option<Vec3> {
    let ground = drag_state.drag_ground_position?;
    let hit = drag_state.drag_surface_position.unwrap_or(ground);
    let normal = if drag_state.drag_surface_normal != Vec3::ZERO {
        drag_state.drag_surface_normal
    } else {
        Vec3::Y
    };
    Some(snap_translation(hit + normal * 0.5, snap, min_offset))
}

/// Round a world position onto the translate-snap grid the same way the gizmo's
/// translate handler does, so a shape doesn't shift the moment you first drag
/// it after dropping. With edge snap on it is the AABB min corner that lands on
/// the gridline, which makes a dropped unit cube fill a grid cell rather than
/// straddle the line through its centre.
fn snap_translation(pos: Vec3, snap: &SnapSettings, min_offset: Vec3) -> Vec3 {
    if !snap.translate_enabled || snap.translate_snap <= 0.0 {
        return pos;
    }
    let step = snap.translate_snap;
    let off = if snap.translate_edge_snap {
        min_offset
    } else {
        Vec3::ZERO
    };
    let target = pos + off;
    Vec3::new(
        (target.x / step).round() * step,
        (target.y / step).round() * step,
        (target.z / step).round() * step,
    ) - off
}

// ── Native (bevy_ui) drop handler ──────────────────────────────────────────

/// On left-mouse release of a drag started from the native shape library
/// (`native_drag`), drop the shape onto the viewport (or cancel). Only acts on
/// native drags.
pub fn native_shape_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag_state: ResMut<ShapeDragState>,
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    if !drag_state.native_drag || drag_state.dragging_shape.is_none() {
        return;
    }
    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    // Cursor over the viewport?
    let over_viewport = window_query
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|c| {
            let min = viewport.screen_position;
            let max = min + viewport.screen_size;
            c.x >= min.x && c.y >= min.y && c.x <= max.x && c.y <= max.y
        })
        .unwrap_or(false);

    if over_viewport {
        let shape_id = drag_state.dragging_shape.unwrap();
        // Commit the preview's own position (snap included) so the shape lands
        // under the ghost. It is only missing when the placement ray found
        // nothing at all — e.g. the camera is looking at the sky — in which
        // case fall back to the origin as this handler always has.
        let position = drag_state.preview_position.unwrap_or(Vec3::ZERO);
        drag_state.pending_drop = Some(PendingShapeDrop { shape_id, position });
    }
    // Clear the drag in both cases (drop or cancel).
    drag_state.dragging_shape = None;
    drag_state.native_drag = false;
    drag_state.drag_ground_position = None;
    drag_state.drag_surface_position = None;
    drag_state.drag_surface_normal = Vec3::ZERO;
    drag_state.preview_position = None;
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
        let surface_normal = if normal == Vec3::ZERO {
            Vec3::Y
        } else {
            normal
        };

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
    mut drag_state: ResMut<ShapeDragState>,
    registry: Res<ShapeRegistry>,
    settings: Option<Res<ViewportSettings>>,
    checker: Option<Res<renzora::core::CheckerTexture>>,
    mut preview_state: ResMut<ShapeDragPreviewState>,
    mut transform_query: Query<&mut Transform, With<ShapeDragPreview>>,
    visibility_query: Query<&Visibility, With<ShapeDragPreview>>,
) {
    let snap = settings.as_deref().map(|s| s.snap).unwrap_or_default();
    let shape_id = drag_state.dragging_shape;

    // Drag ended — clear the placement and the ghost.
    let Some(shape_id) = shape_id else {
        drag_state.preview_position = None;
        if let Some(entity) = preview_state.preview_entity.take() {
            commands.entity(entity).despawn();
            preview_state.preview_shape_id = None;
            preview_state.preview_min_offset = Vec3::ZERO;
        }
        return;
    };

    // Shape changed mid-drag — retire the old ghost so the arm below builds a
    // new one for the new shape (and, with it, a new min offset).
    if preview_state.preview_shape_id != Some(shape_id) {
        if let Some(entity) = preview_state.preview_entity.take() {
            commands.entity(entity).despawn();
        }
        preview_state.preview_shape_id = None;
        preview_state.preview_min_offset = Vec3::ZERO;
    }

    match preview_state.preview_entity {
        // No preview yet — spawn it once the cursor is over the viewport.
        None => {
            // Bail before building the mesh: `create_mesh` inserts a new asset
            // every call, so probing the placement first would leak one per
            // frame for as long as the drag hovers outside the viewport.
            if drag_state.drag_ground_position.is_none() {
                drag_state.preview_position = None;
                return;
            }
            let Some(entry) = registry.get(shape_id) else {
                drag_state.preview_position = None;
                return;
            };

            let mesh = (entry.create_mesh)(&mut meshes);
            let min_offset = meshes
                .get(&mesh)
                .and_then(|m| m.compute_aabb())
                .map(|aabb| Vec3::from(aabb.center - aabb.half_extents))
                .unwrap_or(Vec3::ZERO);

            let Some(spawn_pos) = placement_position(&drag_state, &snap, min_offset) else {
                drag_state.preview_position = None;
                return;
            };

            // Match the checker the committed spawn will get so the preview
            // doesn't visibly "change material" on drop.
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.7, 0.6),
                base_color_texture: checker.as_ref().map(|c| c.0.clone()),
                ..default()
            });

            let entity = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_translation(spawn_pos),
                    Visibility::default(),
                    ShapeDragPreview,
                ))
                .id();

            drag_state.preview_position = Some(spawn_pos);
            preview_state.preview_entity = Some(entity);
            preview_state.preview_shape_id = Some(shape_id);
            preview_state.preview_min_offset = min_offset;
        }
        // Same shape, ghost already up — move it, or hide it off-viewport.
        Some(entity) => {
            let placement = placement_position(&drag_state, &snap, preview_state.preview_min_offset);
            drag_state.preview_position = placement;

            if let Some(pos) = placement {
                if let Ok(mut tf) = transform_query.get_mut(entity) {
                    tf.translation = pos;
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
    }
}

// ── Spawn system: polls pending_drop ───────────────────────────────────────

/// System that spawns a shape entity when `pending_drop` is set.
/// Runs every frame, consumes the pending drop.
pub fn handle_shape_spawn(world: &mut World) {
    let drop = {
        let Some(mut state) = world.get_resource_mut::<ShapeDragState>() else {
            return;
        };
        let Some(d) = state.pending_drop.take() else {
            return;
        };
        d
    };
    let Some((shape_id, name, default_color)) = world
        .get_resource::<ShapeRegistry>()
        .and_then(|r| r.get(drop.shape_id))
        .map(|e| (e.id.to_string(), e.name.to_string(), e.default_color))
    else {
        warn!("Shape '{}' not found in registry", drop.shape_id);
        return;
    };
    // Already the final placement — the preview resolved the surface offset
    // and the grid snap.
    let position = drop.position;

    renzora_undo::execute(
        world,
        UndoContext::Scene,
        Box::new(SpawnShapeCmd {
            entity: Entity::PLACEHOLDER,
            shape_id,
            name,
            position,
            color: default_color,
        }),
    );
}
