//! NavMesh persistence — the pure, runtime-safe `.navmesh` sidecar IO.
//!
//! The editor-side bake/load flow that resolves the *active scene path* (from
//! the editor's document tabs) and drives these helpers lives in
//! `renzora_navmesh_editor::persistence` — it depends on editor-only resources,
//! so only the pure file IO stays here.

use std::path::{Path, PathBuf};

use vleue_navigator::NavMesh;

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
pub fn load_navmesh_from_disk(path: &Path) -> Result<Option<NavMesh>, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(path)?;
    let mesh: polyanya::Mesh = ron::from_str(&data)?;
    Ok(Some(NavMesh::from_polyanya_mesh(mesh)))
}
