//! Node previews: the whole graph re-evaluated in the background at a quarter
//! of the target terrain's resolution after every edit, each node's result
//! drawn into a small image on its node.
//!
//! A height is drawn as shaded relief (lit from the north-west, tinted low to
//! high); a mask as grey. The images are one stable `Handle<Image>` per node
//! whose pixels are rewritten in place, so a new preview never rebuilds the
//! node it lands on.
//!
//! Evaluation runs on the async compute pool. A newer edit cancels the run in
//! flight (it would only be thrown away), and a short settle time keeps a
//! scrub from starting and cancelling a run every frame. Results are cached
//! per node ([`EvalCache`]), so an edit re-evaluates only what it touched.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use renzora_terrain::data::{TerrainChunkData, TerrainData};
use renzora_terrain_graph::catalog::{node_def, PinKind};
use renzora_terrain_graph::eval::fold_seed;
use renzora_terrain_graph::{
    evaluate, gather_generator_layer, Cancelled, EvalCache, EvalControl, EvalRequest, EvalResult, Field, Grid,
    TerrainGenerator, TerrainGeneratorLayer,
};

use crate::state::{target_terrain, TerrainGraphEditorState};

/// Preview images are this many pixels square.
pub const PREVIEW_PX: u32 = 96;
/// Seconds an edit must settle before its preview starts.
const SETTLE_SECS: f64 = 0.1;
/// Fewest and most samples along a preview's long side.
const PREVIEW_MIN: usize = 33;
const PREVIEW_MAX: usize = 129;

/// One image per node of the open graph.
#[derive(Resource, Default)]
pub struct PreviewImages {
    doc: Option<String>,
    pub map: HashMap<u64, Handle<Image>>,
}

struct Running {
    task: Task<Result<EvalResult, Cancelled>>,
    control: EvalControl,
    sig: u64,
}

#[derive(Resource, Default)]
pub struct PreviewJob {
    running: Option<Running>,
    cache: Arc<Mutex<EvalCache>>,
    /// The signature the shown previews were computed for.
    shown: u64,
    /// A signature waiting to settle, and since when.
    pending: Option<(u64, f64)>,
    /// 0..1 while a preview is computing.
    pub progress: Option<f32>,
}

/// What the previews are evaluated against: the target terrain's extent,
/// seed and overrides, at a quarter of its resolution.
fn preview_request(world: &mut World) -> EvalRequest {
    let needs_current = world.resource::<TerrainGraphEditorState>().graph.nodes.iter().any(|n| n.node_type == "current_terrain");
    let Some(terrain) = target_terrain(world) else {
        let mut req = EvalRequest::new(Grid::fitting(Vec2::splat(512.0), 97), 1337);
        req.root = renzora_terrain_graph::world::project_root(world);
        return req;
    };
    let data = world.get::<TerrainData>(terrain).cloned().unwrap_or_default();
    let full = renzora_terrain_graph::terrain_grid(&data);
    let samples = ((full.w.max(full.h) - 1) / 4 + 1).clamp(PREVIEW_MIN, PREVIEW_MAX);
    let generator = world.get::<TerrainGenerator>(terrain).cloned();
    let seed = generator.as_ref().map_or(1337, |g| fold_seed(g.seed));
    let mut req = EvalRequest::new(Grid::fitting(full.extent, samples), seed);
    req.overrides = generator.map(|g| g.override_pairs()).unwrap_or_default();
    req.root = renzora_terrain_graph::world::project_root(world);
    if needs_current {
        let chunks = renzora_terrain_graph::world::terrain_chunks(world, terrain);
        let pairs: Vec<(&TerrainChunkData, Option<&TerrainGeneratorLayer>)> = chunks
            .iter()
            .filter_map(|e| world.get::<TerrainChunkData>(*e).map(|c| (c, world.get::<TerrainGeneratorLayer>(*e))))
            .collect();
        let layer = gather_generator_layer(&data, &pairs);
        req.current = Some(Arc::new(layer.resampled(req.grid.w, req.grid.h)));
    }
    req
}

/// Everything a preview depends on, hashed. Node positions are left out:
/// dragging a node changes nothing a preview shows.
fn signature(st: &TerrainGraphEditorState, req: &EvalRequest) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for n in &st.graph.nodes {
        (n.id, &n.node_type).hash(&mut h);
        for (k, v) in &n.params {
            k.hash(&mut h);
            v.hash_into(&mut h);
        }
    }
    st.graph.connections.hash(&mut h);
    for p in &st.graph.params {
        (&p.name, p.node, &p.param).hash(&mut h);
    }
    (req.grid.w, req.grid.h, req.grid.extent.x.to_bits(), req.grid.extent.y.to_bits(), req.seed).hash(&mut h);
    for (k, v) in &req.overrides {
        (k, v.to_bits()).hash(&mut h);
    }
    if let Some(c) = &req.current {
        for v in c.data.iter().step_by((c.data.len() / 2048).max(1)) {
            v.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

/// Start a preview when the graph (or what it previews against) has changed
/// and settled.
pub fn schedule_previews(world: &mut World) {
    if !world.resource::<TerrainGraphEditorState>().is_open() {
        return;
    }
    let req = preview_request(world);
    let sig = signature(world.resource::<TerrainGraphEditorState>(), &req);
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let mut job = world.resource_mut::<PreviewJob>();
    let in_flight = job.running.as_ref().map(|r| r.sig);
    if sig == job.shown || in_flight == Some(sig) {
        job.pending = None;
        return;
    }
    match job.pending {
        Some((s, since)) if s == sig => {
            if now - since < SETTLE_SECS {
                return;
            }
        }
        _ => {
            job.pending = Some((sig, now));
            // The first preview of a document need not wait.
            if job.shown != 0 || job.running.is_some() {
                return;
            }
        }
    }
    job.pending = None;
    if let Some(old) = job.running.take() {
        old.control.cancel();
    }
    let graph = world.resource::<TerrainGraphEditorState>().graph.clone();
    let mut job = world.resource_mut::<PreviewJob>();
    let cache = job.cache.clone();
    let control = EvalControl::new();
    let ctl = control.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
        evaluate(&graph, &req, &mut cache, &ctl)
    });
    job.running = Some(Running { task, control, sig });
    job.progress = Some(0.0);
}

/// Land a finished preview: node images and node errors.
pub fn poll_previews(world: &mut World) {
    let finished = {
        let mut job = world.resource_mut::<PreviewJob>();
        let Some(running) = job.running.as_mut() else { return };
        match block_on(future::poll_once(&mut running.task)) {
            Some(result) => {
                let sig = running.sig;
                job.running = None;
                job.progress = None;
                Some((sig, result))
            }
            None => {
                let p = running.control.progress();
                if job.progress != Some(p) {
                    job.progress = Some(p);
                }
                None
            }
        }
    };
    let Some((sig, Ok(result))) = finished else { return };
    world.resource_mut::<PreviewJob>().shown = sig;

    // Errors, written only when they moved: panels key on this resource.
    let node_errors: HashMap<u64, String> =
        result.nodes.iter().filter_map(|(id, r)| r.error.clone().map(|e| (*id, e))).collect();
    let graph_errors: Vec<String> = result.errors.iter().filter(|(n, _)| n.is_none()).map(|(_, e)| e.clone()).collect();
    {
        let st = world.resource::<TerrainGraphEditorState>();
        let changed = st.node_errors != node_errors || st.graph_errors != graph_errors;
        if changed {
            let mut st = world.resource_mut::<TerrainGraphEditorState>();
            st.node_errors = node_errors;
            st.graph_errors = graph_errors;
        }
    }

    // Images.
    let (doc, kinds): (Option<String>, Vec<(u64, PinKind)>) = {
        let st = world.resource::<TerrainGraphEditorState>();
        let kinds = st
            .graph
            .nodes
            .iter()
            .filter_map(|n| {
                let def = node_def(&n.node_type)?;
                let kind = match def.outputs.first() {
                    Some(p) => p.kind,
                    None => def.inputs.first().map_or(PinKind::Height, |p| p.kind),
                };
                Some((n.id, kind))
            })
            .collect();
        (st.path.clone(), kinds)
    };
    let extent = result.grid.extent;
    let rendered: Vec<(u64, Vec<u8>)> = kinds
        .iter()
        .filter_map(|(id, kind)| {
            let field = result.nodes.get(id)?.outputs.first()?;
            Some((*id, render(field, *kind, extent, PREVIEW_PX)))
        })
        .collect();
    world.resource_scope(|world, mut images: Mut<PreviewImages>| {
        if images.doc != doc {
            images.map.clear();
            images.doc = doc.clone();
        }
        let mut assets = world.resource_mut::<Assets<Image>>();
        for (id, bytes) in rendered {
            let existing = images.map.get(&id).cloned().filter(|h| assets.contains(h));
            match existing {
                Some(h) => {
                    if let Some(mut img) = assets.get_mut(&h) {
                        img.data = Some(bytes);
                    }
                }
                None => {
                    let handle = assets.add(image(bytes));
                    images.map.insert(id, handle);
                }
            }
        }
    });
}

fn image(bytes: Vec<u8>) -> Image {
    Image::new(
        Extent3d { width: PREVIEW_PX, height: PREVIEW_PX, depth_or_array_layers: 1 },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    )
}

/// Draw `field` as `px` square RGBA, north at the top. See the module docs.
pub fn render(field: &Field, kind: PinKind, extent: Vec2, px: u32) -> Vec<u8> {
    let n = px.max(2) as usize;
    let mut out = vec![255u8; n * n * 4];
    let (lo, hi) = field.min_max();
    let span = (hi - lo).max(1e-3);
    let sample = |u: f32, v: f32| field.bilinear(u.clamp(0.0, 1.0) * (field.w - 1) as f32, v.clamp(0.0, 1.0) * (field.h - 1) as f32);
    let step = 1.0 / (n - 1) as f32;
    // Metres between neighbouring pixels, for slopes in real units.
    let (dx, dz) = (extent.x * step, extent.y * step);
    let light = Vec3::new(-1.0, 1.3, 1.0).normalize();
    for r in 0..n {
        for c in 0..n {
            let (u, v) = (c as f32 * step, 1.0 - r as f32 * step);
            let h = sample(u, v);
            let rgb = match kind {
                PinKind::Mask => {
                    let g = (h.clamp(0.0, 1.0) * 255.0) as u8;
                    [g, g, g]
                }
                PinKind::Height => {
                    let hx = (sample(u + step, v) - sample(u - step, v)) / (2.0 * dx.max(1e-4));
                    let hz = (sample(u, v + step) - sample(u, v - step)) / (2.0 * dz.max(1e-4));
                    // A little exaggeration so gentle ground still reads.
                    let normal = Vec3::new(-hx * 1.5, 1.0, -hz * 1.5).normalize();
                    let lit = 0.3 + 0.7 * normal.dot(light).max(0.0);
                    let t = ((h - lo) / span).clamp(0.0, 1.0);
                    let base = ramp(t);
                    [(base[0] * lit) as u8, (base[1] * lit) as u8, (base[2] * lit) as u8]
                }
            };
            let i = (r * n + c) * 4;
            out[i..i + 3].copy_from_slice(&rgb);
        }
    }
    out
}

/// Low ground slate, high ground near white: elevation readable at a glance
/// without competing with the shading.
fn ramp(t: f32) -> [f32; 3] {
    const STOPS: [(f32, [f32; 3]); 3] = [(0.0, [52.0, 64.0, 78.0]), (0.55, [142.0, 150.0, 146.0]), (1.0, [240.0, 242.0, 246.0])];
    for w in STOPS.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if t <= b {
            let k = ((t - a) / (b - a)).clamp(0.0, 1.0);
            return [ca[0] + (cb[0] - ca[0]) * k, ca[1] + (cb[1] - ca[1]) * k, ca[2] + (cb[2] - ca[2]) * k];
        }
    }
    STOPS[2].1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mask_renders_grey_and_a_height_shaded() {
        let grid = Grid::new(9, 9, Vec2::splat(64.0));
        let mask = Field::from_fn(&grid, |x, _| x as f32 / 8.0);
        let px = render(&mask, PinKind::Mask, grid.extent, 16);
        // Left edge black, right edge white, grey in between.
        assert_eq!(&px[0..3], &[0, 0, 0]);
        assert_eq!(&px[(15 * 4)..(15 * 4 + 3)], &[255, 255, 255]);
        let hill = Field::from_fn(&grid, |x, z| 20.0 - ((x as f32 - 4.0).powi(2) + (z as f32 - 4.0).powi(2)));
        let px = render(&hill, PinKind::Height, grid.extent, 16);
        // The north-west flank faces the light; the south-east one does not.
        let at = |r: usize, c: usize| px[(r * 16 + c) * 4] as i32;
        assert!(at(4, 4) > at(11, 11), "{} vs {}", at(4, 4), at(11, 11));
    }
}
