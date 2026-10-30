//! The builder: a GLB in, an `.rvg` out.
//!
//! Each triangle primitive goes through meshoptimizer's cluster LOD builder,
//! which clusterizes it, partitions the clusters into groups, simplifies every
//! group with its outer border locked and repeats on the result until nothing
//! simplifies further. This module turns what that produces into the file
//! layout: per-cluster error bounds (so the renderer needs no DAG traversal),
//! meshlet-local triangle indices, the bounding hierarchy, and a proxy cut for
//! ray tracing.
//!
//! The vertex arrays are kept whole and unmodified. Simplification only ever
//! chooses a subset of the original vertices, so every level of the DAG can
//! index the same arrays, and every attribute (tangents, second UV set,
//! colours, skin weights) survives without being interpolated.

use std::path::PathBuf;

use crate::{RvgFile, RvgMesh};
#[cfg(not(target_arch = "wasm32"))]
use crate::{Attributes, Bounds, Cluster, Group, Node};

/// A primitive the builder left as ordinary geometry, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Skipped {
    pub mesh_index: u32,
    pub primitive_index: u32,
    pub reason: String,
}

/// What [`build_from_glb`] made.
#[derive(Debug, Default)]
pub struct Built {
    pub file: RvgFile,
    pub skipped: Vec<Skipped>,
}

/// What [`ensure_current`] did.
#[derive(Debug)]
pub enum Ensured {
    /// The `.rvg` was already built from this GLB by this builder.
    UpToDate,
    /// It was (re)built and written.
    Built {
        primitives: usize,
        skipped: Vec<Skipped>,
        bytes: usize,
        seconds: f32,
    },
    /// Nothing in the GLB could be converted, so no `.rvg` is kept for it.
    NothingToConvert { skipped: Vec<Skipped> },
    /// The `.rvg` of that name is a model in its own right, not this GLB's
    /// virtual geometry, and was left alone. The GLB beside it is a leftover
    /// or a copy; replacing the model with clusters alone would lose its
    /// scene, and any rig or blend shapes made on it since.
    Model,
}

/// What [`convert_to_model`] did with one `.glb`.
#[derive(Debug, PartialEq)]
pub enum Converted {
    /// The `.rvg` was written; the `.glb` can go. Sizes in bytes, the second
    /// being the `.glb` plus the sidecar `.rvg` it replaces.
    Written { rvg: PathBuf, bytes: u64, before: u64 },
    /// An `.rvg` model built from this very `.glb` was already there (a
    /// conversion that stopped before the `.glb` was removed), so the `.glb`
    /// can go as well.
    AlreadyConverted { rvg: PathBuf },
    /// A different model already has the `.rvg` name. Both are kept: which
    /// one is wanted is not something to guess.
    NameTaken { rvg: PathBuf },
}

/// Whether the file at `path` is an `.rvg` model (one holding its scene), and
/// its header if so. Reads the whole file, so it is only asked once a cheaper
/// check has not settled the question.
fn model_header(path: &std::path::Path) -> Option<crate::Header> {
    let bytes = std::fs::read(path).ok()?;
    let header = crate::Header::read(&bytes)?;
    RvgFile::from_bytes(&bytes).ok()?.scene.is_some().then_some(header)
}

/// Write `bytes` beside `path` and rename it over, so a reader never sees
/// half a file.
fn replace(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("rvg.tmp");
    std::fs::write(&temp, bytes).map_err(|e| format!("{}: {e}", temp.display()))?;
    std::fs::rename(&temp, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("{}: {e}", path.display())
    })
}

/// Turn a model stored as `.glb` (from before imports were stored as `.rvg`)
/// into an `.rvg` model of the same name, replacing the sidecar `.rvg` that
/// held its virtual geometry. The `.glb` itself is not touched: removing it is
/// the caller's, once every reference to it points at the `.rvg`.
pub fn convert_to_model(glb: &std::path::Path) -> Result<Converted, String> {
    let source = std::fs::read(glb).map_err(|e| format!("{}: {e}", glb.display()))?;
    let rvg = rvg_path_for(glb);
    if let Some(header) = model_header(&rvg) {
        return Ok(if header.source_hash == crate::source_hash(&source) {
            Converted::AlreadyConverted { rvg }
        } else {
            Converted::NameTaken { rvg }
        });
    }
    let before = source.len() as u64 + std::fs::metadata(&rvg).map(|m| m.len()).unwrap_or(0);
    let bytes = build_model(&source)?.to_bytes();
    replace(&rvg, &bytes)?;
    Ok(Converted::Written { rvg, bytes: bytes.len() as u64, before })
}

/// Every `.glb` under `root`, sorted, skipping what `ignored` says to (given
/// the forward-slashed path relative to `root`). The editor and the exporter
/// pass the project watcher's own ignore rule, so both see the same models.
pub fn find_models(root: &std::path::Path, ignored: impl Fn(&str) -> bool) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if ignored(&relative) {
                continue;
            }
            match entry.file_type() {
                Ok(t) if t.is_dir() => dirs.push(path),
                Ok(t) if t.is_file()
                    && path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("glb")) =>
                {
                    out.push(path)
                }
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// The `.rvg` path for a model: the same name with the extension changed.
pub fn rvg_path_for(glb: &std::path::Path) -> std::path::PathBuf {
    glb.with_extension("rvg")
}

/// Make sure the `.rvg` beside `glb` is current, building it if not.
///
/// "Current" is decided from the existing file's header alone, so a model
/// that has not changed costs one read of the GLB (to hash it) and 20 bytes of
/// the `.rvg`. The new file is written beside the old one and renamed over it,
/// so a reader never sees half a file.
pub fn ensure_current(glb: &std::path::Path) -> Result<Ensured, String> {
    use std::io::Read;
    let source = std::fs::read(glb).map_err(|e| format!("{}: {e}", glb.display()))?;
    let out = rvg_path_for(glb);
    let mut header = [0u8; crate::Header::LEN];
    let current = std::fs::File::open(&out)
        .and_then(|mut f| f.read_exact(&mut header))
        .ok()
        .and_then(|_| crate::Header::read(&header))
        .is_some_and(|h| h.is_current(&source));
    if current {
        return Ok(Ensured::UpToDate);
    }
    if model_header(&out).is_some() {
        return Ok(Ensured::Model);
    }

    let started = std::time::Instant::now();
    let built = build_from_glb(&source)?;
    if built.file.meshes.is_empty() {
        // A stale file for a model that no longer converts would be loaded
        // and drawn with the wrong geometry.
        let _ = std::fs::remove_file(&out);
        return Ok(Ensured::NothingToConvert {
            skipped: built.skipped,
        });
    }
    let bytes = built.file.to_bytes();
    replace(&out, &bytes)?;
    Ok(Ensured::Built {
        primitives: built.file.meshes.len(),
        skipped: built.skipped,
        bytes: bytes.len(),
        seconds: started.elapsed().as_secs_f32(),
    })
}

/// One primitive's vertex data, every array either empty or as long as
/// `positions`.
#[derive(Clone, Debug, Default)]
pub struct MeshInput {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv0: Vec<[f32; 2]>,
    pub tangents: Vec<[f32; 4]>,
    pub uv1: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    /// The material is alpha-masked: leaves, grass, fences. Simplified more
    /// conservatively, so a canopy keeps its outline at a distance.
    pub cutout: bool,
    /// Keep every open edge at full detail at every level. For a mesh that
    /// is one tile of a larger surface built separately (a terrain chunk):
    /// its edges meet a neighbour's, and each tile simplifying its own edge
    /// independently opened pinholes along every seam at a distance.
    pub lock_open_borders: bool,
}

/// Up to this many triangles, the proxy is the full-detail mesh.
///
/// The proxy is the ordinary mesh of a model stored as `.rvg`
/// ([`build_model`]): what shadows, physics, picking and Solari read while the
/// clusters draw the detail. Below this size a coarser copy saves too little
/// to be worth the loss.
pub const PROXY_FULL_DETAIL: usize = 1_024;

/// Past [`PROXY_FULL_DETAIL`], the proxy aims for this fraction of the original
/// triangles (never fewer than `PROXY_FULL_DETAIL`): fine enough that shadows
/// and colliders follow the surface, small enough that the model is not
/// stored twice.
pub const PROXY_RATIO: usize = 4;

/// A model stored as `.rvg`: the virtual geometry of every triangle primitive
/// of `glb`, plus the GLB itself as the model's scene, with each converted
/// primitive's triangles replaced by its coarser proxy (see `scene.rs`). The
/// result replaces the GLB: nothing else is needed to place, render, animate
/// or export the model.
#[cfg(not(target_arch = "wasm32"))]
pub fn build_model(glb: &[u8]) -> Result<RvgFile, String> {
    let mut file = build_from_glb(glb)?.file;
    file.scene = Some(std::sync::Arc::new(crate::scene::proxy_scene(glb, &file)?));
    Ok(file)
}

#[cfg(target_arch = "wasm32")]
pub fn build_model(_glb: &[u8]) -> Result<RvgFile, String> {
    Err("models are built into .rvg on the desktop editor; the web build only loads them".into())
}

/// A model `.rvg` as the GLB it was built from, at full detail: its scene with
/// every proxy swapped back for the full-detail triangles. For tools that edit
/// a model's geometry; [`build_model`] turns their result back into an `.rvg`.
#[cfg(not(target_arch = "wasm32"))]
pub fn full_detail_glb(file: &RvgFile) -> Result<Vec<u8>, String> {
    crate::scene::full_detail_scene(file)
}

#[cfg(target_arch = "wasm32")]
pub fn full_detail_glb(_file: &RvgFile) -> Result<Vec<u8>, String> {
    Err("editing a model's geometry needs the desktop editor".into())
}

/// Build the virtual geometry for every triangle primitive of a GLB.
#[cfg(target_arch = "wasm32")]
pub fn build_from_glb(_glb: &[u8]) -> Result<Built, String> {
    Err("virtual geometry is built on the desktop editor; the web build only loads it".into())
}

/// Build the virtual geometry for every triangle primitive of a GLB.
///
/// A primitive that cannot be converted (points or lines, morph targets, an
/// index that points past the vertices) is listed in [`Built::skipped`] and
/// keeps rendering as ordinary geometry; it never fails the whole model.
#[cfg(not(target_arch = "wasm32"))]
pub fn build_from_glb(glb: &[u8]) -> Result<Built, String> {
    let doc = gltf::Gltf::from_slice(glb).map_err(|e| format!("not a readable GLB: {e}"))?;
    let blob = doc.blob.as_deref();
    let mut built = Built {
        file: RvgFile {
            builder_version: crate::BUILDER_VERSION,
            source_hash: crate::source_hash(glb),
            meshes: Vec::new(),
            scene: None,
        },
        skipped: Vec::new(),
    };
    for mesh in doc.meshes() {
        for primitive in mesh.primitives() {
            let mesh_index = mesh.index() as u32;
            let primitive_index = primitive.index() as u32;
            match read_primitive(&primitive, blob).and_then(|input| build_mesh(&input)) {
                Ok(mut out) => {
                    out.mesh_index = mesh_index;
                    out.primitive_index = primitive_index;
                    built.file.meshes.push(std::sync::Arc::new(out));
                }
                Err(reason) => built.skipped.push(Skipped {
                    mesh_index,
                    primitive_index,
                    reason,
                }),
            }
        }
    }
    Ok(built)
}

#[cfg(not(target_arch = "wasm32"))]
fn read_primitive(primitive: &gltf::Primitive<'_>, blob: Option<&[u8]>) -> Result<MeshInput, String> {
    if primitive.mode() != gltf::mesh::Mode::Triangles {
        return Err(format!("{:?} primitives are not triangles", primitive.mode()));
    }
    // Morph targets move vertices per frame, which the per-cluster bounds and
    // the simplified levels cannot follow.
    if primitive.morph_targets().next().is_some() {
        return Err("has morph targets".into());
    }
    let reader = primitive.reader(|buffer| match buffer.source() {
        gltf::buffer::Source::Bin => blob,
        gltf::buffer::Source::Uri(_) => None,
    });
    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or("has no positions")?
        .collect();
    let indices = match reader.read_indices() {
        Some(indices) => indices.into_u32().collect(),
        None => (0..positions.len() as u32).collect(),
    };
    Ok(MeshInput {
        normals: reader.read_normals().map(|n| n.collect()).unwrap_or_default(),
        uv0: reader
            .read_tex_coords(0)
            .map(|t| t.into_f32().collect())
            .unwrap_or_default(),
        tangents: reader.read_tangents().map(|t| t.collect()).unwrap_or_default(),
        uv1: reader
            .read_tex_coords(1)
            .map(|t| t.into_f32().collect())
            .unwrap_or_default(),
        colors: reader
            .read_colors(0)
            .map(|c| c.into_rgba_f32().collect())
            .unwrap_or_default(),
        joints: reader
            .read_joints(0)
            .map(|j| j.into_u16().collect())
            .unwrap_or_default(),
        weights: reader
            .read_weights(0)
            .map(|w| w.into_f32().collect())
            .unwrap_or_default(),
        positions,
        indices,
        cutout: primitive.material().alpha_mode() == gltf::material::AlphaMode::Mask,
        lock_open_borders: false,
    })
}

#[cfg(target_arch = "wasm32")]
pub fn build_mesh(_input: &MeshInput) -> Result<RvgMesh, String> {
    Err("virtual geometry is built on the desktop editor".into())
}

/// Build the virtual geometry of one primitive.
#[cfg(not(target_arch = "wasm32"))]
pub fn build_mesh(input: &MeshInput) -> Result<RvgMesh, String> {
    let n = input.positions.len();
    // meshoptimizer is C++ and asserts on malformed input, which takes the
    // whole editor down with it. Everything it would assert on is checked
    // here and reported as a skipped primitive instead.
    if input.indices.is_empty() || !input.indices.len().is_multiple_of(3) {
        return Err("has no complete triangles".into());
    }
    if input.indices.iter().any(|&i| i as usize >= n) {
        return Err("an index points past the vertices".into());
    }
    if input.positions.iter().flatten().any(|v| !v.is_finite()) {
        return Err("a position is not a finite number".into());
    }
    let lengths_ok = |len: usize| len == 0 || len == n;
    if !(lengths_ok(input.normals.len())
        && lengths_ok(input.uv0.len())
        && lengths_ok(input.tangents.len())
        && lengths_ok(input.uv1.len())
        && lengths_ok(input.colors.len())
        && lengths_ok(input.joints.len())
        && lengths_ok(input.weights.len()))
    {
        return Err("an attribute has a different vertex count from the positions".into());
    }

    let mut flags = 0;
    for (present, flag) in [
        (!input.normals.is_empty(), Attributes::NORMAL),
        (!input.uv0.is_empty(), Attributes::UV0),
        (!input.tangents.is_empty(), Attributes::TANGENT),
        (!input.uv1.is_empty(), Attributes::UV1),
        (!input.colors.is_empty(), Attributes::COLOR),
        (!input.joints.is_empty() && !input.weights.is_empty(), Attributes::SKIN),
    ] {
        if present {
            flags |= flag;
        }
    }
    let attributes = Attributes(flags);

    let (clusters_raw, groups) = run_clod(input);

    let mut mesh = RvgMesh {
        attributes,
        positions: input.positions.clone(),
        normals: input.normals.clone(),
        uv0: input.uv0.clone(),
        tangents: input.tangents.clone(),
        uv1: input.uv1.clone(),
        colors: input.colors.clone(),
        joints: if attributes.has(Attributes::SKIN) { input.joints.clone() } else { Vec::new() },
        weights: if attributes.has(Attributes::SKIN) { input.weights.clone() } else { Vec::new() },
        groups: groups
            .iter()
            .map(|g| Group {
                depth: g.depth.max(0) as u32,
                simplified: bounds(&g.simplified),
            })
            .collect(),
        ..Default::default()
    };

    for raw in &clusters_raw {
        let (local_vertices, local_triangles) = meshopt::clod_local_indices(&raw.indices);
        let culling = bounds(&raw.bounds);
        let lod = if raw.refined < 0 {
            Bounds { error: 0.0, ..culling }
        } else {
            mesh.groups[raw.refined as usize].simplified
        };
        mesh.clusters.push(Cluster {
            vertex_offset: mesh.cluster_vertices.len() as u32,
            vertex_count: local_vertices.len() as u32,
            triangle_offset: (mesh.cluster_triangles.len() / 3) as u32,
            triangle_count: (local_triangles.len() / 3) as u32,
            group: raw.group,
            refined: raw.refined,
            culling,
            lod,
            parent: mesh.groups[raw.group as usize].simplified,
        });
        mesh.cluster_vertices.extend_from_slice(&local_vertices);
        mesh.cluster_triangles.extend_from_slice(&local_triangles);
    }

    let levels = groups.iter().map(|g| g.depth.max(0) as usize).max().unwrap_or(0) + 1;
    mesh.nodes = meshopt::clod_build_hierarchy(&groups, 8, levels)
        .iter()
        .map(|n| Node {
            bounds: bounds(&n.bounds),
            group: n.group,
            child_offset: n.child_offset,
            child_count: n.child_count,
        })
        .collect();

    mesh.proxy_threshold = proxy_threshold(&mesh);
    Ok(mesh)
}

#[cfg(not(target_arch = "wasm32"))]
struct RawCluster {
    group: u32,
    refined: i32,
    bounds: meshopt::ClodBounds,
    indices: Vec<u32>,
}

/// Run meshoptimizer's cluster LOD builder over one primitive.
#[cfg(not(target_arch = "wasm32"))]
fn run_clod(input: &MeshInput) -> (Vec<RawCluster>, Vec<meshopt::ClodGroup>) {
    let position_bytes: Vec<u8> = input
        .positions
        .as_flattened()
        .iter()
        .flat_map(|f| f.to_ne_bytes())
        .collect();
    // Only fails on a stride/offset mismatch, which these constants cannot have.
    let vertices = meshopt::VertexDataAdapter::new(&position_bytes, 12, 0)
        .expect("tightly packed positions");

    // Normals and the first UV set steer the simplifier: it keeps vertices
    // where they bend sharply or a texture seam runs, and the protect mask
    // stops the permissive mode from collapsing across those seams. The
    // weights follow meshoptimizer's guidance: normals count for a little
    // less than UVs, which a visible texture swim punishes more.
    let has_normals = !input.normals.is_empty();
    let has_uv = !input.uv0.is_empty();
    let mut weights: Vec<f32> = Vec::new();
    if has_normals {
        weights.extend([0.5, 0.5, 0.5]);
    }
    if has_uv {
        weights.extend([1.0, 1.0]);
    }
    let stride = weights.len();
    let mut attributes = Vec::with_capacity(input.positions.len() * stride);
    for i in 0..input.positions.len() {
        if has_normals {
            attributes.extend_from_slice(&input.normals[i]);
        }
        if has_uv {
            attributes.extend_from_slice(&input.uv0[i]);
        }
    }

    let mut config = meshopt::clod_default_config(crate::MAX_CLUSTER_TRIANGLES);
    // Tight bounds on simplified clusters too, not just the group's merged
    // sphere: culling is the first thing the renderer does with them.
    config.optimize_bounds = true;
    if !input.joints.is_empty() && !input.weights.is_empty() {
        // A skinned mesh bends at its joints. meshoptimizer recommends
        // regularization for deformable meshes: it keeps triangle density
        // even, so no sliver spanning a bending elbow gets stretched into a
        // spike when the arm moves.
        config.simplify_regularize = true;
    }
    if input.cutout {
        // Foliage is hundreds of separate cards. Simplifying one collapses it
        // to a sliver, and the sloppy fallback merges what is left into
        // blobs, so a tree thins out and vanishes as it recedes. Cut-out
        // meshes therefore lock their open edges (see `open_border_locks`) and
        // never fall back to sloppy: a canopy keeps its outline, and only the
        // interiors of larger leaf meshes simplify.
        config.simplify_fallback_sloppy = false;
    }

    if input.lock_open_borders {
        // The sloppy fallback ignores locks, so it is off for a locked edge
        // as for foliage.
        config.simplify_fallback_sloppy = false;
    }
    let locks = if input.cutout || input.lock_open_borders { open_border_locks(input) } else { Vec::new() };

    let mesh = meshopt::ClodMesh {
        vertex_lock: (!locks.is_empty()).then_some(locks.as_slice()),
        vertex_attributes: &attributes,
        vertex_attributes_stride: stride * std::mem::size_of::<f32>(),
        attribute_weights: &weights,
        attribute_protect_mask: (1u32 << stride) - 1,
        ..meshopt::ClodMesh::new(&input.indices, &vertices)
    };

    let mut groups = Vec::new();
    let mut clusters = Vec::new();
    meshopt::clod_build(config, &mesh, |group, out| {
        let id = groups.len() as u32;
        groups.push(group);
        clusters.extend(out.iter().map(|c| RawCluster {
            group: id,
            refined: c.refined,
            bounds: c.bounds,
            indices: c.indices.to_vec(),
        }));
        id as i32
    });
    (clusters, groups)
}

/// Lock every vertex on an open edge: an edge only one triangle uses, which is
/// the outline of a leaf card or a fence panel. Edges are compared by position,
/// so a UV seam (two vertices at one point) is not mistaken for an outline.
#[cfg(not(target_arch = "wasm32"))]
fn open_border_locks(input: &MeshInput) -> Vec<meshopt::SimplifyVertex> {
    use std::collections::HashMap;
    let mut welded: HashMap<[u32; 3], u32> = HashMap::new();
    let ids: Vec<u32> = input
        .positions
        .iter()
        .map(|p| {
            let key = p.map(f32::to_bits);
            let next = welded.len() as u32;
            *welded.entry(key).or_insert(next)
        })
        .collect();
    let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
    for tri in input.indices.chunks_exact(3) {
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let (a, b) = (ids[a as usize], ids[b as usize]);
            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    let mut on_border = vec![false; welded.len()];
    for ((a, b), count) in edges {
        if count == 1 {
            on_border[a as usize] = true;
            on_border[b as usize] = true;
        }
    }
    ids.iter()
        .map(|&id| {
            if on_border[id as usize] {
                meshopt::SimplifyVertex::Lock
            } else {
                meshopt::SimplifyVertex::None
            }
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn bounds(b: &meshopt::ClodBounds) -> Bounds {
    Bounds {
        center: b.center,
        radius: b.radius,
        error: b.error,
    }
}

/// The threshold of the cut Solari traces: full detail up to
/// [`PROXY_FULL_DETAIL`] triangles, otherwise the finest cut that fits the
/// budget.
#[cfg(not(target_arch = "wasm32"))]
fn proxy_threshold(mesh: &RvgMesh) -> f32 {
    let original = mesh.original_triangle_count();
    let budget = (original / PROXY_RATIO).max(PROXY_FULL_DETAIL);
    if original <= PROXY_FULL_DETAIL {
        return 0.0;
    }
    // The triangle count of a cut only falls as the threshold rises, so the
    // finest cut within budget is a binary search over the distinct errors.
    let mut thresholds: Vec<f32> = mesh
        .groups
        .iter()
        .map(|g| g.simplified.error)
        .filter(|e| *e < f32::MAX)
        .collect();
    thresholds.push(0.0);
    thresholds.sort_by(f32::total_cmp);
    thresholds.dedup();
    let triangles = |t: f32| -> usize { mesh.cut(t).map(|c| c.triangle_count as usize).sum() };
    let fits = thresholds.partition_point(|&t| triangles(t) > budget);
    thresholds
        .get(fits)
        .or(thresholds.last())
        .copied()
        .unwrap_or(0.0)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use bevy::math::Vec3;
    use std::collections::HashMap;

    /// A closed UV sphere with its poles and seam welded, so every edge of
    /// the input is shared by exactly two triangles.
    fn sphere(rings: u32, segments: u32) -> MeshInput {
        let mut positions = vec![[0.0, 1.0, 0.0]];
        for r in 1..rings {
            let phi = std::f32::consts::PI * r as f32 / rings as f32;
            for s in 0..segments {
                let theta = std::f32::consts::TAU * s as f32 / segments as f32;
                // A gentle bump so simplification has error to accumulate.
                let k = 1.0 + 0.05 * (theta * 5.0).sin() * (phi * 3.0).cos();
                positions.push([phi.sin() * theta.cos() * k, phi.cos() * k, phi.sin() * theta.sin() * k]);
            }
        }
        positions.push([0.0, -1.0, 0.0]);
        let ring = |r: u32, s: u32| 1 + (r - 1) * segments + s % segments;
        let bottom = positions.len() as u32 - 1;
        let mut indices = Vec::new();
        for s in 0..segments {
            indices.extend([0, ring(1, s + 1), ring(1, s)]);
            indices.extend([bottom, ring(rings - 1, s), ring(rings - 1, s + 1)]);
        }
        for r in 1..rings - 1 {
            for s in 0..segments {
                let (a, b, c, d) = (ring(r, s), ring(r, s + 1), ring(r + 1, s), ring(r + 1, s + 1));
                indices.extend([a, b, d, a, d, c]);
            }
        }
        let normals = positions
            .iter()
            .map(|p: &[f32; 3]| {
                let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                [p[0] / l, p[1] / l, p[2] / l]
            })
            .collect();
        MeshInput {
            positions,
            normals,
            indices,
            ..Default::default()
        }
    }

    #[test]
    fn full_detail_cut_is_the_input() {
        let input = sphere(48, 96);
        let mesh = build_mesh(&input).unwrap();
        assert!(mesh.groups.iter().any(|g| g.depth > 0), "the DAG has more than one level");
        assert_eq!(mesh.original_triangle_count(), input.indices.len() / 3);
        assert_eq!(mesh.cut_indices(0.0).len(), input.indices.len());
        for c in &mesh.clusters {
            assert!(c.triangle_count as usize <= crate::MAX_CLUSTER_TRIANGLES);
            assert!(c.vertex_count <= 256);
        }
    }

    #[test]
    fn error_grows_up_the_dag() {
        let mesh = build_mesh(&sphere(48, 96)).unwrap();
        for c in &mesh.clusters {
            assert!(c.lod.error <= c.parent.error, "{:?}", c);
        }
    }

    /// Every cut, at any threshold, is a closed surface: each edge is shared
    /// by exactly two triangles. A crack between clusters of different levels
    /// would leave an edge with one.
    #[test]
    fn every_cut_is_watertight() {
        let mesh = build_mesh(&sphere(48, 96)).unwrap();
        let mut thresholds: Vec<f32> = mesh
            .groups
            .iter()
            .map(|g| g.simplified.error)
            .filter(|e| *e < f32::MAX)
            .collect();
        thresholds.push(0.0);
        thresholds.sort_by(f32::total_cmp);
        let mut last = usize::MAX;
        for &t in &thresholds {
            let indices = mesh.cut_indices(t);
            assert!(indices.len() <= last, "coarser threshold, no more triangles");
            last = indices.len();
            let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
            for tri in indices.chunks_exact(3) {
                for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                    if a != b {
                        *edges.entry((a.min(b), a.max(b))).or_default() += 1;
                    }
                }
            }
            let open = edges.values().filter(|&&n| n != 2).count();
            assert_eq!(open, 0, "threshold {t}: {open} edges not shared by two triangles");
        }
        assert!(last < mesh.original_triangle_count(), "the coarsest cut simplified something");
    }

    /// The streaming rule: whatever closed set of groups is resident, at any
    /// threshold, the drawn clusters form a closed surface, and with every
    /// group resident they are exactly the ordinary cut.
    #[test]
    fn every_streamed_cut_is_watertight() {
        let mesh = build_mesh(&sphere(48, 96)).unwrap();
        let graph = mesh.group_graph();
        let groups = mesh.groups.len();
        assert!(graph.roots().count() > 0);
        for (g, range) in graph.clusters.iter().enumerate() {
            assert!(range.start < range.end, "group {g} has no clusters");
        }

        let mut thresholds: Vec<f32> = mesh
            .groups
            .iter()
            .map(|g| g.simplified.error)
            .filter(|e| *e < f32::MAX)
            .collect();
        thresholds.push(0.0);
        thresholds.sort_by(f32::total_cmp);
        thresholds.dedup();

        // A tiny deterministic generator: the test must not flake.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let watertight = |clusters: &[usize]| {
            let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
            for &c in clusters {
                let indices: Vec<u32> = mesh.cluster_indices(&mesh.clusters[c]).collect();
                for tri in indices.chunks_exact(3) {
                    for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                        if a != b {
                            *edges.entry((a.min(b), a.max(b))).or_default() += 1;
                        }
                    }
                }
            }
            edges.values().filter(|&&n| n != 2).count()
        };

        let all = vec![true; groups];
        for &t in &thresholds {
            let streamed: Vec<usize> = graph.cut(&mesh, t, &all).collect();
            let ordinary: Vec<usize> = (0..mesh.clusters.len()).filter(|&i| mesh.clusters[i].selected(t)).collect();
            assert_eq!(streamed, ordinary, "threshold {t}: fully resident is not the ordinary cut");
        }

        for round in 0..40 {
            // Grow a closed resident set from the roots: a group joins only
            // once all its parents are in, as the streaming manager loads them.
            let mut resident = vec![false; groups];
            for root in graph.roots() {
                resident[root as usize] = true;
            }
            let keep = next() % 100;
            loop {
                let mut grew = false;
                for g in 0..groups {
                    if !resident[g] && graph.parents[g].iter().all(|&p| resident[p as usize]) && next() % 100 < keep {
                        resident[g] = true;
                        grew = true;
                    }
                }
                if !grew {
                    break;
                }
            }
            assert!(graph.is_closed(&resident));
            for &t in &thresholds {
                let drawn: Vec<usize> = graph.cut(&mesh, t, &resident).collect();
                assert!(!drawn.is_empty(), "round {round}, threshold {t}: nothing drawn");
                let open = watertight(&drawn);
                assert_eq!(open, 0, "round {round}, threshold {t}: {open} open edges");
            }
        }

        // And the closure is what makes it hold: drop one parent out from
        // under its resident children and some threshold opens a hole. This
        // is also what proves the check above can see a crack at all.
        let parent = (0..groups as u32)
            .find(|&g| !graph.children[g as usize].is_empty() && !graph.parents[g as usize].is_empty())
            .expect("a group in the middle of the DAG");
        let mut broken = vec![true; groups];
        broken[parent as usize] = false;
        assert!(!graph.is_closed(&broken));
        let worst = thresholds
            .iter()
            .map(|&t| watertight(&graph.cut(&mesh, t, &broken).collect::<Vec<_>>()))
            .max()
            .unwrap();
        assert!(worst > 0, "an unclosed resident set should crack somewhere");
    }

    #[test]
    fn proxy_is_bounded() {
        let mesh = build_mesh(&sphere(16, 32)).unwrap();
        // Small mesh: the proxy is the full-detail surface.
        assert!(mesh.original_triangle_count() <= PROXY_FULL_DETAIL);
        assert_eq!(mesh.proxy_indices().len(), mesh.original_triangle_count() * 3);

        let big = build_mesh(&sphere(96, 192)).unwrap();
        let original = big.original_triangle_count();
        assert!(original > PROXY_FULL_DETAIL);
        let proxy = big.proxy_indices().len() / 3;
        assert!(proxy <= (original / PROXY_RATIO).max(PROXY_FULL_DETAIL), "{proxy} of {original}");
        assert!(proxy > 0);
    }

    /// A canopy: thousands of separate small cards scattered through a ball,
    /// as a tree's leaves are.
    fn canopy(cards: u32) -> MeshInput {
        let mut input = MeshInput::default();
        let mut seed = 12345u32;
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        for _ in 0..cards {
            let centre = Vec3::new(next() * 2.0 - 1.0, next() * 2.0 - 1.0, next() * 2.0 - 1.0) * 3.0;
            let a = Vec3::new(next() - 0.5, next() - 0.5, next() - 0.5).normalize() * 0.15;
            let b = a.cross(Vec3::Y).normalize() * 0.15;
            let base = input.positions.len() as u32;
            for (corner, uv) in [(-a - b, [0.0, 0.0]), (a - b, [1.0, 0.0]), (a + b, [1.0, 1.0]), (-a + b, [0.0, 1.0])] {
                input.positions.push((centre + corner).to_array());
                input.normals.push(a.cross(b).normalize().to_array());
                input.uv0.push(uv);
            }
            input.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        input
    }

    fn area(mesh: &RvgMesh, indices: &[u32]) -> f32 {
        indices
            .chunks_exact(3)
            .map(|t| {
                let p = |i: u32| Vec3::from(mesh.positions[i as usize]);
                (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0])).length() * 0.5
            })
            .sum()
    }

    /// The coarsest level of a cut-out canopy keeps most of its leaf area, so
    /// a distant tree does not thin out; the sloppy fallback ordinary meshes
    /// use would merge the cards away.
    #[test]
    fn cutout_foliage_keeps_its_area_when_simplified() {
        let mut leaves = canopy(2000);
        let full = {
            let mesh = build_mesh(&leaves).unwrap();
            area(&mesh, &mesh.cut_indices(0.0))
        };
        leaves.cutout = true;
        let mesh = build_mesh(&leaves).unwrap();
        let coarsest = mesh
            .groups
            .iter()
            .map(|g| g.simplified.error)
            .filter(|e| *e < f32::MAX)
            .fold(0.0, f32::max);
        let kept = area(&mesh, &mesh.cut_indices(coarsest)) / full;
        assert!(kept > 0.75, "a cut-out canopy kept only {:.0}% of its area", kept * 100.0);
    }

    #[test]
    fn malformed_input_is_skipped_not_fatal() {
        let mut input = sphere(8, 16);
        input.indices[4] = 10_000;
        assert!(build_mesh(&input).is_err());
        let mut input = sphere(8, 16);
        input.positions[3][1] = f32::NAN;
        assert!(build_mesh(&input).is_err());
        let mut input = sphere(8, 16);
        input.uv0 = vec![[0.0; 2]; 3];
        assert!(build_mesh(&input).is_err());
    }

    #[test]
    fn round_trips_through_bytes() {
        let file = RvgFile {
            builder_version: crate::BUILDER_VERSION,
            source_hash: 7,
            meshes: vec![std::sync::Arc::new(build_mesh(&sphere(16, 32)).unwrap())],
            scene: None,
        };
        let back = RvgFile::from_bytes(&file.to_bytes()).unwrap();
        // Normals are stored at 16-bit octahedral precision, so the first
        // trip rounds them; everything else, and every later trip, is exact.
        let (original, decoded) = (&file.meshes[0], &back.meshes[0]);
        for (a, b) in original.normals.iter().zip(&decoded.normals) {
            let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            assert!(dot > 0.9999, "{a:?} came back as {b:?}");
        }
        let mut rounded = (**original).clone();
        rounded.normals = decoded.normals.clone();
        assert_eq!(**decoded, rounded);
        assert_eq!(RvgFile::from_bytes(&back.to_bytes()).unwrap(), back);
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("renzora_rvg_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Converting an old `.glb` replaces its sidecar with a model `.rvg`, and
    /// converting again (after a stop before the `.glb` was removed) says the
    /// work is done rather than building it twice.
    #[test]
    fn an_old_glb_converts_over_its_sidecar() {
        let dir = scratch("convert");
        let glb = dir.join("ball.glb");
        std::fs::write(&glb, crate::scene::tests::sphere_glb(24, 48)).unwrap();
        assert!(matches!(ensure_current(&glb), Ok(Ensured::Built { .. })));
        let sidecar = RvgFile::from_bytes(&std::fs::read(dir.join("ball.rvg")).unwrap()).unwrap();
        assert!(sidecar.scene.is_none());

        let Ok(Converted::Written { rvg, bytes, before }) = convert_to_model(&glb) else {
            panic!("not converted");
        };
        assert_eq!(rvg, dir.join("ball.rvg"));
        assert_eq!(bytes, std::fs::metadata(&rvg).unwrap().len());
        assert!(before > 0);
        assert!(RvgFile::from_bytes(&std::fs::read(&rvg).unwrap()).unwrap().scene.is_some());
        assert_eq!(convert_to_model(&glb), Ok(Converted::AlreadyConverted { rvg }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A model `.rvg` is never replaced by a `.glb` of the same name: not by
    /// the sidecar builder, and not by a conversion of a different model.
    #[test]
    fn a_model_rvg_is_never_overwritten() {
        let dir = scratch("keep");
        let glb = dir.join("ball.glb");
        let rvg = dir.join("ball.rvg");
        let model = build_model(&crate::scene::tests::sphere_glb(24, 48)).unwrap().to_bytes();
        std::fs::write(&rvg, &model).unwrap();
        std::fs::write(&glb, crate::scene::tests::sphere_glb(16, 32)).unwrap();

        assert!(matches!(ensure_current(&glb), Ok(Ensured::Model)));
        assert_eq!(convert_to_model(&glb), Ok(Converted::NameTaken { rvg: rvg.clone() }));
        assert_eq!(std::fs::read(&rvg).unwrap(), model);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
