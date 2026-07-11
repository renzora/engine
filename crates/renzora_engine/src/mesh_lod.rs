//! Runtime mesh LODs — consume the `_lodN.glb` variants the exporter bakes.
//!
//! `renzora_rpak`'s packer generates simplified variants beside every model
//! (`models/chair_lod1.glb`, `_lod2`, …) but until now nothing consumed them
//! at runtime. This module spawns each available variant as a sibling subtree
//! under the model's [`MeshInstanceData`] root and tags every mesh with a
//! [`VisibilityRange`] band, so Bevy's built-in HLOD machinery crossfades
//! detail levels by camera distance — no custom render work.
//!
//! The moving parts, in order:
//! 1. [`probe_mesh_lods`] — once a model's base subtree exists, check the
//!    Vfs/rpak and the project directory for `_lodN.glb` files and start
//!    loading the ones that exist.
//! 2. [`finish_lod_spawn`] — when the variants land, spawn one
//!    `WorldAssetRoot` child per level, resolve the distance bands from the
//!    optional [`MeshLod`] config (defaults otherwise), and tag meshes that
//!    already instantiated.
//! 3. [`tag_new_lod_meshes`] — glTF instantiation is async, so meshes keep
//!    appearing after (2); an `Added<Mesh3d>` sweep tags stragglers.
//! 4. [`reapply_lod_config`] — live-retune when the inspector edits `MeshLod`
//!    (including tearing everything down when `enabled` flips off).
//!
//! None of the markers here are reflected: they never serialize, and
//! `save_scene` already excludes `MeshInstanceData` descendants, so every
//! scene load re-probes from scratch — which also picks up newly-baked LODs.

use bevy::camera::visibility::VisibilityRange;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use renzora::{CurrentProject, MeshInstanceData, MeshLod};

use crate::scene_io::MeshInstanceLoadFailed;
use crate::Vfs;

/// Highest `_lodN` suffix probed. The exporter bakes contiguous levels from 1,
/// so probing stops at the first missing file; this only bounds the loop.
const MAX_LOD_LEVELS: u32 = 4;

/// Marker: this model was probed for `_lodN.glb` variants (found or not), so
/// the filesystem probe never re-runs for it this session.
#[derive(Component)]
pub struct LodProbed;

/// Waiting for probed LOD variants' `Gltf` assets to finish loading.
#[derive(Component)]
pub struct PendingLodSpawn {
    pending: Vec<(u32, Handle<Gltf>)>,
}

/// Root of one detail level's spawned subtree (level 0 = the base model).
#[derive(Component)]
pub struct LodSubtree(pub u32);

/// On the `MeshInstanceData` root once LOD subtrees exist: the resolved
/// visibility band per level, consumed by the mesh taggers.
#[derive(Component)]
pub struct LodApplied {
    /// `(level, band_start, band_end)`; `band_end` is `f32::INFINITY` for the
    /// last level when no cull distance is set.
    bands: Vec<(u32, f32, f32)>,
    crossfade: f32,
}

impl LodApplied {
    /// The resolved `(level, band_start, band_end)` list — read by the
    /// Streaming debug panel.
    pub fn bands(&self) -> &[(u32, f32, f32)] {
        &self.bands
    }
}

/// Probe for `_lodN.glb` variants once the base model subtree exists.
pub fn probe_mesh_lods(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    vfs: Option<Res<Vfs>>,
    project: Option<Res<CurrentProject>>,
    query: Query<
        (Entity, &MeshInstanceData, Option<&MeshLod>),
        (
            With<Children>,
            Without<LodProbed>,
            Without<MeshInstanceLoadFailed>,
        ),
    >,
) {
    for (entity, instance, lod_cfg) in &query {
        commands.entity(entity).try_insert(LodProbed);
        if lod_cfg.is_some_and(|c| !c.enabled) {
            continue;
        }
        let Some(ref model_path) = instance.model_path else {
            continue;
        };
        // Only .glb — the packer's LOD bake only emits .glb variants.
        let Some(stem) = model_path.strip_suffix(".glb") else {
            continue;
        };

        let mut pending: Vec<(u32, Handle<Gltf>)> = Vec::new();
        for level in 1..=MAX_LOD_LEVELS {
            let candidate = format!("{stem}_lod{level}.glb");
            // Packed rpak first (exported game), then loose project file
            // (editor / dev). Levels are contiguous, so stop at the first gap.
            let exists = vfs.as_ref().is_some_and(|v| v.exists(&candidate))
                || project
                    .as_ref()
                    .is_some_and(|p| p.path.join(&candidate).exists());
            if !exists {
                break;
            }
            pending.push((level, asset_server.load::<Gltf>(candidate)));
        }

        if !pending.is_empty() {
            info!(
                "[lod] {} has {} LOD variant(s) — streaming them in",
                model_path,
                pending.len()
            );
            commands.entity(entity).try_insert(PendingLodSpawn {
                pending,
            });
        }
    }
}

/// Once every probed variant has loaded (or failed), spawn the LOD subtrees
/// and resolve the visibility bands.
pub fn finish_lod_spawn(
    mut commands: Commands,
    gltf_assets: Option<Res<Assets<Gltf>>>,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &PendingLodSpawn, Option<&MeshLod>, &Children)>,
    world_roots: Query<(), With<bevy::world_serialization::WorldAssetRoot>>,
    children_q: Query<&Children>,
    meshes_q: Query<(), With<Mesh3d>>,
) {
    let Some(gltf_assets) = gltf_assets else {
        return;
    };
    'outer: for (entity, pending, lod_cfg, children) in &query {
        // Wait until every variant has resolved one way or the other — bands
        // depend on how many levels actually exist.
        let mut loaded: Vec<(u32, Handle<bevy::world_serialization::WorldAsset>)> = Vec::new();
        for (level, handle) in &pending.pending {
            if let Some(gltf) = gltf_assets.get(handle) {
                let scene = gltf
                    .default_scene
                    .clone()
                    .or_else(|| gltf.scenes.first().cloned());
                if let Some(scene) = scene {
                    loaded.push((*level, scene));
                }
            } else if matches!(
                asset_server.get_load_state(handle.id()),
                Some(bevy::asset::LoadState::Failed(_))
            ) {
                warn!(
                    "[lod] LOD variant failed to load, skipping level: {}",
                    asset_server
                        .get_path(handle.id())
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "<unknown>".into())
                );
            } else {
                continue 'outer; // still loading
            }
        }

        commands.entity(entity).remove::<PendingLodSpawn>();
        if loaded.is_empty() {
            continue;
        }
        loaded.sort_by_key(|(level, _)| *level);

        // Tag the base model's subtree root(s) as level 0. Only children that
        // carry `WorldAssetRoot` — a user can parent arbitrary entities under
        // a model, and those must never inherit a LOD band (they'd vanish at
        // the level-0 boundary).
        let mut base_roots: Vec<Entity> = Vec::new();
        for child in children.iter() {
            if world_roots.get(child).is_ok() {
                commands.entity(child).try_insert(LodSubtree(0));
                base_roots.push(child);
            }
        }

        let levels: Vec<u32> = std::iter::once(0)
            .chain(loaded.iter().map(|(level, _)| *level))
            .collect();
        let default_cfg = MeshLod::default();
        let cfg = lod_cfg.unwrap_or(&default_cfg);
        let (bands, crossfade) = compute_bands(&levels, cfg);

        for (level, scene) in loaded {
            commands.spawn((
                Name::new(format!("SceneRoot LOD{level}")),
                bevy::world_serialization::WorldAssetRoot(scene),
                Transform::default(),
                Visibility::default(),
                ChildOf(entity),
                LodSubtree(level),
            ));
        }

        // The base subtree may have fully instantiated already — its meshes
        // won't retrigger `Added<Mesh3d>`, so tag them here. The LOD subtrees
        // just spawned have no meshes yet; the Added sweep catches those.
        if let Some(&(_, start, end)) = bands.iter().find(|b| b.0 == 0) {
            for root in base_roots {
                tag_subtree(
                    &mut commands,
                    root,
                    range_for(start, end, crossfade),
                    &children_q,
                    &meshes_q,
                );
            }
        }

        commands.entity(entity).try_insert(LodApplied {
            bands,
            crossfade,
        });
    }
}

/// Tag meshes that instantiate *after* their LOD subtree was set up. glTF
/// world-asset instantiation is async, so this sweep is what tags most LOD
/// meshes. Non-LOD meshes exit on a short ancestor walk.
pub fn tag_new_lod_meshes(
    mut commands: Commands,
    new_meshes: Query<Entity, Added<Mesh3d>>,
    parents: Query<&ChildOf>,
    subtrees: Query<&LodSubtree>,
    applied: Query<&LodApplied>,
) {
    for mesh in &new_meshes {
        let mut cursor = mesh;
        let mut level: Option<u32> = None;
        loop {
            if level.is_none() {
                if let Ok(subtree) = subtrees.get(cursor) {
                    level = Some(subtree.0);
                }
            }
            if let Some(lv) = level {
                if let Ok(app) = applied.get(cursor) {
                    if let Some(&(_, start, end)) = app.bands.iter().find(|b| b.0 == lv) {
                        commands
                            .entity(mesh)
                            .try_insert(range_for(start, end, app.crossfade));
                    }
                    break;
                }
            }
            match parents.get(cursor) {
                Ok(child_of) => cursor = child_of.parent(),
                Err(_) => break,
            }
        }
    }
}

/// Live-retune when the inspector edits [`MeshLod`]: rewrite the bands on
/// every tagged mesh, tear down when disabled, re-probe when re-enabled.
pub fn reapply_lod_config(
    mut commands: Commands,
    mut changed: Query<
        (
            Entity,
            &MeshLod,
            Option<&mut LodApplied>,
            Option<&Children>,
            Has<LodProbed>,
        ),
        Changed<MeshLod>,
    >,
    subtrees: Query<&LodSubtree>,
    children_q: Query<&Children>,
    meshes_q: Query<(), With<Mesh3d>>,
) {
    for (entity, cfg, applied, children, probed) in &mut changed {
        match (cfg.enabled, applied) {
            // Disabled with LODs up: despawn variant subtrees, strip the
            // ranges off the base meshes, forget the application.
            (false, Some(_)) => {
                commands.entity(entity).remove::<LodApplied>();
                let Some(children) = children else { continue };
                for child in children.iter() {
                    let Ok(subtree) = subtrees.get(child) else {
                        continue;
                    };
                    if subtree.0 == 0 {
                        commands.entity(child).remove::<LodSubtree>();
                        strip_subtree(&mut commands, child, &children_q, &meshes_q);
                    } else {
                        commands.entity(child).despawn();
                    }
                }
            }
            // Enabled with LODs up: recompute bands in place.
            (true, Some(mut applied)) => {
                let levels: Vec<u32> = applied.bands.iter().map(|b| b.0).collect();
                let (bands, crossfade) = compute_bands(&levels, cfg);
                applied.bands = bands.clone();
                applied.crossfade = crossfade;
                let Some(children) = children else { continue };
                for child in children.iter() {
                    let Ok(subtree) = subtrees.get(child) else {
                        continue;
                    };
                    if let Some(&(_, start, end)) = bands.iter().find(|b| b.0 == subtree.0) {
                        tag_subtree(
                            &mut commands,
                            child,
                            range_for(start, end, crossfade),
                            &children_q,
                            &meshes_q,
                        );
                    }
                }
            }
            // Just re-enabled (or never found LODs): drop the probe marker so
            // `probe_mesh_lods` takes another look.
            (true, None) if probed => {
                commands.entity(entity).remove::<LodProbed>();
            }
            _ => {}
        }
    }
}

/// Walk `root`'s subtree inserting `range` on every mesh entity.
fn tag_subtree(
    commands: &mut Commands,
    root: Entity,
    range: VisibilityRange,
    children_q: &Query<&Children>,
    meshes_q: &Query<(), With<Mesh3d>>,
) {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if meshes_q.get(e).is_ok() {
            commands.entity(e).try_insert(range.clone());
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
}

/// Walk `root`'s subtree removing `VisibilityRange` from every mesh entity.
fn strip_subtree(
    commands: &mut Commands,
    root: Entity,
    children_q: &Query<&Children>,
    meshes_q: &Query<(), With<Mesh3d>>,
) {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if meshes_q.get(e).is_ok() {
            commands.entity(e).try_remove::<VisibilityRange>();
        }
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter());
        }
    }
}

/// Resolve `(level, start, end)` bands for the present `levels` from `cfg`.
///
/// `cfg.distances[i]` is the boundary between band `i` and `i+1`. When fewer
/// boundaries are configured than needed, extras extend geometrically — a
/// model with 4 baked LODs but the default 3 distances still gets sane bands
/// instead of two levels fighting over one range.
fn compute_bands(levels: &[u32], cfg: &MeshLod) -> (Vec<(u32, f32, f32)>, f32) {
    let needed = levels.len().saturating_sub(1);
    let mut bounds: Vec<f32> = cfg.distances.iter().copied().take(needed).collect();
    while bounds.len() < needed {
        let last = bounds.last().copied().unwrap_or(40.0);
        bounds.push(last * 2.2);
    }
    let bands = levels
        .iter()
        .enumerate()
        .map(|(i, &level)| {
            let start = if i == 0 { 0.0 } else { bounds[i - 1] };
            let end = if i < needed {
                bounds[i]
            } else if cfg.cull_distance > 0.0 {
                cfg.cull_distance
            } else {
                f32::INFINITY
            };
            (level, start, end)
        })
        .collect();
    (bands, cfg.crossfade.max(0.0))
}

/// Build the `VisibilityRange` for one band. Adjacent bands share their
/// boundary distance, so `end_margin` of level N is bit-identical to
/// `start_margin` of level N+1 — the invariant Bevy's crossfade dither needs.
fn range_for(start: f32, end: f32, fade: f32) -> VisibilityRange {
    let start_margin = if start <= 0.0 {
        0.0..0.0
    } else {
        start..start + fade
    };
    let end_margin = if end.is_finite() {
        end..end + fade
    } else {
        f32::MAX..f32::MAX
    };
    VisibilityRange {
        start_margin,
        end_margin,
        use_aabb: false,
    }
}
