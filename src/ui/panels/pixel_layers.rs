//! Pixel Layers Panel
//!
//! Layer list with visibility toggles, opacity, blend modes, reorder,
//! add/delete/duplicate buttons.

use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use crate::pixel_editor::{PixelEditorState, BlendMode};
use crate::theming::Theme;

use egui_phosphor::regular::{EYE, EYE_SLASH, LOCK_SIMPLE, LOCK_SIMPLE_OPEN, PLUS, TRASH, COPY, ARROW_UP, ARROW_DOWN};

/// Render the pixel layers panel
pub fn render_pixel_layers_content(
    ui: &mut egui::Ui,
    state: &mut PixelEditorState,
    theme: &Theme,
) {
    let Some(project) = state.project.as_mut() else {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new("No project open")
                .color(theme.text.muted.to_color32()));
        });
        return;
    };

    let text_color = theme.text.primary.to_color32();
    let muted = theme.text.muted.to_color32();

    // Layer action buttons at top
    ui.horizontal(|ui| {
        if ui.small_button(format!("{} Add", PLUS)).clicked() {
            project.add_layer();
        }
        if ui.small_button(format!("{} Del", TRASH)).clicked() {
            let idx = project.active_layer;
            project.remove_layer(idx);
        }
        if ui.small_button(format!("{} Dup", COPY)).clicked() {
            let idx = project.active_layer;
            project.duplicate_layer(idx);
        }
        if ui.small_button(format!("{}", ARROW_UP)).on_hover_text("Move Up").clicked() {
            let idx = project.active_layer;
            if idx + 1 < project.layers.len() {
                project.move_layer(idx, idx + 1);
            }
        }
        if ui.small_button(format!("{}", ARROW_DOWN)).on_hover_text("Move Down").clicked() {
            let idx = project.active_layer;
            if idx > 0 {
                project.move_layer(idx, idx - 1);
            }
        }
    });

    ui.separator();

    // Collect layer info into a snapshot to avoid borrow conflicts
    let num_layers = project.layers.len();
    let active_layer = project.active_layer;
    let layer_infos: Vec<(String, bool, bool, f32, &'static str)> = project.layers.iter().map(|l| {
        (l.name.clone(), l.visible, l.locked, l.opacity, l.blend_mode.name())
    }).collect();

    // Track mutations to apply after rendering
    let mut toggle_visibility: Option<usize> = None;
    let mut toggle_lock: Option<usize> = None;
    let mut set_active: Option<usize> = None;

    // Layer list (top = highest layer)
    egui::ScrollArea::vertical().show(ui, |ui| {
        for i in (0..num_layers).rev() {
            let is_active = i == active_layer;
            let (name, visible, locked, opacity, blend_name) = &layer_infos[i];

            let row_height = 36.0;
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), row_height),
                Sense::click(),
            );

            // Background
            let bg = if is_active {
                theme.semantic.accent.to_color32().gamma_multiply(0.3)
            } else if response.hovered() {
                Color32::from_gray(45)
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, 2.0, bg);

            if response.clicked() {
                set_active = Some(i);
            }

            // Visibility toggle
            let eye_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 4.0, rect.min.y + 2.0),
                Vec2::new(20.0, row_height - 4.0),
            );
            let eye_resp = ui.allocate_rect(eye_rect, Sense::click());
            let eye_icon = if *visible { EYE } else { EYE_SLASH };
            let eye_color = if *visible { text_color } else { muted };
            ui.painter().text(
                eye_rect.center(),
                egui::Align2::CENTER_CENTER,
                eye_icon,
                egui::FontId::proportional(14.0),
                eye_color,
            );
            if eye_resp.clicked() {
                toggle_visibility = Some(i);
            }

            // Lock toggle
            let lock_rect = Rect::from_min_size(
                Pos2::new(rect.min.x + 26.0, rect.min.y + 2.0),
                Vec2::new(20.0, row_height - 4.0),
            );
            let lock_resp = ui.allocate_rect(lock_rect, Sense::click());
            let lock_icon = if *locked { LOCK_SIMPLE } else { LOCK_SIMPLE_OPEN };
            let lock_color = if *locked { text_color } else { Color32::from_gray(50) };
            ui.painter().text(
                lock_rect.center(),
                egui::Align2::CENTER_CENTER,
                lock_icon,
                egui::FontId::proportional(12.0),
                lock_color,
            );
            if lock_resp.clicked() {
                toggle_lock = Some(i);
            }

            // Layer name
            ui.painter().text(
                Pos2::new(rect.min.x + 50.0, rect.center().y - 4.0),
                egui::Align2::LEFT_CENTER,
                name,
                egui::FontId::proportional(12.0),
                text_color,
            );

            // Opacity indicator
            ui.painter().text(
                Pos2::new(rect.max.x - 8.0, rect.center().y - 4.0),
                egui::Align2::RIGHT_CENTER,
                format!("{:.0}%", opacity * 100.0),
                egui::FontId::proportional(10.0),
                muted,
            );

            // Blend mode
            ui.painter().text(
                Pos2::new(rect.min.x + 50.0, rect.center().y + 10.0),
                egui::Align2::LEFT_CENTER,
                *blend_name,
                egui::FontId::proportional(9.0),
                muted,
            );

            // Bottom separator
            ui.painter().line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(0.5, Color32::from_gray(45)),
            );
        }
    });

    // Apply deferred mutations
    if let Some(i) = set_active {
        project.active_layer = i;
    }
    if let Some(i) = toggle_visibility {
        project.layers[i].visible = !project.layers[i].visible;
        project.texture_dirty = true;
    }
    if let Some(i) = toggle_lock {
        project.layers[i].locked = !project.layers[i].locked;
    }

    // Active layer opacity slider
    if project.active_layer < project.layers.len() {
        ui.separator();
        let al = project.active_layer;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Opacity:").size(11.0).color(muted));
            let mut opacity = project.layers[al].opacity;
            if ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0).show_value(false)).changed() {
                project.layers[al].opacity = opacity;
                project.texture_dirty = true;
            }
        });

        // Blend mode selector
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Blend:").size(11.0).color(muted));
            let current_mode = project.layers[al].blend_mode;
            egui::ComboBox::from_id_salt("blend_mode")
                .selected_text(current_mode.name())
                .width(100.0)
                .show_ui(ui, |ui| {
                    for mode in BlendMode::ALL {
                        if ui.selectable_label(current_mode == *mode, mode.name()).clicked() {
                            project.layers[al].blend_mode = *mode;
                            project.texture_dirty = true;
                        }
                    }
                });
        });
    }
}
