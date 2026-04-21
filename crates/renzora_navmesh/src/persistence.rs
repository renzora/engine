//! NavMesh persistence — bake to `.navmesh` sidecar files alongside
//! scene RON files, and load them back to skip runtime rebuilds.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use vleue_navigator::{
    NavMesh,
    prelude::{ManagedNavMesh, NavMeshStatus, NavMeshUpdateMode},
};

use crate::NavMeshVolume;

/// Derive the `.navmesh` sidecar path from a `.ron` scene path.
pub fn navmesh_path_for_scene(scene_path: &Path) -> PathBuf {
    scene_path.with_extension("navmesh")
}

/// Serialize the current navmesh to a RON file on disk.
pub fn save_navmesh_to_disk(
    navmesh: &NavMesh,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mesh = navmesh.get();
    let config = ron::ser::PrettyConfig::default()
        .depth_limit(8)
        .new_line("\n".into());
    let data = ron::ser::to_string_pretty(&*mesh, config)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, data)?;
    Ok(())
}

/// Deserialize a navmesh from a RON file on disk. Returns `None` if the
/// file doesn't exist, `Err` if it exists but can't be parsed.
pub fn load_navmesh_from_disk(
    path: &Path,
) -> Result<Option<NavMesh>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(path)?;
    let mesh: polyanya::Mesh = ron::from_str(&data)?;
    Ok(Some(NavMesh::from_polyanya_mesh(mesh)))
}

/// Try to load a baked navmesh when a volume is first added. If a
/// `.navmesh` sidecar exists next to the current scene file, inject it
/// into the asset store and skip the runtime build.
pub fn try_load_baked_navmesh(
    world: &mut World,
) {
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
    let handles: Vec<(Entity, bevy::asset::AssetId<NavMesh>)> = q
        .iter(world)
        .map(|(e, m, _)| (e, m.id()))
        .collect();

    if handles.is_empty() { return; }

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
            renzora::clog_success!(
                "NavMesh",
                "Baked navmesh to {}",
                navmesh_path.display()
            );
        }
        Err(e) => {
            warn!("[nav] Failed to bake navmesh: {e}");
            renzora::clog_error!("NavMesh", "Failed to bake navmesh: {e}");
        }
    }
}

fn get_active_scene_absolute_path(world: &World) -> Option<PathBuf> {
    let project = world.get_resource::<renzora::core::CurrentProject>()?;
    let tabs = world.get_resource::<renzora_ui::DocumentTabState>()?;
    let tab = tabs.tabs.get(tabs.active_tab)?;
    let scene_rel = tab.scene_path.as_ref()?;
    Some(project.resolve_path(scene_rel))
}
