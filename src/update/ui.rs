//! Update dialog UI

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};
use bevy_egui::EguiContexts;
use egui_phosphor::regular::{
    DOWNLOAD_SIMPLE, WARNING, ARROW_RIGHT, CHECK_CIRCLE, CIRCLE,
};

use super::UpdateState;
use crate::project::AppConfig;
use crate::theming::ThemeManager;

/// State for the update dialog window
#[derive(Resource, Default)]
pub struct UpdateDialogState {
    /// Whether the dialog is open
    pub open: bool,
}

/// Render the update dialog window
pub fn render_update_dialog(
    mut contexts: EguiContexts,
    mut update_state: ResMut<UpdateState>,
    mut dialog_state: ResMut<UpdateDialogState>,
    mut app_config: ResMut<AppConfig>,
    theme_manager: Res<ThemeManager>,
) {
    if !dialog_state.open {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let theme = &theme_manager.active_theme;
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let panel_bg = theme.surfaces.panel.to_color32();
    let popup_bg = theme.surfaces.popup.to_color32();
    let accent = theme.semantic.accent.to_color32();
    let success = theme.semantic.success.to_color32();
    let error = theme.semantic.error.to_color32();
    let border = theme.widgets.border.to_color32();

    let mut close_dialog = false;

    egui::Window::new("Software Update")
        .id(egui::Id::new("update_dialog"))
        .collapsible(false)
        .resizable(false)
        .default_width(450.0)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .frame(egui::Frame::popup(&ctx.style())
            .fill(popup_bg)
            .stroke(Stroke::new(1.0, border))
            .corner_radius(CornerRadius::same(8)))
        .show(ctx, |ui| {
            ui.set_min_width(430.0);

            // Header
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new(DOWNLOAD_SIMPLE).size(24.0).color(accent));
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("Software Update").size(16.0).strong().color(text_primary));
                    if let Some(ref result) = update_state.check_result {
                        if result.update_available {
                            ui.label(RichText::new("A new version is available!").size(12.0).color(success));
                        } else {
                            ui.label(RichText::new("You're up to date").size(12.0).color(text_secondary));
                        }
                    }
                });
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Show error if any
            if let Some(ref err) = update_state.error {
                egui::Frame::new()
                    .fill(Color32::from_rgb(60, 30, 30))
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(WARNING).color(error));
                            ui.add_space(4.0);
                            ui.label(RichText::new(err).size(12.0).color(error));
                        });
                    });
                ui.add_space(8.0);
            }

            // Version info
            if let Some(ref result) = update_state.check_result {
                egui::Frame::new()
                    .fill(panel_bg)
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Current version
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Current Version").size(11.0).color(text_muted));
                                ui.label(RichText::new(&result.current_version).size(14.0).strong().color(text_primary));
                            });

                            ui.add_space(20.0);
                            ui.label(RichText::new(ARROW_RIGHT).size(18.0).color(text_muted));
                            ui.add_space(20.0);

                            // New version
                            ui.vertical(|ui| {
                                ui.label(RichText::new("New Version").size(11.0).color(text_muted));
                                if let Some(ref version) = result.latest_version {
                                    let color = if result.update_available { success } else { text_primary };
                                    ui.label(RichText::new(version).size(14.0).strong().color(color));
                                } else {
                                    ui.label(RichText::new("-").size(14.0).color(text_muted));
                                }
                            });
                        });
                    });

                ui.add_space(8.0);

                // Release notes
                if result.update_available {
                    if let Some(ref notes) = result.release_notes {
                        ui.label(RichText::new("What's New").size(12.0).strong().color(text_primary));
                        ui.add_space(4.0);

                        egui::Frame::new()
                            .fill(panel_bg)
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::same(8))
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(150.0)
                                    .show(ui, |ui| {
                                        // Simple markdown-ish rendering
                                        for line in notes.lines() {
                                            let line = line.trim();
                                            if line.starts_with("# ") {
                                                ui.label(RichText::new(&line[2..]).size(14.0).strong().color(text_primary));
                                            } else if line.starts_with("## ") {
                                                ui.label(RichText::new(&line[3..]).size(13.0).strong().color(text_primary));
                                            } else if line.starts_with("- ") || line.starts_with("* ") {
                                                ui.horizontal(|ui| {
                                                    ui.label(RichText::new(CIRCLE).size(6.0).color(text_muted));
                                                    ui.label(RichText::new(&line[2..]).size(12.0).color(text_secondary));
                                                });
                                            } else if !line.is_empty() {
                                                ui.label(RichText::new(line).size(12.0).color(text_secondary));
                                            }
                                        }
                                    });
                            });
                    }
                }
            } else if update_state.checking {
                // Checking for updates
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(16.0).color(accent));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Checking for updates...").size(12.0).color(text_secondary));
                });
            }

            // Download progress
            if update_state.downloading {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(14.0).color(accent));
                    ui.add_space(8.0);
                    ui.label(RichText::new("Downloading update...").size(12.0).color(text_secondary));
                });

                if let Some(progress) = update_state.download_progress {
                    ui.add_space(4.0);
                    let bar_height = 8.0;
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), bar_height), egui::Sense::hover());

                    ui.painter().rect_filled(rect, 4.0, theme.widgets.inactive_bg.to_color32());
                    if progress > 0.0 {
                        let fill_width = rect.width() * progress.clamp(0.0, 1.0);
                        let fill_rect = egui::Rect::from_min_size(
                            rect.min,
                            Vec2::new(fill_width, rect.height()),
                        );
                        ui.painter().rect_filled(fill_rect, 4.0, accent);
                    }

                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("{:.0}%", progress * 100.0)).size(11.0).color(text_muted));
                }

                ctx.request_repaint();
            }

            ui.add_space(16.0);

            // Buttons
            ui.horizontal(|ui| {
                let button_height = 28.0;

                // View Release button (if URL available)
                if let Some(ref result) = update_state.check_result {
                    if let Some(ref url) = result.release_url {
                        let url_clone = url.clone();
                        if ui.add(egui::Button::new(RichText::new("View Release").size(12.0).color(text_primary))
                            .fill(panel_bg)
                            .stroke(Stroke::new(1.0, border))
                            .corner_radius(CornerRadius::same(4))
                            .min_size(Vec2::new(100.0, button_height))
                        ).clicked() {
                            let _ = open::that(&url_clone);
                        }
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Close/Later button
                    if ui.add(egui::Button::new(RichText::new("Later").size(12.0).color(text_primary))
                        .fill(panel_bg)
                        .stroke(Stroke::new(1.0, border))
                        .corner_radius(CornerRadius::same(4))
                        .min_size(Vec2::new(80.0, button_height))
                    ).clicked() {
                        close_dialog = true;
                    }

                    // Dynamic main action button
                    if let Some(ref result) = update_state.check_result.clone() {
                        if result.update_available {
                            if update_state.downloaded_path.is_some() && !update_state.downloading {
                                // Download complete - show Install & Restart
                                if ui.add(egui::Button::new(RichText::new("Install & Restart").size(12.0).color(Color32::WHITE))
                                    .fill(success)
                                    .corner_radius(CornerRadius::same(4))
                                    .min_size(Vec2::new(120.0, button_height))
                                ).clicked() {
                                    if let Err(e) = update_state.install_and_restart() {
                                        update_state.error = Some(e);
                                    }
                                }
                            } else if !update_state.downloading {
                                // Not downloading - show Download button
                                let can_download = result.download_url.is_some();
                                if ui.add_enabled(can_download,
                                    egui::Button::new(RichText::new("Download").size(12.0).color(Color32::WHITE))
                                        .fill(if can_download { accent } else { panel_bg })
                                        .corner_radius(CornerRadius::same(4))
                                        .min_size(Vec2::new(100.0, button_height))
                                ).clicked() {
                                    update_state.start_download();
                                }

                                // Skip this version button
                                ui.add_space(8.0);
                                if ui.add(egui::Button::new(RichText::new("Skip Version").size(12.0).color(text_muted))
                                    .fill(Color32::TRANSPARENT)
                                    .corner_radius(CornerRadius::same(4))
                                    .min_size(Vec2::new(90.0, button_height))
                                ).clicked() {
                                    if let Some(ref version) = result.latest_version {
                                        app_config.update_config.skipped_version = Some(version.clone());
                                        let _ = app_config.save();
                                    }
                                    close_dialog = true;
                                }
                            }
                        }
                    } else if !update_state.checking {
                        // No result yet - show Check button
                        if ui.add(egui::Button::new(RichText::new("Check Now").size(12.0).color(Color32::WHITE))
                            .fill(accent)
                            .corner_radius(CornerRadius::same(4))
                            .min_size(Vec2::new(100.0, button_height))
                        ).clicked() {
                            update_state.start_check();
                        }
                    }
                });
            });
        });

    if close_dialog {
        dialog_state.open = false;
    }
}
