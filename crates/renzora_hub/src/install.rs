//! Asset installation — downloads and extracts marketplace assets into the
//! correct project subdirectory based on category.

use std::path::{Path, PathBuf};

/// Map a marketplace category slug to the project subdirectory where assets
/// of that type should be installed.
pub fn install_dir_for_category(category: &str) -> &'static str {
    match category {
        "themes" | "theme" => "themes",
        "plugins" | "plugin" => "plugins",
        "scripts" | "script" => "scripts",
        "textures" | "texture" => "textures",
        "models" | "model" | "3d-models" => "models",
        "audio" | "sound" | "music" | "sfx" => "audio",
        "materials" | "material" => "materials",
        "scenes" | "scene" => "scenes",
        "shaders" | "shader" => "shaders",
        "fonts" | "font" => "fonts",
        "animations" | "animation" => "animations",
        "particles" | "particle" => "particles",
        "blueprints" | "blueprint" => "blueprints",
        "ui" | "ui-kit" => "ui",
        _ => "assets",
    }
}

/// Install downloaded bytes into the project directory.
///
/// If the file is a `.zip`, it is extracted. Otherwise it is written as-is.
/// Returns the path where the asset was installed.
#[cfg(not(target_arch = "wasm32"))]
pub fn install_asset(
    project_path: &Path,
    category: &str,
    asset_name: &str,
    file_url: &str,
    data: &[u8],
) -> Result<PathBuf, String> {
    install_asset_with_filename(project_path, category, asset_name, file_url, "", data)
}

/// Install downloaded bytes into the project directory.
///
/// If `download_filename` is non-empty, it is used instead of the URL-derived name.
/// If the file is a `.zip`, it is extracted. Otherwise it is written as-is.
/// Returns the path where the asset was installed.
#[cfg(not(target_arch = "wasm32"))]
pub fn install_asset_with_filename(
    project_path: &Path,
    category: &str,
    asset_name: &str,
    file_url: &str,
    download_filename: &str,
    data: &[u8],
) -> Result<PathBuf, String> {
    let subdir = install_dir_for_category(category);
    let dest = project_path.join(subdir);
    std::fs::create_dir_all(&dest).map_err(|e| format!("Failed to create directory: {e}"))?;

    // Determine if this is a zip file by URL extension or magic bytes
    let is_zip = file_url.ends_with(".zip") || (data.len() >= 4 && &data[..4] == b"PK\x03\x04");

    if is_zip {
        extract_zip(data, &dest, asset_name)
    } else {
        // Single file — prefer download_filename, fall back to URL-derived name
        let filename = if !download_filename.is_empty() {
            download_filename
        } else {
            file_url
                .rsplit('/')
                .next()
                .unwrap_or(asset_name)
        };
        let file_path = dest.join(filename);
        std::fs::write(&file_path, data)
            .map_err(|e| format!("Failed to write file: {e}"))?;
        Ok(file_path)
    }
}

/// Extract a zip archive into a destination directory, inside a subfolder
/// named after the asset.
#[cfg(not(target_arch = "wasm32"))]
fn extract_zip(data: &[u8], dest: &Path, asset_name: &str) -> Result<PathBuf, String> {
    use std::io::Read;

    let cursor = std::io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| format!("Invalid zip archive: {e}"))?;

    // Sanitize asset name for use as directory name
    let safe_name: String = asset_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let extract_dir = dest.join(&safe_name);
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Failed to create extract directory: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {e}"))?;

        let entry_name = entry.name().to_string();

        // Security: reject path traversal
        if entry_name.contains("..") {
            continue;
        }

        let out_path = extract_dir.join(&entry_name);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .map_err(|e| format!("Failed to create dir {entry_name}: {e}"))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent dir: {e}"))?;
            }
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read {entry_name}: {e}"))?;
            std::fs::write(&out_path, &buf)
                .map_err(|e| format!("Failed to write {entry_name}: {e}"))?;
        }
    }

    Ok(extract_dir)
}
