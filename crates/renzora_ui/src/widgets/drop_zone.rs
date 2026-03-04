//! Reusable file drop zone widget for the editor.

use std::path::PathBuf;

use bevy_egui::egui::{self, Sense, Vec2};
use egui_phosphor::regular::{FILE, FOLDER_OPEN, IMAGE, X_CIRCLE};

use super::ThemeColors;

/// Result from rendering a file drop zone widget.
pub struct FileDropResult {
    /// User clicked the Clear button.
    pub cleared: bool,
    /// User clicked the browse (folder) button — caller handles the file dialog.
    pub browse_clicked: bool,
    /// An OS file was dropped onto the zone (filtered by allowed extensions).
    pub dropped_path: Option<PathBuf>,
    /// Whether an OS file with a matching extension is being hovered over the zone.
    pub os_hovering: bool,
}

/// Render a file drop zone widget.
///
/// Shows a rectangular area with icon + filename (or placeholder text).
/// Handles OS drag hover highlight, OS file drop detection, clear button, and browse button.
///
/// Does NOT handle file dialogs or project-relative path resolution — those are
/// the caller's responsibility based on `browse_clicked` / `dropped_path`.
pub fn file_drop_zone(
    ui: &mut egui::Ui,
    _id: egui::Id,
    current_path: Option<&str>,
    extensions: &[&str],
    label: &str,
    theme: &ThemeColors,
) -> FileDropResult {
    let mut result = FileDropResult {
        cleared: false,
        browse_clicked: false,
        dropped_path: None,
        os_hovering: false,
    };

    let is_matching_ext = |path: &std::path::Path| -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| extensions.iter().any(|&allowed| ext.eq_ignore_ascii_case(allowed)))
            .unwrap_or(false)
    };

    // Check for OS drag hover
    let os_hovered = ui.ctx().input(|i| {
        i.raw.hovered_files.iter().any(|f| f.path.as_ref().map_or(false, |p| is_matching_ext(p)))
    });

    // Check for OS drop
    let os_dropped = ui.ctx().input(|i| {
        i.raw
            .dropped_files
            .iter()
            .find(|f| f.path.as_ref().map_or(false, |p| is_matching_ext(p)))
            .and_then(|f| f.path.clone())
    });

    let drop_zone_height = 60.0;
    let available_width = ui.available_width();

    ui.horizontal(|ui| {
        let drop_width = available_width - 34.0;
        let (rect, _response) =
            ui.allocate_exact_size(Vec2::new(drop_width, drop_zone_height), Sense::click_and_drag());

        // Background
        ui.painter().rect_filled(rect, 4.0, theme.widget_inactive_bg);
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, theme.widget_border),
            egui::StrokeKind::Outside,
        );

        let pointer_pos = ui.ctx().pointer_hover_pos();
        let pointer_in_zone = pointer_pos.map_or(false, |p| rect.contains(p));

        // OS hover highlight
        if os_hovered && pointer_in_zone {
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(2.0, theme.semantic_accent),
                egui::StrokeKind::Inside,
            );
            result.os_hovering = true;
        }

        // OS drop
        if let Some(ref dropped_path) = os_dropped {
            if pointer_in_zone {
                result.dropped_path = Some(dropped_path.clone());
            }
        }

        // Draw contents
        let has_file = current_path.map_or(false, |p| !p.is_empty());
        if has_file {
            let path = current_path.unwrap();
            let file_name = path
                .rsplit('/')
                .next()
                .or_else(|| path.rsplit('\\').next())
                .unwrap_or(path);
            let center = rect.center();
            ui.painter().text(
                egui::pos2(center.x, center.y - 10.0),
                egui::Align2::CENTER_CENTER,
                IMAGE,
                egui::FontId::proportional(24.0),
                theme.semantic_warning,
            );
            ui.painter().text(
                egui::pos2(center.x, center.y + 14.0),
                egui::Align2::CENTER_CENTER,
                file_name,
                egui::FontId::proportional(12.0),
                theme.text_primary,
            );
        } else {
            let center = rect.center();
            ui.painter().text(
                egui::pos2(center.x, center.y - 8.0),
                egui::Align2::CENTER_CENTER,
                FILE,
                egui::FontId::proportional(20.0),
                theme.text_disabled,
            );
            ui.painter().text(
                egui::pos2(center.x, center.y + 12.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(11.0),
                theme.text_muted,
            );
        }

        // Browse button
        if ui
            .add_sized(
                [26.0, drop_zone_height],
                egui::Button::new(FOLDER_OPEN.to_string()),
            )
            .clicked()
        {
            result.browse_clicked = true;
        }
    });

    // Clear button
    if current_path.map_or(false, |p| !p.is_empty()) {
        ui.add_space(4.0);
        if ui
            .button(
                egui::RichText::new(format!("{} Clear", X_CIRCLE))
                    .color(theme.semantic_error),
            )
            .clicked()
        {
            result.cleared = true;
        }
    }

    result
}
