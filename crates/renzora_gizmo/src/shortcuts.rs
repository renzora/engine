//! Keyboard shortcuts that act on the selection or the document: delete,
//! deselect, copy/paste/duplicate, the file commands, and the tool-mode keys.
//!
//! These deliberately are NOT gated on the 3D view — Delete on a 2D entity has
//! to work from the Hierarchy too — so the plugin registers them outside the
//! `in_three_view` chain.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::ViewportState;
use renzora::core::InputFocusState;
use renzora_editor_framework::{EditorCamera, EditorSelection};

use crate::modal_transform;
use crate::types::{GizmoMode, GizmoState};

/// One-shot resource to signal pending modal grab from duplicate-and-move.
#[derive(Resource)]
pub(crate) struct PendingModalGrab;

/// Editor-wide clipboard for Copy/Paste of entities. Stores the source
/// entity ids captured at copy time; paste deep-clones each via
/// `EntityWorldMut::clone_and_spawn`, so all components transfer. Sources
/// that have been despawned between copy and paste are silently skipped.
#[derive(Resource, Default, Clone, Debug)]
pub struct EditorClipboard {
    pub entities: Vec<Entity>,
}

pub(crate) fn handle_selection_shortcuts(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    viewport_state: Res<ViewportState>,
    selection: Res<EditorSelection>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    gizmo_state: Res<GizmoState>,
    modal: Res<modal_transform::ModalTransformState>,
) {
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if gizmo_state.active_axis.is_some() {
        return;
    }
    if modal.active {
        return;
    }

    // Delete fires from any panel (e.g. selecting in the Hierarchy and
    // pressing Delete without moving the cursor into the viewport). It is
    // suppressed while a panel (e.g. the animation timeline with a keyframe
    // selected) is consuming Delete to remove the keyframe, not the entity.
    if keybindings.just_pressed(EditorAction::Delete, &keyboard) && !input_focus.suppress_entity_delete {
        let entities = selection.get_all();
        if !entities.is_empty() {
            // Faithful subtree-snapshot delete (works for lights, cameras, GLB
            // imports, 2D nodes, groups) — not just default-mesh primitives. The
            // reselect variant moves selection to the nearest survivor afterwards
            // so the viewport doesn't go blank (don't pre-clear here — that's what
            // left it blank).
            commands.queue(move |world: &mut World| {
                renzora_undo::delete_entities_with_undo_reselect(world, &entities);
            });
        }
    }

    if input_focus.pointer_over_ui && !viewport_state.hovered {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }

    if keybindings.just_pressed(EditorAction::Deselect, &keyboard) {
        selection.clear();
    }

    if keybindings.just_pressed(EditorAction::CreateNode, &keyboard) {
        commands.insert_resource(renzora::core::CreateNodeRequested);
    }

    // Copy (Ctrl+C) — snapshot the current selection into the clipboard.
    if keybindings.just_pressed(EditorAction::Copy, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            commands.queue(move |world: &mut World| {
                world.insert_resource(EditorClipboard { entities });
            });
        }
    }

    // Paste (Ctrl+V) — clone every entity on the clipboard (filtering out
    // ones that have since been despawned) and select the copies. If the
    // cursor is over the viewport, pasted entities are re-positioned to
    // the ground-plane hit so paste follows the camera/cursor. Otherwise
    // they land at their original world position.
    if keybindings.just_pressed(EditorAction::Paste, &keyboard) {
        commands.queue(move |world: &mut World| {
            let sources = world
                .get_resource::<EditorClipboard>()
                .map(|c| c.entities.clone())
                .unwrap_or_default();
            if sources.is_empty() {
                return;
            }

            let paste_target = compute_paste_target(world);
            duplicate_entities(world, &sources);

            if let Some(target) = paste_target {
                let new_ids = world
                    .get_resource::<EditorSelection>()
                    .map(|s| s.get_all())
                    .unwrap_or_default();
                if new_ids.is_empty() {
                    return;
                }
                reposition_paste_group(world, &new_ids, target);
            }
        });
    }

    // Move selection to cursor (V): teleports the selected entities so their
    // centroid sits under the viewport cursor, bottom snapped to the hit point.
    // Reuses the paste-placement helpers for consistent behavior.
    if keybindings.just_pressed(EditorAction::MoveSelectionToCursor, &keyboard) {
        commands.queue(move |world: &mut World| {
            let selected = world
                .get_resource::<EditorSelection>()
                .map(|s| s.get_all())
                .unwrap_or_default();
            if selected.is_empty() {
                return;
            }
            let Some(target) = compute_paste_target(world) else {
                return;
            };
            reposition_paste_group(world, &selected, target);
        });
    }

    // Duplicate (Ctrl+D)
    if keybindings.just_pressed(EditorAction::Duplicate, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            commands.queue(move |world: &mut World| {
                duplicate_entities(world, &entities);
            });
        }
    }

    // Duplicate & Move (Alt+D) — duplicate then enter grab mode
    if keybindings.just_pressed(EditorAction::DuplicateAndMove, &keyboard) {
        let entities = selection.get_all();
        if !entities.is_empty() {
            commands.queue(move |world: &mut World| {
                duplicate_entities(world, &entities);
            });
            commands.insert_resource(PendingModalGrab);
        }
    }
}

/// Deep-clone each selected entity (all components, via Bevy's
/// `EntityWorldMut::clone_and_spawn`) and replace the selection with the
/// new copies. The suffix " (Copy)" is appended to the `Name` so
/// duplicates are distinguishable in the hierarchy.
fn duplicate_entities(world: &mut World, sources: &[Entity]) {
    let mut new_ids: Vec<Entity> = Vec::with_capacity(sources.len());
    for src in sources {
        let Ok(mut src_mut) = world.get_entity_mut(*src) else {
            continue;
        };
        let new = src_mut.clone_and_spawn();
        // Append " (Copy)" to the cloned entity's Name.
        if let Some(original) = world.get::<Name>(new).map(|n| n.as_str().to_string()) {
            if let Ok(mut ent) = world.get_entity_mut(new) {
                ent.insert(Name::new(format!("{} (Copy)", original)));
            }
        }
        new_ids.push(new);
    }
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.clear();
        for e in &new_ids {
            sel.toggle(*e);
        }
    }
}

/// Shift `entities` so the group's XZ centroid lands at `target.x/z` and
/// the lowest point of the group's world-space AABB sits at `target.y`
/// (i.e. the floor). Preserves relative layout within the group.
fn reposition_paste_group(world: &mut World, entities: &[Entity], target: Vec3) {
    use bevy::camera::primitives::Aabb;

    // Centroid on XZ (where the cursor is).
    let mut centroid_xz = Vec2::ZERO;
    let mut count = 0u32;
    for e in entities {
        if let Some(t) = world.get::<Transform>(*e) {
            centroid_xz += Vec2::new(t.translation.x, t.translation.z);
            count += 1;
        }
    }
    if count == 0 {
        return;
    }
    centroid_xz /= count as f32;

    // Lowest world-space AABB bottom across the group. Mesh entities
    // carry `Aabb` in local space; transform into world space to get the
    // bottom y. Non-mesh entities fall back to their translation.y.
    let mut min_y = f32::INFINITY;
    for e in entities {
        let t_y = world.get::<Transform>(*e).map(|t| t.translation.y);
        let bottom = if let (Some(aabb), Some(gt)) =
            (world.get::<Aabb>(*e), world.get::<GlobalTransform>(*e))
        {
            world_space_min_y(aabb, gt)
        } else {
            t_y.unwrap_or(f32::INFINITY)
        };
        if bottom < min_y {
            min_y = bottom;
        }
    }
    if !min_y.is_finite() {
        // Nothing with a position — nothing to do.
        return;
    }

    let delta = Vec3::new(
        target.x - centroid_xz.x,
        target.y - min_y,
        target.z - centroid_xz.y,
    );
    for e in entities {
        if let Ok(mut ent) = world.get_entity_mut(*e) {
            if let Some(mut t) = ent.get_mut::<Transform>() {
                t.translation += delta;
            }
        }
    }
}

/// Transform the 8 corners of a local-space AABB by `gt` and return the
/// minimum world-space y — the lowest point of the mesh as it currently
/// sits in the world.
fn world_space_min_y(aabb: &bevy::camera::primitives::Aabb, gt: &GlobalTransform) -> f32 {
    let c = Vec3::from(aabb.center);
    let h = Vec3::from(aabb.half_extents);
    let mut min_y = f32::INFINITY;
    for dx in [-1.0_f32, 1.0] {
        for dy in [-1.0_f32, 1.0] {
            for dz in [-1.0_f32, 1.0] {
                let local = c + Vec3::new(dx * h.x, dy * h.y, dz * h.z);
                let world = gt.transform_point(local);
                if world.y < min_y {
                    min_y = world.y;
                }
            }
        }
    }
    min_y
}

/// Project the window cursor onto the ground plane (y=0) through the
/// editor camera. Returns `None` if the cursor isn't over the viewport,
/// the ray misses the ground plane, or any required resource is missing —
/// callers fall back to pasting at the source's original position.
fn compute_paste_target(world: &mut World) -> Option<Vec3> {
    // Read viewport fields into locals so the immutable borrow is dropped
    // before we use `world.query_filtered` (which needs a mutable borrow).
    let (vp_min, vp_size, current_size, hovered) = {
        let vp = world.get_resource::<ViewportState>()?;
        (
            vp.screen_position,
            vp.screen_size,
            vp.current_size,
            vp.hovered,
        )
    };
    if !hovered {
        return None;
    }

    let cursor = {
        let mut window_q = world.query_filtered::<&Window, With<PrimaryWindow>>();
        let window = window_q.single(world).ok()?;
        window.cursor_position()?
    };

    if cursor.x < vp_min.x
        || cursor.y < vp_min.y
        || cursor.x > vp_min.x + vp_size.x
        || cursor.y > vp_min.y + vp_size.y
    {
        return None;
    }

    let ray = {
        let mut cam_q = world.query_filtered::<(&Camera, &GlobalTransform), With<EditorCamera>>();
        let (camera, cam_xform) = cam_q.single(world).ok()?;
        let viewport_pos = Vec2::new(
            (cursor.x - vp_min.x) / vp_size.x * current_size.x as f32,
            (cursor.y - vp_min.y) / vp_size.y * current_size.y as f32,
        );
        camera.viewport_to_world(cam_xform, viewport_pos).ok()?
    };

    let dir = ray.direction.as_vec3();
    if dir.y.abs() <= 1e-6 {
        return None;
    }
    let t = -ray.origin.y / dir.y;
    if t <= 0.0 || t > 10_000.0 {
        return None;
    }
    let hit = ray.origin + dir * t;
    Some(Vec3::new(hit.x, 0.0, hit.z))
}

/// Handle file & edit keyboard shortcuts (save, open, settings, etc.).
pub(crate) fn handle_file_shortcuts(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mut modal: ResMut<modal_transform::ModalTransformState>,
    pending_grab: Option<Res<PendingModalGrab>>,
) {
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if modal.active {
        return;
    }

    // Consume pending grab from duplicate-and-move
    if pending_grab.is_some() {
        commands.remove_resource::<PendingModalGrab>();
        modal.pending_grab = true;
    }

    // Save (Ctrl+S)
    if keybindings.just_pressed(EditorAction::SaveScene, &keyboard) {
        commands.insert_resource(renzora::core::SaveSceneRequested);
    }

    // Save As (Ctrl+Shift+S)
    if keybindings.just_pressed(EditorAction::SaveSceneAs, &keyboard) {
        commands.insert_resource(renzora::core::SaveAsSceneRequested);
    }

    // Open Scene (Ctrl+O)
    if keybindings.just_pressed(EditorAction::OpenScene, &keyboard) {
        commands.insert_resource(renzora::core::OpenSceneRequested);
    }

    // New Scene (Ctrl+N)
    if keybindings.just_pressed(EditorAction::NewScene, &keyboard) {
        commands.insert_resource(renzora::core::NewSceneRequested);
    }

    // Settings (Ctrl+,)
    if keybindings.just_pressed(EditorAction::OpenSettings, &keyboard) {
        commands.insert_resource(renzora::core::ToggleSettingsRequested);
    }
}

// ── Mode switching ──────────────────────────────────────────────────────────

pub(crate) fn switch_gizmo_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<InputFocusState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    modal: Res<modal_transform::ModalTransformState>,
    mut mode: ResMut<GizmoMode>,
    mut active_tool: ResMut<renzora_editor_framework::ActiveTool>,
) {
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }
    if modal.active {
        return;
    }
    if keybindings.just_pressed(EditorAction::ToolSelect, &keyboard) {
        *mode = GizmoMode::Select;
        *active_tool = renzora_editor_framework::ActiveTool::Select;
    }
    if keybindings.just_pressed(EditorAction::GizmoTranslate, &keyboard) {
        *mode = GizmoMode::Translate;
        *active_tool = renzora_editor_framework::ActiveTool::Translate;
    }
    if keybindings.just_pressed(EditorAction::GizmoRotate, &keyboard) {
        *mode = GizmoMode::Rotate;
        *active_tool = renzora_editor_framework::ActiveTool::Rotate;
    }
    if keybindings.just_pressed(EditorAction::GizmoScale, &keyboard) {
        *mode = GizmoMode::Scale;
        *active_tool = renzora_editor_framework::ActiveTool::Scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::primitives::Aabb;
    use std::f32::consts::FRAC_PI_2;

    #[test]
    fn world_space_min_y_handles_rotation() {
        // Half-extents (2,1,1): rotating 90° about Z swings the ±2 X extent
        // onto the Y axis, so the lowest corner sits at y = -2.
        let aabb = Aabb::from_min_max(Vec3::new(-2.0, -1.0, -1.0), Vec3::new(2.0, 1.0, 1.0));
        let gt = GlobalTransform::from(
            Transform::from_translation(Vec3::new(0.0, 5.0, 0.0))
                .with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
        );
        let min_y = world_space_min_y(&aabb, &gt);
        assert!((min_y - 3.0).abs() < 1e-4, "got {min_y}");
    }
}
