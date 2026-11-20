//! A terrain's own instance of its landscape material.
//!
//! The material resolver compiles a `.material` once and shares the result by
//! path. A terrain cannot wear the shared copy, because its weight maps and
//! bounds are its own, so it makes an instance: a clone of the shared
//! `GraphMaterial` with this terrain's weight atlas and [`LandscapeParams`]
//! bound. Every chunk, level of detail and stream stand-in wears that one
//! handle, and virtual geometry is handed it like any other graph material.
//!
//! The handle never changes for the life of the terrain's material: a rebuild
//! (the `.material` saved, the terrain moved or resized, layers renamed)
//! writes a new asset into it. The ray tracer and virtual geometry capture a
//! mesh's material handle once, so keeping it put is what lets them see the
//! change.
//!
//! The shared material is found the way every mesh finds one: a hidden
//! *probe* entity carries a `MaterialRef` to the path, and the resolver gives
//! it its `MeshMaterial3d`. Saving the material in the editor drops the
//! probe's `MaterialResolved` along with everyone else's, so the terrain
//! follows edits with no hot-reload code of its own.

use bevy::prelude::*;
use renzora::core::{MaterialRef, MaterialResolved};
use renzora_shader::material::landscape::{landscape_layer_infos, LandscapeBlend, LandscapeLayerInfo};
use renzora_shader::material::surface_ext::LandscapeParams;
use renzora_shader::material::GraphMaterial;
use serde::{Deserialize, Serialize};

use super::gpu::{chunk_patch, weight_atlas, TerrainWeightPatches};
use super::weights::{channel_sums, solid, weight_resolution, TerrainChunkWeights, WEIGHT_CHANNELS};
use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use crate::material::TerrainCheckerboardMaterial;
use crate::surface::TerrainSurfaceMaterial;

/// The material a terrain is textured with. Saved with the terrain.
#[derive(Component, Clone, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
#[serde(default)]
pub struct TerrainMaterial {
    /// Project-relative `.material` path. Empty draws the blockout grid.
    #[reflect(default)]
    pub path: String,
    /// The layer each weight channel holds, by name: channel `i` is
    /// `channels[i]`. Kept by the terrain, not the material, so reordering,
    /// adding or removing layers in the graph never moves what was painted. A
    /// layer the material no longer has keeps its channel (and its paint)
    /// until a new layer needs the room. Filled in as the material's layers
    /// become known; an empty list maps material layer `i` to channel `i`.
    #[reflect(default)]
    pub channels: Vec<String>,
}

impl TerrainMaterial {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into(), channels: Vec::new() }
    }
}

/// One layer of a terrain's landscape material, as the Paint tab shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct LandscapeLayer {
    pub name: String,
    pub blend: LandscapeBlend,
    /// The weight channel this terrain paints it in.
    pub channel: usize,
    /// 0..1: how much of the terrain it covers by painted weight (rules not
    /// included).
    pub coverage: f32,
}

/// How far along a terrain's material is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum LandscapeStatus {
    /// No `TerrainMaterial`, or an empty path: the blockout grid.
    #[default]
    None,
    /// Waiting for the resolver.
    Loading,
    /// Drawn with the material.
    Ready,
    /// The material resolved to a plain `StandardMaterial` (it has no
    /// Landscape nodes): drawn, but there is nothing to paint.
    Plain,
    /// The material could not be read or compiled.
    Failed(String),
}

/// What a terrain's landscape material offers, kept current for the editor:
/// the layers to paint and their coverage. Not saved; rebuilt from the
/// [`TerrainMaterial`].
#[derive(Component, Clone, Debug, Default)]
pub struct TerrainLandscape {
    pub path: String,
    pub status: LandscapeStatus,
    /// In material order (the order the material's Layer Blends name them).
    pub layers: Vec<LandscapeLayer>,
}

impl TerrainLandscape {
    /// The weight channel of the layer called `name`.
    pub fn channel_of(&self, name: &str) -> Option<usize> {
        self.layers.iter().find(|l| l.name == name).map(|l| l.channel)
    }

    /// The channel unpainted ground and erased weight fall back to: the first
    /// layer's.
    pub fn base_channel(&self) -> usize {
        self.layers.first().map_or(0, |l| l.channel)
    }
}

/// The hidden entity that asks the resolver for a terrain's material.
#[derive(Component, Clone, Copy, Debug)]
pub struct TerrainMaterialProbe(pub Entity);

/// The weight atlas of one terrain.
pub(crate) struct WeightAtlas {
    pub(crate) image: Handle<Image>,
    /// `(chunks_x, chunks_z, resolution)` it was built for.
    pub(crate) dims: (u32, u32, u32),
}

/// Runtime state of a terrain's material. Not saved.
#[derive(Component)]
pub struct TerrainMaterialState {
    pub(crate) path: String,
    pub(crate) probe: Entity,
    /// This terrain's instance; set once the shared material first resolves.
    pub(crate) instance: Option<Handle<GraphMaterial>>,
    /// The shared material the instance was last cloned from.
    pub(crate) source: Option<AssetId<GraphMaterial>>,
    /// The material, when it resolved to a plain `StandardMaterial`.
    pub(crate) standard: Option<Handle<StandardMaterial>>,
    pub(crate) layers: Vec<LandscapeLayerInfo>,
    pub(crate) params: LandscapeParams,
    pub(crate) atlas: Option<WeightAtlas>,
    /// Something the instance is built from moved.
    pub(crate) rebuild: bool,
}

impl TerrainMaterialState {
    /// This terrain's material instance, once it exists. What a chunk wears.
    pub fn instance(&self) -> Option<&Handle<GraphMaterial>> {
        self.instance.as_ref()
    }
}

// ── Channels ────────────────────────────────────────────────────────────────

/// Map the material's layers onto weight channels by name, growing
/// `channels` for layers it has not seen. Returns `mapping[i]`, the channel of
/// material layer `i`, and whether `channels` changed.
///
/// A new layer takes a free channel (past the end, or one whose layer is
/// gone and holds nothing named). With all eight taken it takes the channel of
/// a layer the material no longer has, inheriting that layer's paint. With
/// none of those either, it shares channel 0.
pub fn assign_channels(channels: &mut Vec<String>, layers: &[String]) -> ([u32; WEIGHT_CHANNELS], bool) {
    let mut mapping = LandscapeParams::IDENTITY_CHANNELS;
    let mut changed = false;
    if channels.is_empty() {
        channels.extend(layers.iter().take(WEIGHT_CHANNELS).cloned());
        changed = !layers.is_empty();
    }
    for (i, name) in layers.iter().take(WEIGHT_CHANNELS).enumerate() {
        if let Some(c) = channels.iter().position(|n| n == name) {
            mapping[i] = c as u32;
            continue;
        }
        changed = true;
        let c = if let Some(c) = channels.iter().position(String::is_empty) {
            c
        } else if channels.len() < WEIGHT_CHANNELS {
            channels.push(String::new());
            channels.len() - 1
        } else {
            channels.iter().position(|n| !layers.contains(n)).unwrap_or(0)
        };
        if c < channels.len() {
            channels[c] = name.clone();
        }
        mapping[i] = c as u32;
    }
    (mapping, changed)
}

// ── Reading the material ─────────────────────────────────────────────────────

/// The file path `path` (project-relative) is read from.
fn fs_path(path: &str, project: Option<&renzora::CurrentProject>) -> String {
    if std::path::Path::new(path).is_absolute() {
        return path.to_string();
    }
    match project {
        Some(p) => {
            let raw = p.resolve_path(path).to_string_lossy().replace('\\', "/");
            raw.strip_prefix("./").unwrap_or(&raw).to_string()
        }
        None => path.to_string(),
    }
}

/// The landscape layers the `.material` at `path` declares. A derived
/// material (one with a `master`) declares its master's.
pub fn read_material_layers(
    path: &str,
    project: Option<&renzora::CurrentProject>,
    reader: &renzora::VirtualFileReader,
) -> Result<Vec<LandscapeLayerInfo>, String> {
    let text = reader
        .read_string(&fs_path(path, project))
        .ok_or_else(|| format!("{path} could not be read"))?;
    if let Ok(graph) = serde_json::from_str::<renzora_shader::material::graph::MaterialGraph>(&text) {
        return Ok(landscape_layer_infos(&graph));
    }
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    match value.get("master").and_then(|m| m.as_str()) {
        Some(master) if !master.is_empty() && master != path => read_material_layers(master, project, reader),
        _ => Err(format!("{path} is not a material graph")),
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Start, restart or stop each terrain's material as its [`TerrainMaterial`]
/// comes, changes path, or goes.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn follow_terrain_materials(
    mut commands: Commands,
    mut terrains: Query<
        (Entity, &mut TerrainMaterial, Option<&mut TerrainMaterialState>, Option<&TerrainLandscape>),
        (With<TerrainData>, Or<(Changed<TerrainMaterial>, Without<TerrainMaterialState>)>),
    >,
    mut removed: RemovedComponents<TerrainMaterial>,
    // Disjoint from `terrains` (which writes the state) by the material: a
    // terrain whose material was removed is the only one asked about here.
    states: Query<&TerrainMaterialState, Without<TerrainMaterial>>,
    probes: Query<(Entity, &TerrainMaterialProbe)>,
    roots: Query<(), With<TerrainData>>,
    project: Option<Res<renzora::CurrentProject>>,
    reader: Option<Res<renzora::VirtualFileReader>>,
) {
    let default_reader = renzora::VirtualFileReader::default();
    let reader = reader.as_deref().unwrap_or(&default_reader);
    for root in removed.read() {
        if let Ok(state) = states.get(root) {
            commands.entity(state.probe).try_despawn();
        }
        if let Ok(mut e) = commands.get_entity(root) {
            e.try_remove::<(TerrainMaterialState, TerrainLandscape)>();
        }
    }
    // A probe whose terrain is gone (despawned with the scene, or before it).
    for (probe, of) in &probes {
        if !roots.contains(of.0) {
            commands.entity(probe).try_despawn();
        }
    }

    for (root, mut material, state, landscape) in &mut terrains {
        let path = material.path.trim().to_string();
        if state.as_ref().is_some_and(|s| s.path == path) {
            continue;
        }
        if let Some(state) = &state {
            commands.entity(state.probe).try_despawn();
        }
        if path.is_empty() {
            // Matched every frame while there is no state, so only write
            // what is not already so.
            if state.is_some() {
                commands.entity(root).try_remove::<TerrainMaterialState>();
            }
            if landscape.is_none_or(|l| l.status != LandscapeStatus::None) {
                commands.entity(root).try_insert(TerrainLandscape::default());
            }
            continue;
        }
        let (layers, status) = match read_material_layers(&path, project.as_deref(), reader) {
            Ok(layers) => (layers, LandscapeStatus::Loading),
            Err(e) => (Vec::new(), LandscapeStatus::Failed(e)),
        };
        let names: Vec<String> = layers.iter().map(|l| l.name.clone()).collect();
        let (mapping, changed) = assign_channels(&mut material.bypass_change_detection().channels, &names);
        if changed {
            material.set_changed();
        }
        let probe = commands
            .spawn((MaterialRef(path.clone()), TerrainMaterialProbe(root), renzora::HideInHierarchy))
            .id();
        commands.entity(root).try_insert((
            TerrainMaterialState {
                path: path.clone(),
                probe,
                instance: None,
                source: None,
                standard: None,
                layers: layers.clone(),
                params: LandscapeParams::for_terrain(Vec2::ZERO, Vec2::ONE, mapping),
                atlas: None,
                rebuild: true,
            },
            TerrainLandscape { path, status, layers: landscape_layers(&layers, &mapping) },
        ));
    }
}

/// Try again, once a second, a material that could not be read: the editor
/// may be writing it (Alpine Landscape is copied into a project on first use),
/// or the user may be about to fix it. Success starts the terrain's material
/// over, probe and all, since the resolver will have given up on the old one.
pub(crate) fn retry_failed_materials(
    mut commands: Commands,
    time: Res<Time>,
    mut wait: Local<f32>,
    terrains: Query<(Entity, &TerrainMaterialState, &TerrainLandscape)>,
    project: Option<Res<renzora::CurrentProject>>,
    reader: Option<Res<renzora::VirtualFileReader>>,
) {
    *wait += time.delta_secs();
    if *wait < 1.0 {
        return;
    }
    *wait = 0.0;
    let default_reader = renzora::VirtualFileReader::default();
    let reader = reader.as_deref().unwrap_or(&default_reader);
    for (root, state, landscape) in &terrains {
        if !matches!(landscape.status, LandscapeStatus::Failed(_)) {
            continue;
        }
        if read_material_layers(&state.path, project.as_deref(), reader).is_ok() {
            commands.entity(state.probe).try_despawn();
            commands.entity(root).try_remove::<TerrainMaterialState>();
        }
    }
}

fn landscape_layers(layers: &[LandscapeLayerInfo], mapping: &[u32; WEIGHT_CHANNELS]) -> Vec<LandscapeLayer> {
    layers
        .iter()
        .take(WEIGHT_CHANNELS)
        .enumerate()
        .map(|(i, l)| LandscapeLayer { name: l.name.clone(), blend: l.blend, channel: mapping[i] as usize, coverage: 0.0 })
        .collect()
}

/// Every chunk of a terrain with a material carries weights to paint into.
#[allow(clippy::type_complexity)]
pub(crate) fn ensure_chunk_weights(
    mut commands: Commands,
    chunks: Query<(Entity, &TerrainChunkOf), (With<TerrainChunkData>, Without<TerrainChunkWeights>)>,
    roots: Query<(), With<TerrainMaterial>>,
) {
    for (chunk, of) in &chunks {
        if roots.contains(of.0) {
            commands.entity(chunk).try_insert(TerrainChunkWeights::default());
        }
    }
}

/// Build or rebuild each terrain's weight atlas, and send this frame's painted
/// texels to it.
pub(crate) fn sync_weight_atlases(
    mut terrains: Query<(Entity, &TerrainData, &mut TerrainMaterialState)>,
    mut chunks: Query<(&TerrainChunkData, &TerrainChunkOf, &mut TerrainChunkWeights)>,
    mut images: ResMut<Assets<Image>>,
    mut patches: ResMut<TerrainWeightPatches>,
) {
    for (root, terrain, mut state) in &mut terrains {
        let res = weight_resolution(terrain);
        let dims = (terrain.chunks_x, terrain.chunks_z, res);
        if state.atlas.as_ref().is_some_and(|a| a.dims == dims) {
            continue;
        }
        // New, or the grid changed: one full build from every chunk.
        let mut grid: std::collections::HashMap<(u32, u32), Vec<super::weights::Texel>> = Default::default();
        for (chunk, of, mut weights) in &mut chunks {
            if of.0 != root {
                continue;
            }
            let w = weights.bypass_change_detection();
            if w.ensure_decoded(res) {
                w.sums = channel_sums(res, &w.texels);
            }
            w.gpu_dirty = None;
            if !weights.texels.is_empty() {
                grid.insert((chunk.chunk_x, chunk.chunk_z), weights.texels.clone());
            }
        }
        let image = weight_atlas(dims.0 * res, dims.1 * res, |x, z| {
            grid.get(&(x / res, z / res))
                .and_then(|t| t.get(((z % res) * res + x % res) as usize).copied())
                .unwrap_or_else(|| solid(0))
        });
        state.atlas = Some(super::instance::WeightAtlas { image: images.add(image), dims });
        state.rebuild = true;
    }

    for (chunk, of, mut weights) in &mut chunks {
        let Some(rect) = weights.gpu_dirty else { continue };
        let Ok((_, terrain, state)) = terrains.get(of.0) else { continue };
        let Some(atlas) = &state.atlas else { continue };
        let res = weight_resolution(terrain);
        if weights.resolution != res && !weights.texels.is_empty() {
            weights.ensure_decoded(res);
        }
        patches.0.push(chunk_patch(atlas.image.id(), res, chunk.chunk_x, chunk.chunk_z, &weights, rect));
        weights.bypass_change_detection().gpu_dirty = None;
    }
}

/// Bring each changed chunk's saved form and totals up to date, and the
/// terrain's coverage with them. Skipped while a stroke is in progress: a drag
/// changes the same chunks every frame, and saving them once at the end of it
/// is enough.
#[allow(clippy::type_complexity)]
pub(crate) fn encode_weights(
    stroke: Res<super::LandscapeStroke>,
    mut chunks: Query<(&TerrainChunkOf, &mut TerrainChunkWeights)>,
    mut landscapes: Query<(Entity, &TerrainData, &mut TerrainLandscape)>,
    mut dirty_roots: Local<Vec<Entity>>,
    // The layer list each terrain's coverage was last worked out for, so a
    // new material (or a renamed layer) gets its numbers without a stroke.
    // (Its chunk count is in the key too: chunks gain their weights a frame
    // after the material arrives.)
    mut counted_for: Local<std::collections::HashMap<Entity, (Vec<(String, usize)>, usize)>>,
) {
    if stroke.active {
        return;
    }
    for (of, mut weights) in &mut chunks {
        if weights.stale {
            weights.encode();
            if !dirty_roots.contains(&of.0) {
                dirty_roots.push(of.0);
            }
        }
    }
    for (root, terrain, mut landscape) in &mut landscapes {
        let owned = chunks.iter().filter(|(of, _)| of.0 == root).count();
        let key = (landscape.layers.iter().map(|l| (l.name.clone(), l.channel)).collect::<Vec<_>>(), owned);
        if !dirty_roots.contains(&root) && counted_for.get(&root) == Some(&key) {
            continue;
        }
        counted_for.insert(root, key);
        let res = weight_resolution(terrain);
        let mut totals = [0u64; WEIGHT_CHANNELS];
        let mut chunk_count = 0u64;
        for (of, weights) in &chunks {
            if of.0 != root {
                continue;
            }
            chunk_count += 1;
            let sums = if weights.texels.is_empty() && weights.encoded.is_empty() {
                channel_sums(res, &[])
            } else {
                weights.sums
            };
            for c in 0..WEIGHT_CHANNELS {
                totals[c] += sums[c];
            }
        }
        let whole = (chunk_count * 255 * (res as u64).pow(2)).max(1) as f64;
        let coverage: Vec<f32> = landscape.layers.iter().map(|l| (totals[l.channel.min(7)] as f64 / whole) as f32).collect();
        if landscape.layers.iter().zip(&coverage).any(|(l, c)| (l.coverage - c).abs() > 1e-4) {
            for (layer, c) in landscape.layers.iter_mut().zip(coverage) {
                layer.coverage = c;
            }
        }
    }
    dirty_roots.clear();
}

/// Build or refresh each terrain's instance from the shared material its
/// probe resolved to.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn build_instances(
    mut terrains: Query<(&TerrainData, &GlobalTransform, &mut TerrainMaterial, &mut TerrainMaterialState, &mut TerrainLandscape)>,
    probes: Query<(
        Option<&MeshMaterial3d<GraphMaterial>>,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Has<MaterialResolved>,
    )>,
    mut graph_materials: Option<ResMut<Assets<GraphMaterial>>>,
    mut events: MessageReader<AssetEvent<GraphMaterial>>,
    project: Option<Res<renzora::CurrentProject>>,
    reader: Option<Res<renzora::VirtualFileReader>>,
) {
    let modified: Vec<AssetId<GraphMaterial>> = events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    let Some(graph_materials) = graph_materials.as_deref_mut() else { return };
    let default_reader = renzora::VirtualFileReader::default();
    let reader = reader.as_deref().unwrap_or(&default_reader);

    for (terrain, transform, mut material, mut state, mut landscape) in &mut terrains {
        let Ok((graph, standard, resolved)) = probes.get(state.probe) else { continue };

        if let Some(graph) = graph {
            let source = graph.id();
            if state.source != Some(source) {
                // Re-resolved (saved in the editor, or first time): the file
                // may name other layers now.
                if state.source.is_some() {
                    if let Ok(layers) = read_material_layers(&state.path, project.as_deref(), reader) {
                        state.layers = layers;
                    }
                }
                state.source = Some(source);
                state.rebuild = true;
            }
            if modified.contains(&source) {
                state.rebuild = true;
            }
        }

        // Where the weight map sits (the terrain's corner, by its size), and
        // which channel each layer is in. New layer names get channels here,
        // saved with the terrain.
        let half = Vec2::new(terrain.total_width(), terrain.total_depth()) / 2.0;
        let corner = transform.translation().xz() - half;
        let names: Vec<String> = state.layers.iter().map(|l| l.name.clone()).collect();
        let (mapping, changed) = assign_channels(&mut material.bypass_change_detection().channels, &names);
        if changed {
            material.set_changed();
        }
        let params = LandscapeParams::for_terrain(corner, half * 2.0, mapping);
        if params != state.params {
            state.params = params;
            state.rebuild = true;
        }

        match (graph, standard) {
            (Some(graph), _) => {
                let source = graph.id();
                if !state.rebuild {
                    continue;
                }
                let Some(atlas) = state.atlas.as_ref().map(|a| a.image.clone()) else { continue };
                let Some(shared) = graph_materials.get(source) else { continue };
                let mut instance = shared.clone();
                instance.extension.landscape = state.params;
                instance.extension.landscape_weights = Some(atlas);
                ground_base(&mut instance.base);
                match state.instance.clone() {
                    Some(handle) => {
                        if let Some(mut existing) = graph_materials.get_mut(&handle) {
                            *existing = instance;
                        }
                    }
                    None => state.instance = Some(graph_materials.add(instance)),
                }
                state.standard = None;
                state.rebuild = false;
                let mut fresh = landscape_layers(&state.layers, &mapping);
                // Coverage is `encode_weights`' to work out; carry over what
                // it already knows for a channel rather than zeroing it.
                for layer in &mut fresh {
                    if let Some(old) = landscape.layers.iter().find(|l| l.channel == layer.channel) {
                        layer.coverage = old.coverage;
                    }
                }
                if landscape.status != LandscapeStatus::Ready || landscape.layers.iter().map(|l| (&l.name, l.channel)).ne(fresh.iter().map(|l| (&l.name, l.channel))) {
                    landscape.status = LandscapeStatus::Ready;
                    landscape.layers = fresh;
                }
            }
            (None, Some(standard)) => {
                if state.standard.as_ref() != Some(&standard.0) {
                    state.standard = Some(standard.0.clone());
                    landscape.status = LandscapeStatus::Plain;
                    landscape.layers.clear();
                }
            }
            (None, None) if resolved && !matches!(landscape.status, LandscapeStatus::Failed(_)) => {
                landscape.status =
                    LandscapeStatus::Failed(format!("{} did not compile; the Console says why", state.path));
            }
            _ => {}
        }
    }
}

/// What every pass that cannot run the graph shades the ground with (shadows,
/// the prepass, a ray tracer's hits): the base of the instance. A graph whose
/// output is a blend gives the base approximation nothing to go on, so it
/// would stay white; a mid grass green is what a landscape mostly is.
fn ground_base(base: &mut StandardMaterial) {
    if base.base_color_texture.is_none() && base.base_color == Color::WHITE {
        base.base_color = Color::linear_rgb(0.11, 0.15, 0.035);
    }
    base.perceptual_roughness = base.perceptual_roughness.max(0.8);
    base.metallic = 0.0;
    // Ground reflects less at normal incidence than the default 4%: the
    // sheen that adds at grazing angles is much of what greys out distant
    // slopes.
    base.reflectance = 0.4;
}

/// Put each terrain's material on its chunks, in place of the blockout grid
/// (and of the old surface material, on a terrain that had both). Chunks with
/// their own `MaterialRef` keep it. A terrain whose material is gone goes
/// back to the grid.
#[allow(clippy::type_complexity)]
pub(crate) fn dress_landscape_chunks(
    mut commands: Commands,
    states: Query<Option<&TerrainMaterialState>, With<TerrainData>>,
    chunks: Query<
        (
            Entity,
            &TerrainChunkOf,
            Option<&MeshMaterial3d<GraphMaterial>>,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Has<MeshMaterial3d<TerrainCheckerboardMaterial>>,
            Has<MeshMaterial3d<TerrainSurfaceMaterial>>,
        ),
        (With<TerrainChunkData>, Without<MaterialRef>),
    >,
    mut checkers: ResMut<Assets<TerrainCheckerboardMaterial>>,
    mut checker: Local<Option<Handle<TerrainCheckerboardMaterial>>>,
) {
    for (chunk, of, graph, standard, has_checker, has_surface) in &chunks {
        let Ok(state) = states.get(of.0) else { continue };
        let want_graph = state.and_then(|s| s.instance.clone()).filter(|_| state.is_some_and(|s| s.standard.is_none()));
        let want_standard = state.and_then(|s| s.standard.clone());
        if want_graph.is_none() && want_standard.is_none() {
            // Not (yet) a landscape. Only take back what this system gave: a
            // chunk the old surface dresses, or one still on the grid, is left.
            if graph.is_some() || standard.is_some() {
                let grid = checker.get_or_insert_with(|| checkers.add(TerrainCheckerboardMaterial::default())).clone();
                commands
                    .entity(chunk)
                    .try_remove::<(MeshMaterial3d<GraphMaterial>, MeshMaterial3d<StandardMaterial>)>()
                    .try_insert(MeshMaterial3d(grid));
            }
            continue;
        }
        let graph_ok = graph.map(|m| m.id()) == want_graph.as_ref().map(|h| h.id());
        let standard_ok = standard.map(|m| m.id()) == want_standard.as_ref().map(|h| h.id());
        if graph_ok && standard_ok && !has_checker && !has_surface {
            continue;
        }
        let mut e = commands.entity(chunk);
        e.try_remove::<(
            MeshMaterial3d<TerrainCheckerboardMaterial>,
            MeshMaterial3d<TerrainSurfaceMaterial>,
            MeshMaterial3d<GraphMaterial>,
            MeshMaterial3d<StandardMaterial>,
        )>();
        if let Some(h) = want_graph {
            e.try_insert(MeshMaterial3d(h));
        } else if let Some(h) = want_standard {
            e.try_insert(MeshMaterial3d(h));
        }
    }
}

/// A chunk's levels of detail and its stream stand-in are copies of it for
/// the rasterizer. They wear the same graph material, which would give each a
/// ray-traced stand-in of its own and stack five copies of the ground in the
/// traced scene, so they are marked to be left out of it.
pub(crate) fn keep_twins_untraced(
    mut commands: Commands,
    chunks: Query<(Option<&crate::lod::ChunkLods>, Option<&crate::streaming::StreamProxy>), With<TerrainChunkData>>,
    twins: Query<(), (With<MeshMaterial3d<GraphMaterial>>, Without<renzora::TracedGeometry>)>,
) {
    for (lods, proxy) in &chunks {
        let levels = lods.map(|l| l.levels.iter().map(|(e, _)| *e).collect::<Vec<_>>()).unwrap_or_default();
        for twin in levels.into_iter().chain(proxy.map(|p| p.entity)) {
            if twins.contains(twin) {
                commands.entity(twin).try_insert(renzora::TracedGeometry(None));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_first_material_takes_channels_in_order() {
        let mut channels = Vec::new();
        let (mapping, changed) = assign_channels(&mut channels, &names(&["Grass", "Rock"]));
        assert!(changed);
        assert_eq!(channels, names(&["Grass", "Rock"]));
        assert_eq!(&mapping[..2], &[0, 1]);
    }

    #[test]
    fn reordering_layers_keeps_their_channels() {
        let mut channels = names(&["Grass", "Gravel", "Rock", "Snow"]);
        let (mapping, changed) = assign_channels(&mut channels, &names(&["Snow", "Rock", "Grass"]));
        assert!(!changed);
        assert_eq!(&mapping[..3], &[3, 2, 0]);
    }

    #[test]
    fn a_new_layer_takes_a_free_channel_then_an_orphaned_one() {
        let mut channels = names(&["Grass", "Gravel"]);
        let (mapping, _) = assign_channels(&mut channels, &names(&["Grass", "Sand"]));
        assert_eq!(mapping[1], 2, "past the end, leaving Gravel's paint alone");
        let mut full = names(&["A", "B", "C", "D", "E", "F", "G", "H"]);
        let (mapping, _) = assign_channels(&mut full, &names(&["A", "Z"]));
        assert_eq!(mapping[1], 1, "the first channel whose layer is gone");
        assert_eq!(full[1], "Z");
    }

    /// The whole path headless: a terrain names Alpine Landscape, a stand-in
    /// for the resolver hands its probe a shared graph material, and the
    /// chunk ends up wearing the terrain's own instance, with the weight atlas
    /// and the terrain's bounds bound and the layers listed.
    #[test]
    fn a_terrain_wears_its_own_instance_of_its_material() {
        let dir = std::env::temp_dir().join(format!("renzora_landscape_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("alpine.material");
        let graph = renzora_shader::material::landscape_preset::alpine_landscape_graph();
        std::fs::write(&file, serde_json::to_string(&graph).unwrap()).unwrap();
        let path = file.to_string_lossy().replace('\\', "/");

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<GraphMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<TerrainCheckerboardMaterial>>()
            .add_message::<AssetEvent<GraphMaterial>>()
            .init_resource::<super::super::LandscapeStroke>()
            .init_resource::<TerrainWeightPatches>()
            .add_systems(
                Update,
                (
                    follow_terrain_materials,
                    ensure_chunk_weights,
                    sync_weight_atlases,
                    build_instances,
                    dress_landscape_chunks,
                    encode_weights,
                )
                    .chain(),
            );
        let terrain = TerrainData { chunks_x: 1, chunks_z: 1, chunk_size: 16.0, chunk_resolution: 17, ..default() };
        let root = app
            .world_mut()
            .spawn((terrain.clone(), TerrainMaterial::new(path.clone()), GlobalTransform::from_xyz(100.0, 0.0, 0.0)))
            .id();
        let chunk = app
            .world_mut()
            .spawn((TerrainChunkData::new(0, 0, terrain.chunk_resolution, 0.2), TerrainChunkOf(root)))
            .id();
        app.update();

        // The resolver's part: the probe gets the shared material.
        let shared = app.world_mut().resource_mut::<Assets<GraphMaterial>>().add(GraphMaterial::default());
        let mut probes = app.world_mut().query::<(Entity, &TerrainMaterialProbe, &MaterialRef)>();
        let (probe, of, r) = probes.single(app.world()).expect("one probe");
        assert_eq!((of.0, r.0.as_str()), (root, path.as_str()));
        app.world_mut().entity_mut(probe).insert((MeshMaterial3d(shared.clone()), MaterialResolved { source_path: path.clone() }));
        app.update();
        app.update();

        let worn = app.world().get::<MeshMaterial3d<GraphMaterial>>(chunk).expect("the chunk is dressed").0.clone();
        assert_ne!(worn.id(), shared.id(), "its own instance, not the shared material");
        let materials = app.world().resource::<Assets<GraphMaterial>>();
        let instance = materials.get(&worn).unwrap();
        assert!(instance.extension.landscape_weights.is_some());
        assert_eq!(instance.extension.landscape.info.x, 1.0);
        // The weight map's corner is the terrain's: 100 - 8 in x, -8 in z.
        assert_eq!(instance.extension.landscape.bounds, Vec4::new(92.0, -8.0, 16.0, 16.0));
        let landscape = app.world().get::<TerrainLandscape>(root).unwrap();
        assert_eq!(landscape.status, LandscapeStatus::Ready);
        let names: Vec<&str> = landscape.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Grass", "Gravel", "Rock", "Snow"]);
        assert!((landscape.layers[0].coverage - 1.0).abs() < 1e-4, "unpainted ground is all grass");
        assert!(app.world().get::<TerrainChunkWeights>(chunk).is_some());
        assert_eq!(app.world().get::<TerrainMaterial>(root).unwrap().channels.len(), 4);

        // Taking the material away puts the grid back.
        app.world_mut().entity_mut(root).remove::<TerrainMaterial>();
        app.update();
        app.update();
        assert!(app.world().get::<MeshMaterial3d<GraphMaterial>>(chunk).is_none());
        assert!(app.world().get::<MeshMaterial3d<TerrainCheckerboardMaterial>>(chunk).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn coverage_of_an_unpainted_terrain_is_all_the_first_layer() {
        let sums = channel_sums(8, &[]);
        assert_eq!(sums[0], 255 * 64);
    }
}
