//! Gamepad debug panel for testing controller input

use bevy_egui::egui::{self, Color32, RichText, Stroke, StrokeKind, Vec2};

use crate::core::resources::{GamepadDebugState, GamepadInfo};
use crate::theming::Theme;

/// Render the gamepad debug panel content
pub fn render_gamepad_content(
    ui: &mut egui::Ui,
    gamepad_state: &GamepadDebugState,
    _theme: &Theme,
) {
    let connected_count = gamepad_state.gamepads.len();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Header
        ui.horizontal(|ui| {
            ui.label(RichText::new("Gamepad Debug").size(14.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let status_color = if connected_count > 0 {
                    Color32::from_rgb(100, 200, 100)
                } else {
                    Color32::from_rgb(150, 150, 150)
                };
                ui.label(RichText::new(format!("{} connected", connected_count)).color(status_color));
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        if connected_count == 0 {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(RichText::new("No gamepad detected").size(14.0).color(Color32::from_gray(120)));
                ui.add_space(8.0);
                ui.label(RichText::new("Connect a controller to see input").size(12.0).color(Color32::from_gray(80)));
            });
            return;
        }

        for (idx, gamepad) in gamepad_state.gamepads.iter().enumerate() {
            if idx > 0 {
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);
            }

            render_gamepad(ui, idx, gamepad);
        }
    });
}

fn render_gamepad(ui: &mut egui::Ui, index: usize, gamepad: &GamepadInfo) {
    ui.label(RichText::new(format!("Gamepad {}", index + 1)).size(13.0).strong());
    ui.add_space(8.0);

    // Sticks section
    ui.horizontal(|ui| {
        // Left stick
        ui.vertical(|ui| {
            ui.label(RichText::new("Left Stick").size(11.0).color(Color32::from_gray(150)));
            render_stick(ui, gamepad.left_stick.x, gamepad.left_stick.y);
            ui.label(RichText::new(format!(
                "X: {:.2}  Y: {:.2}",
                gamepad.left_stick.x,
                gamepad.left_stick.y,
            )).size(10.0).color(Color32::from_gray(120)));
        });

        ui.add_space(20.0);

        // Right stick
        ui.vertical(|ui| {
            ui.label(RichText::new("Right Stick").size(11.0).color(Color32::from_gray(150)));
            render_stick(ui, gamepad.right_stick.x, gamepad.right_stick.y);
            ui.label(RichText::new(format!(
                "X: {:.2}  Y: {:.2}",
                gamepad.right_stick.x,
                gamepad.right_stick.y,
            )).size(10.0).color(Color32::from_gray(120)));
        });
    });

    ui.add_space(12.0);

    // Triggers
    ui.label(RichText::new("Triggers").size(11.0).color(Color32::from_gray(150)));
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new("LT").size(10.0));
            render_trigger(ui, gamepad.left_trigger);
            ui.label(RichText::new(format!("{:.2}", gamepad.left_trigger)).size(10.0).color(Color32::from_gray(120)));
        });

        ui.add_space(10.0);

        ui.vertical(|ui| {
            ui.label(RichText::new("RT").size(10.0));
            render_trigger(ui, gamepad.right_trigger);
            ui.label(RichText::new(format!("{:.2}", gamepad.right_trigger)).size(10.0).color(Color32::from_gray(120)));
        });
    });

    ui.add_space(12.0);

    // Buttons
    ui.label(RichText::new("Buttons").size(11.0).color(Color32::from_gray(150)));

    // Face buttons (A, B, X, Y / Cross, Circle, Square, Triangle)
    ui.horizontal(|ui| {
        render_button(ui, "A/Cross", gamepad.buttons.south);
        render_button(ui, "B/Circle", gamepad.buttons.east);
        render_button(ui, "X/Square", gamepad.buttons.west);
        render_button(ui, "Y/Triangle", gamepad.buttons.north);
    });

    // Bumpers and triggers (as buttons)
    ui.horizontal(|ui| {
        render_button(ui, "LB", gamepad.buttons.left_trigger);
        render_button(ui, "RB", gamepad.buttons.right_trigger);
        render_button(ui, "LT", gamepad.buttons.left_trigger2);
        render_button(ui, "RT", gamepad.buttons.right_trigger2);
    });

    // D-Pad
    ui.horizontal(|ui| {
        render_button(ui, "Up", gamepad.buttons.dpad_up);
        render_button(ui, "Down", gamepad.buttons.dpad_down);
        render_button(ui, "Left", gamepad.buttons.dpad_left);
        render_button(ui, "Right", gamepad.buttons.dpad_right);
    });

    // Start/Select and sticks
    ui.horizontal(|ui| {
        render_button(ui, "Start", gamepad.buttons.start);
        render_button(ui, "Select", gamepad.buttons.select);
        render_button(ui, "L3", gamepad.buttons.left_thumb);
        render_button(ui, "R3", gamepad.buttons.right_thumb);
    });
}

fn render_stick(ui: &mut egui::Ui, x: f32, y: f32) {
    let size = Vec2::splat(80.0);
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

    let painter = ui.painter();
    let center = rect.center();
    let radius = size.x / 2.0 - 4.0;

    // Background circle
    painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_gray(60)));

    // Cross hairs
    painter.line_segment(
        [egui::pos2(center.x - radius, center.y), egui::pos2(center.x + radius, center.y)],
        Stroke::new(1.0, Color32::from_gray(40)),
    );
    painter.line_segment(
        [egui::pos2(center.x, center.y - radius), egui::pos2(center.x, center.y + radius)],
        Stroke::new(1.0, Color32::from_gray(40)),
    );

    // Deadzone circle (10%)
    painter.circle_stroke(center, radius * 0.1, Stroke::new(1.0, Color32::from_gray(50)));

    // Stick position - note: Y is inverted for display
    let stick_x = center.x + x * radius;
    let stick_y = center.y - y * radius; // Invert Y so up is up

    // Line from center to stick
    painter.line_segment(
        [center, egui::pos2(stick_x, stick_y)],
        Stroke::new(2.0, Color32::from_rgb(100, 150, 200)),
    );

    // Stick indicator
    let stick_color = if x.abs() > 0.1 || y.abs() > 0.1 {
        Color32::from_rgb(100, 200, 100)
    } else {
        Color32::from_rgb(150, 150, 150)
    };
    painter.circle_filled(egui::pos2(stick_x, stick_y), 6.0, stick_color);
    painter.circle_stroke(egui::pos2(stick_x, stick_y), 6.0, Stroke::new(1.0, Color32::WHITE));
}

fn render_trigger(ui: &mut egui::Ui, value: f32) {
    let size = Vec2::new(30.0, 60.0);
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

    let painter = ui.painter();

    // Background
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_gray(60)), StrokeKind::Outside);

    // Fill based on value
    let fill_height = rect.height() * value.clamp(0.0, 1.0);
    let fill_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 1.0, rect.max.y - fill_height),
        egui::pos2(rect.max.x - 1.0, rect.max.y - 1.0),
    );

    let color = if value > 0.1 {
        Color32::from_rgb(100, 200, 100)
    } else {
        Color32::from_rgb(80, 80, 80)
    };
    painter.rect_filled(fill_rect, 0.0, color);
}

fn render_button(ui: &mut egui::Ui, label: &str, pressed: bool) {
    let size = Vec2::new(60.0, 24.0);
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

    let painter = ui.painter();

    let (bg_color, text_color) = if pressed {
        (Color32::from_rgb(80, 160, 80), Color32::WHITE)
    } else {
        (Color32::from_rgb(50, 52, 58), Color32::from_gray(120))
    };

    painter.rect_filled(rect, 4.0, bg_color);
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_gray(70)), StrokeKind::Outside);

    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(10.0),
        text_color,
    );
}
