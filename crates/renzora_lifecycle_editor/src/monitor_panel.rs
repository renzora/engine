//! Lifecycle Monitor Panel — live state during play mode.

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_editor_framework::{EditorPanel, PanelLocation};
use renzora_lifecycle::LifecycleRuntimeState;
use renzora_network::NetworkStatus;
use renzora_network::status::ConnectionState;
use renzora_theme::ThemeManager;

pub struct LifecycleMonitorPanel;

impl EditorPanel for LifecycleMonitorPanel {
    fn id(&self) -> &str {
        "lifecycle_monitor"
    }

    fn title(&self) -> &str {
        "Lifecycle Monitor"
    }

    fn icon(&self) -> Option<&str> {
        Some(egui_phosphor::regular::MONITOR)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn min_size(&self) -> [f32; 2] {
        [200.0, 100.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let muted = theme.text.muted.to_color32();

        let runtime = world.get_resource::<LifecycleRuntimeState>();
        let net_status = world.get_resource::<NetworkStatus>();

        ui.horizontal(|ui| {
            // ── Scene ──
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Scene").size(12.0));
                ui.separator();

                if let Some(rt) = runtime {
                    let scene_name = if rt.current_scene.is_empty() {
                        "(none)"
                    } else {
                        &rt.current_scene
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(egui_phosphor::regular::FILM_STRIP)
                                .size(12.0)
                                .color(muted),
                        );
                        ui.label(scene_name);
                    });
                } else {
                    ui.label(
                        egui::RichText::new("Not running")
                            .size(11.0)
                            .color(muted),
                    );
                }
            });

            ui.separator();

            // ── Variables ──
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Variables").size(12.0));
                ui.separator();

                if let Some(rt) = runtime {
                    if rt.variables.is_empty() {
                        ui.label(
                            egui::RichText::new("(none)")
                                .size(10.0)
                                .color(muted),
                        );
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(80.0)
                            .show(ui, |ui| {
                                for (name, val) in &rt.variables {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(name)
                                                .size(10.0)
                                                .color(muted),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!("{:?}", val))
                                                .size(10.0),
                                        );
                                    });
                                }
                            });
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Not running")
                            .size(11.0)
                            .color(muted),
                    );
                }
            });

            ui.separator();

            // ── Timers ──
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Timers").size(12.0));
                ui.separator();

                if let Some(rt) = runtime {
                    let active_count = rt.active_waits.len() + rt.named_timers.len();
                    if active_count == 0 {
                        ui.label(
                            egui::RichText::new("(none)")
                                .size(10.0)
                                .color(muted),
                        );
                    } else {
                        for (name, timer) in &rt.named_timers {
                            let label = if timer.repeat { " R" } else { "" };
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}{}: {:.1}s",
                                    name, label, timer.remaining
                                ))
                                .size(10.0),
                            );
                        }
                        if !rt.active_waits.is_empty() {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} wait(s)",
                                    rt.active_waits.len()
                                ))
                                .size(10.0)
                                .color(muted),
                            );
                        }
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Not running")
                            .size(11.0)
                            .color(muted),
                    );
                }
            });

            ui.separator();

            // ── Network ──
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Network").size(12.0));
                ui.separator();

                if let Some(status) = net_status {
                    match status.state {
                        ConnectionState::Disconnected => {
                            ui.label(
                                egui::RichText::new("Disconnected")
                                    .size(11.0)
                                    .color(muted),
                            );
                        }
                        ConnectionState::Connecting => {
                            ui.label(
                                egui::RichText::new("Connecting...")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(255, 200, 80)),
                            );
                        }
                        ConnectionState::Connected => {
                            ui.label(
                                egui::RichText::new("Connected")
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(80, 200, 120)),
                            );
                            if !status.is_server {
                                ui.label(
                                    egui::RichText::new(format!("RTT: {:.0}ms", status.rtt_ms))
                                        .size(10.0)
                                        .color(muted),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} clients",
                                        status.connected_clients.len()
                                    ))
                                    .size(10.0)
                                    .color(muted),
                                );
                            }
                        }
                    }
                } else {
                    ui.label(
                        egui::RichText::new("N/A")
                            .size(11.0)
                            .color(muted),
                    );
                }
            });
        });
    }
}
