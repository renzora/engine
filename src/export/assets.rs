//! Asset discovery and bundling for game export
//!
//! Handles finding all assets referenced by scenes and copying them
//! to the export folder.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::shared::{NodeData, SceneData};

/// Recursively discover all assets referenced by a scene
pub fn discover_assets(scene_path: &Path, project_path: &Path) -> Result<HashSet<PathBuf>, String> {
    let mut assets = HashSet::new();
    let mut visited_scenes = HashSet::new();

    discover_assets_recursive(scene_path, project_path, &mut assets, &mut visited_scenes)?;

    Ok(assets)
}

fn discover_assets_recursive(
    scene_path: &Path,
    project_path: &Path,
    assets: &mut HashSet<PathBuf>,
    visited_scenes: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    // Avoid infinite recursion with circular scene references
    if visited_scenes.contains(scene_path) {
        return Ok(());
    }
    visited_scenes.insert(scene_path.to_path_buf());

    // Read and parse the scene file
    let content = fs::read_to_string(scene_path)
        .map_err(|e| format!("Failed to read scene file {:?}: {}", scene_path, e))?;
    let scene: SceneData = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse scene file {:?}: {}", scene_path, e))?;

    // Process all nodes in the scene
    for node in &scene.root_nodes {
        discover_node_assets(node, project_path, assets, visited_scenes)?;
    }

    Ok(())
}

fn discover_node_assets(
    node: &NodeData,
    project_path: &Path,
    assets: &mut HashSet<PathBuf>,
    visited_scenes: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    // Check for model references in mesh instances
    if node.node_type == "mesh.instance" {
        if let Some(model_path) = node.data.get("model_path").and_then(|v| v.as_str()) {
            if !model_path.is_empty() {
                let full_path = project_path.join("assets").join(model_path);
                if full_path.exists() {
                    assets.insert(PathBuf::from(model_path));
                } else {
                    // Try without assets/ prefix
                    let alt_path = project_path.join(model_path);
                    if alt_path.exists() {
                        assets.insert(PathBuf::from(model_path));
                    }
                }
            }
        }
    }

    // Check for scene instance references
    if node.node_type == "scene.instance" {
        if let Some(scene_path) = node.data.get("scene_path").and_then(|v| v.as_str()) {
            if !scene_path.is_empty() {
                let full_path = project_path.join(scene_path);
                if full_path.exists() {
                    // Add the scene file as an asset
                    assets.insert(PathBuf::from(scene_path));
                    // Recursively discover assets in the nested scene
                    discover_assets_recursive(&full_path, project_path, assets, visited_scenes)?;
                }
            }
        }
    }

    // Process child nodes recursively
    for child in &node.children {
        discover_node_assets(child, project_path, assets, visited_scenes)?;
    }

    Ok(())
}

/// Copy discovered assets to the export folder, maintaining directory structure
pub fn copy_assets_to_folder(
    assets: &HashSet<PathBuf>,
    project_path: &Path,
    export_path: &Path,
) -> Result<(), String> {
    let assets_dest = export_path.join("assets");

    for asset_path in assets {
        let src = project_path.join("assets").join(asset_path);
        let dest = assets_dest.join(asset_path);

        // Create parent directories if they don't exist
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {:?}: {}", parent, e))?;
        }

        // Copy the file
        if src.exists() {
            fs::copy(&src, &dest)
                .map_err(|e| format!("Failed to copy {:?} to {:?}: {}", src, dest, e))?;
        } else {
            // Try without assets/ prefix
            let alt_src = project_path.join(asset_path);
            if alt_src.exists() {
                fs::copy(&alt_src, &dest)
                    .map_err(|e| format!("Failed to copy {:?} to {:?}: {}", alt_src, dest, e))?;
            }
        }
    }

    Ok(())
}

/// Copy all files in the project's assets folder to the export folder
pub fn copy_all_assets(project_path: &Path, export_path: &Path) -> Result<(), String> {
    let assets_src = project_path.join("assets");
    let assets_dest = export_path.join("assets");

    if assets_src.exists() && assets_src.is_dir() {
        copy_dir_recursive(&assets_src, &assets_dest)?;
    }

    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest)
        .map_err(|e| format!("Failed to create directory {:?}: {}", dest, e))?;

    for entry in fs::read_dir(src)
        .map_err(|e| format!("Failed to read directory {:?}: {}", src, e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)
                .map_err(|e| format!("Failed to copy {:?} to {:?}: {}", path, dest_path, e))?;
        }
    }

    Ok(())
}

/// Copy scene files needed for the game
pub fn copy_scene_files(
    main_scene_path: &Path,
    project_path: &Path,
    export_path: &Path,
) -> Result<(), String> {
    // Copy the main scene
    let scene_name = main_scene_path
        .file_name()
        .ok_or_else(|| "Invalid scene path".to_string())?;
    let dest_scenes_dir = export_path.join("scenes");
    fs::create_dir_all(&dest_scenes_dir)
        .map_err(|e| format!("Failed to create scenes directory: {}", e))?;

    let dest_scene = dest_scenes_dir.join(scene_name);
    fs::copy(main_scene_path, &dest_scene)
        .map_err(|e| format!("Failed to copy main scene: {}", e))?;

    // Copy any nested scenes (discovered through asset discovery)
    let assets = discover_assets(main_scene_path, project_path)?;
    for asset_path in assets {
        if asset_path.extension().and_then(|e| e.to_str()) == Some("scene") {
            let src = project_path.join(&asset_path);
            let dest = export_path.join(&asset_path);

            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            }

            if src.exists() {
                fs::copy(&src, &dest)
                    .map_err(|e| format!("Failed to copy scene {:?}: {}", src, e))?;
            }
        }
    }

    Ok(())
}

/// Create the project.toml file for the exported game
pub fn create_project_toml(
    project_name: &str,
    main_scene: &str,
    export_path: &Path,
) -> Result<(), String> {
    let content = format!(
        r#"[project]
name = "{}"
main_scene = "{}"
"#,
        project_name, main_scene
    );

    let dest = export_path.join("project.toml");
    fs::write(&dest, content)
        .map_err(|e| format!("Failed to write project.toml: {}", e))?;

    Ok(())
}
