use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular;
use renzora_editor::icon_button;
use renzora_theme::Theme;

use crate::state::{AssetBrowserState, ViewMode};

/// Renders the toolbar: back, home, breadcrumb path, search bar, zoom slider.
pub fn toolbar_ui(ui: &mut egui::Ui, state: &mut AssetBrowserState, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        // Back button
        let can_go_back = !state.history.is_empty();
        let back_color = if can_go_back { text_primary } else { theme.text.disabled.to_color32() };
        if icon_button(ui, regular::ARROW_LEFT, "Back", back_color) && can_go_back {
            state.go_back();
        }

        // Home button
        if icon_button(ui, regular::HOUSE, "Go to project root", text_primary) {
            state.go_home();
        }

        ui.add_space(4.0);

        // Breadcrumb path
        let root = state.root();
        if let Some(ref current) = state.current_folder.clone() {
            let rel = current.strip_prefix(&root).unwrap_or(current);
            let mut accumulated = root.clone();

            for (i, component) in rel.components().enumerate() {
                let segment = component.as_os_str().to_string_lossy();
                if i > 0 {
                    ui.label(RichText::new("/").size(11.0).color(text_muted));
                }

                accumulated = accumulated.join(component);
                let target = accumulated.clone();

                let resp = ui.add(
                    egui::Label::new(
                        RichText::new(segment.as_ref())
                            .size(11.0)
                            .color(text_primary),
                    )
                    .selectable(false)
                    .sense(egui::Sense::click()),
                );
                if resp.clicked() {
                    state.navigate_to(target);
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        } else {
            ui.label(RichText::new("No folder selected").size(11.0).color(text_muted));
        }

        // Right-aligned: search + zoom
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Zoom slider (only in grid mode)
            if state.view_mode == ViewMode::Grid {
                ui.spacing_mut().slider_width = 60.0;
                ui.add(
                    egui::Slider::new(&mut state.zoom, 0.5..=1.5)
                        .show_value(false)
                        .text(""),
                );
            }

            // View mode toggle
            let grid_color = if state.view_mode == ViewMode::Grid { text_primary } else { text_muted };
            let list_color = if state.view_mode == ViewMode::List { text_primary } else { text_muted };
            if icon_button(ui, regular::LIST, "List view", list_color) {
                state.view_mode = ViewMode::List;
            }
            if icon_button(ui, regular::SQUARES_FOUR, "Grid view", grid_color) {
                state.view_mode = ViewMode::Grid;
            }

            ui.add_space(8.0);

            // Import button
            if icon_button(ui, regular::DOWNLOAD_SIMPLE, "Import 3D model", text_primary) {
                state.import_clicked = true;
            }

            ui.add_space(8.0);

            // Search bar
            ui.add(
                egui::TextEdit::singleline(&mut state.search)
                    .desired_width(120.0)
                    .hint_text(format!("{} Search...", regular::MAGNIFYING_GLASS)),
            );
        });
    });
}
