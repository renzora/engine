//! Custom window chrome for borderless Bevy windows.
//!
//! Provides:
//! - [`WindowAction`] — a result emitted by UI that the Bevy plugin consumes.
//! - [`WindowChromePlugin`] — applies emitted [`WindowAction`]s to the
//!   primary Bevy [`Window`] and owns the maximize-state mirror.

use bevy::app::AppExit;
use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryWindow, Window};

/// Queue of window actions emitted during UI rendering.
/// Drained by [`apply_window_actions`] every frame.
#[derive(Resource, Default)]
pub struct WindowActionQueue {
    actions: Vec<WindowAction>,
    /// Mirror of the window's maximized state (winit doesn't expose a getter).
    pub maximized: bool,
}

impl WindowActionQueue {
    pub fn push(&mut self, action: WindowAction) {
        if !matches!(action, WindowAction::None) {
            self.actions.push(action);
        }
    }
}

/// Request emitted by window-chrome widgets.
#[derive(Clone, Copy)]
pub enum WindowAction {
    None,
    Minimize,
    ToggleMaximize,
    Close,
    StartDrag,
    StartResize(CompassOctant),
}

/// Register the chrome plugin — call this once on your editor App.
pub struct WindowChromePlugin;

impl Plugin for WindowChromePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindowActionQueue>()
            .add_systems(Update, init_maximized_state)
            .add_systems(Last, apply_window_actions);
    }
}

fn apply_window_actions(
    mut queue: ResMut<WindowActionQueue>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    if queue.actions.is_empty() {
        return;
    }
    let actions: Vec<_> = queue.actions.drain(..).collect();
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    for action in actions {
        match action {
            WindowAction::None => {}
            WindowAction::Minimize => window.set_minimized(true),
            WindowAction::ToggleMaximize => {
                // winit's maximize targets the monitor work area (taskbar stays
                // visible) and follows the window's current monitor.
                queue.maximized = !queue.maximized;
                window.set_maximized(queue.maximized);
            }
            WindowAction::Close => {
                exit.write(AppExit::Success);
            }
            WindowAction::StartDrag => {
                // Standard OS behaviour: dragging the title bar of a maximized
                // window restores it first (winit does this under the cursor, on
                // the same monitor), then starts the OS drag.
                if queue.maximized {
                    queue.maximized = false;
                    window.set_maximized(false);
                }
                window.start_drag_move();
            }
            WindowAction::StartResize(dir) => window.start_drag_resize(dir),
        }
    }
}

/// Seed the maximized mirror once at startup: if the window already fills a
/// monitor, treat it as maximized (so the control's icon + first click are
/// correct). Bevy doesn't expose winit's maximized getter, so we infer it.
fn init_maximized_state(
    mut done: Local<bool>,
    mut queue: ResMut<WindowActionQueue>,
    windows: Query<&Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor>,
) {
    if *done {
        return;
    }
    let Ok(window) = windows.single() else { return };
    if monitors.iter().next().is_none() {
        return; // wait until monitors are enumerated
    }
    *done = true;
    let w = window.resolution.physical_width() as i32;
    let h = window.resolution.physical_height() as i32;
    queue.maximized = monitors.iter().any(|m| {
        (w - m.physical_width as i32).abs() < 48 && (h - m.physical_height as i32).abs() < 48
    });
}

