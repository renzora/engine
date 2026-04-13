//! Lifecycle Settings Panel — network config + project info.

use bevy::prelude::*;
use bevy_egui::egui;

use renzora::core::CurrentProject;
use renzora_editor_framework::{EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

pub struct LifecycleSettingsPanel;

impl EditorPanel for LifecycleSettingsPanel {
    fn id(&self) -> &str {
        "lifecycle_settings"
    }

    fn title(&self) -> &str {
        "Lifecycle Settings"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::GEAR_SIX)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }

    fn min_size(&self) -> [f32; 2] {
        [200.0, 150.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let muted = theme.text.muted.to_color32();

        let project = world.get_resource::<CurrentProject>();

        // ── Project Info ──
        ui.label(egui::RichText::new("Project").size(13.0));
        ui.separator();

        if let Some(p) = &project {
            egui::Grid::new("lc_project_info")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Name").size(11.0).color(muted));
                    ui.label(&p.config.name);
                    ui.end_row();

                    ui.label(egui::RichText::new("Version").size(11.0).color(muted));
                    ui.label(&p.config.version);
                    ui.end_row();

                    ui.label(egui::RichText::new("Main Scene").size(11.0).color(muted));
                    ui.label(&p.config.main_scene);
                    ui.end_row();
                });
        } else {
            ui.label(
                egui::RichText::new("No project loaded")
                    .size(11.0)
                    .color(muted),
            );
        }

        ui.add_space(12.0);

        // ── Network Config ──
        ui.label(egui::RichText::new("Network Configuration").size(13.0));
        ui.separator();

        let net_config = project.and_then(|p| p.config.network.as_ref());

        match net_config {
            Some(config) => {
                egui::Grid::new("lc_net_settings")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Server Address").size(11.0).color(muted));
                        ui.label(&config.server_addr);
                        ui.end_row();

                        ui.label(egui::RichText::new("Port").size(11.0).color(muted));
                        ui.label(format!("{}", config.port));
                        ui.end_row();

                        ui.label(egui::RichText::new("Transport").size(11.0).color(muted));
                        ui.label(&config.transport);
                        ui.end_row();

                        ui.label(egui::RichText::new("Tick Rate").size(11.0).color(muted));
                        ui.label(format!("{} Hz", config.tick_rate));
                        ui.end_row();

                        ui.label(egui::RichText::new("Max Clients").size(11.0).color(muted));
                        ui.label(format!("{}", config.max_clients));
                        ui.end_row();
                    });

                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Edit [network] in project.toml to change settings.")
                        .size(10.0)
                        .color(muted),
                );
            }
            None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::CLOUD_SLASH)
                            .size(24.0)
                            .color(muted),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Networking not configured")
                            .size(12.0)
                            .color(muted),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Add [network] to project.toml\nto enable networking.")
                            .size(10.0)
                            .color(muted),
                    );
                });
            }
        }
    }
}
