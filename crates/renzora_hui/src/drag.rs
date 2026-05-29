//! Runtime drag for markup nodes tagged `draggable="true"`.
//!
//! The loader inserts a [`Draggable`] marker when a markup node has
//! `tag["draggable"]` set to anything other than `"false"`. This module owns
//! the bevy_ui-side: it watches `Interaction::Pressed` on draggable entities,
//! captures the cursor offset on press, and drives `Node.left/top` while held.
//!
//! When a drag starts the entity is forced to `PositionType::Absolute` so the
//! `left/top` we write actually moves it (a relative-flex child would just be
//! re-laid-out by its parent every frame). For the first drag this pops the
//! element out of layout flow — siblings shift. Subsequent drags just update
//! the absolute coords.
//!
//! Drag end keys off mouse-button release, not `Interaction`, so dragging
//! beyond the original entity bounds still works.

use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;

/// Marker: this entity follows the mouse while LMB is held after a press
/// originating on it.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct Draggable;

/// Per-entity drag bookkeeping. Lives only while a drag is in progress; the
/// system removes it on mouse-up.
#[derive(Component, Debug, Clone, Copy)]
struct DragState {
    /// Cursor pos at the moment we started the drag (window pixels).
    start_cursor: Vec2,
    /// `Node.left/top` at the moment we started the drag, resolved to pixels
    /// (we only support `Val::Px` for the dragged coords; anything else snaps
    /// to 0 as the baseline so the cursor delta still works).
    start_left_px: f32,
    start_top_px: f32,
}

fn drag_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    interactions: Query<(Entity, &Interaction, &Node), (With<Draggable>, Without<DragState>)>,
    mut dragging: Query<(Entity, &mut Node, &DragState), With<Draggable>>,
    mut commands: Commands,
) {
    let Ok(window) = windows.single() else { return };
    let cursor = window.cursor_position();

    // Start: LMB just went down on a draggable while it was Pressed/Hovered.
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(pos) = cursor {
            for (entity, interaction, node) in &interactions {
                if matches!(interaction, Interaction::Pressed | Interaction::Hovered) {
                    let l = match node.left {
                        Val::Px(v) => v,
                        _ => 0.0,
                    };
                    let t = match node.top {
                        Val::Px(v) => v,
                        _ => 0.0,
                    };
                    commands.entity(entity).insert(DragState {
                        start_cursor: pos,
                        start_left_px: l,
                        start_top_px: t,
                    });
                    // Make sure decorative overlays don't catch the drag — see
                    // `cursor.rs` for the same trick on the custom cursor.
                    commands.entity(entity).insert(FocusPolicy::Block);
                }
            }
        }
    }

    // Continue: while we have a DragState, keep writing cursor delta.
    if let Some(pos) = cursor {
        for (_, mut node, drag) in &mut dragging {
            let delta = pos - drag.start_cursor;
            node.left = Val::Px(drag.start_left_px + delta.x);
            node.top = Val::Px(drag.start_top_px + delta.y);
            node.position_type = PositionType::Absolute;
        }
    }

    // End: LMB released → drop DragState on every entity that had one.
    if mouse.just_released(MouseButton::Left) {
        for (entity, _, _) in &dragging {
            commands.entity(entity).remove::<DragState>();
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, drag_system);
}
