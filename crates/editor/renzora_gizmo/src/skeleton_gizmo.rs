//! Skeleton gizmo — draws bone hierarchy for entities with AnimatorComponent.
//!
//! When the selected entity has an `AnimatorComponent` with an initialized
//! `AnimatorState`, draws lines between parent/child joints and spheres at
//! joint positions using `Gizmos<OverlayGizmoGroup>`.
//!
//! Bones are identified by walking the child hierarchy and finding named
//! entities that have `AnimationTarget` components (set up by rehydration).

use bevy::prelude::*;
use bevy::animation::AnimationTargetId;

use renzora_editor::EditorSelection;
use renzora_animation::{AnimatorComponent, AnimatorState};

use crate::OverlayGizmoGroup;

/// Resource tracking hovered/selected bone for gizmo interaction.
#[derive(Resource, Default)]
pub struct BoneSelection {
    pub selected_bone: Option<Entity>,
    pub hovered_bone: Option<Entity>,
}

/// Draw skeleton overlay for the selected entity.
pub fn draw_skeleton_gizmo(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    bone_selection: Res<BoneSelection>,
    animator_q: Query<(&AnimatorComponent, Option<&AnimatorState>)>,
    global_transforms: Query<&GlobalTransform>,
    children_q: Query<&Children>,
    parent_q: Query<&ChildOf>,
    target_q: Query<(), With<AnimationTargetId>>,
) {
    let Some(selected) = selection.get() else { return };

    // Only draw if the selected entity has an animator with initialized state
    let Ok((_, state_opt)) = animator_q.get(selected) else { return };
    let Some(state) = state_opt else { return };
    if !state.initialized { return; }

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
