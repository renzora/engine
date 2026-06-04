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

/// Mouse-wheel over the canvas zooms.
fn canvas_zoom(mut wheel: MessageReader<MouseWheel>, hit: Query<&RelativeCursorPosition, With<CanvasHitLayer>>, mut state: ResMut<NativeCanvasState>) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy == 0.0 || !hit.iter().any(|r| r.cursor_over) {
        return;
    }
    state.zoom = (state.zoom * (1.0 + dy * 0.1)).clamp(0.1, 8.0);
}

fn apply_pan(state: Res<NativeCanvasState>, mut q: Query<&mut UiTransform, With<CanvasFrame>>) {
    for mut tf in &mut q {
        let want = Val2::px(state.pan.x, state.pan.y);
        if tf.translation != want {
            tf.translation = want;
        }
    }
}
