//! Skeleton gizmo — draws bone hierarchy for entities with AnimatorComponent.
//!
//! When the selected entity has an `AnimatorComponent` with an initialized
//! `AnimatorState`, draws lines between parent/child joints and spheres at
//! joint positions using `Gizmos<OverlayGizmoGroup>`.
//!
//! Bones are identified by walking the child hierarchy and finding named
//! entities that have `AnimationTarget` components (set up by rehydration).
//!
//! Uses reflection to detect AnimatorComponent/AnimatorState so this crate
//! does not depend on renzora_animation.

use bevy::prelude::*;
use bevy::animation::AnimationTargetId;

use renzora::editor::EditorSelection;

use crate::OverlayGizmoGroup;

/// Resource tracking hovered/selected bone for gizmo interaction.
#[derive(Resource, Default)]
pub struct BoneSelection {
    pub selected_bone: Option<Entity>,
    pub hovered_bone: Option<Entity>,
}

/// Check if an entity has a component whose type name contains the given substring.
fn has_component_by_name(world: &World, entity: Entity, name: &str) -> bool {
    let Ok(er) = world.get_entity(entity) else { return false };
    for &component_id in er.archetype().components() {
        if let Some(info) = world.components().get_info(component_id) {
            if info.name().contains(name) {
                return true;
            }
        }
    }
    false
}

/// Read a bool field from a reflected component by type name.
fn get_reflected_bool_field(world: &World, entity: Entity, type_substr: &str, field: &str) -> Option<bool> {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    let registration = registry.iter().find(|reg| {
        let path = reg.type_info().type_path();
        path.contains(type_substr)
    })?;

    let reflect_component = registration.data::<ReflectComponent>()?;
    let entity_ref = world.get_entity(entity).ok()?;
    let reflected = reflect_component.reflect(entity_ref)?;

    if let bevy::reflect::ReflectRef::Struct(s) = reflected.reflect_ref() {
        let field_val = s.field(field)?;
        field_val.try_downcast_ref::<bool>().copied()
    } else {
        None
    }
}

/// Draw skeleton overlay for the selected entity.
pub fn draw_skeleton_gizmo(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    bone_selection: Res<BoneSelection>,
    global_transforms: Query<&GlobalTransform>,
    children_q: Query<&Children>,
    parent_q: Query<&ChildOf>,
    target_q: Query<(), With<AnimationTargetId>>,
    world: &World,
) {
    let Some(selected) = selection.get() else { return };

    // Only draw if the selected entity has an AnimatorComponent
    if !has_component_by_name(world, selected, "AnimatorComponent") {
        return;
    }

    // Check if AnimatorState exists and is initialized
    let has_state = has_component_by_name(world, selected, "AnimatorState");
    if !has_state {
        return;
    }
    let initialized = get_reflected_bool_field(world, selected, "AnimatorState", "initialized");
    if !initialized.unwrap_or(false) {
        return;
    }

    let default_color = Color::srgba(0.9, 0.9, 0.9, 0.6);
    let hovered_color = Color::srgb(1.0, 1.0, 0.3);
    let selected_color = Color::srgb(0.3, 1.0, 1.0);

    // Collect all animation target entities in the hierarchy
    let mut bones = Vec::new();
    collect_bones(selected, &children_q, &target_q, &mut bones);

    for &bone in &bones {
        let Ok(bone_gt) = global_transforms.get(bone) else { continue };
        let bone_pos = bone_gt.translation();

        let color = if bone_selection.selected_bone == Some(bone) {
            selected_color
        } else if bone_selection.hovered_bone == Some(bone) {
            hovered_color
        } else {
            default_color
        };

        // Draw sphere at joint position
        gizmos.sphere(Isometry3d::from_translation(bone_pos), 0.02, color);

        // Draw line to parent if parent is also a bone
        if let Ok(child_of) = parent_q.get(bone) {
            let parent = child_of.parent();
            if target_q.get(parent).is_ok() {
                if let Ok(parent_gt) = global_transforms.get(parent) {
                    gizmos.line(parent_gt.translation(), bone_pos, color);
                }
            }
        }
    }
}

/// Recursively collect entities with AnimationTargetId.
fn collect_bones(
    entity: Entity,
    children_q: &Query<&Children>,
    target_q: &Query<(), With<AnimationTargetId>>,
    out: &mut Vec<Entity>,
) {
    if target_q.get(entity).is_ok() {
        out.push(entity);
    }
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            collect_bones(child, children_q, target_q, out);
        }
    }
}
