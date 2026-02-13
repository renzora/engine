use bevy::prelude::*;

/// Convert WGS84 latitude/longitude to Web Mercator (EPSG:3857) meters
pub fn lat_lon_to_mercator(lat: f64, lon: f64) -> (f64, f64) {
    let x = lon * 20037508.34 / 180.0;
    let lat_rad = lat.to_radians();
    let y = ((std::f64::consts::PI / 4.0 + lat_rad / 2.0).tan()).ln() / std::f64::consts::PI * 20037508.34;
    (x, y)
}

/// Convert Web Mercator meters to WGS84 latitude/longitude
pub fn mercator_to_lat_lon(x: f64, y: f64) -> (f64, f64) {
    let lon = x / 20037508.34 * 180.0;
    let lat_rad = (std::f64::consts::PI * y / 20037508.34).exp();
    let lat = (2.0 * lat_rad.atan() - std::f64::consts::PI / 2.0).to_degrees();
    (lat, lon)
}

/// Convert a geographic coordinate to a local 3D position relative to a map center
pub fn geo_to_local(
    lat: f64,
    lon: f64,
    center_lat: f64,
    center_lon: f64,
    tile_world_size: f32,
    zoom: u8,
) -> Vec3 {
    let n = 2.0_f64.powi(zoom as i32);
    let (cx, cy) = lat_lon_to_mercator(center_lat, center_lon);
    let (px, py) = lat_lon_to_mercator(lat, lon);

    // Meters per tile at this zoom level
    let total_mercator = 2.0 * 20037508.34;
    let meters_per_tile = total_mercator / n;

    // Convert mercator offset to tile units, then to world units
    let dx = (px - cx) / meters_per_tile * tile_world_size as f64;
    let dz = -(py - cy) / meters_per_tile * tile_world_size as f64; // flip Z (north = -Z in Bevy)

    Vec3::new(dx as f32, 0.0, dz as f32)
}

/// Convert a local 3D position back to geographic coordinates
#[allow(dead_code)]
pub fn local_to_geo(
    pos: Vec3,
    center_lat: f64,
    center_lon: f64,
    tile_world_size: f32,
    zoom: u8,
) -> (f64, f64) {
    let n = 2.0_f64.powi(zoom as i32);
    let (cx, cy) = lat_lon_to_mercator(center_lat, center_lon);

    let total_mercator = 2.0 * 20037508.34;
    let meters_per_tile = total_mercator / n;

    let mx = cx + pos.x as f64 / tile_world_size as f64 * meters_per_tile;
    let my = cy - pos.z as f64 / tile_world_size as f64 * meters_per_tile;

    mercator_to_lat_lon(mx, my)
}
