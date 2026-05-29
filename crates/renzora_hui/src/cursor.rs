//! Custom cursor as a UI component.
//!
//! Any markup entity named `cursor_follow` (via `name="cursor_follow"`) is
//! tracked by [`cursor_follow_system`] each frame: its `Node.left/top` are
//! driven from the primary window's cursor position. While at least one such
//! entity exists, the OS cursor is hidden so the markup cursor is the only
//! one visible.
//!
//! This makes the cursor authorable like any other UI — drop `<cursor />`
//! (the bundled component template) into any markup and you get one. Use any
//! visuals you like: an `<image src="...">`, a styled `<node>`, a composed
//! component template, etc. — just give the root `name="cursor_follow"`.

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, GlobalZIndex, UiScale};
use bevy::window::{CursorOptions, PrimaryWindow};

/// Markup name that gets followed by the cursor system. Any entity with
/// `Name(CURSOR_NAME)` has its `Node.left/top` driven from the cursor pos.
const CURSOR_NAME: &str = "cursor_follow";

fn cursor_follow_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cursor_opts: Query<&mut CursorOptions, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut cursors: Query<
        (
            Entity,
            &mut Node,
            &Name,
            Option<&FocusPolicy>,
            Option<&GlobalZIndex>,
        ),
    >,
    mut commands: Commands,
) {
    let Ok(window) = windows.single() else { return };

    // Count + reposition in one pass. Also fixes up `FocusPolicy::Pass` (so
    // clicks pass through to the buttons underneath) and `GlobalZIndex` (so
    // the cursor always paints on top regardless of hierarchy order, which
    // bevy_ui's sibling ordering can otherwise scramble for absolute children
    // intermixed with relative ones). Both inserts are idempotent.
    //
    // `Val::Px` on a UI node is in *design pixels* — bevy_ui multiplies by
    // `UiScale` at render time. `window.cursor_position()` returns the cursor
    // in already-scaled render pixels, so we have to divide by `UiScale` to
    // bring it back into design space. Otherwise the cursor visual drifts
    // away from where buttons think the mouse is whenever the project sets
    // a custom render-target size (renzora_game_ui drives `UiScale` to make
    // design pixels × scale == render pixels).
    let scale = ui_scale.0.max(f32::EPSILON);
    let mut any_cursor_entity = false;
    if let Some(pos) = window.cursor_position() {
        let design_x = pos.x / scale;
        let design_y = pos.y / scale;
        for (entity, mut node, name, focus, zindex) in &mut cursors {
            if name.as_str() == CURSOR_NAME {
                node.left = Val::Px(design_x);
                node.top = Val::Px(design_y);
                node.position_type = PositionType::Absolute;
                if focus.copied() != Some(FocusPolicy::Pass) {
                    commands.entity(entity).insert(FocusPolicy::Pass);
                }
                if zindex.is_none() {
                    commands.entity(entity).insert(GlobalZIndex(i32::MAX));
                }
                any_cursor_entity = true;
            }
        }
    } else {
        // Cursor outside the window — don't move the entity, but still note
        // whether one exists for the OS-cursor toggle below.
        for (_, _, name, _, _) in &cursors {
            if name.as_str() == CURSOR_NAME {
                any_cursor_entity = true;
                break;
            }
        }
    }

    if let Ok(mut opts) = cursor_opts.single_mut() {
        let want_visible = !any_cursor_entity;
        if opts.visible != want_visible {
            opts.visible = want_visible;
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, cursor_follow_system);
}
