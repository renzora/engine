//! Asset discovery and bundling for game export
//!
//! Handles finding all assets referenced by scenes and copying them
//! to the export folder. Discovers assets by scanning the assets folder
//! and following GLTF dependencies.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Known asset file extensions (lowercase)
const ASSET_EXTENSIONS: &[&str] = &[
    // 3D models
    "gltf", "glb", "obj", "fbx", "usd", "usdz",
    // Images/textures
    "png", "jpg", "jpeg", "webp", "ktx2", "dds", "tga", "bmp", "hdr", "exr",
    // Audio
    "mp3", "ogg", "wav", "flac", "aac",
    // Fonts
    "ttf", "otf", "woff", "woff2",
    // Scenes (Bevy format)
    "ron",
    // Other
    "json", "toml",
];

/// Recursively discover all assets in the project's assets folder
pub fn discover_assets(project_path: &Path) -> Result<HashSet<PathBuf>, String> {
    let mut assets = HashSet::new();
    let mut visited_gltfs = HashSet::new();

    let assets_dir = project_path.join("assets");
    if assets_dir.exists() && assets_dir.is_dir() {
        discover_assets_in_dir(&assets_dir, &assets_dir, &mut assets, &mut visited_gltfs)?;
    }

    Ok(assets)
}

fn discover_assets_in_dir(
    dir: &Path,
    assets_root: &Path,
    assets: &mut HashSet<PathBuf>,
    visited_gltfs: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {:?}: {}", dir, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            discover_assets_in_dir(&path, assets_root, assets, visited_gltfs)?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if is_asset_extension(ext) {
                // Get relative path from assets root
                if let Ok(rel_path) = path.strip_prefix(assets_root) {
                    let rel_path = PathBuf::from(rel_path.to_string_lossy().replace('\\', "/"));
                    assets.insert(rel_path.clone());

                    // Check for GLTF dependencies
                    if ext.to_lowercase() == "gltf" {
                        discover_gltf_dependencies(&path, &rel_path, assets_root, assets, visited_gltfs)?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Check if a file extension is a known asset type
fn is_asset_extension(ext: &str) -> bool {
    ASSET_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// Parse a GLTF file and discover external texture/buffer dependencies
fn discover_gltf_dependencies(
    gltf_path: &Path,
    asset_rel_path: &Path,
    assets_root: &Path,
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
                let full_path = assets_root.join(&normalized);
                if full_path.exists() {
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
                let full_path = assets_root.join(&normalized);
                if full_path.exists() {
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
    let assets_src = project_path.join("assets");
    let assets_dest = export_path.join("assets");

    for asset_path in assets {
        let src = assets_src.join(asset_path);
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

/// Copy scene files needed for the game, stripping editor metadata
pub fn copy_scene_files(
    main_scene_path: &Path,
    project_path: &Path,
    export_path: &Path,
) -> Result<(), String> {
    let dest_scenes_dir = export_path.join("scenes");
    fs::create_dir_all(&dest_scenes_dir)
        .map_err(|e| format!("Failed to create scenes directory: {}", e))?;

    // Copy the main scene (with editor metadata stripped)
    let scene_name = main_scene_path
        .file_name()
        .ok_or_else(|| "Invalid scene path".to_string())?;
    let dest_scene = dest_scenes_dir.join(scene_name);
    copy_scene_stripped(main_scene_path, &dest_scene)?;

    // Copy any other scenes in the scenes directory
    let scenes_src = project_path.join("scenes");
    if scenes_src.exists() && scenes_src.is_dir() {
        for entry in fs::read_dir(&scenes_src)
            .map_err(|e| format!("Failed to read scenes directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "ron" {
                    let file_name = path.file_name().unwrap();
                    let dest = dest_scenes_dir.join(file_name);
                    if !dest.exists() {
                        copy_scene_stripped(&path, &dest)?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Copy a scene file, stripping the EditorSceneMetadata resource
fn copy_scene_stripped(src: &Path, dest: &Path) -> Result<(), String> {
    let content = fs::read_to_string(src)
        .map_err(|e| format!("Failed to read scene {:?}: {}", src, e))?;

    // Strip the EditorSceneMetadata resource from the scene
    // The resource appears in the RON file as:
    //   "bevy_imgui_editor::scene::saver::EditorSceneMetadata": ( ... ),
    let stripped = strip_editor_metadata(&content);

    fs::write(dest, stripped)
        .map_err(|e| format!("Failed to write scene {:?}: {}", dest, e))?;

    Ok(())
}

/// Strip EditorSceneMetadata from a scene's RON content
fn strip_editor_metadata(content: &str) -> String {
    // Find and remove the EditorSceneMetadata resource entry
    // Pattern: "...EditorSceneMetadata": ( ... ),
    // This is a simple text-based approach that handles nested parentheses

    let marker = "EditorSceneMetadata";
    if let Some(start_idx) = content.find(marker) {
        // Find the start of this resource entry (the opening quote)
        let entry_start = content[..start_idx].rfind('"').unwrap_or(start_idx);

        // Find the matching closing parenthesis for the value
        let after_marker = &content[start_idx..];
        if let Some(paren_start) = after_marker.find('(') {
            let value_start = start_idx + paren_start;
            let mut depth = 0;
            let mut value_end = value_start;

            for (i, c) in content[value_start..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            value_end = value_start + i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }

            // Find the trailing comma (if any)
            let trailing = &content[value_end..];
            let entry_end = if trailing.trim_start().starts_with(',') {
                value_end + trailing.find(',').unwrap() + 1
            } else {
                value_end
            };

            // Also handle leading comma if this wasn't the first entry
            let before_entry = &content[..entry_start];
            let actual_start = if before_entry.trim_end().ends_with(',') {
                before_entry.rfind(',').unwrap()
            } else {
                entry_start
            };

            // Reconstruct without the metadata entry
            let mut result = String::new();
            result.push_str(&content[..actual_start]);
            result.push_str(&content[entry_end..]);
            return result;
        }
    }

    // No metadata found, return as-is
    content.to_string()
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
