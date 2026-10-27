//! Setting the selection down on the scene's surfaces: Move to Cursor (`V`)
//! brings it to the surface under the cursor, Drop to Surface (`Page Down`)
//! lowers each entity onto whatever is beneath it, and Paste lands copies
//! where the cursor points.
//!
//! An entity's extent is its whole subtree's: a model's root carries no
//! bounds of its own, its meshes are children, so measuring the root alone put
//! a model's origin on the surface instead of its wheels. Surfaces are found by
//! ray casting the scene's meshes, not a plane at height zero, so a kerb, a
//! table top or terrain is landed on rather than sunk into.

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemState;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::ViewportState;
use renzora_editor_framework::{EditorCamera, EditorSelection, HideInHierarchy};

use crate::types::GizmoMesh;

/// One moved entity: its transform before and after.
type Move = (Entity, Transform, Transform);

/// Moves the selection so the group sits on the surface under the cursor,
/// centred on it, as one undo step.
pub(crate) fn move_selection_to_cursor(world: &mut World) {
    let selection = top_level(world, &selected(world));
    if selection.is_empty() {
        return;
    }
    let Some(target) = surface_under_cursor(world, &selection) else {
        return;
    };
    let moves = place_on(world, &selection, target);
    record(world, "Move to Cursor", moves);
}

/// Lowers (or raises) each selected entity so its bottom rests on the first
/// surface below its middle, as one undo step. An entity with nothing under
/// it stays where it is.
pub(crate) fn drop_selection_to_surface(world: &mut World) {
    let selection = top_level(world, &selected(world));
    let mut moves = Vec::new();
    for entity in selection {
        let Some((min, max)) = subtree_bounds(world, entity) else {
            continue;
        };
        // From the middle rather than the bottom, so an entity already sunk
        // a little into the floor still finds the floor, and one resting on a
        // table finds the table rather than the ground under it.
        let centre = (min + max) * 0.5;
        let Some(distance) = scene_hit(world, Ray3d::new(centre, Dir3::NEG_Y), &[entity]) else {
            continue;
        };
        let surface = centre.y - distance;
        moves.extend(shift(world, entity, Vec3::Y * (surface - min.y)));
    }
    record(world, "Drop to Surface", moves);
}

/// Where Paste puts its copies: the surface under the cursor, or `None` when
/// the cursor is not over the viewport.
pub(crate) fn paste_target(world: &mut World) -> Option<Vec3> {
    surface_under_cursor(world, &[])
}

/// Moves `entities` together so their combined bounds sit centred on `target`
/// with their bottom at its height. Returns what moved.
pub(crate) fn place_on(world: &mut World, entities: &[Entity], target: Vec3) -> Vec<Move> {
    let mut min = Vec3::INFINITY;
    let mut max = Vec3::NEG_INFINITY;
    for &entity in entities {
        if let Some((lo, hi)) = subtree_bounds(world, entity) {
            min = min.min(lo);
            max = max.max(hi);
        }
    }
    if !min.is_finite() {
        return Vec::new();
    }
    let centre = (min + max) * 0.5;
    let delta = Vec3::new(target.x - centre.x, target.y - min.y, target.z - centre.z);
    entities.iter().filter_map(|&entity| shift(world, entity, delta)).collect()
}

/// The surface point under the viewport cursor, ignoring `ignore` and
/// everything under it (the entities being moved must not land on
/// themselves). Falls back to the ground plane when no mesh is hit, as
/// placement did before it looked at meshes.
fn surface_under_cursor(world: &mut World, ignore: &[Entity]) -> Option<Vec3> {
    let ray = cursor_ray(world)?;
    if let Some(distance) = scene_hit(world, ray, ignore) {
        return Some(ray.get_point(distance));
    }
    let direction = ray.direction.as_vec3();
    if direction.y.abs() <= 1e-6 {
        return None;
    }
    let t = -ray.origin.y / direction.y;
    (t > 0.0 && t <= 10_000.0).then(|| ray.get_point(t))
}

/// The editor camera's ray through the cursor, when the cursor is over the
/// viewport.
fn cursor_ray(world: &mut World) -> Option<Ray3d> {
    let (screen_position, screen_size, current_size) = {
        let viewport = world.get_resource::<ViewportState>()?;
        if !viewport.hovered {
            return None;
        }
        (viewport.screen_position, viewport.screen_size, viewport.current_size)
    };
    let cursor = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(world)
        .ok()?
        .cursor_position()?;
    let local = cursor - screen_position;
    if local.x < 0.0 || local.y < 0.0 || local.x > screen_size.x || local.y > screen_size.y {
        return None;
    }
    let (camera, transform) = world
        .query_filtered::<(&Camera, &GlobalTransform), With<EditorCamera>>()
        .single(world)
        .ok()?;
    let target_position = local / screen_size * current_size.as_vec2();
    camera.viewport_to_world(transform, target_position).ok()
}

/// How far along `ray` the nearest scene surface is, skipping `ignore` and
/// its descendants. Editor helpers (gizmo handles, collider handles, preview
/// scenes) carry `HideInHierarchy` or draw off the scene's render layer, and
/// never count.
fn scene_hit(world: &mut World, ray: Ray3d, ignore: &[Entity]) -> Option<f32> {
    let ignored = subtree_entities(world, ignore);
    let mut state = SystemState::<(
        MeshRayCast,
        Query<(Has<HideInHierarchy>, Has<GizmoMesh>, Option<&RenderLayers>)>,
    )>::new(world);
    let (mut ray_cast, helpers) = state.get_mut(world).ok()?;
    let scene_only = |entity: Entity| {
        !ignored.contains(&entity)
            && helpers.get(entity).is_ok_and(|(hidden, gizmo, layers)| {
                !hidden && !gizmo && layers.is_none_or(|l| l.intersects(&RenderLayers::layer(0)))
            })
    };
    let settings = MeshRayCastSettings::default().with_filter(&scene_only);
    ray_cast.cast_ray(ray, &settings).first().map(|(_, hit)| hit.distance)
}

/// The world-space box around `root` and everything under it, from each
/// mesh's bounds. `None` when nothing in it has bounds; a light or an empty
/// then counts as the point it stands at.
pub(crate) fn subtree_bounds(world: &World, root: Entity) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::INFINITY;
    let mut max = Vec3::NEG_INFINITY;
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let (Some(aabb), Some(transform)) = (world.get::<Aabb>(entity), world.get::<GlobalTransform>(entity)) {
            let centre = Vec3::from(aabb.center);
            let half = Vec3::from(aabb.half_extents);
            for corner in 0..8 {
                let sign = Vec3::new(
                    if corner & 1 == 0 { -1.0 } else { 1.0 },
                    if corner & 2 == 0 { -1.0 } else { 1.0 },
                    if corner & 4 == 0 { -1.0 } else { 1.0 },
                );
                let point = transform.transform_point(centre + sign * half);
                min = min.min(point);
                max = max.max(point);
            }
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend_from_slice(children);
        }
    }
    if min.is_finite() {
        return Some((min, max));
    }
    let point = world.get::<GlobalTransform>(root)?.translation();
    Some((point, point))
}

/// Moves `entity` by `delta` in world space, converted into its parent's
/// space so a child of a rotated or scaled parent still moves by `delta`.
fn shift(world: &mut World, entity: Entity, delta: Vec3) -> Option<Move> {
    if delta == Vec3::ZERO {
        return None;
    }
    let parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
    let local = match parent.and_then(|p| world.get::<GlobalTransform>(p)) {
        Some(parent) => parent.affine().inverse().transform_vector3(delta),
        None => delta,
    };
    let mut transform = world.get_mut::<Transform>(entity)?;
    let old = *transform;
    transform.translation += local;
    Some((entity, old, *transform))
}

fn selected(world: &World) -> Vec<Entity> {
    world.get_resource::<EditorSelection>().map(|s| s.get_all()).unwrap_or_default()
}

/// `entities` without any whose ancestor is also among them: moving both a
/// model and one of its parts would move the part twice.
fn top_level(world: &World, entities: &[Entity]) -> Vec<Entity> {
    let set: HashSet<Entity> = entities.iter().copied().collect();
    entities
        .iter()
        .copied()
        .filter(|&entity| {
            let mut current = entity;
            while let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) {
                if set.contains(&parent) {
                    return false;
                }
                current = parent;
            }
            true
        })
        .collect()
}

fn subtree_entities(world: &World, roots: &[Entity]) -> HashSet<Entity> {
    let mut found = HashSet::default();
    let mut stack = roots.to_vec();
    while let Some(entity) = stack.pop() {
        if found.insert(entity) {
            if let Some(children) = world.get::<Children>(entity) {
                stack.extend_from_slice(children);
            }
        }
    }
    found
}

/// Records `moves` as one undo step; they have already been applied.
fn record(world: &mut World, label: &str, moves: Vec<Move>) {
    if moves.is_empty() {
        return;
    }
    let cmds = moves
        .into_iter()
        .map(|(entity, old, new)| {
            Box::new(renzora_undo::TransformCmd { entity, old, new }) as Box<dyn renzora_undo::UndoCommand>
        })
        .collect();
    renzora_undo::record(
        world,
        renzora_undo::UndoContext::Scene,
        Box::new(renzora_undo::CompoundCmd { label: label.to_string(), cmds }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model as an import spawns one: a root with no bounds of its own, and
    /// a mesh child a metre tall whose bottom is half a metre below the root.
    fn model(world: &mut World, at: Vec3) -> Entity {
        let root = world.spawn((Transform::from_translation(at), GlobalTransform::from_translation(at))).id();
        world.spawn((
            ChildOf(root),
            Transform::default(),
            GlobalTransform::from_translation(at),
            Aabb::from_min_max(Vec3::new(-1.0, -0.5, -2.0), Vec3::new(1.0, 0.5, 2.0)),
        ));
        root
    }

    #[test]
    fn a_model_is_measured_by_its_meshes_not_its_root() {
        let mut world = World::new();
        let root = model(&mut world, Vec3::new(3.0, 5.0, 0.0));
        let (min, max) = subtree_bounds(&world, root).unwrap();
        assert_eq!(min, Vec3::new(2.0, 4.5, -2.0));
        assert_eq!(max, Vec3::new(4.0, 5.5, 2.0));
    }

    #[test]
    fn a_rotated_mesh_is_measured_by_its_turned_corners() {
        // Half-extents (2, 1, 1) turned 90 degrees about Z put the 2 on the
        // Y axis, so the bottom is 2 below the centre, at 3.
        let mut world = World::new();
        let transform = Transform::from_xyz(0.0, 5.0, 0.0).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
        let mesh = world
            .spawn((
                transform,
                GlobalTransform::from(transform),
                Aabb::from_min_max(Vec3::new(-2.0, -1.0, -1.0), Vec3::new(2.0, 1.0, 1.0)),
            ))
            .id();
        let (min, _) = subtree_bounds(&world, mesh).unwrap();
        assert!((min.y - 3.0).abs() < 1e-4, "bottom at {}", min.y);
    }

    #[test]
    fn a_model_lands_on_its_bottom_centred_on_the_target() {
        let mut world = World::new();
        let root = model(&mut world, Vec3::new(3.0, 5.0, 0.0));
        let moves = place_on(&mut world, &[root], Vec3::new(10.0, 1.0, -4.0));
        assert_eq!(moves.len(), 1, "only the root moves; its parts follow it");
        assert_eq!(world.get::<Transform>(root).unwrap().translation, Vec3::new(10.0, 1.5, -4.0));
    }

    #[test]
    fn a_child_of_a_scaled_parent_moves_by_the_world_distance() {
        let mut world = World::new();
        let parent_transform = Transform::from_scale(Vec3::splat(2.0));
        let parent = world.spawn((parent_transform, GlobalTransform::from(parent_transform))).id();
        let child = world.spawn((ChildOf(parent), Transform::default(), GlobalTransform::default())).id();
        shift(&mut world, child, Vec3::new(4.0, 0.0, 0.0));
        assert_eq!(world.get::<Transform>(child).unwrap().translation, Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn a_part_selected_with_its_model_moves_only_with_the_model() {
        let mut world = World::new();
        let root = model(&mut world, Vec3::ZERO);
        let part = world.get::<Children>(root).unwrap()[0];
        assert_eq!(top_level(&world, &[part, root]), vec![root]);
    }
}
