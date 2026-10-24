//! Skeleton gizmo: the selected model's bones, drawn over the scene.
//!
//! Each bone is a line from its joint to its child's with a ring around it,
//! coloured by side, in the palette the Animation workspace's stage uses
//! (`renzora::core::bone_style`), so a skeleton looks the same in both. Drawn
//! with the always-on-top overlay lines, so a bone inside the body still
//! shows.
//!
//! The bones used to be solid octahedral meshes, one entity per bone,
//! despawned and spawned again every frame: around seventy entities a frame for
//! a Mixamo rig, for a picture lines draw with none.

use bevy::animation::AnimationTargetId;
use bevy::prelude::*;

use renzora::core::bone_style::{bone_colour, ring_radius};
use renzora_editor_framework::{EditorCamera, EditorSelection};

use crate::OverlayGizmoGroup;

/// Resource tracking hovered/selected bone for gizmo interaction.
#[derive(Resource, Default)]
pub struct BoneSelection {
    pub selected_bone: Option<Entity>,
    pub hovered_bone: Option<Entity>,
}

/// The bones drawn this frame, as world-space segments, for click picking.
///
/// Picked by distance to the drawn segment rather than by raycasting the
/// model: a bone sits inside the body, so a ray meets the skin first, while a
/// bone is drawn on top and must win the click.
#[derive(Resource, Default)]
pub struct DrawnBones {
    /// `(bone, head, tail)`. The bone is the joint the segment grows out of,
    /// the one it rotates around; a leaf joint is a zero-length segment.
    pub segments: Vec<(Entity, Vec3, Vec3)>,
}

/// Draw the skeleton of the selected model, or of the model the selected bone
/// belongs to, and record it in [`DrawnBones`] for picking.
///
/// Drawn from the model rather than from the selection itself, so selecting a
/// bone (to pose it) keeps the whole rig on screen instead of only that bone's
/// children.
#[allow(clippy::too_many_arguments)]
pub fn draw_skeleton_gizmo(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    settings: Option<Res<renzora::core::viewport_types::ViewportSettings>>,
    selection: Res<EditorSelection>,
    (mut bone_selection, mut drawn): (ResMut<BoneSelection>, ResMut<DrawnBones>),
    global_transforms: Query<&GlobalTransform>,
    children_q: Query<&Children>,
    parent_q: Query<&ChildOf>,
    target_q: Query<(), With<AnimationTargetId>>,
    skins: Query<&bevy::mesh::skinning::SkinnedMesh>,
    name_q: Query<&Name>,
    cameras: Query<&GlobalTransform, With<EditorCamera>>,
) {
    drawn.segments.clear();

    // Gizmos dropdown → Rigging → "Skeleton".
    if !settings.map(|s| s.show_skeleton_gizmos).unwrap_or(true) {
        set_if_changed(&mut bone_selection, None);
        return;
    }

    let Some(selected) = selection.get() else {
        set_if_changed(&mut bone_selection, None);
        return;
    };

    let root = rig_root(selected, &parent_q, &target_q);
    let bones = collect_joints(root, &children_q, &target_q, &skins);
    // A bone selected anywhere (a click here, the hierarchy, the outliner) is
    // the bone being posed.
    let posed = bones.contains(&selected).then_some(selected);
    set_if_changed(&mut bone_selection, posed);
    if bones.is_empty() {
        return;
    }
    let is_bone = |e: Entity| bones.contains(&e);
    let camera = cameras.iter().next().map(|c| c.translation()).unwrap_or(Vec3::ZERO);
    let colour = |bone: Entity| {
        let name = name_q.get(bone).map(|n| n.as_str()).unwrap_or("");
        let c = bone_colour(name, selection.is_selected(bone), bone_selection.hovered_bone == Some(bone));
        if selection.is_selected(bone) { c } else { c.with_alpha(0.9) }
    };

    for &bone in &bones {
        let Ok(bone_gt) = global_transforms.get(bone) else {
            continue;
        };
        let bone_pos = bone_gt.translation();

        let has_bone_child = children_q
            .get(bone)
            .ok()
            .is_some_and(|children| children.iter().any(is_bone));

        // The segment from the parent joint to this one is the PARENT's bone:
        // it grows out of the parent and turns around the parent's joint, so
        // that is what clicking it selects and what it is coloured as.
        if let Ok(child_of) = parent_q.get(bone) {
            let parent = child_of.parent();
            if is_bone(parent) {
                if let Ok(parent_gt) = global_transforms.get(parent) {
                    let head = parent_gt.translation();
                    let c = colour(parent);
                    gizmos.line(head, bone_pos, c);
                    let length = head.distance(bone_pos);
                    if length > 1e-5 {
                        let dir = (bone_pos - head) / length;
                        let r = ring_radius(length, camera.distance(head));
                        gizmos.circle(Isometry3d::new(head + dir * length * 0.25, Quat::from_rotation_arc(Vec3::Z, dir)), r, c);
                    }
                    drawn.segments.push((parent, head, bone_pos));
                }
            }
        }

        // End bones get a dot, so they are still there to see and to click.
        if !has_bone_child {
            let r = (camera.distance(bone_pos) * 0.006).max(1e-4);
            gizmos.sphere(Isometry3d::from_translation(bone_pos), r, colour(bone));
            drawn.segments.push((bone, bone_pos, bone_pos));
        }
    }
}

fn set_if_changed(sel: &mut ResMut<BoneSelection>, bone: Option<Entity>) {
    if sel.selected_bone != bone {
        sel.selected_bone = bone;
    }
}

/// The model a selection belongs to: up from `entity` past every ancestor that
/// is itself an animation target, to the first that is not (the model root the
/// animator sits on), or to the top. Selecting the model itself returns it.
fn rig_root(entity: Entity, parents: &Query<&ChildOf>, targets: &Query<(), With<AnimationTargetId>>) -> Entity {
    let mut current = entity;
    while targets.contains(current) {
        match parents.get(current) {
            Ok(p) => current = p.parent(),
            Err(_) => break,
        }
    }
    current
}

/// The joints under `root`: the union of its skinned meshes' joint lists, so
/// meshes, the armature node and other named non-bones are not drawn as bones.
/// A rig with no skin (an animated node hierarchy) falls back to every
/// animation target under it.
fn collect_joints(
    root: Entity,
    children_q: &Query<&Children>,
    targets: &Query<(), With<AnimationTargetId>>,
    skins: &Query<&bevy::mesh::skinning::SkinnedMesh>,
) -> Vec<Entity> {
    let mut joints: Vec<Entity> = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok(skin) = skins.get(e) {
            for j in &skin.joints {
                if !joints.contains(j) {
                    joints.push(*j);
                }
            }
        }
        if let Ok(children) = children_q.get(e) {
            stack.extend(children.iter());
        }
    }
    if joints.is_empty() {
        collect_bones(root, children_q, targets, &mut joints);
    }
    joints
}

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
