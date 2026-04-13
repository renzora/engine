//! Header drawer shown when the viewport is in Edit mode.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, FontId, Sense, Vec2};
use renzora_editor_framework::EditorCommands;
use renzora_theme::ThemeManager;

use crate::selection::{MeshSelection, SelectMode};

const BTN_H: f32 = 22.0;

pub fn draw_edit_header(ui: &mut egui::Ui, world: &World) {
    let Some(theme_mgr) = world.get_resource::<ThemeManager>() else { return };
    let Some(cmds) = world.get_resource::<EditorCommands>() else { return };
    let current_mode = world
        .get_resource::<MeshSelection>()
        .map(|s| s.mode)
        .unwrap_or(SelectMode::Vertex);

    let theme = &theme_mgr.active_theme;
    let active = theme.semantic.accent.to_color32();
    let inactive = theme.widgets.inactive_bg.to_color32();
    let hovered = theme.widgets.hovered_bg.to_color32();

    ui.spacing_mut().interact_size.y = BTN_H;
    ui.spacing_mut().item_spacing.x = 3.0;

    mode_btn(ui, "Vertex", "1", SelectMode::Vertex, current_mode, active, inactive, hovered, cmds);
    mode_btn(ui, "Edge",   "2", SelectMode::Edge,   current_mode, active, inactive, hovered, cmds);
    mode_btn(ui, "Face",   "3", SelectMode::Face,   current_mode, active, inactive, hovered, cmds);

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("G: grab   E: extrude   X/Y/Z: axis lock   A: select all   Esc/RMB: cancel")
            .size(11.0)
            .color(theme.text.muted.to_color32()),
    );
}

fn mode_btn(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: &str,
    mode: SelectMode,
    current: SelectMode,
    active: Color32,
    inactive: Color32,
    hovered: Color32,
    cmds: &EditorCommands,
) {
    let is_active = mode == current;
    let size = Vec2::new(64.0, BTN_H);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
    if ui.is_rect_visible(rect) {
        let bg = if is_active { active } else if resp.hovered() { hovered } else { inactive };
        ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
    let clicked = resp.clicked();
    let _ = resp.on_hover_text(format!("{} select ({})", label, shortcut));
    if clicked {
        cmds.push(move |w: &mut World| {
            if let Some(mut s) = w.get_resource_mut::<MeshSelection>() {
                s.mode = mode;
            }
        });
    }
}
