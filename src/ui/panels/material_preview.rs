//! Material Preview Panel
//!
//! Displays a real-time 3D preview of the current material blueprint.

use bevy_egui::egui::{self, Color32, CursorIcon, RichText, Sense, TextureId, Vec2};

use crate::blueprint::{BlueprintEditorState, MaterialPreviewState, PreviewMeshShape};

/// Render the material preview panel content
pub fn render_material_preview_content(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    preview_state: &mut MaterialPreviewState,
    blueprint_editor: &BlueprintEditorState,
    preview_texture_id: Option<TextureId>,
) {
    // Check if we have an active material blueprint
    let has_material = blueprint_editor
        .active_graph()
        .map(|g| g.is_material())
        .unwrap_or(false);

    if !has_material {
        render_no_material_state(ui);
        return;
    }

    ui.vertical(|ui| {
        // Toolbar
        render_preview_toolbar(ui, preview_state);

        ui.separator();

        // Preview viewport
        let available = ui.available_size();
        let preview_size = available.x.min(available.y - 30.0).max(100.0);

        ui.vertical_centered(|ui| {
            render_preview_viewport(ui, ctx, preview_state, preview_texture_id, preview_size);
        });
    });
}

/// Render toolbar with mesh selector and controls
fn render_preview_toolbar(ui: &mut egui::Ui, preview_state: &mut MaterialPreviewState) {
    ui.horizontal(|ui| {
        // Mesh shape selector
        ui.label("Mesh:");
        egui::ComboBox::from_id_salt("preview_mesh_shape")
            .selected_text(preview_state.mesh_shape.label())
            .width(80.0)
            .show_ui(ui, |ui| {
                for shape in PreviewMeshShape::ALL {
                    let selected = preview_state.mesh_shape == *shape;
                    if ui.selectable_label(selected, format!("{} {}", shape.icon(), shape.label())).clicked() {
                        preview_state.mesh_shape = *shape;
                    }
                }
            });

        ui.separator();

        // Auto-rotate toggle
        let auto_rotate_btn = ui.selectable_label(preview_state.auto_rotate, "\u{f021}");
        if auto_rotate_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if auto_rotate_btn.on_hover_text("Auto Rotate").clicked() {
            preview_state.auto_rotate = !preview_state.auto_rotate;
        }

        // Reset view button
        let reset_btn = ui.button("\u{f015}");
        if reset_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if reset_btn.on_hover_text("Reset View").clicked() {
            preview_state.yaw = 0.5;
            preview_state.pitch = 0.3;
            preview_state.distance = 3.0;
        }

        // Force recompile button
        let refresh_btn = ui.button("\u{f021}");
        if refresh_btn.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if refresh_btn.on_hover_text("Refresh Material").clicked() {
            preview_state.needs_recompile = true;
        }
    });
}

/// Render the 3D preview viewport
fn render_preview_viewport(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    preview_state: &mut MaterialPreviewState,
    preview_texture_id: Option<TextureId>,
    size: f32,
) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());

    // Background
    ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(30, 30, 35));

    // Display preview texture if available
    if let Some(texture_id) = preview_texture_id {
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter().image(texture_id, rect, uv, Color32::WHITE);
    } else {
        // No texture yet
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Preview Loading...",
            egui::FontId::proportional(12.0),
            Color32::from_gray(100),
        );
    }

    // Handle orbit controls (when not auto-rotating)
    if !preview_state.auto_rotate && response.dragged() {
        let delta = response.drag_delta();
        preview_state.yaw -= delta.x * 0.01;
        preview_state.pitch += delta.y * 0.01;
        preview_state.pitch = preview_state.pitch.clamp(-1.5, 1.5);
    }

    // Handle zoom with scroll
    if response.hovered() {
        let scroll = ctx.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            preview_state.distance -= scroll * 0.01;
            preview_state.distance = preview_state.distance.clamp(1.5, 10.0);
        }
    }

    // Border
    ui.painter().rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 70)),
        egui::StrokeKind::Inside,
    );

    // Instructions hint at bottom
    if response.hovered() {
        ui.painter().text(
            egui::pos2(rect.center().x, rect.max.y - 10.0),
            egui::Align2::CENTER_CENTER,
            "Drag to rotate • Scroll to zoom",
            egui::FontId::proportional(10.0),
            Color32::from_rgba_unmultiplied(150, 150, 160, 180),
        );
    }
}

/// Render state when no material blueprint is open
fn render_no_material_state(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);

        ui.label(RichText::new("\u{1F3A8}").size(32.0).color(Color32::from_gray(80)));
        ui.add_space(8.0);

        ui.label(RichText::new("No Material Blueprint").size(14.0).color(Color32::from_gray(120)));
        ui.add_space(4.0);

        ui.label(
            RichText::new("Open or create a material blueprint\nto see the preview")
                .size(11.0)
                .weak()
        );
    });
}
