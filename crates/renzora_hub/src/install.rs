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
            file_url.rsplit('/').next().unwrap_or(asset_name)
        };
        let file_path = dest.join(filename);
        std::fs::write(&file_path, data).map_err(|e| format!("Failed to write file: {e}"))?;
        Ok(file_path)
    }
}

/// Install downloaded bytes into an **explicit** destination directory (the
/// folder the user picked in the install prompt), rather than the
/// category-derived default. Zips extract into an asset-named subfolder; single
/// files are written directly into `dest`.
#[cfg(not(target_arch = "wasm32"))]
pub fn install_asset_into(
    dest: &Path,
    asset_name: &str,
    file_url: &str,
    download_filename: &str,
    data: &[u8],
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dest).map_err(|e| format!("Failed to create directory: {e}"))?;

    let is_zip = file_url.ends_with(".zip") || (data.len() >= 4 && &data[..4] == b"PK\x03\x04");
    if is_zip {
        extract_zip(data, dest, asset_name)
    } else {
        let filename = if !download_filename.is_empty() {
            download_filename
        } else {
            file_url.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(asset_name)
        };
        let file_path = dest.join(filename);
        std::fs::write(&file_path, data).map_err(|e| format!("Failed to write file: {e}"))?;
        Ok(file_path)
    }
}

/// Metadata sidecar written next to an installed marketplace plugin dll, as
/// `<crate>.plugin.toml`. It ties the prebuilt dll back to its marketplace
/// identity and (eventually) its buildable source, which is what lets:
///   * a **lean export** fetch the plugin's source and compile it INTO the
///     static binary (a static binary can't dlopen), and
///   * the **official editor** fetch the right prebuilt dll per engine release.
///
/// `crate_name` (the dll stem / workspace crate name) is known at install time.
/// The source/release fields are written empty until the marketplace server's
/// build pipeline exists to populate them — this struct IS the contract that
/// pipeline must satisfy. See `docs/r1-alpha6` and `renzora_export::build`.
#[cfg(not(target_arch = "wasm32"))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct PluginSidecar {
    pub asset_id: String,
    pub name: String,
    pub slug: String,
    pub version: String,
    pub category: String,
    /// Workspace crate name = dll stem (e.g. `renzora_lumen`). What the lean
    /// exporter keys on to compile the plugin in from source.
    pub crate_name: String,
    /// Engine release whose frozen ABI this dll was built against. Empty until
    /// the server records it.
    #[serde(default)]
    pub engine_release: String,
    /// URL to download the plugin's buildable source for `engine_release`. Empty
    /// until the server exposes a `/source` endpoint.
    #[serde(default)]
    pub source_url: String,
    /// Per-release prebuilt dll URLs (`engine_release -> url`). A dll is
    /// ABI-frozen per release, so this is a matrix, not one URL. Empty for now.
    #[serde(default)]
    pub dylib_urls: std::collections::BTreeMap<String, String>,
}

/// Write a [`PluginSidecar`] next to an installed plugin. `installed` is the
/// path an `install_*` call returned: for a single-file dll the sidecar is
/// `<stem>.plugin.toml` beside it; for an extracted dir it's `plugin.toml`
/// inside it.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_plugin_sidecar(installed: &Path, meta: &PluginSidecar) -> Result<PathBuf, String> {
    let sidecar = if installed.is_dir() {
        installed.join("plugin.toml")
    } else {
        installed.with_extension("plugin.toml")
    };
    let toml = toml::to_string_pretty(meta).map_err(|e| format!("serialize plugin sidecar: {e}"))?;
    std::fs::write(&sidecar, toml).map_err(|e| format!("write plugin sidecar: {e}"))?;
    Ok(sidecar)
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
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
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
