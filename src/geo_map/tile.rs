use std::hash::{Hash, Hasher};

/// Represents a slippy map tile coordinate (x, y, zoom)
#[derive(Debug, Clone, Copy, Eq)]
pub struct TileCoord {
    pub x: u32,
    pub y: u32,
    pub z: u8,
}

impl PartialEq for TileCoord {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}

impl Hash for TileCoord {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
        self.z.hash(state);
    }
}

impl TileCoord {
    /// Convert latitude/longitude to a tile coordinate at the given zoom level
    pub fn from_lat_lon(lat: f64, lon: f64, zoom: u8) -> Self {
        let n = 2.0_f64.powi(zoom as i32);
        let x = ((lon + 180.0) / 360.0 * n).floor() as u32;
        let lat_rad = lat.to_radians();
        let y = ((1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / std::f64::consts::PI) / 2.0 * n)
            .floor() as u32;
        let max = n as u32;
        Self {
            x: x.min(max - 1),
            y: y.min(max - 1),
            z: zoom,
        }
    }

    /// Convert tile coordinate to the latitude/longitude of its top-left corner
    pub fn to_lat_lon(&self) -> (f64, f64) {
        let n = 2.0_f64.powi(self.z as i32);
        let lon = self.x as f64 / n * 360.0 - 180.0;
        let lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * self.y as f64 / n)).sinh().atan();
        (lat_rad.to_degrees(), lon)
    }

    /// Replace {z}, {x}, {y} placeholders in a URL template
    pub fn to_url(&self, template: &str) -> String {
        template
            .replace("{z}", &self.z.to_string())
            .replace("{x}", &self.x.to_string())
            .replace("{y}", &self.y.to_string())
    }
}
