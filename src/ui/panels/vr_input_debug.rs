//! VR Input Debug panel
//!
//! Real-time visualization of all VR input state: controllers, buttons,
//! thumbsticks, poses, and hand tracking.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, Vec2};
use std::collections::VecDeque;
use renzora_theme::Theme;

/// Snapshot of one hand's controller state for UI display
#[derive(Clone, Default)]
pub struct HandSnapshot {
    pub tracked: bool,
    pub trigger: f32,
    pub trigger_pressed: bool,
    pub grip: f32,
    pub grip_pressed: bool,
    pub thumbstick_x: f32,
    pub thumbstick_y: f32,
    pub thumbstick_clicked: bool,
    pub button_a: bool,
    pub button_b: bool,
    pub menu: bool,
    pub grip_position: bevy::math::Vec3,
    pub grip_rotation: bevy::math::Quat,
    pub aim_position: bevy::math::Vec3,
    pub aim_rotation: bevy::math::Quat,
    // Hand tracking
    pub hand_tracked: bool,
    pub pinch_strength: f32,
    pub grab_strength: f32,
}

/// VR Input Debug panel state
#[derive(Resource)]
pub struct VrInputDebugState {
    pub show_raw_values: bool,
    pub selected_hand: usize, // 0=Both, 1=Left, 2=Right
    pub position_history: VecDeque<[bevy::math::Vec3; 2]>,
    pub left: HandSnapshot,
    pub right: HandSnapshot,
}

impl Default for VrInputDebugState {
    fn default() -> Self {
        Self {
            show_raw_values: false,
            selected_hand: 0,
            position_history: VecDeque::with_capacity(60),
            left: HandSnapshot::default(),
            right: HandSnapshot::default(),
        }
    }
}

pub fn render_vr_input_debug_content(
    ui: &mut egui::Ui,
    state: &mut VrInputDebugState,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();
    let green = Color32::from_rgb(60, 200, 100);
    let red = Color32::from_rgb(200, 60, 60);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // ---- Hand Filter ----
        ui.horizontal(|ui| {
            ui.label("Show:");
            egui::ComboBox::from_id_salt("vr_debug_hand")
                .show_index(ui, &mut state.selected_hand, 3, |i| match i {
                    0 => "Both",
                    1 => "Left Only",
                    _ => "Right Only",
                });
            ui.checkbox(&mut state.show_raw_values, "Raw Values");
        });

        ui.separator();

        // ---- Controller Status ----
        ui.label(RichText::new("Controller Status").size(13.0).color(muted));
        ui.separator();

        if state.selected_hand != 2 {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Left:").strong());
                let (color, text) = if state.left.tracked { (green, "Tracked") } else { (red, "Lost") };
                ui.colored_label(color, text);
            });
        }
        if state.selected_hand != 1 {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Right:").strong());
                let (color, text) = if state.right.tracked { (green, "Tracked") } else { (red, "Lost") };
                ui.colored_label(color, text);
            });
        }

        ui.add_space(8.0);

        // ---- Buttons Section ----
        ui.label(RichText::new("Buttons & Axes").size(13.0).color(muted));
        ui.separator();

        let button_indicator = |ui: &mut egui::Ui, name: &str, pressed: bool| {
            ui.horizontal(|ui| {
                let color = if pressed { green } else { Color32::GRAY };
                let text = if pressed { "\u{25CF}" } else { "\u{25CB}" }; // filled/empty circle
                ui.colored_label(color, text);
                ui.label(name);
            });
        };

        let analog_bar = |ui: &mut egui::Ui, name: &str, value: f32| {
            ui.horizontal(|ui| {
                ui.label(format!("{name}:"));
                let (rect, _) = ui.allocate_exact_size(Vec2::new(100.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 2.0, Color32::from_gray(40));
                let fill_width = rect.width() * value.clamp(0.0, 1.0);
                let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill_width, rect.height()));
                ui.painter().rect_filled(fill_rect, 2.0, green);
                if state.show_raw_values {
                    ui.label(format!("{value:.3}"));
                }
            });
        };

        // Show buttons for the selected hand(s)
        let hands_to_show: Vec<(&str, &HandSnapshot)> = match state.selected_hand {
            1 => vec![("L", &state.left)],
            2 => vec![("R", &state.right)],
            _ => vec![("L", &state.left), ("R", &state.right)],
        };

        for (prefix, hand) in &hands_to_show {
            if hands_to_show.len() > 1 {
                ui.label(RichText::new(format!("— {prefix} —")).color(muted));
            }
            button_indicator(ui, &format!("{prefix} Trigger"), hand.trigger_pressed);
            analog_bar(ui, &format!("{prefix} Trigger"), hand.trigger);
            button_indicator(ui, &format!("{prefix} Grip"), hand.grip_pressed);
            analog_bar(ui, &format!("{prefix} Grip"), hand.grip);
            button_indicator(ui, &format!("{prefix} A/X"), hand.button_a);
            button_indicator(ui, &format!("{prefix} B/Y"), hand.button_b);
            button_indicator(ui, &format!("{prefix} Menu"), hand.menu);
            button_indicator(ui, &format!("{prefix} Stick"), hand.thumbstick_clicked);
        }

        ui.add_space(8.0);

        // ---- Thumbstick Section ----
        ui.label(RichText::new("Thumbsticks").size(13.0).color(muted));
        ui.separator();

        let render_thumbstick = |ui: &mut egui::Ui, label: &str, x: f32, y: f32| {
            ui.label(label);
            let size = Vec2::splat(80.0);
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let center = rect.center();
            let painter = ui.painter();

            // Background circle
            painter.circle_stroke(center, 38.0, egui::Stroke::new(1.0, Color32::from_gray(60)));
            // Deadzone circle
            painter.circle_stroke(center, 8.0, egui::Stroke::new(1.0, Color32::from_gray(40)));
            // Thumbstick position
            let dot_pos = center + egui::Vec2::new(x * 36.0, -y * 36.0);
            painter.circle_filled(dot_pos, 4.0, green);

            if state.show_raw_values {
                ui.label(format!("X: {x:.3}  Y: {y:.3}"));
            }
        };

        ui.horizontal(|ui| {
            if state.selected_hand != 2 {
                render_thumbstick(ui, "Left", state.left.thumbstick_x, state.left.thumbstick_y);
            }
            if state.selected_hand != 1 {
                render_thumbstick(ui, "Right", state.right.thumbstick_x, state.right.thumbstick_y);
            }
        });

        ui.add_space(8.0);

        // ---- Poses Section ----
        ui.label(RichText::new("Poses").size(13.0).color(muted));
        ui.separator();

        let render_pose = |ui: &mut egui::Ui, label: &str, pos: bevy::math::Vec3, rot: bevy::math::Quat| {
            ui.horizontal(|ui| {
                ui.label(format!("{label}:"));
                ui.label(format!("({:.3}, {:.3}, {:.3})", pos.x, pos.y, pos.z));
            });
            if state.show_raw_values {
                ui.horizontal(|ui| {
                    ui.label("  Rot:");
                    ui.label(format!("({:.3}, {:.3}, {:.3}, {:.3})", rot.x, rot.y, rot.z, rot.w));
                });
            }
        };

        if state.selected_hand != 2 {
            render_pose(ui, "Grip L", state.left.grip_position, state.left.grip_rotation);
            render_pose(ui, "Aim L", state.left.aim_position, state.left.aim_rotation);
        }
        if state.selected_hand != 1 {
            render_pose(ui, "Grip R", state.right.grip_position, state.right.grip_rotation);
            render_pose(ui, "Aim R", state.right.aim_position, state.right.aim_rotation);
        }

        ui.add_space(8.0);

        // ---- Hand Tracking Section ----
        ui.label(RichText::new("Hand Tracking").size(13.0).color(muted));
        ui.separator();

        let render_hand_tracking = |ui: &mut egui::Ui, label: &str, tracked: bool, pinch: f32, grab: f32| {
            ui.horizontal(|ui| {
                ui.label(format!("{label}:"));
                let color = if tracked { green } else { red };
                ui.colored_label(color, if tracked { "Tracked" } else { "Lost" });
            });
            analog_bar(ui, "Pinch", pinch);
            analog_bar(ui, "Grab", grab);
        };

        if state.selected_hand != 2 {
            render_hand_tracking(ui, "Left Hand", state.left.hand_tracked, state.left.pinch_strength, state.left.grab_strength);
        }
        if state.selected_hand != 1 {
            render_hand_tracking(ui, "Right Hand", state.right.hand_tracked, state.right.pinch_strength, state.right.grab_strength);
        }
    });
}
