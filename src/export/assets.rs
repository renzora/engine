//! Asset discovery and bundling for game export
//!
//! Handles finding all assets referenced by scenes and copying them
//! to the export folder. Discovers assets by:
//! 1. Scanning all node data fields for asset path strings
//! 2. Parsing GLTF files to find external texture/buffer dependencies
//! 3. Recursively following scene references

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::shared::{NodeData, SceneData};

/// Known asset file extensions (lowercase)
const ASSET_EXTENSIONS: &[&str] = &[
    // 3D models
    "gltf", "glb", "obj", "fbx",
    // Images/textures
    "png", "jpg", "jpeg", "webp", "ktx2", "dds", "tga", "bmp", "hdr", "exr",
    // Audio
    "mp3", "ogg", "wav", "flac", "aac",
    // Fonts
    "ttf", "otf", "woff", "woff2",
    // Scenes
    "scene",
    // Other
    "ron", "json", "toml",
];

/// Recursively discover all assets referenced by a scene
pub fn discover_assets(scene_path: &Path, project_path: &Path) -> Result<HashSet<PathBuf>, String> {
    let mut assets = HashSet::new();
    let mut visited_scenes = HashSet::new();
    let mut visited_gltfs = HashSet::new();

    discover_assets_recursive(
        scene_path,
        project_path,
        &mut assets,
        &mut visited_scenes,
        &mut visited_gltfs,
    )?;

    Ok(assets)
}

fn discover_assets_recursive(
    scene_path: &Path,
    project_path: &Path,
    assets: &mut HashSet<PathBuf>,
    visited_scenes: &mut HashSet<PathBuf>,
    visited_gltfs: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    // Avoid infinite recursion with circular scene references
    if visited_scenes.contains(scene_path) {
        return Ok(());
    }
    visited_scenes.insert(scene_path.to_path_buf());

    // Read and parse the scene file (RON format)
    let content = fs::read_to_string(scene_path)
        .map_err(|e| format!("Failed to read scene file {:?}: {}", scene_path, e))?;
    let scene: SceneData = ron::from_str(&content)
        .map_err(|e| format!("Failed to parse scene file {:?}: {}", scene_path, e))?;

    // Process all nodes in the scene
    for node in &scene.root_nodes {
        discover_node_assets(node, project_path, assets, visited_scenes, visited_gltfs)?;
    }

    Ok(())
}

fn discover_node_assets(
    node: &NodeData,
    project_path: &Path,
    assets: &mut HashSet<PathBuf>,
    visited_scenes: &mut HashSet<PathBuf>,
    visited_gltfs: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    // Scan ALL data fields for asset paths
    for (_key, value) in &node.data {
        discover_assets_in_json_value(value, project_path, assets, visited_scenes, visited_gltfs)?;
    }

    // Process child nodes recursively
    for child in &node.children {
        discover_node_assets(child, project_path, assets, visited_scenes, visited_gltfs)?;
    }

    Ok(())
}

/// Check if a string looks like an asset path
fn is_asset_path(s: &str) -> bool {
    if s.is_empty() || s.len() > 500 {
        return false;
    }

    // Must have an extension
    let path = Path::new(s);
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        ASSET_EXTENSIONS.contains(&ext.to_lowercase().as_str())
    } else {
        false
    }
}

/// Recursively scan a JSON value for asset paths
fn discover_assets_in_json_value(
    value: &serde_json::Value,
    project_path: &Path,
    assets: &mut HashSet<PathBuf>,
    visited_scenes: &mut HashSet<PathBuf>,
    visited_gltfs: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    match value {
        serde_json::Value::String(s) => {
            if is_asset_path(s) {
                add_asset_with_dependencies(
                    s,
                    project_path,
                    assets,
                    visited_scenes,
                    visited_gltfs,
                )?;
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                discover_assets_in_json_value(
                    item,
                    project_path,
                    assets,
                    visited_scenes,
                    visited_gltfs,
                )?;
            }
        }
        serde_json::Value::Object(obj) => {
            for (_k, v) in obj {
                discover_assets_in_json_value(
                    v,
                    project_path,
                    assets,
                    visited_scenes,
                    visited_gltfs,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Add an asset and discover its dependencies
fn add_asset_with_dependencies(
    asset_path_str: &str,
    project_path: &Path,
    assets: &mut HashSet<PathBuf>,
    visited_scenes: &mut HashSet<PathBuf>,
    visited_gltfs: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    let asset_path = PathBuf::from(asset_path_str.replace('\\', "/"));

    // Try to find the asset file
    let full_path = resolve_asset_path(&asset_path, project_path);
    if full_path.is_none() {
        // Asset not found, skip it (might be optional or missing)
        return Ok(());
    }
    let full_path = full_path.unwrap();

    // Add the asset itself
    assets.insert(asset_path.clone());

    // Check for dependencies based on file type
    let ext = asset_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());

    match ext.as_deref() {
        // Scene files - recursively discover assets
        Some("scene") => {
            discover_assets_recursive(
                &full_path,
                project_path,
                assets,
                visited_scenes,
                visited_gltfs,
            )?;
        }
        // GLTF files - parse for external dependencies
        Some("gltf") => {
            discover_gltf_dependencies(
                &full_path,
                &asset_path,
                project_path,
                assets,
                visited_gltfs,
            )?;
        }
        // GLB files are self-contained, no external deps
        Some("glb") => {}
        _ => {}
    }

    Ok(())
}

/// Resolve an asset path to a full filesystem path
fn resolve_asset_path(asset_path: &Path, project_path: &Path) -> Option<PathBuf> {
    // Try with assets/ prefix
    let with_assets = project_path.join("assets").join(asset_path);
    if with_assets.exists() {
        return Some(with_assets);
    }

    // Try as-is from project root
    let from_root = project_path.join(asset_path);
    if from_root.exists() {
        return Some(from_root);
    }

    // Try stripping assets/ prefix if present
    if let Ok(stripped) = asset_path.strip_prefix("assets/") {
        let stripped_path = project_path.join("assets").join(stripped);
        if stripped_path.exists() {
            return Some(stripped_path);
        }
    }

    None
}

/// Parse a GLTF file and discover external texture/buffer dependencies
fn discover_gltf_dependencies(
    gltf_path: &Path,
    asset_rel_path: &Path,
    project_path: &Path,
    assets: &mut HashSet<PathBuf>,
    visited_gltfs: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    // Avoid parsing the same GLTF twice
    if visited_gltfs.contains(gltf_path) {
        return Ok(());
    }
    visited_gltfs.insert(gltf_path.to_path_buf());

    // Parse the GLTF file
    let gltf = gltf::Gltf::open(gltf_path)
        .map_err(|e| format!("Failed to parse GLTF {:?}: {}", gltf_path, e))?;

    // Get the directory containing the GLTF file for relative path resolution
    let gltf_dir = asset_rel_path.parent().unwrap_or(Path::new(""));

    // Find external buffer references
    for buffer in gltf.buffers() {
        if let gltf::buffer::Source::Uri(uri) = buffer.source() {
            // Skip data URIs
            if !uri.starts_with("data:") {
                let buffer_path = gltf_dir.join(uri);
                let normalized = PathBuf::from(buffer_path.to_string_lossy().replace('\\', "/"));
                if resolve_asset_path(&normalized, project_path).is_some() {
                    assets.insert(normalized);
                }
            }
        }
    }

    // Find external image references
    for image in gltf.images() {
        if let gltf::image::Source::Uri { uri, .. } = image.source() {
            // Skip data URIs
            if !uri.starts_with("data:") {
                let image_path = gltf_dir.join(uri);
                let normalized = PathBuf::from(image_path.to_string_lossy().replace('\\', "/"));
                if resolve_asset_path(&normalized, project_path).is_some() {
                    assets.insert(normalized);
                }
            }
        }
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
