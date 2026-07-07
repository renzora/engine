//! Renzora Tilemap — 2D tilemap data types.
//!
//! A [`TilemapLayer`] is the authored, scene-saved palette config: which
//! tileset atlas the layer paints with, how big a tile is, and how the atlas
//! is sliced. The painted tiles themselves are **ordinary sprite entities** —
//! children of the layer entity carrying [`TilemapTile`] (their grid cell)
//! plus the engine's persisted sprite components (`SpriteImagePath`,
//! `SpriteSheet`, `SpriteCustomSize`), so tiles render, save, load, pick and
//! animate exactly like hand-placed sprites in both the editor and the
//! shipped game. (An earlier iteration rendered tiles as chunked meshes; that
//! made tiles invisible to the hierarchy and the 2D picker, so it was
//! replaced with real entities.)
//!
//! The atlas handle is *not* stored in the saved component — handle ids are
//! runtime-only and don't survive save/load. Instead `TilemapLayer.tileset_path`
//! holds the asset-relative image path (mirroring `SpriteImagePath`), and
//! [`sync_tilesets`] rehydrates the `Handle<Image>` on load or whenever the
//! path changes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::asset::{AssetServer, RenderAssetUsages};
use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use serde::{Deserialize, Serialize};

/// The authored tilemap, saved to scenes: the palette configuration painting
/// reads. Its painted tiles are child entities (see [`TilemapTile`]), so
/// moving this entity's `Transform` moves the whole map and toggling its
/// `Visibility` hides it.
#[derive(Component, Reflect, Clone, Debug, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct TilemapLayer {
    /// Asset-relative path of the tileset atlas image. Empty → nothing to
    /// paint with (the panel shows a drop slot).
    pub tileset_path: String,
    /// World units per tile (square). Matches the sprite convention where a
    /// 1-pixel source maps to 1 world unit, so a 16px atlas tile at
    /// `tile_size = 16` renders 1:1.
    pub tile_size: f32,
    /// Pixel size of one tile in the atlas (square). Used to slice the atlas
    /// into cells.
    pub atlas_tile_px: u32,
    /// Atlas columns. `0` → derive from the image width (`image_w / atlas_tile_px`).
    pub columns: u32,
}

impl Default for TilemapLayer {
    fn default() -> Self {
        Self {
            tileset_path: String::new(),
            tile_size: 16.0,
            atlas_tile_px: 16,
            columns: 0,
        }
    }
}

impl TilemapLayer {
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
}

/// A painted tile's grid cell within its parent [`TilemapLayer`]. The visual
/// is the entity's own `Sprite` (+ `SpriteSheet` picking the atlas frame) —
/// this component is what lets the painter find/replace/erase the tile at a
/// cell. Plain `i32` fields (not an `IVec2`) because the reflection-based
/// scene serializer round-trips simple fields more reliably than math types.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct TilemapTile {
    pub x: i32,
    pub y: i32,
}

/// Runtime-only (not saved): the rehydrated atlas image, plus the path it was
/// built from so [`sync_tilesets`] can detect a path change.
#[derive(Component)]
pub struct TilesetHandle {
    pub path: String,
    pub image: Handle<Image>,
}

/// A composite tilemap **object**: the set of atlas cells the user picked in
/// the palette (a tree = trunk + canopy branches), baked into ONE sprite so it
/// selects, moves, rotates, saves and ships as a single entity.
///
/// A single sprite can only show a rectangular slice of one image, so a
/// non-rectangular pick (wide canopy over a narrow trunk) can't just crop the
/// atlas — the bounding box would drag in the neighbouring bush/dirt tiles.
/// Instead [`build_tile_object_sprites`] bakes a fresh texture: it copies just
/// the picked cells into the object's `w × h` grid, leaving the gaps
/// transparent. This component is the saved, image-independent description
/// (mirroring how [`TilemapTile`]/`SpriteSheet` persist while `Sprite` doesn't),
/// so the texture is regenerated on load and in the exported game.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct TileObject {
    /// Asset-relative path of the source tileset atlas.
    pub tileset_path: String,
    /// Pixel size of one atlas cell (square).
    pub tile_px: u32,
    /// Bounding-box width in cells.
    pub w: u32,
    /// Bounding-box height in cells.
    pub h: u32,
    /// The picked cells: each maps a source atlas cell to a slot in the object.
    pub cells: Vec<TileObjectCell>,
}

/// One picked cell of a [`TileObject`]: which atlas cell to copy (`col`, `row`)
/// and where it sits in the object's grid (`dx` right, `dy` down from the
/// bounding-box top-left). Plain `u32` fields so the reflection scene
/// serializer round-trips it cleanly.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileObjectCell {
    pub dx: u32,
    pub dy: u32,
    pub col: u32,
    pub row: u32,
}

impl TileObject {
    /// Content hash of what the baked texture depends on — the atlas, cell
    /// size, and the exact picked cells. Re-bake only when this changes.
    fn bake_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.tileset_path.hash(&mut hasher);
        self.tile_px.hash(&mut hasher);
        self.w.hash(&mut hasher);
        self.h.hash(&mut hasher);
        for c in &self.cells {
            (c.dx, c.dy, c.col, c.row).hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// Runtime-only marker: the entity's `Sprite` was baked from a [`TileObject`]
/// whose content hashes to this key. Re-baking only when the key changes keeps
/// the baker from rebuilding a texture (and re-uploading it) every frame.
#[derive(Component)]
struct TileObjectBaked(u64);

/// Bake every [`TileObject`] into a single-sprite texture: allocate a
/// transparent `w·tile_px × h·tile_px` RGBA image and copy each picked atlas
/// cell into its slot. This is what makes a hand-picked, possibly
/// non-rectangular set of tiles render as ONE sprite (unpicked cells stay
/// transparent). Inserts the `Sprite` (the entity carries no `SpriteImagePath`,
/// so nothing else builds it), and re-runs when the picked set changes or after
/// a scene load rebuilds the entity without its runtime texture. Editor and
/// shipped runtime both, so painted objects look identical in the exported game.
fn build_tile_object_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    objects: Query<(
        Entity,
        &TileObject,
        Option<&TileObjectBaked>,
        Option<&renzora::core::SpriteCustomSize>,
    )>,
) {
    for (entity, obj, baked, custom) in &objects {
        let key = obj.bake_key();
        if baked.map(|b| b.0) == Some(key) {
            continue;
        }
        if obj.cells.is_empty() || obj.tile_px == 0 || obj.w == 0 || obj.h == 0 {
            continue;
        }
        let tp = obj.tile_px;
        let atlas_handle = asset_server.load::<Image>(obj.tileset_path.clone());
        // Scope the atlas borrow so it ends before `images.add()` needs `&mut`.
        let baked_img = {
            let Some(atlas) = images.get(&atlas_handle) else {
                continue; // atlas still loading — retry next frame
            };
            let asize = atlas.size_f32();
            let (aw, ah) = (asize.x as u32, asize.y as u32);
            let mut img = Image::new_fill(
                Extent3d {
                    width: obj.w * tp,
                    height: obj.h * tp,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[0, 0, 0, 0],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            );
            for cell in &obj.cells {
                for py in 0..tp {
                    for px in 0..tp {
                        let (sx, sy) = (cell.col * tp + px, cell.row * tp + py);
                        if sx >= aw || sy >= ah {
                            continue;
                        }
                        if let Ok(color) = atlas.get_color_at(sx, sy) {
                            let _ = img.set_color_at(cell.dx * tp + px, cell.dy * tp + py, color);
                        }
                    }
                }
            }
            // Nearest sampling keeps the baked pixels crisp, same as the atlas.
            img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());
            img
        };
        let handle = images.add(baked_img);
        commands.entity(entity).insert((
            Sprite {
                image: handle,
                custom_size: custom.map(|c| c.0),
                ..default()
            },
            TileObjectBaked(key),
        ));
    }
}

#[derive(Default)]
pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] TilemapPlugin");
        app.register_type::<TilemapLayer>()
            .register_type::<TilemapTile>()
            .register_type::<TileObject>()
            .register_type::<TileObjectCell>();
        app.add_systems(
            Update,
            (sync_tilesets, force_nearest_tileset_sampler).chain(),
        );
        // Bake picked-cell objects into single sprites (editor + shipped game).
        app.add_systems(Update, build_tile_object_sprites);
    }
}

renzora::add!(TilemapPlugin);

/// On a new or changed [`TilemapLayer`]: (re)load the atlas when the path
/// changed. Reloading only on an actual path change keeps config edits (which
/// also trigger `Changed`) from thrashing the asset server.
fn sync_tilesets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    changed: Query<
        (Entity, &TilemapLayer, Option<&TilesetHandle>),
        Or<(Added<TilemapLayer>, Changed<TilemapLayer>)>,
    >,
) {
    for (entity, layer, existing) in &changed {
        let path_changed = existing.map(|h| h.path != layer.tileset_path).unwrap_or(true);
        if path_changed && !layer.tileset_path.is_empty() {
            let image = load_tileset_nearest(&asset_server, layer.tileset_path.clone());
            commands.entity(entity).insert(TilesetHandle {
                path: layer.tileset_path.clone(),
                image,
            });
        }
    }
}

/// Pin every loaded tileset atlas to **nearest** filtering by mutating the
/// `Image` asset itself.
///
/// The per-load sampler override in [`load_tileset_nearest`] only takes effect
/// when the tileset is the FIRST loader of its path — `load_with_settings` on
/// an already-loaded path returns the existing asset with the existing
/// (possibly linear) sampler. In practice an asset-browser thumbnail or a
/// plain sprite often loads the image first, so tiles rendered blurry and bled
/// neighbouring atlas cells across tile seams until the next project load
/// happened to reorder the loads. Forcing the sampler on the asset is
/// load-order-proof and applies to every user of the image.
fn force_nearest_tileset_sampler(
    tilesets: Query<&TilesetHandle>,
    mut images: ResMut<Assets<Image>>,
) {
    for tileset in &tilesets {
        // Read first, mutate only when needed — `get_mut` marks the asset
        // changed (GPU re-upload), which must not happen every frame.
        let needs_fix = images
            .get(&tileset.image)
            .is_some_and(|img| !matches!(
                &img.sampler,
                ImageSampler::Descriptor(d)
                    if d.min_filter == ImageFilterMode::Nearest
                        && d.mag_filter == ImageFilterMode::Nearest
            ));
        if needs_fix {
            if let Some(mut img) = images.get_mut(&tileset.image) {
                img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());
            }
        }
    }
}

/// Load a tileset atlas with **nearest** filtering. Pixel-art tiles must not be
/// linearly filtered — it blurs them and bleeds neighbouring atlas cells across
/// tile seams. (`load_with_settings` is deprecated upstream but is still the
/// per-load sampler override the engine uses; silenced locally. See
/// [`force_nearest_tileset_sampler`] for why this alone isn't enough.)
#[allow(deprecated)]
fn load_tileset_nearest(asset_server: &AssetServer, path: String) -> Handle<Image> {
    use bevy::image::ImageLoaderSettings;
    asset_server.load_with_settings::<Image, ImageLoaderSettings>(
        path,
        |settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());
        },
    )
}
