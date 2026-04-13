//! Networking editor panels for the Renzora editor.
//!
//! Panels: Network Monitor, Network Entities, Network Settings.

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_editor_framework::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_network::{NetworkStatus, Networked, NetworkId, NetworkOwner, OwnerKind};
use renzora_network::status::ConnectionState;
use renzora::core::CurrentProject;
use renzora_theme::ThemeManager;

// ============================================================================
// Network Monitor Panel
// ============================================================================

struct NetworkMonitorPanel;

impl EditorPanel for NetworkMonitorPanel {
    fn id(&self) -> &str { "network_monitor" }
    fn title(&self) -> &str { "Network Monitor" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::WIFI_HIGH) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Bottom }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let muted = theme.text.muted.to_color32();

        let Some(status) = world.get_resource::<NetworkStatus>() else {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::WIFI_SLASH)
                        .size(32.0)
                        .color(muted),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Network plugin not loaded")
                        .size(13.0)
                        .color(muted),
                );
            });
            return;
        };

        match status.state {
            ConnectionState::Disconnected => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::WIFI_SLASH)
                            .size(32.0)
                            .color(muted),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Disconnected")
                            .size(13.0)
                            .color(muted),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Configure networking in Network Settings to get started.")
                            .size(11.0)
                            .color(muted),
                    );
                });
            }
            ConnectionState::Connecting => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Connecting...")
                            .size(13.0)
                            .color(muted),
                    );
                });
            }
            ConnectionState::Connected => {
                ui.vertical(|ui| {
                    // Connection header
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(egui_phosphor::regular::WIFI_HIGH)
                                .size(16.0)
                                .color(egui::Color32::from_rgb(80, 200, 120)),
                        );
                        ui.label(
                            egui::RichText::new("Connected")
                                .size(13.0)
                                .color(egui::Color32::from_rgb(80, 200, 120)),
                        );
                        if status.is_server {
                            ui.label(
                                egui::RichText::new("(Server)")
                                    .size(11.0)
                                    .color(muted),
                            );
                        }
                    });

                    ui.separator();

                    // Stats
                    if !status.is_server {
                        egui::Grid::new("net_stats").num_columns(2).show(ui, |ui| {
                            ui.label(egui::RichText::new("RTT").size(11.0).color(muted));
                            ui.label(format!("{:.1} ms", status.rtt_ms));
                            ui.end_row();

                            ui.label(egui::RichText::new("Jitter").size(11.0).color(muted));
                            ui.label(format!("{:.1} ms", status.jitter_ms));
                            ui.end_row();

                            ui.label(egui::RichText::new("Packet Loss").size(11.0).color(muted));
                            ui.label(format!("{:.1}%", status.packet_loss * 100.0));
                            ui.end_row();

                            if let Some(id) = status.client_id {
                                ui.label(egui::RichText::new("Client ID").size(11.0).color(muted));
                                ui.label(format!("{}", id));
                                ui.end_row();
                            }
                        });
                    } else {
                        // Server view: show connected clients
                        ui.label(
                            egui::RichText::new(format!(
                                "Clients: {}",
                                status.connected_clients.len()
                            ))
                            .size(12.0),
                        );
                        ui.add_space(4.0);

                        for client in &status.connected_clients {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(egui_phosphor::regular::USER)
                                        .size(12.0)
                                        .color(muted),
                                );
                                ui.label(format!("ID: {}", client.client_id));
                                ui.label(
                                    egui::RichText::new(format!("{:.0}ms", client.rtt_ms))
                                        .size(11.0)
                                        .color(muted),
                                );
                            });
                        }

                        if status.connected_clients.is_empty() {
                            ui.label(
                                egui::RichText::new("Waiting for clients...")
                                    .size(11.0)
                                    .color(muted),
                            );
                        }
                    }
                });
            }
        }
    }
}

// ============================================================================
// Network Entities Panel
// ============================================================================

struct NetworkEntitiesPanel;

impl EditorPanel for NetworkEntitiesPanel {
    fn id(&self) -> &str { "network_entities" }
    fn title(&self) -> &str { "Network Entities" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::SHARE_NETWORK) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Left }
    fn min_size(&self) -> [f32; 2] { [180.0, 100.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let muted = theme.text.muted.to_color32();

        // Gather networked entities by iterating archetypes
        let mut networked_entities: Vec<(Entity, Option<String>, Option<u64>, Option<OwnerKind>)> = Vec::new();
        for archetype in world.archetypes().iter() {
            for arch_entity in archetype.entities() {
                let entity = arch_entity.id();
                if world.get::<Networked>(entity).is_some() {
                    let name = world.get::<Name>(entity).map(|n| n.as_str().to_string());
                    let net_id = world.get::<NetworkId>(entity).map(|n| n.0);
                    let owner = world.get::<NetworkOwner>(entity).map(|o| o.0);
                    networked_entities.push((entity, name, net_id, owner));
                }
            }
        }

        if networked_entities.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::SHARE_NETWORK)
                        .size(32.0)
                        .color(muted),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("No networked entities")
                        .size(13.0)
                        .color(muted),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Add a Networked component to entities\nto see them listed here.")
                        .size(11.0)
                        .color(muted),
                );
            });
            return;
        }

        ui.label(
            egui::RichText::new(format!("{} networked entities", networked_entities.len()))
                .size(12.0),
        );
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (entity, name, net_id, owner) in &networked_entities {
                ui.horizontal(|ui| {
                    // Entity name
                    let fallback = format!("{:?}", entity);
                    let display_name = name
                        .as_deref()
                        .unwrap_or(&fallback);
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::CUBE)
                            .size(12.0)
                            .color(muted),
                    );
                    ui.label(display_name);

                    // Network ID
                    if let Some(nid) = net_id {
                        ui.label(
                            egui::RichText::new(format!("#{}", nid))
                                .size(10.0)
                                .color(muted),
                        );
                    }

                    // Owner
                    if let Some(own) = owner {
                        let owner_str = match own {
                            OwnerKind::Server => "Server".to_string(),
                            OwnerKind::Client(id) => format!("Client {}", id),
                        };
                        ui.label(
                            egui::RichText::new(owner_str)
                                .size(10.0)
                                .color(muted),
                        );
                    }
                });
            }
        });
    }
}

// ============================================================================
// Network Settings Panel
// ============================================================================

struct NetworkSettingsPanel;

impl EditorPanel for NetworkSettingsPanel {
    fn id(&self) -> &str { "network_settings" }
    fn title(&self) -> &str { "Network Settings" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::GEAR_SIX) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world.get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let muted = theme.text.muted.to_color32();

        let project = world.get_resource::<CurrentProject>();
        let net_config = project.and_then(|p| p.config.network.as_ref());

        match net_config {
            Some(config) => {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Network Configuration")
                            .size(13.0),
                    );
                    ui.separator();

                    egui::Grid::new("net_settings").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
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

                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("Edit [network] in project.toml to change settings.")
                            .size(10.0)
                            .color(muted),
                    );
                });
            }
            None => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::CLOUD_SLASH)
                            .size(32.0)
                            .color(muted),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Networking not configured")
                            .size(13.0)
                            .color(muted),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Add [network] to project.toml to configure\nserver mode, transport, and connection settings.")
                            .size(11.0)
                            .color(muted),
                    );
                });
            }
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct NetworkEditorPlugin;

impl Plugin for NetworkEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] NetworkEditorPlugin");
        app.register_panel(NetworkMonitorPanel);
        app.register_panel(NetworkEntitiesPanel);
        app.register_panel(NetworkSettingsPanel);
    }
}

renzora::add!(NetworkEditorPlugin, Editor);
