use bevy::prelude::*;
use bevy::image::Image as BevyImage;
use bevy::asset::RenderAssetUsages;

use super::coordinate::geo_to_local;
use super::data::*;
use super::tile::TileCoord;
use super::tile_cache::GeoTileCache;
use super::tile_fetcher;
use super::style::GeoFeatureToggles;

/// System 1: Watch for GeoMapData changes and request needed tiles
pub fn geo_tile_request_system(
    query: Query<&GeoMapData, Changed<GeoMapData>>,
    mut cache: ResMut<GeoTileCache>,
) {
    for map_data in query.iter() {
        let center = TileCoord::from_lat_lon(map_data.latitude, map_data.longitude, map_data.zoom);
        let radius = map_data.tile_radius as i32;
        let max_tile = 2u32.pow(map_data.zoom as u32);

        let url_template = map_data.effective_url();
        let features = GeoFeatureToggles::default();
        let url_template = features.apply_to_url(&url_template);

        let sender = cache.sender.clone();
        let cache_dir = cache.cache_dir.clone();

        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let tx = (center.x as i64 + dx as i64).rem_euclid(max_tile as i64) as u32;
                let ty = center.y as i64 + dz as i64;
                if ty < 0 || ty >= max_tile as i64 {
                    continue;
                }
                let ty = ty as u32;

                let coord = TileCoord { x: tx, y: ty, z: map_data.zoom };

                if cache.contains(&coord) || cache.is_pending(&coord) {
                    continue;
                }

                let url = coord.to_url(&url_template);
                cache.mark_pending(coord);
                tile_fetcher::fetch_tile(
                    coord,
                    url,
                    cache_dir.clone(),
                    sender.clone(),
                );
            }
        }
    }
}

/// System 2: Poll the tile receiver channel, decode PNG, create Image handles
pub fn geo_tile_receive_system(
    mut cache: ResMut<GeoTileCache>,
    mut images: ResMut<Assets<BevyImage>>,
) {
    for _ in 0..8 {
        let Some(fetched) = cache.try_recv() else {
            break;
        };

        let img = match image::load_from_memory(&fetched.bytes) {
            Ok(img) => img.into_rgba8(),
            Err(e) => {
                warn!("Failed to decode tile image: {}", e);
                cache.pending.remove(&fetched.coord);
                continue;
            }
        };

        let (width, height) = img.dimensions();
        let bevy_image = BevyImage::new(
            bevy::render::render_resource::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            img.into_raw(),
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );

        let handle = images.add(bevy_image);
        cache.insert(fetched.coord, handle);
    }
}

/// System 3: Stitch cached tiles into a single atlas texture and apply
/// it to the entity's existing material. The texture wraps around the
/// mesh using whatever UVs it already has (sphere, plane, etc.).
pub fn geo_atlas_build_system(
    mut query: Query<(&GeoMapData, &mut GeoMapAtlas, &MeshMaterial3d<StandardMaterial>)>,
    cache: Res<GeoTileCache>,
    mut images: ResMut<Assets<BevyImage>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (map_data, mut atlas, mat_handle) in query.iter_mut() {
        let center = TileCoord::from_lat_lon(map_data.latitude, map_data.longitude, map_data.zoom);
        let radius = map_data.tile_radius as i32;
        let grid = map_data.grid_size();
        let max_tile = 2u32.pow(map_data.zoom as u32);
        let expected = grid * grid;

        // Check if config changed since last build
        let config_changed = atlas.built_lat != map_data.latitude
            || atlas.built_lon != map_data.longitude
            || atlas.built_zoom != map_data.zoom
            || atlas.built_radius != map_data.tile_radius
            || atlas.built_generation != map_data.generation;

        // Count how many tiles are available now
        let mut available = 0u32;
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let tx = (center.x as i64 + dx as i64).rem_euclid(max_tile as i64) as u32;
                let ty = center.y as i64 + dz as i64;
                if ty < 0 || ty >= max_tile as i64 {
                    continue;
                }
                let coord = TileCoord { x: tx, y: ty as u32, z: map_data.zoom };
                if cache.tiles.contains_key(&coord) {
                    available += 1;
                }
            }
        }

        // Skip rebuild if nothing changed
        if !config_changed && available == atlas.tiles_filled {
            continue;
        }

        // Need at least 1 tile to build
        if available == 0 {
            continue;
        }

        // Determine tile pixel size from the first available tile
        let tile_px = {
            let mut found_size = 256u32;
            'outer: for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let tx = (center.x as i64 + dx as i64).rem_euclid(max_tile as i64) as u32;
                    let ty = center.y as i64 + dz as i64;
                    if ty < 0 || ty >= max_tile as i64 { continue; }
                    let coord = TileCoord { x: tx, y: ty as u32, z: map_data.zoom };
                    if let Some(h) = cache.tiles.get(&coord) {
                        if let Some(img) = images.get(h) {
                            found_size = img.width();
                            break 'outer;
                        }
                    }
                }
            }
            found_size
        };

        let atlas_w = grid * tile_px;
        let atlas_h = grid * tile_px;
        let mut atlas_data = vec![80u8; (atlas_w * atlas_h * 4) as usize];

        // Blit each tile into the atlas
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let tx = (center.x as i64 + dx as i64).rem_euclid(max_tile as i64) as u32;
                let ty = center.y as i64 + dz as i64;
                if ty < 0 || ty >= max_tile as i64 { continue; }
                let coord = TileCoord { x: tx, y: ty as u32, z: map_data.zoom };

                let Some(handle) = cache.tiles.get(&coord) else { continue };
                let Some(tile_img) = images.get(handle) else { continue };
                let Some(tile_data) = tile_img.data.as_ref() else { continue };

                let gx = (dx + radius) as u32;
                let gz = (dz + radius) as u32;
                let src_w = tile_img.width().min(tile_px);
                let src_h = tile_img.height().min(tile_px);

                for py in 0..src_h {
                    for px in 0..src_w {
                        let src_idx = ((py * tile_img.width() + px) * 4) as usize;
                        let dst_x = gx * tile_px + px;
                        let dst_y = gz * tile_px + py;
                        let dst_idx = ((dst_y * atlas_w + dst_x) * 4) as usize;

                        if src_idx + 3 < tile_data.len() && dst_idx + 3 < atlas_data.len() {
                            atlas_data[dst_idx]     = tile_data[src_idx];
                            atlas_data[dst_idx + 1] = tile_data[src_idx + 1];
                            atlas_data[dst_idx + 2] = tile_data[src_idx + 2];
                            atlas_data[dst_idx + 3] = tile_data[src_idx + 3];
                        }
                    }
                }
            }
        }

        // Create the atlas image
        let bevy_atlas = BevyImage::new(
            bevy::render::render_resource::Extent3d {
                width: atlas_w,
                height: atlas_h,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            atlas_data,
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );

        let new_handle = images.add(bevy_atlas);

        // Apply to the entity's existing material
        if let Some(mat) = materials.get_mut(mat_handle) {
            mat.base_color_texture = Some(new_handle.clone());
            mat.base_color = Color::WHITE;
            mat.unlit = true;
        }

        // Update atlas tracking
        atlas.atlas_handle = Some(new_handle);
        atlas.tiles_filled = available;
        atlas.tiles_expected = expected;
        atlas.built_lat = map_data.latitude;
        atlas.built_lon = map_data.longitude;
        atlas.built_zoom = map_data.zoom;
        atlas.built_radius = map_data.tile_radius;
        atlas.built_generation = map_data.generation;
    }
}

/// System 4: Sync GeoPositionData entities to local 3D position relative to nearest GeoMap
pub fn geo_position_sync_system(
    mut pos_query: Query<(&GeoPositionData, &mut Transform), (Changed<GeoPositionData>, Without<GeoMapData>)>,
    map_query: Query<&GeoMapData>,
) {
    let Some(map_data) = map_query.iter().next() else {
        return;
    };

    let tile_world_size = 10.0;
    for (pos_data, mut transform) in pos_query.iter_mut() {
        let local = geo_to_local(
            pos_data.latitude,
            pos_data.longitude,
            map_data.latitude,
            map_data.longitude,
            tile_world_size,
            map_data.zoom,
        );
        transform.translation = Vec3::new(local.x, pos_data.altitude, local.z);
    }
}

/// System 5: Sync GeoMarkerData entities to position on the map
pub fn geo_marker_sync_system(
    mut marker_query: Query<(&GeoMarkerData, &mut Transform), (Changed<GeoMarkerData>, Without<GeoMapData>)>,
    map_query: Query<&GeoMapData>,
) {
    let Some(map_data) = map_query.iter().next() else {
        return;
    };

    let tile_world_size = 10.0;
    for (marker, mut transform) in marker_query.iter_mut() {
        let local = geo_to_local(
            marker.latitude,
            marker.longitude,
            map_data.latitude,
            map_data.longitude,
            tile_world_size,
            map_data.zoom,
        );
        transform.translation = Vec3::new(local.x, marker.scale * 0.5, local.z);
        transform.scale = Vec3::splat(marker.scale);
    }
}
