//! Distance-driven texture tier streaming for `.rmip` materials.
//!
//! The `.rmip` loader publishes every texture twice: the full asset and a
//! `#low` labeled subasset holding only the tail of the mip chain (base ≤
//! `renzora_rmip::LOW_RES_CAP`). This module owns the *policy*: while world
//! streaming is in effect, materials whose meshes are all far from the camera
//! swap their texture handles to `#low`; materials with any nearby user swap
//! back to full. Materials hold the only strong handles, so a swap to `#low`
//! drops the last reference to the full-resolution image and Bevy unloads it
//! — GPU memory actually comes back. Approaching re-loads the full asset
//! from the Vfs/disk; until it lands, Bevy keeps rendering the previously
//! prepared material, so the transition is a sharpness pop-in, never a hole.
//!
//! Deliberately stateless: the current tier is read off each handle's asset
//! path (label `low` present or not), so there is no side table to fall out
//! of sync with the material assets.
//!
//! Scope: `StandardMaterial`'s five heavyweight slots. `GraphMaterial`
//! custom-shader textures and terrain splat layers keep full resolution.

use bevy::prelude::*;

/// Tuning for texture tier streaming. Distances are measured from the
/// streaming camera to the nearest mesh using the material.
#[derive(Resource)]
pub struct TextureStreamingSettings {
    /// Master switch. On by default — it only acts while world streaming is
    /// active (shipped game / editor play), never in edit mode.
    pub enabled: bool,
    /// Within this distance a material's textures are full resolution.
    pub full_distance: f32,
    /// Beyond this distance they drop to the `#low` tier. Kept above
    /// `full_distance` (hysteresis) so materials on the boundary don't
    /// re-decode their textures every evaluation tick.
    pub low_distance: f32,
}

impl Default for TextureStreamingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            full_distance: 60.0,
            low_distance: 90.0,
        }
    }
}

/// Evaluate texture tiers. Runs on a coarse timer (see plugin registration) —
/// tier decisions don't need frame-rate reactivity, and each evaluation walks
/// every mesh with a `StandardMaterial`.
pub fn stream_texture_tiers(world: &mut World, mut was_active: Local<bool>) {
    let enabled = world
        .get_resource::<TextureStreamingSettings>()
        .is_none_or(|s| s.enabled);
    let active = enabled && renzora::world_streaming_active(world);
    if !active {
        // Leaving play (or streaming toggled off): put every demoted material
        // back on full resolution so edit mode never shows the low tier, and
        // drop any in-flight swap schedules so they can't fire in edit mode.
        if *was_active {
            if let Some(mut pending) = world.get_resource_mut::<PendingTierSwaps>() {
                pending.swaps.clear();
            }
            restore_all_full(world);
        }
        *was_active = false;
        return;
    }
    *was_active = true;

    let Some(camera_pos) = renzora::streaming_camera_pos(world) else {
        return;
    };
    let (full_distance, low_distance) = {
        let s = world.resource::<TextureStreamingSettings>();
        (s.full_distance, s.low_distance.max(s.full_distance + 5.0))
    };

    // Nearest user per material. A material with no mesh user this tick is
    // left untouched — it may drive UI, previews, or a mesh that's merely
    // hidden, and demoting those wins nothing.
    let mut nearest: bevy::platform::collections::HashMap<AssetId<StandardMaterial>, f32> =
        Default::default();
    {
        let mut q = world.query::<(&GlobalTransform, &MeshMaterial3d<StandardMaterial>)>();
        for (transform, material) in q.iter(world) {
            let dist = camera_pos.distance(transform.translation());
            nearest
                .entry(material.0.id())
                .and_modify(|d| *d = d.min(dist))
                .or_insert(dist);
        }
    }
    if nearest.is_empty() {
        return;
    }

    let asset_server = world.resource::<AssetServer>().clone();

    // Phase 1: decide tiers and (re)schedule swaps. Swaps are NOT applied
    // here — writing a not-yet-loaded image into a material makes its bind
    // group fail to prepare, and every mesh using it skips drawing until the
    // texture lands: a visible flash on each tier crossing. Instead the
    // decision only *kicks the load* and parks the target handles in
    // `PendingTierSwaps`; phase 2 applies a swap in the frame the images are
    // actually resident, so the re-prepare succeeds immediately.
    {
        let mut pending = std::mem::take(
            &mut *world.get_resource_or_insert_with(PendingTierSwaps::default),
        );
        let materials = world.resource::<Assets<StandardMaterial>>();
        for (&id, &dist) in &nearest {
            let want_low = dist > low_distance;
            let want_full = dist < full_distance;
            if !want_low && !want_full {
                continue; // hysteresis band — keep whatever tier it has
            }
            let Some(material) = materials.get(id) else {
                pending.swaps.remove(&id);
                continue;
            };
            let targets: Vec<Handle<Image>> = material_slots(material)
                .into_iter()
                .flatten()
                .filter_map(|h| tier_swap(&asset_server, h, want_low))
                .collect();
            if targets.is_empty() {
                // Already on the wanted tier — a stale opposite-direction
                // schedule (camera turned around mid-load) must not fire.
                pending.swaps.remove(&id);
                continue;
            }
            match pending.swaps.get(&id) {
                // Same direction already scheduled — keep the existing entry
                // (its handles are keeping the loads alive; re-inserting
                // would churn them).
                Some(swap) if swap.want_low == want_low => {}
                _ => {
                    pending.swaps.insert(id, PendingSwap { want_low, targets });
                }
            }
        }
        world.insert_resource(pending);
    }

    // Phase 2: apply every scheduled swap whose target images are all
    // resident. Targets are re-derived from the live material at apply time
    // (it may have been edited since scheduling); the parked handles only
    // exist to hold the loads.
    apply_ready_swaps(world);
}

/// One scheduled tier change for a material: the direction and the strong
/// handles keeping the target images loading/alive until applied.
struct PendingSwap {
    want_low: bool,
    /// Held, not read — dropping these before the swap applies would abort
    /// the in-flight image loads.
    #[allow(dead_code)]
    targets: Vec<Handle<Image>>,
}

/// Tier swaps waiting for their target images to finish loading.
#[derive(Resource, Default)]
pub struct PendingTierSwaps {
    swaps: bevy::platform::collections::HashMap<AssetId<StandardMaterial>, PendingSwap>,
}

/// Apply scheduled swaps whose target images are all in `Assets<Image>`.
fn apply_ready_swaps(world: &mut World) {
    let asset_server = world.resource::<AssetServer>().clone();
    let mut pending = std::mem::take(
        &mut *world.get_resource_or_insert_with(PendingTierSwaps::default),
    );
    if pending.swaps.is_empty() {
        world.insert_resource(pending);
        return;
    }
    world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
        let images = world.resource::<Assets<Image>>();
        pending.swaps.retain(|&id, swap| {
            let Some(material) = materials.get(id) else {
                return false; // material gone — drop the schedule
            };
            let targets: Vec<Handle<Image>> = material_slots(material)
                .into_iter()
                .flatten()
                .filter_map(|h| tier_swap(&asset_server, h, swap.want_low))
                .collect();
            if targets.is_empty() {
                return false; // already on the wanted tier
            }
            if !targets.iter().all(|t| images.contains(t.id())) {
                return true; // still loading — try again next tick
            }
            // All resident: one `get_mut` (one re-prepare), swap every slot.
            let Some(mut material) = materials.get_mut(id) else {
                return false;
            };
            for slot in material_slots_mut(&mut material) {
                if let Some(handle) = slot {
                    if let Some(swapped) = tier_swap(&asset_server, handle, swap.want_low) {
                        *slot = Some(swapped);
                    }
                }
            }
            false
        });
    });
    world.insert_resource(pending);
}

/// The handle a slot should hold for the requested tier, or `None` when the
/// slot is already correct / isn't a tierable `.rmip` texture.
fn tier_swap(
    asset_server: &AssetServer,
    handle: &Handle<Image>,
    want_low: bool,
) -> Option<Handle<Image>> {
    let path = asset_server.get_path(handle.id())?;
    let is_low = path.label() == Some("low");
    if want_low == is_low {
        return None;
    }
    let bare = path.path().to_string_lossy().replace('\\', "/");
    if !bare.ends_with(".rmip") {
        return None;
    }
    if want_low {
        Some(asset_server.load(format!("{bare}#low")))
    } else {
        Some(asset_server.load(bare))
    }
}

/// Promote every demoted `StandardMaterial` slot back to the full asset.
fn restore_all_full(world: &mut World) {
    let asset_server = world.resource::<AssetServer>().clone();
    world.resource_scope(|_, mut materials: Mut<Assets<StandardMaterial>>| {
        let ids: Vec<AssetId<StandardMaterial>> = materials.iter().map(|(id, _)| id).collect();
        for id in ids {
            let Some(material) = materials.get(id) else {
                continue;
            };
            let demoted = material_slots(material)
                .into_iter()
                .flatten()
                .any(|h| tier_swap(&asset_server, h, false).is_some());
            if !demoted {
                continue;
            }
            let Some(mut material) = materials.get_mut(id) else {
                continue;
            };
            for slot in material_slots_mut(&mut material) {
                if let Some(handle) = slot {
                    if let Some(full) = tier_swap(&asset_server, handle, false) {
                        *slot = Some(full);
                    }
                }
            }
        }
    });
}

/// The streamed texture slots — the ones that dominate a material's memory.
fn material_slots(m: &StandardMaterial) -> [Option<&Handle<Image>>; 5] {
    [
        m.base_color_texture.as_ref(),
        m.normal_map_texture.as_ref(),
        m.metallic_roughness_texture.as_ref(),
        m.occlusion_texture.as_ref(),
        m.emissive_texture.as_ref(),
    ]
}

fn material_slots_mut(m: &mut StandardMaterial) -> [&mut Option<Handle<Image>>; 5] {
    [
        &mut m.base_color_texture,
        &mut m.normal_map_texture,
        &mut m.metallic_roughness_texture,
        &mut m.occlusion_texture,
        &mut m.emissive_texture,
    ]
}
