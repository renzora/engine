//! Export dialog UI

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, Vec2};

use crate::core::{ExportState, SceneManagerState};
use crate::export::{ExportConfig, run_export, is_target_installed, ExportTarget};
use crate::project::CurrentProject;

use egui_phosphor::regular::{EXPORT, WINDOWS_LOGO, LINUX_LOGO, APPLE_LOGO, FOLDER_OPEN, CHECK, X};

/// Render the export dialog window
pub fn render_export_dialog(
    ctx: &egui::Context,
    export_state: &mut ExportState,
    scene_state: &SceneManagerState,
    current_project: Option<&CurrentProject>,
) {
    if !export_state.show_dialog {
        // Reset targets_checked when dialog is closed so we re-check next time
        export_state.targets_checked = false;
        return;
    }

    // Check targets once when dialog opens
    if !export_state.targets_checked {
        export_state.windows_installed = is_target_installed(&ExportTarget::Windows);
        export_state.linux_installed = is_target_installed(&ExportTarget::Linux);
        export_state.macos_installed = is_target_installed(&ExportTarget::MacOS);
        export_state.macos_arm_installed = is_target_installed(&ExportTarget::MacOSArm);
        export_state.targets_checked = true;
    }

    egui::Window::new(format!("{} Export Game", EXPORT))
        .collapsible(false)
        .resizable(false)
        .default_width(450.0)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;

            // Game Name
            ui.horizontal(|ui| {
                ui.label("Game Name:");
                ui.add_sized(
                    Vec2::new(250.0, 20.0),
                    egui::TextEdit::singleline(&mut export_state.game_name),
                );
            });

            ui.separator();

            // Target Platforms
            ui.label(RichText::new("Target Platforms").strong());
            ui.horizontal(|ui| {
                // Windows
                ui.checkbox(&mut export_state.target_windows, "");
                ui.label(RichText::new(WINDOWS_LOGO).size(16.0));
                ui.label("Windows");
                if !export_state.windows_installed {
                    ui.label(RichText::new("(not installed)").color(Color32::YELLOW).small());
                }
            });

            ui.horizontal(|ui| {
                // Linux
                ui.checkbox(&mut export_state.target_linux, "");
                ui.label(RichText::new(LINUX_LOGO).size(16.0));
                ui.label("Linux");
                if !export_state.linux_installed {
                    ui.label(RichText::new("(not installed)").color(Color32::YELLOW).small());
                }
            });

            ui.horizontal(|ui| {
                // macOS Intel
                ui.checkbox(&mut export_state.target_macos_intel, "");
                ui.label(RichText::new(APPLE_LOGO).size(16.0));
                ui.label("macOS (Intel)");
                if !export_state.macos_installed {
                    ui.label(RichText::new("(not installed)").color(Color32::YELLOW).small());
                }
            });

            ui.horizontal(|ui| {
                // macOS ARM
                ui.checkbox(&mut export_state.target_macos_arm, "");
                ui.label(RichText::new(APPLE_LOGO).size(16.0));
                ui.label("macOS (Apple Silicon)");
                if !export_state.macos_arm_installed {
                    ui.label(RichText::new("(not installed)").color(Color32::YELLOW).small());
                }
            });

            ui.separator();

            // Build Options
            ui.label(RichText::new("Build Options").strong());
            ui.horizontal(|ui| {
                ui.label("Build Type:");
                ui.radio_value(&mut export_state.build_release, false, "Debug");
                ui.radio_value(&mut export_state.build_release, true, "Release");
            });

            ui.checkbox(&mut export_state.copy_all_assets, "Copy all assets (recommended)");

            ui.separator();

            // Output Directory
            ui.horizontal(|ui| {
                ui.label("Output Directory:");
                let dir_str = export_state.output_dir.to_string_lossy().to_string();
                ui.add_enabled_ui(false, |ui| {
                    ui.text_edit_singleline(&mut dir_str.clone());
                });
                if ui.button(RichText::new(FOLDER_OPEN)).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(&export_state.output_dir)
                        .pick_folder()
                    {
                        export_state.output_dir = path;
                    }
                }
            });

            ui.separator();

            // Status/Progress
            if export_state.exporting {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(&export_state.status_message);
                });

                if export_state.progress >= 0.0 {
                    ui.add(egui::ProgressBar::new(export_state.progress).show_percentage());
                }
            } else if !export_state.status_message.is_empty() {
                ui.label(&export_state.status_message);
            }

            // Errors
            if !export_state.errors.is_empty() {
                ui.separator();
                ui.label(RichText::new("Errors:").color(Color32::RED));
                egui::ScrollArea::vertical()
                    .max_height(100.0)
                    .show(ui, |ui| {
                        for error in &export_state.errors {
                            ui.label(RichText::new(error).color(Color32::RED).small());
                        }
                    });
            }

            ui.separator();

            // Buttons
            ui.horizontal(|ui| {
                // Export button
                let can_export = !export_state.exporting
                    && export_state.selected_targets().len() > 0
                    && scene_state.current_scene_path.is_some();

                if ui
                    .add_enabled(
                        can_export,
                        egui::Button::new(RichText::new(format!("{} Export", CHECK)).strong()),
                    )
                    .clicked()
                {
                    // Start export
                    if let (Some(project), Some(scene_path)) =
                        (current_project, &scene_state.current_scene_path)
                    {
                        let config = ExportConfig {
                            game_name: export_state.game_name.clone(),
                            targets: export_state.selected_targets(),
                            build_type: export_state.build_type(),
                            output_dir: export_state.output_dir.clone(),
                            main_scene: scene_path.clone(),
                            project_dir: project.path.clone(),
                            copy_all_assets: export_state.copy_all_assets,
                        };

                        export_state.exporting = true;
                        export_state.status_message = "Starting export...".to_string();
                        export_state.errors.clear();

                        // Run export (this blocks - in a real implementation, use async)
                        let result = run_export(&config);

                        export_state.exporting = false;
                        export_state.status_message = result.message;
                        export_state.errors = result.errors;
                    }
                }

                // Cancel button
                if ui
                    .button(RichText::new(format!("{} Close", X)))
                    .clicked()
                {
                    export_state.show_dialog = false;
                }
            });

            // Validation messages
            if scene_state.current_scene_path.is_none() {
                ui.label(RichText::new("Save your scene before exporting").color(Color32::YELLOW));
            }
            if export_state.selected_targets().is_empty() {
                ui.label(RichText::new("Select at least one target platform").color(Color32::YELLOW));
            }
        });
}
