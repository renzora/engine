//! Custom window chrome for borderless Bevy+egui windows.
//!
//! Provides:
//! - [`WindowAction`] — a result emitted by UI that the Bevy plugin consumes.
//! - [`render_window_controls`] — min/max/close phosphor buttons for a titlebar.
//! - [`render_drag_handle`] — a region that initiates window-drag on mousedown.
//! - [`render_resize_zones`] — grippable invisible hit zones around the
//!   window perimeter that initiate edge/corner resize.
//! - [`WindowChromePlugin`] — applies emitted [`WindowAction`]s to the
//!   primary Bevy [`Window`] and owns the maximize-state mirror.

use bevy::app::AppExit;
use bevy::math::CompassOctant;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, Pos2, Sense, Stroke, StrokeKind, Vec2};
use egui_phosphor::regular as icons;

/// Queue of window actions emitted during UI rendering.
/// Drained by [`apply_window_actions`] every frame.
#[derive(Resource, Default)]
pub struct WindowActionQueue {
    actions: Vec<WindowAction>,
    /// Mirror of the primary window's maximized state (Bevy doesn't expose a getter).
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
    let Ok(mut window) = windows.single_mut() else { return };
    for action in actions {
        match action {
            WindowAction::None => {}
            WindowAction::Minimize => window.set_minimized(true),
            WindowAction::ToggleMaximize => {
                queue.maximized = !queue.maximized;
                window.set_maximized(queue.maximized);
            }
            WindowAction::Close => {
                exit.write(AppExit::Success);
            }
            WindowAction::StartDrag => {
                // Standard OS behaviour: dragging the title bar of a maximized
                // window restores it first, then starts the drag.
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

/// Listens for a title-bar drag without registering an interactive widget.
///
/// Using an egui widget here would compete with the MenuBar's `menu_button`s
/// for hover detection and break egui's built-in "hover-switch between menus
/// while one is open" behaviour. Instead we read pointer events directly and
/// only act when **no** other egui widget wants the pointer at that position
/// — meaning the cursor is genuinely over empty title-bar space.
pub fn render_drag_handle(ui: &mut egui::Ui, rect: egui::Rect, queue: &mut WindowActionQueue) {
    let ctx = ui.ctx();
    let pointer_pos = ctx.pointer_latest_pos();
    let pointer_in_rect = pointer_pos.map(|p| rect.contains(p)).unwrap_or(false);
    // If any egui widget wants the pointer (menu button, tab, icon button),
    // we stay out of its way — clicks and hovers go to the widget.
    let widget_wants = ctx.wants_pointer_input() || ctx.is_using_pointer();

    if pointer_in_rect && !widget_wants {
        ctx.set_cursor_icon(CursorIcon::Grab);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            queue.push(WindowAction::StartDrag);
        }
    }
}

/// Renders the three window control buttons (minimize, toggle maximize, close)
/// right-aligned inside `rect`. Returns the rect left unused on the left.
pub fn render_window_controls(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    is_maximized: bool,
    queue: &mut WindowActionQueue,
) -> egui::Rect {
    let btn_w = 40.0_f32;
    let btn_size = Vec2::new(btn_w, rect.height());
    let close_rect = egui::Rect::from_min_size(
        Pos2::new(rect.right() - btn_w, rect.top()),
        btn_size,
    );
    let max_rect = egui::Rect::from_min_size(
        Pos2::new(rect.right() - btn_w * 2.0, rect.top()),
        btn_size,
    );
    let min_rect = egui::Rect::from_min_size(
        Pos2::new(rect.right() - btn_w * 3.0, rect.top()),
        btn_size,
    );

    let max_icon = if is_maximized { icons::ARROWS_IN_SIMPLE } else { icons::SQUARE };
    if window_button(ui, min_rect, "renzora_win_min", icons::MINUS, false) {
        queue.push(WindowAction::Minimize);
    }
    if window_button(ui, max_rect, "renzora_win_max", max_icon, false) {
        queue.push(WindowAction::ToggleMaximize);
    }
    if window_button(ui, close_rect, "renzora_win_close", icons::X, true) {
        queue.push(WindowAction::Close);
    }

    egui::Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width() - btn_w * 3.0, rect.height()),
    )
}

fn window_button(ui: &mut egui::Ui, rect: egui::Rect, id_src: &str, icon: &str, is_close: bool) -> bool {
    // Use `interact` (no cursor advance) so we can overlay on top of a
    // flow-layout parent (like the MenuBar in the editor's title bar).
    let id = ui.id().with(id_src);
    let resp = ui.interact(rect, id, Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let bg = if hovered {
        if is_close {
            Color32::from_rgb(232, 17, 35)
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 34)
        }
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        Color32::WHITE,
    );
    resp.clicked()
}

/// Grippable resize zones overlaid at the outer `screen_rect` edges.
/// Dimensions are generous (18 px corners, 10 px edges) so users don't have
/// to pixel-hunt. Skipped when the window is maximized.
pub fn render_resize_zones(
    ui: &mut egui::Ui,
    screen_rect: egui::Rect,
    is_maximized: bool,
    queue: &mut WindowActionQueue,
) {
    if is_maximized {
        return;
    }

    let t: f32 = 10.0;
    let c: f32 = 18.0;

    let top = egui::Rect::from_min_size(
        Pos2::new(screen_rect.left() + c, screen_rect.top()),
        Vec2::new(screen_rect.width() - 2.0 * c, t),
    );
    let bottom = egui::Rect::from_min_size(
        Pos2::new(screen_rect.left() + c, screen_rect.bottom() - t),
        Vec2::new(screen_rect.width() - 2.0 * c, t),
    );
    let left = egui::Rect::from_min_size(
        Pos2::new(screen_rect.left(), screen_rect.top() + c),
        Vec2::new(t, screen_rect.height() - 2.0 * c),
    );
    let right = egui::Rect::from_min_size(
        Pos2::new(screen_rect.right() - t, screen_rect.top() + c),
        Vec2::new(t, screen_rect.height() - 2.0 * c),
    );

    let nw = egui::Rect::from_min_size(screen_rect.min, Vec2::splat(c));
    let ne = egui::Rect::from_min_size(
        Pos2::new(screen_rect.right() - c, screen_rect.top()),
        Vec2::splat(c),
    );
    let sw = egui::Rect::from_min_size(
        Pos2::new(screen_rect.left(), screen_rect.bottom() - c),
        Vec2::splat(c),
    );
    let se = egui::Rect::from_min_size(
        Pos2::new(screen_rect.right() - c, screen_rect.bottom() - c),
        Vec2::splat(c),
    );

    if resize_zone(ui, nw, "resize_nw", CursorIcon::ResizeNorthWest) { queue.push(WindowAction::StartResize(CompassOctant::NorthWest)); return; }
    if resize_zone(ui, ne, "resize_ne", CursorIcon::ResizeNorthEast) { queue.push(WindowAction::StartResize(CompassOctant::NorthEast)); return; }
    if resize_zone(ui, sw, "resize_sw", CursorIcon::ResizeSouthWest) { queue.push(WindowAction::StartResize(CompassOctant::SouthWest)); return; }
    if resize_zone(ui, se, "resize_se", CursorIcon::ResizeSouthEast) { queue.push(WindowAction::StartResize(CompassOctant::SouthEast)); return; }
    if resize_zone(ui, top, "resize_n", CursorIcon::ResizeNorth) { queue.push(WindowAction::StartResize(CompassOctant::North)); return; }
    if resize_zone(ui, bottom, "resize_s", CursorIcon::ResizeSouth) { queue.push(WindowAction::StartResize(CompassOctant::South)); return; }
    if resize_zone(ui, left, "resize_w", CursorIcon::ResizeWest) { queue.push(WindowAction::StartResize(CompassOctant::West)); return; }
    if resize_zone(ui, right, "resize_e", CursorIcon::ResizeEast) { queue.push(WindowAction::StartResize(CompassOctant::East)); return; }
}

fn resize_zone(ui: &mut egui::Ui, rect: egui::Rect, id_src: &str, cursor: CursorIcon) -> bool {
    let id = ui.id().with(id_src);
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(cursor);
    }
    resp.drag_started()
}

/// Draws a 1px border inside `screen_rect` so a borderless window has a frame.
pub fn render_border(ui: &egui::Ui, screen_rect: egui::Rect, color: Color32) {
    ui.painter().rect_stroke(
        screen_rect.shrink(0.5),
        CornerRadius::ZERO,
        Stroke::new(1.0, color),
        StrokeKind::Inside,
    );
}
