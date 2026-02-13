use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::style::GeoMapStyle;

/// Core map component — controls what region of the world is displayed.
/// Stitches map tiles into an atlas texture and applies it to the entity's material,
/// wrapping around whatever mesh geometry is present (sphere, plane, etc.).
#[derive(Component, Debug, Clone, Serialize, Deserialize, Reflect)]
#[reflect(Component)]
pub struct GeoMapData {
    pub latitude: f64,
    pub longitude: f64,
    pub zoom: u8,
    /// Number of tiles in each direction from center (grid = (2r+1)^2)
    pub tile_radius: u32,
    #[serde(default)]
    pub style: GeoMapStyle,
    #[serde(default)]
    pub tile_url_template: String,
    #[serde(default = "default_true")]
    pub auto_update: bool,
    /// Transient — not serialized
    #[serde(skip)]
    #[reflect(ignore)]
    pub loading: bool,
    /// Bumped to force tile refresh
    #[serde(skip)]
    #[reflect(ignore)]
    pub generation: u64,
}

fn default_true() -> bool {
    true
}

impl Default for GeoMapData {
    fn default() -> Self {
        Self {
            latitude: 40.7128,
            longitude: -74.006,
            zoom: 15,
            tile_radius: 2,
            style: GeoMapStyle::Street,
            tile_url_template: GeoMapStyle::Street.default_url().to_string(),
            auto_update: true,
            loading: false,
            generation: 0,
        }
    }
}

impl GeoMapData {
    pub fn effective_url(&self) -> String {
        if self.style == GeoMapStyle::Custom {
            self.tile_url_template.clone()
        } else {
            self.style.default_url().to_string()
        }
    }

    /// Total number of tiles across one axis of the atlas grid
    pub fn grid_size(&self) -> u32 {
        2 * self.tile_radius + 1
    }
}

/// Tracks the stitched atlas texture for a GeoMap entity.
/// Inserted alongside GeoMapData by the custom add function.
#[derive(Component, Default)]
pub struct GeoMapAtlas {
    /// The stitched atlas image applied to the material
    pub atlas_handle: Option<Handle<Image>>,
    /// Generation that was last built (matches GeoMapData.generation)
    pub built_generation: u64,
    /// Number of tiles that were available when atlas was last built
    pub tiles_filled: u32,
    /// Total tiles expected for current config
    pub tiles_expected: u32,
    /// Snapshot of lat/lon/zoom/radius/style the atlas was built for
    pub built_lat: f64,
    pub built_lon: f64,
    pub built_zoom: u8,
    pub built_radius: u32,
}

/// Place any entity at a geographic position relative to a nearby GeoMap
#[derive(Component, Debug, Clone, Serialize, Deserialize, Reflect, Default)]
#[reflect(Component)]
pub struct GeoPositionData {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f32,
    #[serde(default)]
    pub align_to_terrain: bool,
}

/// Visual map pin/marker
#[derive(Component, Debug, Clone, Serialize, Deserialize, Reflect)]
#[reflect(Component)]
pub struct GeoMarkerData {
    pub latitude: f64,
    pub longitude: f64,
    pub label: String,
    pub color: [f32; 4],
    pub scale: f32,
    #[serde(default = "default_true")]
    pub show_label: bool,
}

impl Default for GeoMarkerData {
    fn default() -> Self {
        Self {
            latitude: 40.7128,
            longitude: -74.006,
            label: "Marker".to_string(),
            color: [1.0, 0.2, 0.2, 1.0],
            scale: 1.0,
            show_label: true,
        }
    }
}
