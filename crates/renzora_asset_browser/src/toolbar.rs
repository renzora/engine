#![allow(deprecated)] // egui API rename pending; will migrate at next bevy_egui bump.

use bevy_egui::egui::{self, RichText};
use egui_phosphor::regular;
use renzora_editor::icon_button;
use renzora_theme::Theme;

use crate::state::{AssetBrowserState, SortDirection, SortMode, ViewMode};

/// Renders the toolbar: breadcrumb path, search bar, import, view toggle, zoom slider.
/// When the panel is narrow (tree-only layout), the toolbar collapses to the
/// essentials — search and import only — so controls don't overflow.
pub fn toolbar_ui(ui: &mut egui::Ui, state: &mut AssetBrowserState, theme: &Theme) {
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let compact = state.tree_show_files;

    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_space(8.0);

        // New Folder + Add (asset creation) buttons
        let new_folder_label = RichText::new(format!("{} New Folder", regular::FOLDER_PLUS))
            .size(11.0)
            .color(text_muted);
        let new_folder_resp = ui
            .add(egui::Button::new(new_folder_label).frame(false))
            .on_hover_text("Create a new folder");
        if new_folder_resp.clicked() {
            state.create_inline("New Folder", "");
        }

        let add_label = RichText::new(format!("{} Add", regular::PLUS))
            .size(11.0)
            .color(text_muted);
        ui.menu_button(add_label, |ui| {
            if ui.button(format!("{} Material", regular::PALETTE)).clicked() {
                state.create_inline("NewMaterial.material", "{}");
                ui.close();
            }
            if ui.button(format!("{} Blueprint", regular::BLUEPRINT)).clicked() {
                state.create_inline("NewBlueprint.blueprint", "{}");
                ui.close();
            }
            if ui.button(format!("{} Lua Script", regular::CODE)).clicked() {
                state.create_inline("new_script.lua", "-- New Lua script\n");
                ui.close();
            }
            if ui.button(format!("{} Rhai Script", regular::CODE)).clicked() {
                state.create_inline("new_script.rhai", "// New Rhai script\n");
                ui.close();
            }
            if ui.button(format!("{} Particle", regular::SPARKLE)).clicked() {
                state.create_inline("NewParticle.particle", "(name: \"New Particle\")");
                ui.close();
            }
        });

        ui.add_space(6.0);

        // Breadcrumb navigation
        let root = state.root();
        let root_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Project")
            .to_string();

        // Breadcrumb (hidden in compact mode — the tree already gives a clear
        // sense of hierarchy, and the breadcrumb crowds the narrow toolbar).
        if !compact {
            if let Some(ref current) = state.current_folder.clone() {
                let is_root = current == &root;
                let root_resp = ui.add(
                    egui::Label::new(
                        RichText::new(format!("{} {}", regular::HOUSE, root_name))
                            .size(11.0)
                            .color(if is_root { text_primary } else { text_muted }),
                    )
                    .selectable(false)
                    .sense(egui::Sense::click()),
                );
                if root_resp.clicked() {
                    state.navigate_to(root.clone());
                }
                if root_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                if let Ok(rel) = current.strip_prefix(&root) {
                    let mut accumulated = root.clone();
                    for component in rel.components() {
                        let segment = component.as_os_str().to_string_lossy();
                        accumulated = accumulated.join(component);
                        let target = accumulated.clone();

                        ui.label(RichText::new(regular::CARET_RIGHT).size(10.0).color(text_muted));

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
                }
            } else {
                ui.label(RichText::new("No folder selected").size(11.0).color(text_muted));
            }
        }

        // Right-aligned controls. In compact mode we drop zoom / view toggle
        // / sort (they're irrelevant to the tree-only layout) and squeeze the
        // import button down to an icon so search has room to breathe.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);

            let accent = theme.semantic.accent.to_color32();

            if compact {
                // Same framed button as the wide layout, just placed first
                // so the search field can fill the remainder.
                let import_resp = ui.add(
                    egui::Button::new(
                        RichText::new(format!("{} Import", regular::DOWNLOAD_SIMPLE))
                            .size(11.0)
                            .color(accent),
                    )
                    .rounding(4.0)
                    .stroke(egui::Stroke::new(1.0, accent.linear_multiply(0.4))),
                );
                if import_resp.on_hover_text("Import 3D model").clicked() {
                    state.import_clicked = true;
                }

                ui.add_space(4.0);

                let avail = ui.available_width().max(60.0);
                ui.add(
                    egui::TextEdit::singleline(&mut state.search)
                        .desired_width(avail - 8.0)
                        .hint_text(format!("{}", regular::MAGNIFYING_GLASS)),
                );
            } else {
                if state.view_mode == ViewMode::Grid {
                    ui.spacing_mut().slider_width = 60.0;
                    ui.add(
                        egui::Slider::new(&mut state.zoom, 0.5..=1.5)
                            .show_value(false)
                            .text(""),
                    );
                }

                let grid_color = if state.view_mode == ViewMode::Grid { text_primary } else { text_muted };
                let list_color = if state.view_mode == ViewMode::List { text_primary } else { text_muted };
                if icon_button(ui, regular::LIST, "List view", list_color) {
                    state.view_mode = ViewMode::List;
                }
                if icon_button(ui, regular::SQUARES_FOUR, "Grid view", grid_color) {
                    state.view_mode = ViewMode::Grid;
                }

                ui.add_space(4.0);

                let import_resp = ui.add(
                    egui::Button::new(
                        RichText::new(format!("{} Import", regular::DOWNLOAD_SIMPLE))
                            .size(11.0)
                            .color(accent),
                    )
                    .rounding(4.0)
                    .stroke(egui::Stroke::new(1.0, accent.linear_multiply(0.4))),
                );
                if import_resp.on_hover_text("Import 3D model").clicked() {
                    state.import_clicked = true;
                }

                ui.add_space(4.0);

                let sort_icon = match state.sort_direction {
                    SortDirection::Ascending => regular::SORT_ASCENDING,
                    SortDirection::Descending => regular::SORT_DESCENDING,
                };
                if icon_button(ui, sort_icon, "Toggle sort direction", text_muted) {
                    state.sort_direction = match state.sort_direction {
                        SortDirection::Ascending => SortDirection::Descending,
                        SortDirection::Descending => SortDirection::Ascending,
                    };
                }

                egui::ComboBox::from_id_salt("sort_mode")
                    .selected_text(RichText::new(state.sort_mode.label()).size(11.0).color(text_muted))
                    .width(55.0)
                    .height(120.0)
                    .show_ui(ui, |ui| {
                        for mode in [SortMode::Name, SortMode::DateModified, SortMode::Type, SortMode::Size] {
                            ui.selectable_value(&mut state.sort_mode, mode, mode.label());
                        }
                    });

                ui.add_space(4.0);

                ui.add(
                    egui::TextEdit::singleline(&mut state.search)
                        .desired_width(120.0)
                        .hint_text(format!("{} Search...", regular::MAGNIFYING_GLASS)),
                );
            }
        });
    });
}
