//! `.rvg`: Renzora Virtual Geometry.
//!
//! A mesh cut into clusters of at most [`MAX_CLUSTER_TRIANGLES`] triangles,
//! arranged in a LOD DAG: groups of neighbouring clusters are merged,
//! simplified with their shared border locked, and split again into the
//! clusters of the next coarser level. Every cluster carries two error bounds,
//! its own and its parent group's, so the renderer picks the level of detail
//! per cluster with no traversal at all: a cluster is drawn exactly when its
//! own error is acceptable and its parent's is not ([`Cluster::selected`]).
//! Because the parent bound is shared by every cluster of a group and errors
//! only grow up the DAG, the clusters chosen at any threshold tile the surface
//! without cracks, whatever mix of levels that is.
//!
//! The file sits beside the `.glb` it was built from and never replaces it:
//! the GLB still carries the materials, the node hierarchy, skins and
//! animations. Each [`RvgMesh`] names the glTF mesh and primitive it stands in
//! for, and the vertex data keeps every attribute the material can read, plus
//! joints and weights for skinned meshes.
//!
//! ## On-disk layout
//!
//! All integers and floats are little-endian.
//!
//! ```text
//! "RVG1"            magic
//! u32               format version (VERSION)
//! u32               builder version (BUILDER_VERSION of the builder that wrote it)
//! u64               source hash: source_hash() of the .glb it was built from
//! u64               length of the body once decompressed
//! zstd frame        the body:
//!   u32             mesh count, then per mesh:
//!     u32 u32       glTF mesh index, primitive index
//!     u32           attribute flags (Attributes)
//!     u32           vertex count, then the present attribute arrays in flag order:
//!                   positions [f32;3]*, normals u32 (octahedral),
//!                   uv0 [f32;2]*, tangents u32 (octahedral, bitangent sign in
//!                   bit 0), uv1 [f32;2]*, colors [f32;4]*,
//!                   joints [u16;4] + weights [f32;4]*
//!     u32 + Cluster[]
//!     u32 + u32[]*  cluster vertices, each the difference from the one before
//!                   (zigzag), indices into the vertex arrays once summed
//!     u32 + u8[]    cluster triangles: 3 local indices each, padded to 4 bytes
//!     u32 + Group[]
//!     u32 + Node[]
//!     f32           proxy threshold: the error of the coarse cut ray tracing uses
//!   u32             1 if the model's scene follows, else 0, then:
//!     u64 + u8[]    the scene as a GLB (`RvgFile::scene`)
//! ```
//!
//! The 20-byte header is outside the compressed body so staleness can be
//! checked from a file's first bytes ([`Header::read`]). Arrays marked `*` are
//! stored byte-shuffled: every element's first byte, then every element's
//! second, and so on. Neighbouring floats and indices share their high bytes,
//! which shuffling lines up into the long runs zstd compresses well. Normals
//! and tangents are stored the way the renderer uploads them, 32 bits each.
//!
//! An exported game packs the file into an `.rpak`, which zstd-compresses
//! every entry again; that second pass gains little but costs little.
//!
//! ## Crate layout
//!
//! - the spec: [`RvgFile`], [`RvgMesh`] and friends, [`RvgFile::to_bytes`] and
//!   [`RvgFile::from_bytes`];
//! - [`RvgLoader`], the Bevy `AssetLoader`;
//! - [`bake`] (feature `bake`), the builder the importer and the converter call.

mod loader;
pub use loader::{RvgLoadError, RvgLoader};

mod residency;
pub use residency::GroupGraph;

#[cfg(all(feature = "bake", not(target_arch = "wasm32")))]
mod scene;

#[cfg(feature = "model")]
pub mod model;

/// The label of a model's virtual geometry within its `.rvg`: every `.rvg`
/// loads as a model, its clusters beside its scene (`model.rs`).
pub const CLUSTERS_LABEL: &str = "Clusters";

/// The glTF JSON of a model `.rvg`'s scene, for tools that read what a model
/// references (textures, materials) without loading it. `None` for an `.rvg`
/// that is not a model, or one that does not read.
pub fn scene_json(rvg: &[u8]) -> Option<String> {
    let file = RvgFile::from_bytes(rvg).ok()?;
    let glb = file.scene?;
    let len = u32::from_le_bytes(glb.get(12..16)?.try_into().ok()?) as usize;
    if glb.get(16..20)? != b"JSON" {
        return None;
    }
    std::str::from_utf8(glb.get(20..20 + len)?).ok().map(str::to_string)
}

#[cfg(feature = "bake")]
pub mod bake;

use std::sync::Arc;

use bevy::asset::Asset;
use bevy::reflect::TypePath;

pub const MAGIC: [u8; 4] = *b"RVG1";

/// The layout above. Bumped when the bytes change shape.
/// 2: compressed body, packed normals and tangents, delta-coded cluster
/// vertices, and the proxy stored as its threshold instead of its indices.
/// 3: clusters record the group they were simplified from (`refined`).
/// 4: the body may end with the model's scene (`RvgFile::scene`).
pub const VERSION: u32 = 4;

/// Bumped when the builder would produce a different file from the same input
/// (new simplification settings, a fixed bug). A file written by an older
/// builder is stale even when its source has not changed, so the converter
/// rebuilds it.
/// 2: alpha-masked primitives no longer use sloppy simplification.
/// 3: skinned primitives are simplified with regularization.
/// 4: writes `Cluster::refined`.
/// 5: finer proxies (a quarter of the triangles, not a sixteenth), which
/// models stored as `.rvg` use as their ordinary mesh.
pub const BUILDER_VERSION: u32 = 5;

/// The most triangles one cluster holds. Local triangle indices are `u8`, and
/// the builder keeps vertices per cluster at the same limit.
pub const MAX_CLUSTER_TRIANGLES: usize = 128;

/// FNV-1a over the source file, recorded in the header so the editor can tell
/// whether an `.rvg` still matches its `.glb` without rebuilding it. Not a
/// cryptographic hash: it only has to notice that a model was re-imported.
pub fn source_hash(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The first 20 bytes of a file: enough to tell whether it is current without
/// reading the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub version: u32,
    pub builder_version: u32,
    pub source_hash: u64,
}

impl Header {
    pub const LEN: usize = 20;

    pub fn read(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::LEN || bytes[..4] != MAGIC {
            return None;
        }
        let u32_at = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        let mut hash = [0u8; 8];
        hash.copy_from_slice(&bytes[12..20]);
        Some(Self {
            version: u32_at(4),
            builder_version: u32_at(8),
            source_hash: u64::from_le_bytes(hash),
        })
    }

    /// Whether a file with this header was built from `source` by this
    /// builder, in a layout this build reads.
    pub fn is_current(&self, source: &[u8]) -> bool {
        self.version == VERSION
            && self.builder_version == BUILDER_VERSION
            && self.source_hash == source_hash(source)
    }
}

/// Which optional vertex arrays a mesh carries. Positions always exist.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Attributes(pub u32);

impl Attributes {
    pub const NORMAL: u32 = 1;
    pub const UV0: u32 = 1 << 1;
    pub const TANGENT: u32 = 1 << 2;
    pub const UV1: u32 = 1 << 3;
    pub const COLOR: u32 = 1 << 4;
    /// Joints and weights, always together.
    pub const SKIN: u32 = 1 << 5;

    pub fn has(self, flag: u32) -> bool {
        self.0 & flag != 0
    }
}

/// A sphere plus the simplification error of the geometry inside it, in the
/// mesh's own space. The renderer projects the error to pixels using the
/// sphere's distance from the camera; the sphere is what keeps that projection
/// monotonic up the DAG.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub center: [f32; 3],
    pub radius: f32,
    pub error: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cluster {
    /// Range in [`RvgMesh::cluster_vertices`].
    pub vertex_offset: u32,
    pub vertex_count: u32,
    /// Range in [`RvgMesh::cluster_triangles`], in triangles (3 bytes each).
    pub triangle_offset: u32,
    pub triangle_count: u32,
    /// The group this cluster belongs to. Clusters are stored grouped: each
    /// group's clusters are one contiguous range ([`RvgMesh::group_graph`]).
    pub group: u32,
    /// The group whose simplification produced this cluster, or -1 for a
    /// full-detail cluster. Its clusters are what replaces this one when the
    /// camera needs more detail; streaming keeps this one drawn while they
    /// are not resident.
    pub refined: i32,
    /// Tight bounds of the cluster's own triangles, for frustum and occlusion
    /// culling. Its `error` is unused.
    pub culling: Bounds,
    /// The error of this cluster itself: zero for original geometry, otherwise
    /// the simplified bounds of the group that was simplified to produce it.
    pub lod: Bounds,
    /// The error of replacing this cluster's group with its simplified
    /// version. `f32::MAX` when nothing coarser exists.
    pub parent: Bounds,
}

impl Cluster {
    /// Whether this cluster is part of the cut at a world-space error
    /// threshold. The renderer does the same with both errors projected to
    /// pixels; this form is what the proxy cut and the tests use.
    pub fn selected(&self, threshold: f32) -> bool {
        self.parent.error > threshold && self.lod.error <= threshold
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Group {
    /// DAG level: 0 for groups of original clusters.
    pub depth: u32,
    /// Bounds and error of the simplified version of this group.
    pub simplified: Bounds,
}

/// One node of the per-level bounding hierarchy over groups, for culling whole
/// subtrees before looking at clusters. Nodes `0..level_count` are the roots,
/// one per DAG level.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Node {
    pub bounds: Bounds,
    /// The group a leaf stands for, or -1 for an inner node.
    pub group: i32,
    pub child_offset: u32,
    pub child_count: u32,
}

/// The virtual geometry of one glTF primitive.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RvgMesh {
    pub mesh_index: u32,
    pub primitive_index: u32,
    pub attributes: Attributes,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    pub tangents: Vec<[f32; 4]>,
    pub uv1: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub clusters: Vec<Cluster>,
    pub cluster_vertices: Vec<u32>,
    pub cluster_triangles: Vec<u8>,
    pub groups: Vec<Group>,
    pub nodes: Vec<Node>,
    /// The world-space error of the coarse cut ray tracing uses
    /// ([`Self::proxy_indices`]). Ray tracing cannot pick a level per pixel,
    /// so Solari traces one fixed cut instead of the full-detail mesh. Only
    /// the threshold is kept: the cut follows from the clusters, and storing
    /// its indices doubled the file for any mesh small enough that the cut is
    /// the full-detail surface.
    pub proxy_threshold: f32,
}

impl RvgMesh {
    /// Triangles at full detail.
    pub fn original_triangle_count(&self) -> usize {
        self.clusters
            .iter()
            .filter(|c| c.lod.error == 0.0)
            .map(|c| c.triangle_count as usize)
            .sum()
    }

    /// The clusters drawn at a world-space error threshold.
    pub fn cut(&self, threshold: f32) -> impl Iterator<Item = &Cluster> {
        self.clusters.iter().filter(move |c| c.selected(threshold))
    }

    /// A cluster's triangles as indices into the vertex arrays.
    pub fn cluster_indices(&self, cluster: &Cluster) -> impl Iterator<Item = u32> + '_ {
        let vertices = &self.cluster_vertices[cluster.vertex_offset as usize..]
            [..cluster.vertex_count as usize];
        let start = cluster.triangle_offset as usize * 3;
        let end = start + cluster.triangle_count as usize * 3;
        self.cluster_triangles[start..end]
            .iter()
            .map(move |&local| vertices[local as usize])
    }

    /// The cut at `threshold` as one index list.
    pub fn cut_indices(&self, threshold: f32) -> Vec<u32> {
        self.cut(threshold)
            .flat_map(|c| self.cluster_indices(c))
            .collect()
    }

    /// The coarse cut ray tracing uses, as an index list into the vertex
    /// arrays.
    pub fn proxy_indices(&self) -> Vec<u32> {
        self.cut_indices(self.proxy_threshold)
    }
}

/// Octahedral encoding of a unit vector into two snorm16s, `x` in the low half.
/// The inverse of Bevy's `octahedral_decode_signed(unpack2x16snorm(word))`,
/// which is how the renderer reads it back.
pub fn octahedral_encode(v: [f32; 3]) -> u32 {
    let [x, y, z] = v;
    let l1 = x.abs() + y.abs() + z.abs();
    if l1 == 0.0 || !l1.is_finite() {
        // No direction: encode +Z, as the renderer's own packing does.
        return 0;
    }
    let (mut px, mut py) = (x / l1, y / l1);
    if z < 0.0 {
        let sign = |s: f32| if s >= 0.0 { 1.0 } else { -1.0 };
        (px, py) = ((1.0 - py.abs()) * sign(px), (1.0 - px.abs()) * sign(py));
    }
    let snorm = |s: f32| ((s.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16 as u32;
    snorm(px) | (snorm(py) << 16)
}

/// The inverse of [`octahedral_encode`], normalized.
pub fn octahedral_decode(word: u32) -> [f32; 3] {
    let unpack = |s: u32| ((s & 0xffff) as u16 as i16 as f32 / 32767.0).max(-1.0);
    let (x, y) = (unpack(word), unpack(word >> 16));
    let mut n = [x, y, 1.0 - x.abs() - y.abs()];
    let t = (-n[2]).clamp(0.0, 1.0);
    n[0] += if n[0] >= 0.0 { -t } else { t };
    n[1] += if n[1] >= 0.0 { -t } else { t };
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    [n[0] / len, n[1] / len, n[2] / len]
}

/// A whole `.rvg` file: the virtual geometry of every primitive of one model
/// that could be converted, and (for a model imported as `.rvg`) the model's
/// scene. A primitive that is missing from `meshes` renders as ordinary
/// geometry.
#[derive(Asset, TypePath, Clone, Debug, Default, PartialEq)]
pub struct RvgFile {
    pub builder_version: u32,
    pub source_hash: u64,
    /// Shared, so the renderer can hand a mesh to the render world without
    /// copying a few hundred megabytes of vertex data out of the asset.
    pub meshes: Vec<Arc<RvgMesh>>,
    /// The model's scene as a GLB: nodes, names, materials, skins, morph
    /// targets, cameras, lights, as the source model had them, except that
    /// each primitive in `meshes` carries only its coarse proxy (the full
    /// detail is the clusters). `None` for a file built beside a `.glb`, which
    /// holds the scene itself. See [`bake::build_model`].
    pub scene: Option<Arc<Vec<u8>>>,
}

impl RvgFile {
    /// Whether this file was built from `source` by the current builder.
    pub fn is_current(&self, source: &[u8]) -> bool {
        self.builder_version == BUILDER_VERSION && self.source_hash == source_hash(source)
    }

    /// The position in [`Self::meshes`] of a glTF primitive's virtual geometry.
    pub fn find(&self, mesh_index: u32, primitive_index: u32) -> Option<usize> {
        self.meshes
            .iter()
            .position(|m| m.mesh_index == mesh_index && m.primitive_index == primitive_index)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut body = Writer(Vec::new());
        body.u32(self.meshes.len() as u32);
        for mesh in &self.meshes {
            write_mesh(&mut body, mesh);
        }
        match &self.scene {
            Some(scene) => {
                body.u32(1);
                body.u64(scene.len() as u64);
                body.0.extend_from_slice(scene);
            }
            None => body.u32(0),
        }
        let mut w = Writer(Vec::new());
        w.0.extend_from_slice(&MAGIC);
        w.u32(VERSION);
        w.u32(self.builder_version);
        w.u64(self.source_hash);
        w.u64(body.0.len() as u64);
        ruzstd::encoding::compress(
            body.0.as_slice(),
            &mut w.0,
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        w.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RvgLoadError> {
        let mut r = Reader { bytes, at: 0 };
        if r.take(4)? != MAGIC {
            return Err(RvgLoadError::BadMagic);
        }
        let version = r.u32()?;
        if version != VERSION {
            return Err(RvgLoadError::UnsupportedVersion(version, VERSION));
        }
        let builder_version = r.u32()?;
        let source_hash = r.u64()?;
        let body_len = r.u64()?;
        // Decompressed into a buffer sized up front, but never past what the
        // frame actually yields: a corrupt length must not reserve gigabytes.
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(&bytes[r.at..])
            .map_err(|_| RvgLoadError::Corrupt("body is not a zstd frame"))?;
        let mut body = Vec::with_capacity((body_len as usize).min(bytes.len().saturating_mul(16)));
        std::io::Read::read_to_end(&mut decoder, &mut body)
            .map_err(|_| RvgLoadError::Corrupt("body does not decompress"))?;
        if body.len() as u64 != body_len {
            return Err(RvgLoadError::Truncated);
        }
        let mut r = Reader { bytes: &body, at: 0 };
        let count = r.u32()? as usize;
        let mut meshes = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            meshes.push(Arc::new(read_mesh(&mut r)?));
        }
        let scene = match r.u32()? {
            0 => None,
            _ => {
                let len = usize::try_from(r.u64()?).map_err(|_| RvgLoadError::Truncated)?;
                let bytes = r.take(len)?;
                if bytes.get(..4) != Some(b"glTF") {
                    return Err(RvgLoadError::Corrupt("scene is not a GLB"));
                }
                Some(Arc::new(bytes.to_vec()))
            }
        };
        Ok(Self {
            builder_version,
            source_hash,
            meshes,
            scene,
        })
    }
}

fn write_mesh(w: &mut Writer, m: &RvgMesh) {
    let a = m.attributes;
    w.u32(m.mesh_index);
    w.u32(m.primitive_index);
    w.u32(a.0);
    w.u32(m.positions.len() as u32);
    w.shuffled_floats(m.positions.as_flattened());
    if a.has(Attributes::NORMAL) {
        let words: Vec<u32> = m.normals.iter().map(|&n| octahedral_encode(n)).collect();
        w.shuffled(&words);
    }
    if a.has(Attributes::UV0) {
        w.shuffled_floats(m.uv0.as_flattened());
    }
    if a.has(Attributes::TANGENT) {
        let words: Vec<u32> = m
            .tangents
            .iter()
            .map(|t| (octahedral_encode([t[0], t[1], t[2]]) & !1) | (t[3] < 0.0) as u32)
            .collect();
        w.shuffled(&words);
    }
    if a.has(Attributes::UV1) {
        w.shuffled_floats(m.uv1.as_flattened());
    }
    if a.has(Attributes::COLOR) {
        w.shuffled_floats(m.colors.as_flattened());
    }
    if a.has(Attributes::SKIN) {
        for j in m.joints.as_flattened() {
            w.0.extend_from_slice(&j.to_le_bytes());
        }
        w.shuffled_floats(m.weights.as_flattened());
    }

    w.u32(m.clusters.len() as u32);
    for c in &m.clusters {
        w.u32(c.vertex_offset);
        w.u32(c.vertex_count);
        w.u32(c.triangle_offset);
        w.u32(c.triangle_count);
        w.u32(c.group);
        w.u32(c.refined as u32);
        w.bounds(&c.culling);
        w.bounds(&c.lod);
        w.bounds(&c.parent);
    }
    w.u32(m.cluster_vertices.len() as u32);
    // A cluster's vertices sit close together in the vertex arrays, so the
    // step from one to the next is small where the index itself is not.
    let mut previous = 0u32;
    let deltas: Vec<u32> = m
        .cluster_vertices
        .iter()
        .map(|&v| {
            let delta = v.wrapping_sub(previous) as i32;
            previous = v;
            ((delta << 1) ^ (delta >> 31)) as u32
        })
        .collect();
    w.shuffled(&deltas);
    w.u32(m.cluster_triangles.len() as u32);
    w.0.extend_from_slice(&m.cluster_triangles);
    w.pad4();
    w.u32(m.groups.len() as u32);
    for g in &m.groups {
        w.u32(g.depth);
        w.bounds(&g.simplified);
    }
    w.u32(m.nodes.len() as u32);
    for n in &m.nodes {
        w.bounds(&n.bounds);
        w.u32(n.group as u32);
        w.u32(n.child_offset);
        w.u32(n.child_count);
    }
    w.floats(&[m.proxy_threshold]);
}

fn read_mesh(r: &mut Reader) -> Result<RvgMesh, RvgLoadError> {
    let mesh_index = r.u32()?;
    let primitive_index = r.u32()?;
    let a = Attributes(r.u32()?);
    let n = r.u32()? as usize;
    let mut m = RvgMesh {
        mesh_index,
        primitive_index,
        attributes: a,
        positions: r.arrays(n)?,
        ..Default::default()
    };
    if a.has(Attributes::NORMAL) {
        m.normals = r.shuffled(n)?.into_iter().map(octahedral_decode).collect();
    }
    if a.has(Attributes::UV0) {
        m.uv0 = r.arrays(n)?;
    }
    if a.has(Attributes::TANGENT) {
        m.tangents = r
            .shuffled(n)?
            .into_iter()
            .map(|word| {
                let [x, y, z] = octahedral_decode(word & !1);
                [x, y, z, if word & 1 != 0 { -1.0 } else { 1.0 }]
            })
            .collect();
    }
    if a.has(Attributes::UV1) {
        m.uv1 = r.arrays(n)?;
    }
    if a.has(Attributes::COLOR) {
        m.colors = r.arrays(n)?;
    }
    if a.has(Attributes::SKIN) {
        let raw = r.take(n * 8)?;
        m.joints = raw
            .chunks_exact(8)
            .map(|c| {
                let j = |i: usize| u16::from_le_bytes([c[i * 2], c[i * 2 + 1]]);
                [j(0), j(1), j(2), j(3)]
            })
            .collect();
        m.weights = r.arrays(n)?;
    }

    let count = r.count(4 * 6 + 15 * 4)?;
    m.clusters.reserve(count);
    for _ in 0..count {
        m.clusters.push(Cluster {
            vertex_offset: r.u32()?,
            vertex_count: r.u32()?,
            triangle_offset: r.u32()?,
            triangle_count: r.u32()?,
            group: r.u32()?,
            refined: r.u32()? as i32,
            culling: r.bounds()?,
            lod: r.bounds()?,
            parent: r.bounds()?,
        });
    }
    let count = r.count(4)?;
    let mut previous = 0u32;
    m.cluster_vertices = r
        .shuffled(count)?
        .into_iter()
        .map(|zigzag| {
            let delta = ((zigzag >> 1) as i32) ^ -((zigzag & 1) as i32);
            previous = previous.wrapping_add(delta as u32);
            previous
        })
        .collect();
    let count = r.count(1)?;
    m.cluster_triangles = r.take(count)?.to_vec();
    r.skip_pad4();
    let count = r.count(4 + 5 * 4)?;
    for _ in 0..count {
        m.groups.push(Group {
            depth: r.u32()?,
            simplified: r.bounds()?,
        });
    }
    let count = r.count(5 * 4 + 12)?;
    for _ in 0..count {
        m.nodes.push(Node {
            bounds: r.bounds()?,
            group: r.u32()? as i32,
            child_offset: r.u32()?,
            child_count: r.u32()?,
        });
    }
    m.proxy_threshold = r.f32()?;
    validate(&m)?;
    Ok(m)
}

/// Every index in the file must land inside the array it points into. The
/// renderer uploads these straight to the GPU, where an out-of-range index is
/// not an error but garbage on screen or a device loss, so a damaged file is
/// refused here instead.
fn validate(m: &RvgMesh) -> Result<(), RvgLoadError> {
    let bad = |what: &'static str| Err(RvgLoadError::Corrupt(what));
    let vertex_count = m.positions.len() as u32;
    if m.cluster_vertices.iter().any(|&v| v >= vertex_count) {
        return bad("cluster vertex out of range");
    }
    // Grouped storage is what lets a group be streamed as one range.
    if m.clusters.windows(2).any(|w| w[1].group < w[0].group) {
        return bad("clusters are not stored by group");
    }
    for c in &m.clusters {
        if c.refined < -1 || c.refined >= m.groups.len() as i32 {
            return bad("cluster refined from a group out of range");
        }
        let vertices_end = c.vertex_offset as usize + c.vertex_count as usize;
        let triangles_end = (c.triangle_offset as usize + c.triangle_count as usize) * 3;
        if vertices_end > m.cluster_vertices.len()
            || triangles_end > m.cluster_triangles.len()
            || c.group as usize >= m.groups.len()
        {
            return bad("cluster range out of bounds");
        }
        let start = c.triangle_offset as usize * 3;
        if m.cluster_triangles[start..triangles_end]
            .iter()
            .any(|&t| t as u32 >= c.vertex_count)
        {
            return bad("cluster triangle out of range");
        }
    }
    for n in &m.nodes {
        if n.group >= m.groups.len() as i32
            || n.child_offset as usize + n.child_count as usize > m.nodes.len()
        {
            return bad("hierarchy node out of range");
        }
    }
    Ok(())
}

struct Writer(Vec<u8>);

impl Writer {
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn floats(&mut self, v: &[f32]) {
        for f in v {
            self.0.extend_from_slice(&f.to_le_bytes());
        }
    }
    fn bounds(&mut self, b: &Bounds) {
        self.floats(&b.center);
        self.floats(&[b.radius, b.error]);
    }
    /// 32-bit words byte-shuffled: all first bytes, then all second bytes, and
    /// so on (see the layout notes at the top).
    fn shuffled(&mut self, words: &[u32]) {
        for byte in 0..4 {
            self.0.extend(words.iter().map(|w| (w >> (byte * 8)) as u8));
        }
    }
    fn shuffled_floats(&mut self, v: &[f32]) {
        let words: Vec<u32> = v.iter().map(|f| f.to_bits()).collect();
        self.shuffled(&words);
    }
    fn pad4(&mut self) {
        while !self.0.len().is_multiple_of(4) {
            self.0.push(0);
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], RvgLoadError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(RvgLoadError::Truncated)?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u32(&mut self) -> Result<u32, RvgLoadError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Result<u64, RvgLoadError> {
        let b = self.take(8)?;
        let mut a = [0; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }
    fn f32(&mut self) -> Result<f32, RvgLoadError> {
        self.u32().map(f32::from_bits)
    }
    /// A count prefix, refused when the elements it announces could not fit in
    /// what is left of the file. Without this a corrupt count would reserve
    /// gigabytes before the first read failed.
    fn count(&mut self, element_size: usize) -> Result<usize, RvgLoadError> {
        let n = self.u32()? as usize;
        if n.saturating_mul(element_size) > self.bytes.len() - self.at {
            return Err(RvgLoadError::Truncated);
        }
        Ok(n)
    }
    /// `n` words written by [`Writer::shuffled`].
    fn shuffled(&mut self, n: usize) -> Result<Vec<u32>, RvgLoadError> {
        let raw = self.take(n.checked_mul(4).ok_or(RvgLoadError::Truncated)?)?;
        let (b0, rest) = raw.split_at(n);
        let (b1, rest) = rest.split_at(n);
        let (b2, b3) = rest.split_at(n);
        Ok((0..n)
            .map(|i| u32::from_le_bytes([b0[i], b1[i], b2[i], b3[i]]))
            .collect())
    }
    /// `n` arrays of `N` floats, written by [`Writer::shuffled_floats`].
    fn arrays<const N: usize>(&mut self, n: usize) -> Result<Vec<[f32; N]>, RvgLoadError> {
        let words = self.shuffled(n.checked_mul(N).ok_or(RvgLoadError::Truncated)?)?;
        Ok(words
            .chunks_exact(N)
            .map(|c| std::array::from_fn(|i| f32::from_bits(c[i])))
            .collect())
    }
    fn bounds(&mut self) -> Result<Bounds, RvgLoadError> {
        Ok(Bounds {
            center: [self.f32()?, self.f32()?, self.f32()?],
            radius: self.f32()?,
            error: self.f32()?,
        })
    }
    fn skip_pad4(&mut self) {
        self.at = (self.at + 3) & !3;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RvgFile {
        let b = |e: f32| Bounds {
            center: [1.0, 2.0, 3.0],
            radius: 0.5,
            error: e,
        };
        RvgFile {
            builder_version: BUILDER_VERSION,
            source_hash: 42,
            meshes: vec![Arc::new(RvgMesh {
                mesh_index: 3,
                primitive_index: 1,
                attributes: Attributes(Attributes::NORMAL | Attributes::UV0 | Attributes::SKIN),
                positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                uv0: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                joints: vec![[0, 1, 2, 3]; 3],
                weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
                clusters: vec![Cluster {
                    vertex_offset: 0,
                    vertex_count: 3,
                    triangle_offset: 0,
                    triangle_count: 1,
                    group: 0,
                    refined: -1,
                    culling: b(0.0),
                    lod: b(0.0),
                    parent: b(f32::MAX),
                }],
                cluster_vertices: vec![0, 1, 2],
                cluster_triangles: vec![0, 1, 2],
                groups: vec![Group {
                    depth: 0,
                    simplified: b(f32::MAX),
                }],
                nodes: vec![Node {
                    bounds: b(f32::MAX),
                    group: 0,
                    child_offset: 0,
                    child_count: 0,
                }],
                proxy_threshold: 0.0,
                ..Default::default()
            })],
            scene: None,
        }
    }

    #[test]
    fn round_trips() {
        let file = sample();
        assert_eq!(RvgFile::from_bytes(&file.to_bytes()).unwrap(), file);
        // And with a scene: any GLB's bytes, carried as they are.
        let mut with_scene = sample();
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&[2, 0, 0, 0, 12, 0, 0, 0]);
        with_scene.scene = Some(Arc::new(glb));
        assert_eq!(RvgFile::from_bytes(&with_scene.to_bytes()).unwrap(), with_scene);
    }

    /// Normals and tangents come back within the 16-bit octahedral precision
    /// the renderer uploads anyway, tangent signs exactly, and cluster
    /// vertices exactly in any order (the deltas between them go negative).
    #[test]
    fn packs_normals_tangents_and_vertex_order() {
        let mut file = sample();
        let mesh = Arc::make_mut(&mut file.meshes[0]);
        mesh.attributes.0 |= Attributes::TANGENT;
        mesh.normals = vec![[0.6, -0.8, 0.0], [0.0, 0.0, -1.0], [0.267, 0.535, -0.802]];
        mesh.tangents = vec![[1.0, 0.0, 0.0, -1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 0.0, 1.0, -1.0]];
        mesh.cluster_vertices = vec![2, 0, 1];
        let back = RvgFile::from_bytes(&file.to_bytes()).unwrap();
        let got = &back.meshes[0];
        let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3);
        for (a, b) in got.normals.iter().zip(&file.meshes[0].normals) {
            assert!(close(*a, *b), "{a:?} vs {b:?}");
        }
        for (a, b) in got.tangents.iter().zip(&file.meshes[0].tangents) {
            assert!(close([a[0], a[1], a[2]], [b[0], b[1], b[2]]), "{a:?} vs {b:?}");
            assert_eq!(a[3], b[3]);
        }
        assert_eq!(got.cluster_vertices, vec![2, 0, 1]);
    }

    #[test]
    fn octahedral_round_trips() {
        for v in [
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [0.577, -0.577, -0.577],
        ] {
            let back = octahedral_decode(octahedral_encode(v));
            let dot: f32 = (0..3).map(|i| back[i] * v[i]).sum::<f32>() / (v.iter().map(|x| x * x).sum::<f32>()).sqrt();
            assert!(dot > 0.9999, "{v:?} came back as {back:?}");
        }
    }

    #[test]
    fn refuses_truncation_and_bad_indices() {
        let bytes = sample().to_bytes();
        for len in [0, 3, 20, bytes.len() - 1] {
            assert!(RvgFile::from_bytes(&bytes[..len]).is_err(), "len {len}");
        }
        let mut file = sample();
        Arc::make_mut(&mut file.meshes[0]).cluster_vertices[2] = 99;
        assert!(matches!(
            RvgFile::from_bytes(&file.to_bytes()),
            Err(RvgLoadError::Corrupt(_))
        ));
    }

    #[test]
    fn staleness_follows_source_and_builder() {
        let source = b"model bytes";
        let mut file = sample();
        file.source_hash = source_hash(source);
        assert!(file.is_current(source));
        assert!(!file.is_current(b"other bytes"));
        file.builder_version = BUILDER_VERSION + 1;
        assert!(!file.is_current(source));
    }

    #[test]
    fn header_reads_without_the_body() {
        let source = b"model bytes";
        let mut file = sample();
        file.source_hash = source_hash(source);
        let bytes = file.to_bytes();
        let header = Header::read(&bytes[..Header::LEN]).unwrap();
        assert!(header.is_current(source));
        assert!(!header.is_current(b"changed"));
        assert!(Header::read(b"RVG").is_none());
        assert!(Header::read(&[0; 32]).is_none());
    }
}
