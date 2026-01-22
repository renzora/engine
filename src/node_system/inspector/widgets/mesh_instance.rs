#![allow(dead_code)]

use bevy_egui::egui::{self, Color32, RichText, Sense, Vec2};
use std::path::PathBuf;

use crate::core::AssetBrowserState;
use crate::node_system::MeshInstanceData;

// Phosphor icons
use egui_phosphor::regular::{CUBE, FILE, FOLDER_OPEN, X_CIRCLE};

/// Render the mesh instance inspector
/// Returns (changed, new_model_path) if the model should be loaded
pub fn render_mesh_instance_inspector(
    ui: &mut egui::Ui,
    instance: &mut MeshInstanceData,
    assets: &AssetBrowserState,
) -> (bool, Option<PathBuf>) {
    let mut changed = false;
    let mut new_model_to_load: Option<PathBuf> = None;

    ui.add_space(4.0);

    // Model path display with drop zone
    ui.horizontal(|ui| {
        ui.label("Model");
    });

    ui.add_space(4.0);

    // Create a drop zone for model files
    let drop_zone_height = 60.0;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), drop_zone_height),
        Sense::click_and_drag(),
    );

    // Check if we're currently dragging an asset
    let is_drag_target = assets.dragging_asset.is_some();
    let is_hovered = response.hovered();

    // Background color based on state
    let bg_color = if is_drag_target && is_hovered {
        Color32::from_rgb(66, 120, 180)
    } else if is_drag_target {
        Color32::from_rgb(50, 80, 120)
    } else {
        Color32::from_rgb(40, 40, 48)
    };

    ui.painter().rect_filled(rect, 4.0, bg_color);
    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0, Color32::from_rgb(70, 70, 80)), egui::StrokeKind::Outside);

    // Content inside drop zone
    if let Some(ref model_path) = instance.model_path {
        // Show current model
        let file_name = model_path.rsplit('/').next()
            .or_else(|| model_path.rsplit('\\').next())
            .unwrap_or(model_path);

        let center = rect.center();

        // Icon
        ui.painter().text(
            egui::pos2(center.x, center.y - 10.0),
            egui::Align2::CENTER_CENTER,
            CUBE,
            egui::FontId::proportional(24.0),
            Color32::from_rgb(242, 166, 115),
        );

        // File name
        ui.painter().text(
            egui::pos2(center.x, center.y + 14.0),
            egui::Align2::CENTER_CENTER,
            file_name,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(200, 200, 210),
        );
    } else {
        // Show empty state with hint
        let center = rect.center();

        if is_drag_target {
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                "Drop model here",
                egui::FontId::proportional(12.0),
                Color32::from_rgb(140, 180, 220),
            );
        } else {
            ui.painter().text(
                egui::pos2(center.x, center.y - 8.0),
                egui::Align2::CENTER_CENTER,
                FILE,
                egui::FontId::proportional(20.0),
                Color32::from_rgb(100, 100, 110),
            );
            ui.painter().text(
                egui::pos2(center.x, center.y + 12.0),
                egui::Align2::CENTER_CENTER,
                "Drag a model here",
                egui::FontId::proportional(11.0),
                Color32::from_rgb(120, 120, 130),
            );
        }
    }

    // Handle drop
    if is_hovered && !response.dragged() {
        if let Some(dragging_path) = &assets.dragging_asset {
            // Check if it's a valid model file
            let ext = dragging_path.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            if matches!(ext.as_str(), "glb" | "gltf" | "obj" | "fbx") {
                // Convert path to relative path for storage
                let path_str = dragging_path.to_string_lossy().to_string();
                instance.model_path = Some(path_str);
                new_model_to_load = Some(dragging_path.clone());
                changed = true;
            }
        }
    }

    ui.add_space(8.0);

    // Clear button if a model is assigned
    if instance.model_path.is_some() {
        ui.horizontal(|ui| {
            if ui.button(RichText::new(format!("{} Clear", X_CIRCLE)).color(Color32::from_rgb(200, 100, 100))).clicked() {
                instance.model_path = None;
                changed = true;
            }

            if ui.button(RichText::new(format!("{} Browse...", FOLDER_OPEN))).clicked() {
                // TODO: Open file browser
            }
        });
    } else {
        if ui.button(RichText::new(format!("{} Browse...", FOLDER_OPEN))).clicked() {
            // TODO: Open file browser
        }
    }

    ui.add_space(4.0);

    (changed, new_model_to_load)
}
