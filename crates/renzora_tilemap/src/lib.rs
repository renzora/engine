//! Renzora Tilemap — a 2D tilemap runtime.
//!
//! A [`TilemapLayer`] is the authored, scene-saved data: which atlas to use,
//! how big a tile is, and a flat list of placed [`Tile`]s. The renderer turns
//! that into geometry by **chunking** — tiles are grouped into fixed-size
//! buckets and each bucket becomes one `Mesh2d` quad-soup sharing the atlas
//! material. Editing a single tile only rebuilds the chunk it falls in, so
//! painting stays cheap on large maps instead of respawning thousands of
//! per-tile sprite entities.
//!
//! The atlas handle is *not* stored in the saved component — handle ids are
//! runtime-only and don't survive save/load. Instead `TilemapLayer.tileset_path`
//! holds the asset-relative image path (mirroring `SpriteImagePath`), and
//! [`sync_tilesets`] rehydrates the `Handle<Image>` + `ColorMaterial` on load or
//! whenever the path changes.

mod render;

use bevy::asset::AssetServer;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Chunk side length in tiles. A chunk holds up to `CHUNK_TILES²` tiles in one
/// mesh; editing a tile only rebuilds its chunk. 32 keeps each rebuilt mesh
/// small while not fragmenting a typical screen into too many draw calls.
pub const CHUNK_TILES: i32 = 32;

/// One placed tile: a grid cell and the atlas index drawn there.
///
/// A flat `Vec<Tile>` (rather than a `HashMap<IVec2, u32>`) is deliberate — the
/// reflection-based scene serializer round-trips a `Vec` of a simple reflected
/// struct reliably, whereas keyed maps of math types do not.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    /// Index into the tileset atlas, row-major (`row * columns + col`).
    pub index: u32,
}

/// The authored tilemap, saved to scenes. Place it on an entity (optionally a
/// child of a `Node2d`); the renderer spawns chunk children under it, so moving
/// this entity's `Transform` moves the whole map.
#[derive(Component, Reflect, Clone, Debug, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct TilemapLayer {
    /// Asset-relative path of the tileset atlas image. Empty → nothing renders
    /// (the inspector shows a drop slot).
    pub tileset_path: String,
    /// World units per tile (square). Matches the sprite convention where a
    /// 1-pixel source maps to 1 world unit, so a 16px atlas tile at
    /// `tile_size = 16` renders 1:1.
    pub tile_size: f32,
    /// Pixel size of one tile in the atlas (square). Used to slice the atlas
    /// into UV cells.
    pub atlas_tile_px: u32,
    /// Atlas columns. `0` → derive from the image width (`image_w / atlas_tile_px`).
    pub columns: u32,
    /// Every placed tile.
    pub tiles: Vec<Tile>,
}

impl Default for TilemapLayer {
    fn default() -> Self {
        Self {
            tileset_path: String::new(),
            tile_size: 16.0,
            atlas_tile_px: 16,
            columns: 0,
            tiles: Vec::new(),
        }
    }
}

impl TilemapLayer {
    /// Grid cell → chunk coordinate.
    pub fn chunk_of(cell: IVec2) -> IVec2 {
        IVec2::new(cell.x.div_euclid(CHUNK_TILES), cell.y.div_euclid(CHUNK_TILES))
    }

    /// Atlas columns actually in effect, given the loaded image width in pixels.
    pub fn effective_columns(&self, image_width_px: f32) -> u32 {
        if self.columns > 0 {
            self.columns
        } else if self.atlas_tile_px > 0 {
            ((image_width_px / self.atlas_tile_px as f32).floor() as u32).max(1)
        } else {
            1
        }
    }

    /// The index drawn at `cell`, if any.
    pub fn get(&self, cell: IVec2) -> Option<u32> {
        self.tiles
            .iter()
            .find(|t| t.x == cell.x && t.y == cell.y)
            .map(|t| t.index)
    }

    /// Paint `index` at `cell`, replacing any existing tile there. Returns
    /// `true` if anything changed (so callers can skip a needless rebuild).
    pub fn set(&mut self, cell: IVec2, index: u32) -> bool {
        if let Some(t) = self.tiles.iter_mut().find(|t| t.x == cell.x && t.y == cell.y) {
            if t.index == index {
                return false;
            }
            t.index = index;
            true
        } else {
            self.tiles.push(Tile {
                x: cell.x,
                y: cell.y,
                index,
            });
            true
        }
    }

    /// Erase any tile at `cell`. Returns `true` if one was removed.
    pub fn erase(&mut self, cell: IVec2) -> bool {
        let before = self.tiles.len();
        self.tiles.retain(|t| !(t.x == cell.x && t.y == cell.y));
        self.tiles.len() != before
    }
}

/// Runtime-only (not saved): the rehydrated atlas image + its 2D material, plus
/// the path they were built from so [`sync_tilesets`] can detect a path change.
#[derive(Component)]
pub struct TilesetHandle {
    pub path: String,
    pub image: Handle<Image>,
    pub material: Handle<ColorMaterial>,
}

/// Runtime-only: the chunk child entities spawned for a layer, so a rebuild can
/// despawn the previous set before regenerating.
#[derive(Component, Default)]
pub struct TilemapChunks(pub Vec<Entity>);

/// Marker on a spawned chunk-mesh child.
#[derive(Component)]
pub struct TilemapChunk;

/// Marker requesting a (re)build. Set when the layer is added/changed or its
/// tileset finishes loading; cleared by [`rebuild_tilemaps`] once the build runs.
#[derive(Component)]
pub struct TilemapDirty;

#[derive(Default)]
pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TilemapPlugin");
        app.register_type::<TilemapLayer>().register_type::<Tile>();
        app.add_systems(
            Update,
            (sync_tilesets, render::rebuild_tilemaps)
                .chain(),
        );
    }
}

renzora::add!(TilemapPlugin);

/// On a new or changed [`TilemapLayer`]: (re)load the atlas when the path
/// changed and mark the layer dirty so the renderer rebuilds. Reloading only on
/// an actual path change keeps tile edits (which also trigger `Changed`) from
/// thrashing the asset server.
fn sync_tilesets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    changed: Query<
        (Entity, &TilemapLayer, Option<&TilesetHandle>),
        Or<(Added<TilemapLayer>, Changed<TilemapLayer>)>,
    >,
) {
    for (entity, layer, existing) in &changed {
        let path_changed = existing.map(|h| h.path != layer.tileset_path).unwrap_or(true);
        if path_changed && !layer.tileset_path.is_empty() {
            let image = load_tileset_nearest(&asset_server, layer.tileset_path.clone());
            let material = materials.add(ColorMaterial {
                texture: Some(image.clone()),
                ..default()
            });
            commands.entity(entity).insert(TilesetHandle {
                path: layer.tileset_path.clone(),
                image,
                material,
            });
        }
        commands.entity(entity).insert(TilemapDirty);
    }
}

/// Load a tileset atlas with **nearest** filtering. Pixel-art tiles must not be
/// linearly filtered — it blurs them and bleeds neighbouring atlas cells across
/// tile seams. (`load_with_settings` is deprecated upstream but is still the
/// per-load sampler override the engine uses; silenced locally.)
#[allow(deprecated)]
fn load_tileset_nearest(asset_server: &AssetServer, path: String) -> Handle<Image> {
    use bevy::image::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
    asset_server.load_with_settings::<Image, ImageLoaderSettings>(
        path,
        |settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());
        },
    )
}
