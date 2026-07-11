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
    /// Atlas cell indices (row-major, `row * columns + col`) marked **solid**
    /// in the palette. Painted tiles showing one of these frames grow merged
    /// static 2D colliders — see [`rebuild_tile_colliders`]. Defaults keep
    /// pre-collision scenes loading unchanged.
    #[serde(default)]
    #[reflect(default)]
    pub solid_tiles: Vec<u32>,
    /// Collision boxes authored in the palette for multi-tile **objects**
    /// (trees, houses), keyed by the pick's atlas bounding box. Stamping an
    /// object whose pick matches a key auto-inserts the equivalent
    /// `CollisionShapeData` on the spawned entity — see
    /// [`TileObjectCollider::shape_data`].
    #[serde(default)]
    #[reflect(default)]
    pub object_colliders: Vec<TileObjectCollider>,
}

impl Default for TilemapLayer {
    fn default() -> Self {
        Self {
            tileset_path: String::new(),
            tile_size: 16.0,
            atlas_tile_px: 16,
            columns: 0,
            solid_tiles: Vec::new(),
            object_colliders: Vec::new(),
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

    /// The authored object collider for a palette pick with this atlas
    /// bounding box, if any.
    pub fn collider_for(&self, col: u32, row: u32, w: u32, h: u32) -> Option<TileObjectCollider> {
        self.object_colliders
            .iter()
            .copied()
            .find(|c| c.col == col && c.row == row && c.w == w && c.h == h)
    }
}

/// A collision box authored in the tilemap palette for one multi-tile object
/// pick, keyed by the pick's atlas bounding box (`col`/`row`/`w`/`h`, cells).
/// The box itself (`rect_*`) is in **cell units relative to the bounding
/// box's top-left**, x right / y down — palette orientation, so the editor
/// overlay maps 1:1 onto the atlas. Keying by the bounding box (rather than
/// the exact cell set) means a re-pick of the same tree region recalls its
/// collider; two different picks sharing a bounding box share one collider,
/// which in practice is the same object. Plain fields (not Rect/Vec2) for the
/// reflection scene serializer, like [`TilemapTile`].
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileObjectCollider {
    pub col: u32,
    pub row: u32,
    pub w: u32,
    pub h: u32,
    pub rect_x: f32,
    pub rect_y: f32,
    pub rect_w: f32,
    pub rect_h: f32,
    /// Collision shape the rect authors: Box (the rect itself), Sphere (the
    /// 2D circle inscribed in the rect — radius from its shorter side) or
    /// Capsule (vertical; radius = half the rect width, caps inside the rect
    /// ends). Default keeps pre-shape scenes loading as Box.
    #[serde(default)]
    #[reflect(default)]
    pub shape: renzora_physics::CollisionShapeType,
}

impl TileObjectCollider {
    /// The entity collider equivalent for an object stamped at `tile_size`
    /// world units per cell, with the offset measured from the centre-anchored
    /// object sprite. Palette y grows DOWN, world y UP — hence the flip.
    pub fn shape_data(&self, tile_size: f32) -> renzora_physics::CollisionShapeData {
        use renzora_physics::{CollisionShapeData, CollisionShapeType};
        let (w, h) = (self.w.max(1) as f32, self.h.max(1) as f32);
        let cx = self.rect_x + self.rect_w * 0.5;
        let cy = self.rect_y + self.rect_h * 0.5;
        let offset = Vec3::new((cx - w * 0.5) * tile_size, (h * 0.5 - cy) * tile_size, 0.0);
        // Half-extents of the authored rect in world units.
        let hw = (self.rect_w * 0.5 * tile_size).max(0.5);
        let hh = (self.rect_h * 0.5 * tile_size).max(0.5);
        match self.shape {
            CollisionShapeType::Sphere => CollisionShapeData {
                shape_type: CollisionShapeType::Sphere,
                offset,
                radius: hw.min(hh),
                ..Default::default()
            },
            CollisionShapeType::Capsule => {
                // Vertical capsule (avian's capsule axis is Y): radius from the
                // narrower dimension, cylinder part fills the rest of the rect
                // height. A rect wider than tall degenerates to a circle.
                let radius = hw.min(hh);
                CollisionShapeData {
                    shape_type: CollisionShapeType::Capsule,
                    offset,
                    radius,
                    half_height: (hh - radius).max(0.0),
                    ..Default::default()
                }
            }
            _ => CollisionShapeData {
                shape_type: CollisionShapeType::Box,
                offset,
                half_extents: Vec3::new(hw, hh, tile_size * 0.5),
                ..Default::default()
            },
        }
    }
}

/// One **paint layer** of a tilemap — Ground / Decoration / Overhead — an
/// intermediate child entity between the [`TilemapLayer`] root and its painted
/// tiles. All paint layers share the root's tileset/palette config; each owns
/// its own tiles (they're its children), its own draw order and opacity, and
/// an editor lock. A tilemap without paint layers still works: tiles painted
/// directly under the root behave as an implicit base layer, so pre-layer
/// scenes load unchanged.
#[derive(Component, Reflect, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct TilemapPaintLayer {
    /// Draw order: the layer entity's local Z is `order * 10` (see
    /// [`apply_paint_layer_order`]). The 10-unit step leaves the whole ±0.5
    /// y-sort band (plus its `z_base` of 1) inside one layer, so an overhead
    /// layer always draws above a lower layer's y-sorted props.
    pub order: i32,
    /// 0..1, multiplied into child tile sprite alpha (editor QoL for e.g.
    /// dimming an overhead layer while painting under it — it ships too).
    pub opacity: f32,
    /// Editor-only meaning: a locked layer can't be painted or erased.
    pub locked: bool,
}

impl Default for TilemapPaintLayer {
    fn default() -> Self {
        Self { order: 0, opacity: 1.0, locked: false }
    }
}

/// Local Z per unit of [`TilemapPaintLayer::order`].
pub const PAINT_LAYER_Z_STEP: f32 = 10.0;

/// Keep each paint layer's transform Z and its tiles' alpha in sync with the
/// authored `order`/`opacity`. Runs on change only; new tiles are painted
/// opaque, so a repaint on a translucent layer re-applies via the layer
/// being `Changed` when the panel next touches it (acceptable for an editor
/// dimming aid).
fn apply_paint_layer_visuals(
    mut layers: Query<
        (&TilemapPaintLayer, &mut Transform, Option<&Children>),
        Changed<TilemapPaintLayer>,
    >,
    mut sprites: Query<&mut Sprite>,
) {
    for (layer, mut transform, children) in &mut layers {
        let z = layer.order as f32 * PAINT_LAYER_Z_STEP;
        if transform.translation.z != z {
            transform.translation.z = z;
        }
        let alpha = layer.opacity.clamp(0.0, 1.0);
        if let Some(children) = children {
            for child in children.iter() {
                if let Ok(mut sprite) = sprites.get_mut(child) {
                    if sprite.color.alpha() != alpha {
                        sprite.color.set_alpha(alpha);
                    }
                }
            }
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
            .register_type::<TileObjectCell>()
            .register_type::<TileObjectCollider>()
            .register_type::<TilemapPaintLayer>();
        app.add_systems(
            Update,
            (sync_tilesets, force_nearest_tileset_sampler).chain(),
        );
        // Bake picked-cell objects into single sprites (editor + shipped game).
        app.add_systems(Update, build_tile_object_sprites);
        // Merged static colliders for solid-marked tiles (editor + shipped game).
        app.add_systems(Update, rebuild_tile_colliders);
        // Paint-layer draw order (Z) + opacity → tiles (editor + shipped game).
        app.add_systems(Update, apply_paint_layer_visuals);
    }
}

renzora::add!(TilemapPlugin);

/// Runtime marker on a generated tile-collider child. Lets a rebuild find and
/// despawn the previous generation; never saved (the entities also carry
/// `HideInHierarchy`, which the scene saver excludes) — colliders are derived
/// data, regenerated from the painted tiles wherever the scene loads.
#[derive(Component)]
struct TilemapColliderShape;

/// Runtime cache on the layer: hash of the solid-cell picture the current
/// collider children were built from. Rebuild only when it changes.
#[derive(Component)]
struct TileColliderKey(u64);

/// Grow merged static 2D colliders under every layer with solid-marked tiles.
///
/// One collider per painted solid tile would hand avian thousands of bodies
/// and seam-catch moving characters on every tile boundary, so contiguous
/// solid cells are greedy-merged into rectangles first (extend right, then
/// down). Each rectangle becomes a hidden child carrying `CollisionShapeData`
/// (+ `Physics2d`, which routes it to the avian2d backend regardless of the
/// entity having no sprite). Runs in the editor and the shipped game — the
/// children are never saved, so every load regenerates them from the tiles.
///
/// The change gate is two-stage: cheap `Changed`/`Removed` queries decide
/// whether to look at all, then a content hash of the layer's solid-cell set
/// decides whether the colliders actually need rebuilding (a repaint that
/// swaps grass for other grass hashes identically and is skipped).
fn rebuild_tile_colliders(
    mut commands: Commands,
    dirty_layers: Query<(), Changed<TilemapLayer>>,
    dirty_tiles: Query<(), (With<TilemapTile>, Or<(Changed<TilemapTile>, Changed<renzora::core::SpriteSheet>)>)>,
    mut removed_tiles: RemovedComponents<TilemapTile>,
    // Every entity that can OWN tiles: a tilemap root, or one of its paint
    // layers. Colliders hang under whichever entity owns the tiles.
    owners: Query<
        (Entity, Option<&TileColliderKey>, Option<&ChildOf>),
        Or<(With<TilemapLayer>, With<TilemapPaintLayer>)>,
    >,
    configs: Query<&TilemapLayer>,
    tiles: Query<(&TilemapTile, &renzora::core::SpriteSheet, &ChildOf)>,
    shapes: Query<(Entity, &ChildOf), With<TilemapColliderShape>>,
) {
    let any_removed = removed_tiles.read().next().is_some();
    if dirty_layers.is_empty() && dirty_tiles.is_empty() && !any_removed {
        return;
    }
    for (owner, key, owner_parent) in &owners {
        // The palette config (tile size + solid set) lives on the tilemap
        // ROOT; a paint layer reads its parent's.
        let Some(layer) = configs
            .get(owner)
            .ok()
            .or_else(|| owner_parent.and_then(|p| configs.get(p.parent()).ok()))
        else {
            continue; // orphan paint layer — nothing sensible to build
        };
        // Never marked anything solid and never built → nothing to do or undo.
        if layer.solid_tiles.is_empty() && key.is_none() {
            continue;
        }
        let solid: std::collections::HashSet<u32> = layer.solid_tiles.iter().copied().collect();

        // Solid cells + an order-independent content hash (query iteration
        // order is not stable, so per-cell hashes are combined by addition).
        let mut cells: Vec<IVec2> = Vec::new();
        let mut acc: u64 = {
            let mut h = DefaultHasher::new();
            layer.tile_size.to_bits().hash(&mut h);
            let mut sorted: Vec<u32> = layer.solid_tiles.clone();
            sorted.sort_unstable();
            sorted.hash(&mut h);
            h.finish()
        };
        for (tile, sheet, child_of) in &tiles {
            if child_of.parent() != owner {
                continue;
            }
            let idx = sheet.frame % (sheet.hframes.max(1) * sheet.vframes.max(1));
            if !solid.contains(&idx) {
                continue;
            }
            cells.push(IVec2::new(tile.x, tile.y));
            let mut h = DefaultHasher::new();
            (tile.x, tile.y).hash(&mut h);
            acc = acc.wrapping_add(h.finish());
        }
        if key.map(|k| k.0) == Some(acc) {
            continue;
        }

        for (shape, child_of) in &shapes {
            if child_of.parent() == owner {
                commands.entity(shape).try_despawn();
            }
        }
        let ts = layer.tile_size;
        for (min, max) in greedy_rects(&cells) {
            // Tiles are centre-anchored at `cell * ts + ts/2` in layer space,
            // so an inclusive cell span [min, max] centres at the midpoint of
            // its outer corners.
            let center = Vec2::new(
                (min.x + max.x + 1) as f32 * ts * 0.5,
                (min.y + max.y + 1) as f32 * ts * 0.5,
            );
            let half = Vec2::new(
                (max.x - min.x + 1) as f32 * ts * 0.5,
                (max.y - min.y + 1) as f32 * ts * 0.5,
            );
            commands.spawn((
                Name::new(format!(
                    "Tile Collider ({}..{}, {}..{})",
                    min.x, max.x, min.y, max.y
                )),
                renzora::core::HideInHierarchy,
                renzora_physics::Physics2d,
                renzora_physics::auto_fit::SkipAutoFit,
                renzora_physics::CollisionShapeData {
                    shape_type: renzora_physics::CollisionShapeType::Box,
                    half_extents: half.extend(ts * 0.5),
                    ..Default::default()
                },
                Transform::from_translation(center.extend(0.0)),
                TilemapColliderShape,
                ChildOf(owner),
            ));
        }
        commands.entity(owner).insert(TileColliderKey(acc));
    }
}

/// Greedy rectangle merge over a set of grid cells: take the first unclaimed
/// cell (row-major), extend the run rightward, then extend the row block
/// downward while every column stays solid. Not globally optimal, but it turns
/// the common shapes (walls, platforms, filled areas) into a handful of
/// rectangles instead of one collider per tile.
fn greedy_rects(cells: &[IVec2]) -> Vec<(IVec2, IVec2)> {
    use std::collections::BTreeSet;
    // (y, x) ordering = row-major iteration.
    let mut remaining: BTreeSet<(i32, i32)> = cells.iter().map(|c| (c.y, c.x)).collect();
    let mut out = Vec::new();
    while let Some(&(y0, x0)) = remaining.iter().next() {
        let mut x1 = x0;
        while remaining.contains(&(y0, x1 + 1)) {
            x1 += 1;
        }
        let mut y1 = y0;
        'grow: loop {
            for x in x0..=x1 {
                if !remaining.contains(&(y1 + 1, x)) {
                    break 'grow;
                }
            }
            y1 += 1;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                remaining.remove(&(y, x));
            }
        }
        out.push((IVec2::new(x0, y0), IVec2::new(x1, y1)));
    }
    out
}

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
