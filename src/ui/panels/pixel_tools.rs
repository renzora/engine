//! Pixel Tools Panel
//!
//! Tool buttons grid with keyboard shortcuts and active tool highlight.

use bevy_egui::egui::{self, Color32, Sense, Stroke, Vec2};
use crate::pixel_editor::{PixelEditorState, PixelTool};
use crate::theming::Theme;

use egui_phosphor::regular::{
    PENCIL_SIMPLE, ERASER, PAINT_BUCKET, LINE_SEGMENT, RECTANGLE, CIRCLE,
    SELECTION, EYEDROPPER, ARROWS_OUT_CARDINAL,
};

/// Render the pixel tools panel
pub fn render_pixel_tools_content(
    ui: &mut egui::Ui,
    state: &mut PixelEditorState,
    theme: &Theme,
) {
    let tools: &[(PixelTool, &str)] = &[
        (PixelTool::Pencil, PENCIL_SIMPLE),
        (PixelTool::Eraser, ERASER),
        (PixelTool::Fill, PAINT_BUCKET),
        (PixelTool::Line, LINE_SEGMENT),
        (PixelTool::Rect, RECTANGLE),
        (PixelTool::Circle, CIRCLE),
        (PixelTool::Select, SELECTION),
        (PixelTool::Eyedropper, EYEDROPPER),
        (PixelTool::Move, ARROWS_OUT_CARDINAL),
    ];

    let text_color = theme.text.primary.to_color32();
    let accent = theme.semantic.accent.to_color32();
    let muted = theme.text.muted.to_color32();

    // Handle keyboard shortcuts
    ui.ctx().input(|i| {
        if !i.modifiers.ctrl && !i.modifiers.alt {
            if i.key_pressed(egui::Key::P) { state.tool = PixelTool::Pencil; }
            if i.key_pressed(egui::Key::E) { state.tool = PixelTool::Eraser; }
            if i.key_pressed(egui::Key::G) { state.tool = PixelTool::Fill; }
            if i.key_pressed(egui::Key::L) { state.tool = PixelTool::Line; }
            // R conflicts with Rect - only use if not ctrl
            if i.key_pressed(egui::Key::U) { state.tool = PixelTool::Rect; }
            if i.key_pressed(egui::Key::O) { state.tool = PixelTool::Circle; }
            if i.key_pressed(egui::Key::S) && !i.modifiers.ctrl { state.tool = PixelTool::Select; }
            if i.key_pressed(egui::Key::I) { state.tool = PixelTool::Eyedropper; }
            if i.key_pressed(egui::Key::M) { state.tool = PixelTool::Move; }
            // X to swap colors
            if i.key_pressed(egui::Key::X) { state.swap_colors(); }
        }
    });

    ui.add_space(4.0);
    ui.label(egui::RichText::new("Tools").size(11.0).color(muted));
    ui.add_space(4.0);

    let btn_size = 32.0;

    // Tool grid
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
        for (tool, icon) in tools {
            let is_active = state.tool == *tool;
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(btn_size), Sense::click());

            let bg = if is_active {
                accent.gamma_multiply(0.4)
            } else if resp.hovered() {
                Color32::from_gray(55)
            } else {
                Color32::from_gray(40)
            };

            ui.painter().rect_filled(rect, 4.0, bg);

            if is_active {
                ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.5, accent), egui::StrokeKind::Outside);
            }

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                *icon,
                egui::FontId::proportional(16.0),
                if is_active { text_color } else { muted },
            );

            if resp.clicked() {
                state.tool = *tool;
            }

            resp.on_hover_text(format!("{} ({})", tool.name(), tool.shortcut()));
        }
    });

    ui.add_space(8.0);
    ui.separator();

    // Active tool info
    ui.label(egui::RichText::new(format!("Active: {}", state.tool.name()))
        .size(11.0).color(text_color));

    // Export button
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    if state.project.is_some() {
        if ui.button("Export PNG...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .set_file_name("sprite.png")
                .save_file()
            {
                if let Err(e) = state.export_as_png(&path) {
                    eprintln!("Export error: {}", e);
                }
            }
        }

        ui.add_space(4.0);

        // New project
        if ui.button("New Project...").clicked() {
            state.new_project_dialog = true;
        }
    }
}
