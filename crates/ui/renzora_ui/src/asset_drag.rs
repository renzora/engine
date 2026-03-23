//! Asset drag-and-drop state — tracks assets being dragged from the asset browser
//! to inspector drop targets, hierarchy items, or the viewport.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::egui::{self, Align2, Color32, FontId, Pos2, Stroke, Vec2};
use renzora_theme::Theme;

/// Active asset drag state — inserted as a resource when an asset drag begins,
/// removed on drop or cancel.
#[derive(Resource, Clone, Debug)]
pub struct AssetDragPayload {
    /// Full path to the asset being dragged.
    pub path: PathBuf,
    /// Display name (filename).
    pub name: String,
    /// Phosphor icon string for this file type.
    pub icon: String,
    /// Accent color for this file type.
    pub color: Color32,
    /// Screen position where the drag started.
    pub origin: Pos2,
    /// True once the pointer moves >5px from origin.
    pub is_detached: bool,
}

impl AssetDragPayload {
    /// File extension (lowercase, no dot). Empty string if none.
    pub fn extension(&self) -> String {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
    }

    /// Check if this payload's extension matches any in the given list.
    pub fn matches_extensions(&self, extensions: &[&str]) -> bool {
        if extensions.is_empty() {
            return true; // empty = accept all
        }
        let ext = self.extension();
        extensions.iter().any(|&allowed| ext.eq_ignore_ascii_case(allowed))
    }
}

/// Draw a floating ghost showing the dragged asset following the cursor.
pub fn draw_asset_drag_ghost(ctx: &egui::Context, payload: &AssetDragPayload, pointer: Pos2, theme: &Theme) {
    let offset = Vec2::new(14.0, -10.0);
    let pos = pointer + offset;

    egui::Area::new(egui::Id::new("asset_drag_ghost"))
        .fixed_pos(pos)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::NONE
                .fill(theme.surfaces.panel.to_color32())
                .stroke(Stroke::new(1.5, payload.color))
                .inner_margin(egui::Margin::symmetric(8, 5))
                .corner_radius(egui::CornerRadius::same(6));
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(
                        egui::RichText::new(&payload.icon)
                            .font(FontId::proportional(14.0))
                            .color(payload.color),
                    );
                    ui.label(
                        egui::RichText::new(&payload.name)
                            .font(FontId::proportional(11.0))
                            .color(theme.text.primary.to_color32()),
                    );
                });
            });
        });
}

/// Result from an asset drop target widget.
#[derive(Default)]
pub struct AssetDropResult {
    /// An asset was dropped onto this target (path from the asset browser).
    pub dropped_path: Option<PathBuf>,
    /// A compatible asset is currently hovering over this target.
    pub is_hovering: bool,
    /// User clicked the clear button.
    pub cleared: bool,
    /// User clicked the browse button.
    pub browse_clicked: bool,
}

/// Render an inline asset drop target field.
///
/// Shows the current value (filename or placeholder), highlights when a compatible
/// asset is being dragged over it, and accepts the drop.
pub fn asset_drop_target(
    ui: &mut egui::Ui,
    _id: egui::Id,
    current_path: Option<&str>,
    extensions: &[&str],
    placeholder: &str,
    theme: &Theme,
    payload: Option<&AssetDragPayload>,
) -> AssetDropResult {
    let mut result = AssetDropResult::default();

    let height = 22.0;
    let available = ui.available_width();

    // Reserve space for clear button if we have a value
    let has_value = current_path.map_or(false, |p| !p.is_empty());
    let field_width = if has_value { available - 20.0 } else { available };

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(field_width, height),
        egui::Sense::click_and_drag(),
    );

    let pointer_pos = ui.ctx().pointer_hover_pos();
    let pointer_in_rect = pointer_pos.map_or(false, |p| rect.contains(p));

    // Check if a compatible asset is hovering
    let compatible_hover = payload
        .filter(|p| p.is_detached && p.matches_extensions(extensions))
        .is_some();
    let is_hovering = compatible_hover && pointer_in_rect;

    // Check if dropped (compatible payload + pointer released in rect)
    if is_hovering {
        result.is_hovering = true;
        if !ui.ctx().input(|i| i.pointer.any_down()) {
            if let Some(p) = payload {
                result.dropped_path = Some(p.path.clone());
            }
        }
    }

    // Background
    let bg = if is_hovering {
        let c = payload.unwrap().color;
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 30)
    } else {
        theme.surfaces.faint.to_color32()
    };
    ui.painter().rect_filled(rect, 3.0, bg);

    // Border
    let border_color = if is_hovering {
        payload.unwrap().color
    } else {
        theme.widgets.border.to_color32()
    };
    let border_width = if is_hovering { 1.5 } else { 1.0 };
    ui.painter().rect_stroke(
        rect,
        3.0,
        Stroke::new(border_width, border_color),
        egui::StrokeKind::Inside,
    );

    // Content
    let text_pos = Pos2::new(rect.min.x + 6.0, rect.center().y);
    if let Some(path) = current_path.filter(|p| !p.is_empty()) {
        let filename = path
            .rsplit('/')
            .next()
            .or_else(|| path.rsplit('\\').next())
            .unwrap_or(path);
        ui.painter().text(
            text_pos,
            Align2::LEFT_CENTER,
            filename,
            FontId::proportional(11.0),
            theme.text.primary.to_color32(),
        );
    } else if is_hovering {
        ui.painter().text(
            text_pos,
            Align2::LEFT_CENTER,
            "Drop here",
            FontId::proportional(11.0),
            payload.unwrap().color,
        );
    } else {
        ui.painter().text(
            text_pos,
            Align2::LEFT_CENTER,
            placeholder,
            FontId::proportional(11.0),
            theme.text.disabled.to_color32(),
        );
    }

    // Browse on click (when no drag active)
    if response.clicked() && payload.is_none() {
        result.browse_clicked = true;
    }

    // Clear button
    if has_value {
        let clear_rect = egui::Rect::from_min_size(
            Pos2::new(rect.max.x + 2.0, rect.min.y),
            Vec2::new(18.0, height),
        );
        let clear_response = ui.allocate_rect(clear_rect, egui::Sense::click());
        let clear_color = if clear_response.hovered() {
            theme.semantic.error.to_color32()
        } else {
            theme.text.muted.to_color32()
        };
        ui.painter().text(
            clear_rect.center(),
            Align2::CENTER_CENTER,
            egui_phosphor::regular::X,
            FontId::proportional(12.0),
            clear_color,
        );
        if clear_response.clicked() {
            result.cleared = true;
        }
    }

    result
}
