//! Chunked mesh generation for [`TilemapLayer`].
//!
//! Each dirty layer whose atlas has finished loading is turned into one
//! `Mesh2d` child per non-empty chunk: a soup of textured quads sharing the
//! layer's single `ColorMaterial`. UVs slice the atlas per `Tile.index`. The
//! chunk children are parented to the layer entity, so the layer's `Transform`
//! positions the whole map and its `Visibility` hides it.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;

use crate::{Tile, TilemapChunk, TilemapChunks, TilemapDirty, TilemapLayer, TilesetHandle, CHUNK_TILES};

/// Rebuild every dirty layer whose atlas is loaded. Layers with no tileset (or
/// whose image hasn't loaded yet) stay dirty and are retried next frame — the
/// atlas load is async, so we can't read its pixel size until it's ready.
pub fn rebuild_tilemaps(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    dirty: Query<
        (Entity, &TilemapLayer, &TilesetHandle, Option<&TilemapChunks>),
        With<TilemapDirty>,
    >,
) {
    for (entity, layer, tileset, old_chunks) in &dirty {
        // Need the atlas dimensions to compute UVs — wait for the load.
        let Some(image) = images.get(&tileset.image) else {
            continue;
        };
        let img_size = image.size_f32();
        if img_size.x <= 0.0 || img_size.y <= 0.0 {
            continue;
        }

        // Despawn the previous chunk set before regenerating.
        if let Some(old) = old_chunks {
            for &c in &old.0 {
                commands.entity(c).try_despawn();
            }
        }

        let new_chunks = build_chunks(&mut commands, &mut meshes, entity, layer, tileset, img_size);
        commands
            .entity(entity)
            .insert(TilemapChunks(new_chunks))
            .remove::<TilemapDirty>();
    }
}

/// Group the layer's tiles by chunk and spawn one mesh child per non-empty
/// chunk. Returns the spawned chunk entities.
fn build_chunks(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    layer_entity: Entity,
    layer: &TilemapLayer,
    tileset: &TilesetHandle,
    img_size: Vec2,
) -> Vec<Entity> {
    let mut by_chunk: HashMap<IVec2, Vec<Tile>> = HashMap::new();
    for &tile in &layer.tiles {
        let chunk = TilemapLayer::chunk_of(IVec2::new(tile.x, tile.y));
        by_chunk.entry(chunk).or_default().push(tile);
    }

    let columns = layer.effective_columns(img_size.x);
    let tile_world = layer.tile_size;
    let atlas_px = layer.atlas_tile_px.max(1) as f32;

    let mut spawned = Vec::with_capacity(by_chunk.len());
    for (chunk, tiles) in by_chunk {
        let Some(mesh) = build_chunk_mesh(chunk, &tiles, columns, tile_world, atlas_px, img_size)
        else {
            continue;
        };
        // Chunk origin, in the layer's local space, at the chunk's bottom-left
        // tile. Vertices below are relative to this.
        let ox = (chunk.x * CHUNK_TILES) as f32 * tile_world;
        let oy = (chunk.y * CHUNK_TILES) as f32 * tile_world;
        let id = commands
            .spawn((
                Mesh2d(meshes.add(mesh)),
                MeshMaterial2d(tileset.material.clone()),
                Transform::from_xyz(ox, oy, 0.0),
                TilemapChunk,
                ChildOf(layer_entity),
                Name::new(format!("Tilemap Chunk ({}, {})", chunk.x, chunk.y)),
            ))
            .id();
        spawned.push(id);
    }
    spawned
}

/// Build one chunk's mesh: a textured quad per tile. `None` if the chunk has no
/// drawable tiles.
fn build_chunk_mesh(
    chunk: IVec2,
    tiles: &[Tile],
    columns: u32,
    tile_world: f32,
    atlas_px: f32,
    img_size: Vec2,
) -> Option<Mesh> {
    if tiles.is_empty() || columns == 0 {
        return None;
    }

    let base_x = chunk.x * CHUNK_TILES;
    let base_y = chunk.y * CHUNK_TILES;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(tiles.len() * 4);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(tiles.len() * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(tiles.len() * 6);

    // Atlas UV cell size (0..1). Tiles reference a cell by row-major index.
    let uw = atlas_px / img_size.x;
    let vh = atlas_px / img_size.y;

    for tile in tiles {
        let lx = (tile.x - base_x) as f32 * tile_world;
        let ly = (tile.y - base_y) as f32 * tile_world;
        let x0 = lx;
        let x1 = lx + tile_world;
        let y0 = ly;
        let y1 = ly + tile_world;

        let col = tile.index % columns;
        let row = tile.index / columns;
        let u0 = col as f32 * uw;
        let u1 = u0 + uw;
        // Texture V grows downward; world Y grows upward, so the tile's bottom
        // edge (low world Y) maps to the cell's bottom (high V).
        let v0 = row as f32 * vh;
        let v1 = v0 + vh;

        let base = positions.len() as u32;
        // BL, BR, TR, TL
        positions.push([x0, y0, 0.0]);
        positions.push([x1, y0, 0.0]);
        positions.push([x1, y1, 0.0]);
        positions.push([x0, y1, 0.0]);
        uvs.push([u0, v1]);
        uvs.push([u1, v1]);
        uvs.push([u1, v0]);
        uvs.push([u0, v0]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}
