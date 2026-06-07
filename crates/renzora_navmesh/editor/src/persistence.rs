//! Editor-side navmesh persistence: resolve the active scene path (from the
//! editor's document tabs) and bake / load the `.navmesh` sidecar beside it.
//!
//! The pure file IO (`navmesh_path_for_scene`, `save_navmesh_to_disk`,
//! `load_navmesh_from_disk`) lives in the lean `renzora_navmesh::persistence`;
//! only the scene-path resolution is editor-coupled (it reads the editor's
//! `DocumentTabState`), so it stays here.

use std::path::PathBuf;

use bevy::prelude::*;
use vleue_navigator::{
    prelude::{ManagedNavMesh, NavMeshStatus, NavMeshUpdateMode},
    NavMesh,
};

use renzora_navmesh::persistence::{load_navmesh_from_disk, navmesh_path_for_scene, save_navmesh_to_disk};
use renzora_navmesh::NavMeshVolume;

/// Try to load a baked navmesh when a volume is first added. If a `.navmesh`
/// sidecar exists next to the active scene file, inject it into the asset store
/// and skip the runtime build.
pub fn try_load_baked_navmesh(world: &mut World) {
    let scene_path = get_active_scene_absolute_path(world);
    let Some(scene_path) = scene_path else { return };
    let navmesh_path = navmesh_path_for_scene(&scene_path);

    let navmesh = match load_navmesh_from_disk(&navmesh_path) {
        Ok(Some(nm)) => nm,
        Ok(None) => return,
        Err(e) => {
            warn!("[nav] failed to load baked navmesh: {e}");
            renzora::clog_warn!("NavMesh", "Failed to load baked navmesh: {e}");
            return;
        }
    };

    // Find the volume entity → its ManagedNavMesh handle
    let mut q = world.query::<(Entity, &ManagedNavMesh, &NavMeshVolume)>();
    let handles: Vec<(Entity, bevy::asset::AssetId<NavMesh>)> =
        q.iter(world).map(|(e, m, _)| (e, m.id())).collect();

    if handles.is_empty() {
        return;
    }

    // Insert the loaded navmesh as the asset for each volume's handle
    let mut assets = world.resource_mut::<Assets<NavMesh>>();
    for (_entity, id) in &handles {
        let _ = assets.insert(*id, navmesh.clone());
    }

    // Mark status as Built and mode as OnDemand(false) — skip auto-rebuild
    for (entity, _) in &handles {
        if let Ok(mut entity_mut) = world.get_entity_mut(*entity) {
            entity_mut.insert(NavMeshStatus::Built);
            entity_mut.insert(NavMeshUpdateMode::OnDemand(false));
        }
    }

    info!("[nav] Loaded baked navmesh from {}", navmesh_path.display());
    renzora::clog_success!("NavMesh", "Loaded baked navmesh from disk");
}

/// Save the current navmesh to disk alongside the active scene.
pub fn bake_navmesh_to_disk(world: &mut World) {
    let scene_path = get_active_scene_absolute_path(world);
    let Some(scene_path) = scene_path else {
        renzora::clog_warn!("NavMesh", "Cannot bake — no active scene path");
        return;
    };
    let navmesh_path = navmesh_path_for_scene(&scene_path);

    // Collect the handle ID first, then look up the asset.
    let handle_id = {
        let mut q = world.query::<&ManagedNavMesh>();
        q.iter(world).next().map(|m| m.id())
    };
    let Some(handle_id) = handle_id else {
        renzora::clog_warn!("NavMesh", "Cannot bake — no NavMeshVolume in scene");
        return;
    };

    let assets = world.resource::<Assets<NavMesh>>();
    let Some(navmesh) = assets.get(handle_id) else {
        renzora::clog_warn!("NavMesh", "Cannot bake — navmesh not yet built");
        return;
    };

    match save_navmesh_to_disk(navmesh, &navmesh_path) {
        Ok(()) => {
            info!("[nav] Baked navmesh to {}", navmesh_path.display());
            renzora::clog_success!("NavMesh", "Baked navmesh to {}", navmesh_path.display());
        }
        Err(e) => {
            warn!("[nav] Failed to bake navmesh: {e}");
            renzora::clog_error!("NavMesh", "Failed to bake navmesh: {e}");
        }
    }
}

/// Resolve the active scene's absolute path from the editor's document tabs.
fn get_active_scene_absolute_path(world: &World) -> Option<PathBuf> {
    let project = world.get_resource::<renzora::core::CurrentProject>()?;
    let tabs = world.get_resource::<renzora_ui::DocumentTabState>()?;
    let tab = tabs.tabs.get(tabs.active_tab)?;
    let scene_rel = tab.scene_path.as_ref()?;
    Some(project.resolve_path(scene_rel))
}
