//! Terrain drawn as virtual geometry: the shared half.
//!
//! A chunk is drawn as clusters, at whatever detail each part of it needs from
//! where the camera is, instead of swapping whole meshes by distance (`lod`).
//! The clusters are built in the editor, which has the builder, and saved as
//! one `.rvg` per chunk under the terrain's [`TerrainClusters`] folder, so a
//! game reads them and never builds anything. A game build does not contain
//! the builder at all (`renzora_rvg`'s `bake` feature).
//!
//! A chunk's file records the mesh it was built from as [`mesh_hash`]. The
//! runtime draws the clusters only while that matches the chunk's mesh: the
//! moment a brush changes the ground, the chunk falls back to its ordinary
//! mesh (and its LOD levels), and the editor builds new clusters once the
//! ground has stopped changing. Nothing here depends on the renderer: the
//! runtime crate, which links both this crate and virtual geometry, attaches
//! the clusters (`renzora_runtime::terrain_vg`).

use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;
pub use renzora_rvg::{RvgFile, CLUSTERS_LABEL};

/// Where a terrain's clusters live, project-relative: `<dir>/chunk_<x>_<z>.rvg`.
/// Saved with the scene; the editor gives every terrain a folder of its own
/// the first time it builds clusters for it.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct TerrainClusters {
    #[reflect(default)]
    pub dir: String,
}

impl TerrainClusters {
    /// The project-relative path of one chunk's clusters.
    pub fn chunk_path(&self, chunk_x: u32, chunk_z: u32) -> String {
        format!("{}/chunk_{chunk_x}_{chunk_z}.rvg", self.dir.trim_end_matches('/'))
    }
}

/// What is known about one chunk's clusters file, kept by the runtime and
/// read by the editor to decide whether the file needs building again.
#[derive(Component, Debug)]
pub struct ChunkClusters {
    /// The file this was set up for, so a moved folder starts over.
    pub path: String,
    pub state: ClusterFile,
    /// [`mesh_hash`] of the chunk's current mesh, recomputed when it changes.
    pub mesh_hash: Option<u64>,
}

#[derive(Debug)]
pub enum ClusterFile {
    /// Asking the asset source whether the file exists, so a chunk with none
    /// yet does not log a load error.
    Probing(bevy::tasks::Task<bool>),
    /// Loading, or loaded: the handle is also where the editor puts a file it
    /// has just built, so the change is seen without reading it back.
    Loaded(Handle<RvgFile>),
    Missing,
}

/// The clusters inside a chunk's `.rvg`. An `.rvg` loads as a model, with its
/// clusters as this labelled sub-asset.
pub fn clusters_asset_path(path: &str) -> bevy::asset::AssetPath<'static> {
    bevy::asset::AssetPath::from(path.to_string()).with_label(CLUSTERS_LABEL)
}

/// A hash of a mesh's positions, normals and triangles: what a chunk's
/// clusters were built from. FNV-1a over the raw bytes, so it is the same in
/// every build and on every machine (the std hasher promises neither).
pub fn mesh_hash(mesh: &Mesh) -> Option<u64> {
    // Seeded with how chunks are built, so changing that (locking their open
    // edges, version 2) makes every file on disk stale and rebuilt, without
    // touching the builder version every model in a project is checked by.
    const CHUNK_BUILD_VERSION: u64 = 2;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ CHUNK_BUILD_VERSION;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for attribute in [Mesh::ATTRIBUTE_POSITION, Mesh::ATTRIBUTE_NORMAL] {
        let Some(VertexAttributeValues::Float32x3(values)) = mesh.attribute(attribute) else {
            return None;
        };
        for v in values {
            for c in v {
                feed(&c.to_le_bytes());
            }
        }
    }
    match mesh.indices()? {
        Indices::U32(indices) => indices.iter().for_each(|i| feed(&i.to_le_bytes())),
        Indices::U16(indices) => indices.iter().for_each(|i| feed(&(*i as u32).to_le_bytes())),
    }
    Some(h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{TerrainChunkData, TerrainData};
    use crate::mesh::generate_chunk_mesh;

    #[test]
    fn the_hash_follows_the_ground_and_nothing_else() {
        let t = TerrainData::default();
        let mut chunk = TerrainChunkData::new(0, 0, t.chunk_resolution, 0.5);
        let a = mesh_hash(&generate_chunk_mesh(&t, &chunk)).unwrap();
        assert_eq!(a, mesh_hash(&generate_chunk_mesh(&t, &chunk)).unwrap(), "same ground, same hash");
        chunk.heights[500] += 0.01;
        assert_ne!(a, mesh_hash(&generate_chunk_mesh(&t, &chunk)).unwrap(), "one vertex moved");
    }

    #[test]
    fn chunk_paths_sit_in_the_terrains_folder() {
        let c = TerrainClusters { dir: "terrain_clusters/ab12/".into() };
        assert_eq!(c.chunk_path(3, 7), "terrain_clusters/ab12/chunk_3_7.rvg");
    }
}
