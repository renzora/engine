use bevy::prelude::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Reflect)]
pub enum GeoMapStyle {
    #[default]
    Street,
    Dark,
    Light,
    Satellite,
    Terrain,
    Retro,
    Custom,
}

impl GeoMapStyle {
    pub const ALL: &'static [GeoMapStyle] = &[
        GeoMapStyle::Street,
        GeoMapStyle::Dark,
        GeoMapStyle::Light,
        GeoMapStyle::Satellite,
        GeoMapStyle::Terrain,
        GeoMapStyle::Retro,
        GeoMapStyle::Custom,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            GeoMapStyle::Street => "Street",
            GeoMapStyle::Dark => "Dark",
            GeoMapStyle::Light => "Light",
            GeoMapStyle::Satellite => "Satellite",
            GeoMapStyle::Terrain => "Terrain",
            GeoMapStyle::Retro => "Retro",
            GeoMapStyle::Custom => "Custom",
        }
    }

    pub fn default_url(&self) -> &'static str {
        match self {
            GeoMapStyle::Street => "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            GeoMapStyle::Dark => "https://cartodb-basemaps-a.global.ssl.fastly.net/dark_all/{z}/{x}/{y}.png",
            GeoMapStyle::Light => "https://cartodb-basemaps-a.global.ssl.fastly.net/light_all/{z}/{x}/{y}.png",
            GeoMapStyle::Satellite => "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
            GeoMapStyle::Terrain => "https://stamen-tiles.a.ssl.fastly.net/terrain/{z}/{x}/{y}.png",
            GeoMapStyle::Retro => "https://stamen-tiles.a.ssl.fastly.net/watercolor/{z}/{x}/{y}.jpg",
            GeoMapStyle::Custom => "",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoFeatureToggles {
    pub roads: bool,
    pub labels: bool,
    pub water: bool,
    pub buildings: bool,
    pub parks: bool,
    pub transit: bool,
}

impl Default for GeoFeatureToggles {
    fn default() -> Self {
        Self {
            roads: true,
            labels: true,
            water: true,
            buildings: true,
            parks: false,
            transit: false,
        }
    }
}

impl GeoFeatureToggles {
    /// Apply feature toggles to a URL template by switching to nolabels variant etc.
    pub fn apply_to_url(&self, base_url: &str) -> String {
        // CartoDB supports nolabels variants
        if !self.labels && base_url.contains("cartodb-basemaps") {
            return base_url
                .replace("/dark_all/", "/dark_nolabels/")
                .replace("/light_all/", "/light_nolabels/");
        }
        base_url.to_string()
    }
}
