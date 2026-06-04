//! Canvas navigation: middle-drag to pan, mouse-wheel to zoom. Pan is stored in
//! `NativeCanvasState.pan` and applied to the design frame's `UiTransform`.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{RelativeCursorPosition, UiTransform, Val2};
use bevy::window::PrimaryWindow;

use renzora_editor::SplashState;

use crate::overlay::CanvasHitLayer;
use crate::NativeCanvasState;

/// The zoomed/panned design frame — its `UiTransform.translation` carries the pan.
#[derive(Component)]
pub(crate) struct CanvasFrame;

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, (canvas_pan, canvas_zoom, apply_pan).run_if(in_state(SplashState::Editor)));
}

fn cursor(windows: &Query<&Window, With<PrimaryWindow>>) -> Option<Vec2> {
    windows.iter().next().and_then(|w| w.cursor_position())
}

/// Middle-drag (started over the canvas) pans the view.
fn canvas_pan(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut last: Local<Option<Vec2>>,
    hit: Query<&RelativeCursorPosition, With<CanvasHitLayer>>,
    mut state: ResMut<NativeCanvasState>,
) {
    if !mouse.pressed(MouseButton::Middle) {
        *last = None;
        return;
    }
    let Some(c) = cursor(&windows) else { return };
    if mouse.just_pressed(MouseButton::Middle) {
        // Only begin a pan if the press landed over the canvas.
        if hit.iter().any(|r| r.cursor_over) {
            *last = Some(c);
        }
        return;
    }
    if let Some(prev) = *last {
        state.pan += c - prev;
        *last = Some(c);
    }
}

/// Mouse-wheel over the canvas zooms, keeping the point under the cursor fixed.
fn canvas_zoom(mut wheel: MessageReader<MouseWheel>, hit: Query<&RelativeCursorPosition, With<CanvasHitLayer>>, mut state: ResMut<NativeCanvasState>) {
    let mut scroll = 0.0;
    for ev in wheel.read() {
        scroll += ev.y;
    }
    if scroll == 0.0 {
        return;
    }
    let Some(norm) = hit.iter().find_map(|r| r.cursor_over.then_some(r.normalized).flatten()) else {
        return;
    };
    let old = state.zoom;
    let new = (old * (1.0 + scroll * 0.1)).clamp(0.1, 8.0);
    // Design-space point under the cursor; offset pan so it stays on screen.
    // The frame is centered, so its center is the pan origin: a point d shifts by
    // (old - new) * (d - reference/2) when the zoom changes.
    let (cw, ch) = (state.canvas_width, state.canvas_height);
    let dx = (norm.x + 0.5) * cw;
    let dy = (norm.y + 0.5) * ch;
    state.pan.x += (old - new) * (dx - cw * 0.5);
    state.pan.y += (old - new) * (dy - ch * 0.5);
    state.zoom = new;
}

fn apply_pan(state: Res<NativeCanvasState>, mut q: Query<&mut UiTransform, With<CanvasFrame>>) {
    for mut tf in &mut q {
        let want = Val2::px(state.pan.x, state.pan.y);
        if tf.translation != want {
            tf.translation = want;
        }
    }
}
