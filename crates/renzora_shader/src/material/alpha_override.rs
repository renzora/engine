//! Honouring [`MaterialAlphaOverride`] on top of a resolved `MaterialRef`.
//!
//! The resolver caches one compiled material per `.material` path and hands
//! that same handle to every mesh pointing at it, which is exactly right until
//! something needs the material at a *different* transparency than the file
//! declares. Terrain paint layers are that something: a layer mesh carries its
//! coverage in per-vertex alpha and has to fade out at the brush edge, whether
//! the material it wears is an opaque rock or a transmissive ocean.
//!
//! So the override is applied by *deriving* a variant rather than by mutating
//! anything: clone the resolved asset, set the alpha mode on the clone, hand
//! the entity the clone. Every handle inside (textures, the compiled shader,
//! the extension's parameter block) is shared with the master, so a variant
//! costs one asset entry and no recompile, and the master keeps rendering
//! unchanged everywhere else.
//!
//! Variants are keyed by the *master's asset id*, not by its path. Saving a
//! material invalidates the cache and re-resolves every entity onto a fresh
//! master handle; a path key would have matched the stale variant and the edit
//! would have shown up everywhere except the overlay.

use std::collections::HashMap;

use bevy::asset::UntypedAssetId;
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;
use renzora::core::{MaterialAlphaOverride, MaterialRef, MaterialResolved, PbrAlphaMode};

use super::runtime::GraphMaterial;

/// Derived alpha-mode variants, keyed by `(master asset, override key)`.
///
/// Shared across entities so a hundred paint layers wearing one material hold
/// one variant between them.
#[derive(Resource, Default)]
pub struct AlphaVariantCache {
    standard: HashMap<(AssetId<StandardMaterial>, (u8, u32)), Handle<StandardMaterial>>,
    graph: HashMap<(AssetId<GraphMaterial>, (u8, u32)), Handle<GraphMaterial>>,
}

/// What the override system last put on an entity.
///
/// Comparing the entity's *current* material handle against this is what makes
/// the system self-correcting: anything that re-resolves the entity (a changed
/// path, a material save) replaces the handle with a master, the ids stop
/// matching, and the variant is derived again from whatever is there now.
#[derive(Component)]
pub(super) struct AlphaOverrideApplied {
    variant: UntypedAssetId,
    key: (u8, u32),
}

fn bevy_alpha_mode(over: &MaterialAlphaOverride) -> AlphaMode {
    match over.mode {
        PbrAlphaMode::Opaque => AlphaMode::Opaque,
        PbrAlphaMode::Mask => AlphaMode::Mask(over.cutoff),
        PbrAlphaMode::Blend => AlphaMode::Blend,
    }
}

/// Give every entity carrying a [`MaterialAlphaOverride`] a variant of its
/// resolved material at the requested alpha mode.
///
/// Runs after `resolve_material_refs` so the handle it reads is the one the
/// resolver just attached.
pub(super) fn apply_material_alpha_overrides(
    mut commands: Commands,
    mut cache: ResMut<AlphaVariantCache>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut graph_materials: ResMut<Assets<GraphMaterial>>,
    query: Query<
        (
            Entity,
            &MaterialAlphaOverride,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&MeshMaterial3d<GraphMaterial>>,
            Option<&AlphaOverrideApplied>,
        ),
        With<MaterialResolved>,
    >,
) {
    for (entity, over, std_mat, graph_mat, applied) in query.iter() {
        let key = over.key();
        let current: Option<UntypedAssetId> = std_mat
            .map(|m| m.0.id().untyped())
            .or_else(|| graph_mat.map(|m| m.0.id().untyped()));
        let Some(current) = current else {
            // Resolved but material-less: the file failed to compile. Nothing
            // to derive from, and no reason to complain twice — the resolver
            // already logged it.
            continue;
        };
        if applied.is_some_and(|a| a.key == key && a.variant == current) {
            continue;
        }

        let variant: UntypedAssetId = if let Some(handle) = std_mat.map(|m| m.0.clone()) {
            let cache_key = (handle.id(), key);
            let variant = match cache.standard.get(&cache_key) {
                Some(h) => h.clone(),
                None => {
                    let Some(master) = standard_materials.get(&handle) else {
                        continue; // Asset not in yet; try again next frame.
                    };
                    let mut derived = master.clone();
                    derived.alpha_mode = bevy_alpha_mode(over);
                    let h = standard_materials.add(derived);
                    cache.standard.insert(cache_key, h.clone());
                    h
                }
            };
            let id = variant.id().untyped();
            commands.entity(entity).insert(MeshMaterial3d(variant));
            id
        } else if let Some(handle) = graph_mat.map(|m| m.0.clone()) {
            let cache_key = (handle.id(), key);
            let variant = match cache.graph.get(&cache_key) {
                Some(h) => h.clone(),
                None => {
                    let Some(master) = graph_materials.get(&handle) else {
                        continue;
                    };
                    let mut derived = master.clone();
                    derived.base.alpha_mode = bevy_alpha_mode(over);
                    let h = graph_materials.add(derived);
                    cache.graph.insert(cache_key, h.clone());
                    h
                }
            };
            let id = variant.id().untyped();
            commands.entity(entity).insert(MeshMaterial3d(variant));
            id
        } else {
            continue;
        };

        commands
            .entity(entity)
            .insert(AlphaOverrideApplied { variant, key });
    }

    // Drop variants whose master is gone. Cheap: the map only ever holds one
    // entry per (material, mode) an overlay actually asked for.
    cache
        .standard
        .retain(|(id, _), _| standard_materials.contains(*id));
    cache.graph.retain(|(id, _), _| graph_materials.contains(*id));
}

/// Strip the compiled graph material off an entity whose `MaterialRef` was
/// removed.
///
/// The resolver is careful to remove the *other* `MeshMaterial3d` whenever it
/// attaches one, because an entity holding both draws twice — once through
/// each `MaterialPlugin`. Clearing a material had no such care: the panels that
/// remove a `MaterialRef` (the inspector's remove, the drawer's clear) left a
/// `MeshMaterial3d<GraphMaterial>` behind, so a mesh given a plain
/// `StandardMaterial` afterwards kept rendering the procedural one on top of
/// it. A terrain paint layer whose material is cleared back to the default is
/// the case that made it visible.
pub(super) fn clear_material_on_ref_removed(
    mut commands: Commands,
    mut removed: RemovedComponents<MaterialRef>,
    orphaned: Query<(), (With<MeshMaterial3d<GraphMaterial>>, Without<MaterialRef>)>,
) {
    for entity in removed.read() {
        if orphaned.get(entity).is_ok() {
            commands
                .entity(entity)
                .remove::<MeshMaterial3d<GraphMaterial>>();
        }
    }
}
