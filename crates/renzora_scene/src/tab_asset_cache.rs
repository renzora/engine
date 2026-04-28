//! Tab asset cache — keeps GLBs (and their transitive textures, meshes,
//! materials) resident in memory across tab switches so respawn is
//! near-instant.
//!
//! ## The problem this solves
//!
//! Switching tabs today:
//!   1. Serialize current scene → RON snapshot for the leaving tab.
//!   2. Despawn every scene entity. The `Handle<Gltf>` each entity held
//!      drops, Bevy's refcount falls to zero, the GLB and every texture
//!      it references is evicted from `Assets<Gltf>`, `Assets<Image>`,
//!      `Assets<Mesh>`, `Assets<StandardMaterial>`.
//!   3. Deserialize target tab's snapshot → new entities flow in,
//!      `rehydrate_mesh_instances` calls `asset_server.load(model_path)`,
//!      which re-reads from disk, re-decodes textures, re-uploads to GPU.
//!
//! Step 2 makes step 3 expensive. A cargo-sized GLB taking 3 seconds to
//! decode the first time will take 3 seconds to decode the second time
//! too — even though we only despawned the entities and could have
//! reused the asset bytes.
//!
//! ## The fix
//!
//! Hold strong `Handle<Gltf>` clones in a resource keyed by tab id.
//! When entities despawn, the tab cache still holds a strong handle, so
//! Bevy's refcount stays > 0 and the asset stays resident. The next
//! `asset_server.load(path)` returns the same handle pointing at the
//! already-decoded asset — `finish_mesh_instance_rehydrate` runs the same
//! frame, `SceneSpawner::write_to_world` runs the same frame, and the
//! tab switch is essentially instant.
//!
//! ## Lifecycle
//!
//! - **Add** — every time a `MeshInstanceData` enters the world (drag
//!   drop, project load, tab respawn), the system below records its
//!   `model_path` against the active tab.
//! - **Hit** — tab switches respawn entities; their `asset_server.load`
//!   calls return the cached handle. No I/O.
//! - **Evict** — when the user closes a tab, the editor's tab-close
//!   handler calls [`TabAssetCache::drop_tab`]. The handles for that tab
//!   drop. If another tab still references the same path, its handle in
//!   *its* tab's set keeps the asset alive (Bevy refcount). If not, the
//!   refcount falls to zero and the asset is freed.
//!
//! Trusts Bevy's per-handle refcounting for cross-tab dedup — we don't
//! reference-count paths ourselves.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use renzora::core::{MeshInstanceData, TabClosed};
use renzora_ui::DocumentTabState;

/// Strong-handle cache keyed by tab id. While a tab id maps to a
/// non-empty handle set, the GLBs (and everything they transitively
/// reference) stay resident in `Assets<...>`.
#[derive(Resource, Default)]
pub struct TabAssetCache {
    /// `tab_id → { asset_path → Handle<Gltf> }`. The inner map dedups
    /// within a tab so dragging the same model twice into one tab
    /// doesn't double-count. Across tabs, AssetServer hands out the same
    /// handle for the same path, so refcount-based reuse is automatic.
    by_tab: HashMap<u64, HashMap<String, Handle<Gltf>>>,
}

impl TabAssetCache {
    /// Drop every handle a tab claimed. Called by the editor's tab-close
    /// path. Other tabs holding the same path keep the asset alive via
    /// Bevy's refcount — we don't need to track sharing ourselves.
    pub fn drop_tab(&mut self, tab_id: u64) {
        if let Some(removed) = self.by_tab.remove(&tab_id) {
            debug!(
                "[tab_asset_cache] dropped tab {} ({} handles)",
                tab_id,
                removed.len()
            );
        }
    }

    /// Record (or refresh) a handle against a tab. Called by the system
    /// below as `MeshInstanceData` entities arrive. Idempotent: storing
    /// the same path twice replaces the handle with itself.
    fn record(&mut self, tab_id: u64, path: String, handle: Handle<Gltf>) {
        self.by_tab
            .entry(tab_id)
            .or_default()
            .insert(path, handle);
    }
}

/// System: snapshot every freshly-arrived `MeshInstanceData` into the
/// active tab's handle set. Uses `Added<MeshInstanceData>` so we only
/// pay for new entities — the query is empty in steady state.
///
/// Runs in both `Loading` and `Editor` states because models can arrive
/// in either: the splash-loaded scene rehydrates entities during
/// `Loading`, and drag-drop / tab-respawn add them during `Editor`.
pub fn cache_added_mesh_instances(
    asset_server: Res<AssetServer>,
    tabs: Option<Res<DocumentTabState>>,
    mut cache: ResMut<TabAssetCache>,
    new_instances: Query<&MeshInstanceData, Added<MeshInstanceData>>,
) {
    let Some(tabs) = tabs else { return };
    let Some(active_id) = tabs.active_tab_id() else { return };

    for instance in new_instances.iter() {
        let Some(ref path) = instance.model_path else {
            continue;
        };
        // `asset_server.load` returns the already-cached handle if the
        // asset is loaded; for a fresh model it kicks the load and we
        // get a strong handle to the future asset. Either way, holding
        // it in the cache pins the asset's lifetime to the tab's.
        let handle: Handle<Gltf> = asset_server.load(path);
        cache.record(active_id, path.clone(), handle);
    }
}

/// Observer: drop the closed tab's handle set when the editor fires
/// `TabClosed`. Bevy's per-handle refcount handles cross-tab sharing —
/// if another tab still references the same path, its own handle keeps
/// the asset alive and only the closed tab's claim is released.
pub fn evict_closed_tab(
    trigger: On<TabClosed>,
    mut cache: ResMut<TabAssetCache>,
) {
    cache.drop_tab(trigger.event().tab_id);
}

/// Convenience accessor used by diagnostic/debug tooling — reports how
/// many distinct paths the cache currently pins, and how many tabs.
#[allow(dead_code)]
pub fn cache_stats(cache: &TabAssetCache) -> (usize, usize) {
    let tab_count = cache.by_tab.len();
    let mut paths: HashSet<&str> = HashSet::new();
    for tab in cache.by_tab.values() {
        for path in tab.keys() {
            paths.insert(path.as_str());
        }
    }
    (tab_count, paths.len())
}
