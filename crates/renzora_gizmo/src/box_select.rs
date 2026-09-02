//! Marquee selection, and the click/drag decision it shares with `picking.rs`.
//!
//! [`entity_pick_system`](crate::picking::entity_pick_system) arms the gesture
//! on press; this finalises it on release. Under the drag threshold it commits
//! the pending pick (or deselects on empty space); over it, everything whose
//! origin projects inside the rectangle is selected.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{NavOverlayState, ViewportState};
use renzora_editor_framework::{EditorCamera, EditorSelection, HideInHierarchy};

use crate::types::{BoxSelectionState, GizmoMesh, GizmoRoot};

pub(crate) fn box_selection_system(
    mut box_sel: ResMut<BoxSelectionState>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    collider_edit: Option<Res<renzora_physics::ColliderEditMode>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    viewport: Option<Res<ViewportState>>,
    nav_overlay: Option<Res<NavOverlayState>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    selection: Res<EditorSelection>,
    named_entities: Query<(Entity, &GlobalTransform), With<Name>>,
    hidden_entities: Query<(), With<HideInHierarchy>>,
    gizmo_meshes: Query<(), Or<(With<GizmoMesh>, With<GizmoRoot>)>>,
    box_select_excluded: Query<
        (),
        Or<(
            With<renzora_terrain::data::TerrainData>,
            With<renzora_terrain::data::TerrainChunkOf>,
            With<renzora_lighting::Sun>,
        )>,
    >,
) {
    if collider_edit.map(|c| c.active).unwrap_or(false) {
        box_sel.active = false;
        return;
    }
    if !box_sel.active {
        return;
    }
    // Cancel box selection if nav overlay is being used
    if let Some(ref nav) = nav_overlay {
        if nav.pan_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav.zoom_dragging.load(std::sync::atomic::Ordering::Relaxed)
            || nav
                .orbit_dragging
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            box_sel.active = false;
            return;
        }
    }

    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    // Update current position while dragging
    if mouse_button.pressed(MouseButton::Left) {
        box_sel.current_pos = cursor;
        return;
    }

    // Mouse released — finalize gesture.
    box_sel.active = false;
    let pending_pick = box_sel.pending_pick.take();

    let ctrl = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    if !box_sel.is_drag() {
        // Click (no drag): commit the pending pick, or deselect on empty space.
        match pending_pick {
            Some(target) => {
                if ctrl {
                    selection.toggle(target);
                } else if shift {
                    if !selection.is_selected(target) {
                        selection.toggle(target);
                    }
                } else {
                    selection.set(Some(target));
                }
            }
            None => {
                if !shift && !ctrl {
                    selection.set(None);
                }
            }
        }
        return;
    }

    let Some(viewport) = viewport.as_ref() else {
        return;
    };
    let Ok((camera, cam_gt)) = camera_q.single() else {
        return;
    };

    let (box_min, box_max) = box_sel.get_rect();

    // Find all named entities whose screen projection falls within the box
    let mut entities_in_box = Vec::new();

    for (entity, global_transform) in named_entities.iter() {
        if hidden_entities.get(entity).is_ok() {
            continue;
        }
        if gizmo_meshes.get(entity).is_ok() {
            continue;
        }
        if box_select_excluded.get(entity).is_ok() {
            continue;
        }

        let world_pos = global_transform.translation();
        let Some(ndc) = camera.world_to_ndc(cam_gt, world_pos) else {
            continue;
        };

        // Must be in front of camera
        if ndc.z < 0.0 || ndc.z > 1.0 {
            continue;
        }

        // Convert NDC to screen coordinates
        let screen_x = viewport.screen_position.x + (ndc.x + 1.0) * 0.5 * viewport.screen_size.x;
        let screen_y = viewport.screen_position.y + (1.0 - ndc.y) * 0.5 * viewport.screen_size.y;

        if screen_x >= box_min.x
            && screen_x <= box_max.x
            && screen_y >= box_min.y
            && screen_y <= box_max.y
        {
            entities_in_box.push(entity);
        }
    }

    if entities_in_box.is_empty() {
        if !shift && !ctrl {
            selection.set(None);
        }
        return;
    }

    if shift {
        // Add to existing selection
        let mut current = selection.get_all();
        for e in entities_in_box {
            if !current.contains(&e) {
                current.push(e);
            }
        }
        selection.set_multiple(current);
    } else if ctrl {
        // Toggle each entity
        for e in entities_in_box {
            selection.toggle(e);
        }
    } else {
        // Replace selection
        selection.set_multiple(entities_in_box);
    }
}

// ── Box selection overlay ────────────────────────────────────────────────────

/// Marker for the native bevy_ui box-selection rectangle node.
#[derive(Component)]
pub(crate) struct BoxSelectionRect;

/// Native (bevy_ui) box-selection overlay — a translucent blue rectangle node
/// sized to the drag rect. Replaces the egui-painted version. `get_rect`
/// returns window logical pixels, which map directly to an absolute UI node.
///
/// The node is `Pickable::IGNORE` + `FocusPolicy::Pass` so it never occludes the
/// viewport's hover/pick (the drag itself is driven by `box_selection_system`
/// reading the raw cursor, not UI interaction).
pub(crate) fn render_box_selection(
    mut commands: Commands,
    box_sel: Res<BoxSelectionState>,
    mut existing: Query<(Entity, &mut Node), With<BoxSelectionRect>>,
) {
    if !box_sel.active || !box_sel.is_drag() {
        for (e, _) in &existing {
            commands.entity(e).despawn();
        }
        return;
    }

    let (min, max) = box_sel.get_rect();
    let w = (max.x - min.x).max(0.0);
    let h = (max.y - min.y).max(0.0);

    if let Some((_, mut node)) = existing.iter_mut().next() {
        node.left = Val::Px(min.x);
        node.top = Val::Px(min.y);
        node.width = Val::Px(w);
        node.height = Val::Px(h);
    } else {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(min.x),
                top: Val::Px(min.y),
                width: Val::Px(w),
                height: Val::Px(h),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(66.0 / 255.0, 150.0 / 255.0, 250.0 / 255.0, 0.157)),
            BorderColor::all(Color::srgb_u8(66, 150, 250)),
            GlobalZIndex(8000),
            bevy::ui::FocusPolicy::Pass,
            bevy::picking::Pickable::IGNORE,
            BoxSelectionRect,
            Name::new("box-selection-rect"),
        ));
    }
}
