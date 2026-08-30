//! Canvas navigation: middle/right-drag to pan, modifier + wheel to scroll, and
//! plain wheel to zoom. Pan is stored in `NativeCanvasState.pan` and applied to
//! the design frame's `UiTransform`.
//!
//! Wheel scheme (matches common 2D editors): **plain** wheel zooms toward the
//! cursor, **Shift**+wheel scrolls horizontally, **Ctrl**+wheel scrolls
//! vertically.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{RelativeCursorPosition, UiTransform, Val2};
use bevy::window::PrimaryWindow;

use renzora::SplashState;

use crate::game_ui::overlay::CanvasHitLayer;
use crate::game_ui::NativeCanvasState;

/// Screen pixels panned per wheel notch when scrolling with a modifier held.
const WHEEL_PAN_STEP: f32 = 60.0;

/// The zoomed/panned design frame — its `UiTransform.translation` carries the pan.
#[derive(Component)]
pub(crate) struct CanvasFrame;

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        // Chained: the input systems write `pan`/`zoom` and `apply_view` reads
        // them, so a scroll shows up on the same frame it happened rather than
        // the next one.
        (canvas_pan, canvas_wheel, apply_view)
            .chain()
            .run_if(in_state(SplashState::Editor))
            // Only while the canvas panel is actually mounted — otherwise the
            // wheel / right-drag would pan an unseen canvas from the 3D viewport.
            .run_if(any_with_component::<CanvasHitLayer>),
    );
}

fn cursor(windows: &Query<&Window, With<PrimaryWindow>>) -> Option<Vec2> {
    windows.iter().next().and_then(|w| w.cursor_position())
}

/// Middle- or right-drag (started over the canvas viewport) pans the view.
fn canvas_pan(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut last: Local<Option<Vec2>>,
    hit: Query<&RelativeCursorPosition, With<CanvasHitLayer>>,
    background: Query<&Interaction, With<crate::game_ui::interaction::CanvasBackground>>,
    mut state: ResMut<NativeCanvasState>,
) {
    let panning = mouse.pressed(MouseButton::Middle) || mouse.pressed(MouseButton::Right);
    if !panning {
        *last = None;
        return;
    }
    let Some(c) = cursor(&windows) else { return };
    if mouse.just_pressed(MouseButton::Middle) || mouse.just_pressed(MouseButton::Right) {
        // Begin a pan if the press landed anywhere over the canvas viewport —
        // the design frame itself or the dark area around it — but not on some
        // other editor panel.
        let over_frame = hit.iter().any(|r| r.cursor_over);
        let over_bg = background.iter().any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
        if over_frame || over_bg {
            *last = Some(c);
        }
        return;
    }
    if let Some(prev) = *last {
        state.pan += c - prev;
        *last = Some(c);
    }
}

/// Mouse-wheel over the canvas: plain = zoom toward the cursor, Shift = scroll
/// horizontally, Ctrl = scroll vertically. Works anywhere over the canvas
/// viewport — the design frame or the dark area around it.
fn canvas_wheel(
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    hit: Query<&RelativeCursorPosition, With<CanvasHitLayer>>,
    background: Query<&Interaction, With<crate::game_ui::interaction::CanvasBackground>>,
    mut state: ResMut<NativeCanvasState>,
) {
    let mut scroll = 0.0;
    for ev in wheel.read() {
        scroll += ev.y;
    }
    if scroll == 0.0 {
        return;
    }
    // Only over the canvas viewport (the frame or the background around it) —
    // not some other editor panel.
    let over_frame = hit.iter().any(|r| r.cursor_over);
    let over_bg = background.iter().any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
    if !over_frame && !over_bg {
        return;
    }

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if shift {
        state.pan.x += scroll * WHEEL_PAN_STEP;
        return;
    }
    if ctrl {
        state.pan.y += scroll * WHEEL_PAN_STEP;
        return;
    }

    // Plain wheel → zoom, keeping the point under the cursor fixed. `normalized`
    // stays valid (running past ±0.5) even when the cursor is over the
    // background, so the zoom still anchors to the cursor out there.
    let Some(norm) = hit.iter().find_map(|r| r.normalized) else {
        return;
    };
    let old = state.zoom;
    let new = (old * (1.0 + scroll * 0.1)).clamp(0.1, 8.0);
    // The frame is centered, so its center is the pan origin: a point d shifts by
    // (old - new) * (d - reference/2) when the zoom changes.
    let (cw, ch) = (state.canvas_width, state.canvas_height);
    let dx = (norm.x + 0.5) * cw;
    let dy = (norm.y + 0.5) * ch;
    state.pan.x += (old - new) * (dx - cw * 0.5);
    state.pan.y += (old - new) * (dy - ch * 0.5);
    state.zoom = new;
}

/// Push the whole view — frame size from `zoom`, offset from `pan` — onto the
/// design frame.
///
/// Both halves are written here, together, because a zoom moves the frame *and*
/// resizes it and the two have to land on the same frame to look like one
/// gesture. The size used to come from a `bind_with` on the frame's `Node` while
/// the offset came from this system, and nothing ordered `run_reactions` against
/// it: whichever ran first that tick decided whether you saw the new pan against
/// the old size. Since the winner could change from frame to frame, a scroll
/// produced a lateral twitch that snapped back — the canvas appearing to move in
/// x and y before settling.
///
/// The `!=` guards are load-bearing. Reading a field through `Mut` does not mark
/// the component changed, but assigning does, and dirtying `Node` every frame
/// would re-run layout for the whole tree.
fn apply_view(
    state: Res<NativeCanvasState>,
    mut q: Query<(&mut Node, &mut UiTransform), With<CanvasFrame>>,
) {
    let w = Val::Px(state.canvas_width * state.zoom);
    let h = Val::Px(state.canvas_height * state.zoom);
    let want = Val2::px(state.pan.x, state.pan.y);
    for (mut node, mut tf) in &mut q {
        if node.width != w {
            node.width = w;
        }
        if node.height != h {
            node.height = h;
        }
        if tf.translation != want {
            tf.translation = want;
        }
    }
}
