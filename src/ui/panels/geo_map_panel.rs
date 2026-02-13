use bevy::prelude::*;
use bevy_egui::egui;

use crate::geo_map::style::{GeoMapStyle, GeoFeatureToggles};
use crate::geo_map::tile_cache::GeoTileCache;
use crate::theming::Theme;

/// State resource for the Geo Map customization panel
#[derive(Resource)]
pub struct GeoMapPanelState {
    /// Currently selected GeoMapData entity
    pub selected_entity: Option<Entity>,
    /// Section collapse state
    pub style_open: bool,
    pub features_open: bool,
    pub markers_open: bool,
    pub overlays_open: bool,
    pub cache_open: bool,
    /// Feature toggles (applied to tile URLs)
    pub features: GeoFeatureToggles,
    /// Custom URL field (synced with GeoMapData)
    pub custom_url: String,
    /// Current style selection
    pub current_style: GeoMapStyle,
}

impl Default for GeoMapPanelState {
    fn default() -> Self {
        Self {
            selected_entity: None,
            style_open: true,
            features_open: true,
            markers_open: false,
            overlays_open: false,
            cache_open: false,
            features: GeoFeatureToggles::default(),
            custom_url: String::new(),
            current_style: GeoMapStyle::Street,
        }
    }
}

pub fn render_geo_map_panel_content(
    ui: &mut egui::Ui,
    state: &mut GeoMapPanelState,
    cache: Option<&GeoTileCache>,
    theme: &Theme,
) {
    let text_color = theme.text.primary.to_color32();
    let muted_color = theme.text.secondary.to_color32();

    ui.spacing_mut().item_spacing.y = 4.0;

    // ── Map Style Section ──
    egui::CollapsingHeader::new(egui::RichText::new("Map Style").color(text_color))
        .default_open(state.style_open)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Style preset").color(muted_color));
                egui::ComboBox::from_id_salt("geo_panel_style")
                    .selected_text(state.current_style.label())
                    .show_ui(ui, |ui| {
                        for style in GeoMapStyle::ALL {
                            ui.selectable_value(&mut state.current_style, *style, style.label());
                        }
                    });
            });

            if state.current_style == GeoMapStyle::Custom {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Custom URL").color(muted_color));
                    ui.text_edit_singleline(&mut state.custom_url);
                });
            }
        });

    ui.separator();

    // ── Features Section ──
    egui::CollapsingHeader::new(egui::RichText::new("Features").color(text_color))
        .default_open(state.features_open)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.features.roads, "Roads");
                ui.checkbox(&mut state.features.labels, "Labels");
                ui.checkbox(&mut state.features.water, "Water");
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.features.buildings, "Buildings");
                ui.checkbox(&mut state.features.parks, "Parks");
                ui.checkbox(&mut state.features.transit, "Transit");
            });
        });

    ui.separator();

    // ── Markers Section ──
    egui::CollapsingHeader::new(egui::RichText::new("Markers").color(text_color))
        .default_open(state.markers_open)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Add GeoMarker components to entities to create map pins.").color(muted_color));
        });

    ui.separator();

    // ── Overlays Section ──
    egui::CollapsingHeader::new(egui::RichText::new("Overlays").color(text_color))
        .default_open(state.overlays_open)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Overlay support coming soon.").color(muted_color));
        });

    ui.separator();

    // ── Cache Section ──
    egui::CollapsingHeader::new(egui::RichText::new("Cache").color(text_color))
        .default_open(state.cache_open)
        .show(ui, |ui| {
            if let Some(cache) = cache {
                let disk_count = cache.disk_tile_count;
                let disk_mb = cache.estimate_disk_usage() as f64 / (1024.0 * 1024.0);
                let mem_count = cache.tiles.len();

                ui.label(egui::RichText::new(format!("Tiles in memory: {}", mem_count)).color(muted_color));
                ui.label(egui::RichText::new(format!("Tiles on disk: {}", disk_count)).color(muted_color));
                ui.label(egui::RichText::new(format!("Disk usage: {:.1} MB", disk_mb)).color(muted_color));
            } else {
                ui.label(egui::RichText::new("No tile cache available.").color(muted_color));
            }
        });
}
