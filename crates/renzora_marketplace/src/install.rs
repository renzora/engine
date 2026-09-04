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
        let file_path = single_asset_path(&dest, category, asset_name, file_url, download_filename);
        std::fs::write(&file_path, data).map_err(|e| format!("Failed to write file: {e}"))?;
        Ok(file_path)
    }
}

/// Install downloaded bytes into an **explicit** destination directory (the
/// folder the user picked in the install prompt), rather than the
/// category-derived default. Zips extract into an asset-named subfolder; single
/// files are written directly into `dest`.
///
/// `category` is the asset's marketplace category (not the folder the user
/// picked): it's what tells us a lone `.toml` is a *theme*, which needs its
/// filename derived from the human asset name — see [`single_asset_path`].
#[cfg(not(target_arch = "wasm32"))]
pub fn install_asset_into(
    dest: &Path,
    category: &str,
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
        let file_path = single_asset_path(dest, category, asset_name, file_url, download_filename);
        std::fs::write(&file_path, data).map_err(|e| format!("Failed to write file: {e}"))?;
        Ok(file_path)
    }
}

/// Pick the on-disk path for a single (non-zip) downloaded asset.
///
/// Theme `.toml`s are the special case. The editor's theme picker shows a flat
/// theme by its **file stem**, so keeping the download's name would make the
/// dropdown read the asset UUID (e.g. `3e5959e4-….toml`) instead of the real
/// name like "Amber Terminal". For themes we therefore name the file from the
/// sanitized human asset name; every other category keeps the server's
/// `download_filename` / URL-tail name unchanged. (Zipped themes already land
/// in an asset-named folder via [`extract_zip`], so only the single-file path
/// was affected.)
#[cfg(not(target_arch = "wasm32"))]
fn single_asset_path(
    dest: &Path,
    category: &str,
    asset_name: &str,
    file_url: &str,
    download_filename: &str,
) -> PathBuf {
    if install_dir_for_category(category) == "themes" {
        if let Some(stem) = sanitize_theme_stem(asset_name) {
            return non_clobbering_theme_path(dest, &stem, asset_name);
        }
        // The name was all punctuation/control chars and sanitized to nothing:
        // fall through to the download's original name rather than write a
        // nameless file. Rare enough that the UUID fallback is acceptable.
    }
    let filename = if !download_filename.is_empty() {
        download_filename
    } else {
        file_url.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(asset_name)
    };
    dest.join(filename)
}

/// Sanitize a human asset name into a filename-safe stem for a single-file
/// theme. This stem is exactly what the user reads in the theme dropdown, so we
/// keep letters and spaces and only strip what a filesystem rejects: the path
/// separators / reserved characters `/ \ : * ? " < > |` and control chars.
/// Whitespace runs collapse to one space, and leading/trailing spaces and dots
/// are trimmed (both are illegal at the end of a Windows filename). Returns
/// `None` when nothing usable survives, so the caller falls back to the
/// download's original name rather than writing a blank one.
#[cfg(not(target_arch = "wasm32"))]
fn sanitize_theme_stem(name: &str) -> Option<String> {
    let mut out = String::with_capacity(name.len());
    let mut pending_space = false;
    for c in name.chars() {
        if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
            continue;
        }
        if c.is_whitespace() {
            pending_space = true;
            continue;
        }
        // Emit a single separating space only between kept characters.
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        out.push(c);
    }
    let trimmed = out.trim_matches(|c: char| c == ' ' || c == '.').to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Resolve `<stem>.toml` inside `dest` without silently clobbering a *different*
/// theme. Reinstalling the same theme (the existing file's `[meta] name` equals
/// the incoming asset name) overwrites in place — that's a legitimate update. A
/// genuine clash with an unrelated theme that sanitized to the same stem is
/// parked under a ` (2)`, ` (3)`… suffix instead of destroying it.
#[cfg(not(target_arch = "wasm32"))]
fn non_clobbering_theme_path(dest: &Path, stem: &str, asset_name: &str) -> PathBuf {
    let primary = dest.join(format!("{stem}.toml"));
    if !primary.exists() || theme_meta_name(&primary).as_deref() == Some(asset_name) {
        return primary;
    }
    for n in 2..1000 {
        let candidate = dest.join(format!("{stem} ({n}).toml"));
        if !candidate.exists() {
            return candidate;
        }
    }
    // Pathological (1000 same-named themes): overwrite rather than loop forever.
    primary
}

/// Read `[meta] name` from an existing theme `.toml`, if it reads and parses.
/// Lets an install tell "same theme, update it" from "different theme, same
/// filename", so it won't overwrite a theme it doesn't own.
#[cfg(not(target_arch = "wasm32"))]
fn theme_meta_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: toml::Value = toml::from_str(&content).ok()?;
    value
        .get("meta")
        .and_then(|meta| meta.get("name"))
        .and_then(|name| name.as_str())
        .map(str::to_string)
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

// ── Plugins ─────────────────────────────────────────────────────────────────

/// Is this category's download a buildable plugin source tree?
pub fn is_plugin_category(category: &str) -> bool {
    matches!(category, "plugins" | "plugin")
}

/// Where a plugin's source has to land: `<install root>/plugins/`.
///
/// Not the project's `plugins/` folder. `renzora_native_plugin::prebuild` scans
/// the directory beside the executable and compiles what it finds there, and
/// `NativePluginLoader` loads from the same place — a plugin installed anywhere
/// else is never seen by either.
#[cfg(not(target_arch = "wasm32"))]
pub fn engine_plugins_dir() -> Result<PathBuf, String> {
    renzora_native_build::install::root()
        .map(|r| r.join("plugins"))
        .ok_or_else(|| "Could not locate the engine install directory".to_string())
}

/// The crate name from a plugin zip's `Cargo.toml`, and the archive prefix to
/// strip when extracting.
///
/// Mirrors the check the marketplace server runs at upload time
/// (`plugin_crate_name`), and for the same reason: the manifest is the only
/// authority on what the crate is called. Accepts the manifest at the archive
/// root (prefix `""`) or one directory down (prefix `"<dir>/"`), because
/// zipping the crate folder and zipping its contents are both normal.
#[cfg(not(target_arch = "wasm32"))]
fn plugin_crate_and_prefix(data: &[u8]) -> Result<(String, String), String> {
    use std::io::Read;

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
        .map_err(|e| format!("Invalid zip archive: {e}"))?;

    let mut best: Option<(String, String)> = None; // (manifest text, prefix)
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {e}"))?;
        let name = f.name().to_string();
        if !name.ends_with("Cargo.toml") || name.contains("..") {
            continue;
        }
        let depth = name.matches('/').count();
        if depth > 1 {
            continue;
        }
        let prefix = match name.rsplit_once('/') {
            Some((dir, _)) => format!("{dir}/"),
            None => String::new(),
        };
        let mut text = String::new();
        if f.read_to_string(&mut text).is_err() {
            return Err("Cargo.toml is not valid UTF-8".to_string());
        }
        if depth == 0 {
            best = Some((text, prefix));
            break;
        }
        best.get_or_insert((text, prefix));
    }

    let (text, prefix) = best.ok_or_else(|| {
        "This plugin's archive has no Cargo.toml — it does not contain buildable source"
            .to_string()
    })?;

    let parsed: toml::Table = text
        .parse()
        .map_err(|e| format!("Cargo.toml is not valid TOML: {e}"))?;
    let name = parsed
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .ok_or_else(|| "Cargo.toml has no [package] name".to_string())?;

    // The name becomes a directory and a library filename.
    if name.is_empty()
        || name.len() > 64
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!("Invalid crate name '{name}' in Cargo.toml"));
    }
    Ok((name.to_string(), prefix))
}

/// The outcome of installing a plugin's source.
#[cfg(not(target_arch = "wasm32"))]
pub struct PluginInstall {
    pub path: PathBuf,
    /// The directory name, which is also the name it compiles under.
    pub dir_name: String,
    /// This replaced an existing install of the same asset.
    pub updated: bool,
    /// The crate name wanted was taken by a different asset, so it went in
    /// under `dir_name` instead. Worth telling the user.
    pub renamed_from: Option<String>,
}

/// Install a plugin by extracting its source into `<install root>/plugins/`,
/// ready for `prebuild` to compile on the next launch.
///
/// The directory name is what the plugin compiles as —
/// `renzora_plugin_build::crate_name` reads it back off the directory, and
/// `layout` derives `<dir>/build/<dir>.<ext>` from it — so it starts from the
/// crate's own name. Which directory it actually gets is decided by
/// [`crate::installed::destination_for`], because two different listings can
/// ship the same crate name and the second must not overwrite the first.
///
/// Reinstalling the same asset replaces its directory wholesale, dropping the
/// `build/` inside it — no stamp means `prebuild` rebuilds, which is what a new
/// version needs anyway.
#[cfg(not(target_arch = "wasm32"))]
pub fn install_plugin_source(asset_id: &str, data: &[u8]) -> Result<PluginInstall, String> {
    use std::io::Read;

    let (crate_name, prefix) = plugin_crate_and_prefix(data)?;
    let (dir_name, updated) = crate::installed::destination_for(asset_id, &crate_name);
    let renamed_from = (dir_name != crate_name).then(|| crate_name.clone());
    let plugins = engine_plugins_dir()?;
    let dest = plugins.join(&dir_name);

    // Extract to a staging directory and swap, so a failure part-way through
    // cannot leave a half-written plugin behind. Staged *outside* `plugins/`,
    // on the same filesystem so the rename is atomic: `prebuild` compiles any
    // directory under `plugins/` with a `src/lib.rs`, so a crash mid-extract
    // would otherwise leave it a broken tree to build.
    let root = plugins
        .parent()
        .ok_or_else(|| "Install directory has no parent".to_string())?;
    let staging = root.join(format!(".plugin-incoming-{dir_name}"));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("Failed to create {}: {e}", staging.display()))?;

    let extract = || -> Result<(), String> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
            .map_err(|e| format!("Invalid zip archive: {e}"))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Failed to read zip entry: {e}"))?;
            let raw = entry.name().to_string();
            if raw.contains("..") {
                continue;
            }
            // Strip the wrapper directory, if the archive has one, so the tree
            // always lands as `<crate>/Cargo.toml` rather than
            // `<crate>/<crate>/Cargo.toml`.
            let Some(rel) = raw.strip_prefix(prefix.as_str()) else {
                continue;
            };
            if rel.is_empty() {
                continue;
            }
            let out = staging.join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&out)
                    .map_err(|e| format!("Failed to create {}: {e}", out.display()))?;
                continue;
            }
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
            }
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read {rel}: {e}"))?;
            std::fs::write(&out, &buf)
                .map_err(|e| format!("Failed to write {}: {e}", out.display()))?;
        }
        Ok(())
    };

    if let Err(e) = extract() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }

    // `prebuild` keys off `src/lib.rs`; without it the plugin would be silently
    // ignored rather than reported, so say so now.
    if !staging.join("src").join("lib.rs").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!(
            "Plugin '{crate_name}' has no src/lib.rs — it does not look like a plugin crate"
        ));
    }

    let _ = std::fs::remove_dir_all(&dest);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    std::fs::rename(&staging, &dest).map_err(|e| {
        let _ = std::fs::remove_dir_all(&staging);
        format!("Failed to move plugin into {}: {e}", dest.display())
    })?;
    Ok(PluginInstall { path: dest, dir_name, updated, renamed_from })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod plugin_install_tests {
    use super::*;
    use std::io::Write;

    fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            for (name, body) in entries {
                w.start_file::<_, ()>(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                w.write_all(body.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        buf
    }

    const MANIFEST: &str = "[package]\nname = \"renzora_lumen\"\nversion = \"0.1.0\"\n";

    #[test]
    fn reads_a_root_manifest_with_no_prefix_to_strip() {
        let zip = zip_of(&[("Cargo.toml", MANIFEST), ("src/lib.rs", "")]);
        let (name, prefix) = plugin_crate_and_prefix(&zip).unwrap();
        assert_eq!(name, "renzora_lumen");
        assert_eq!(prefix, "");
    }

    /// Zipping the crate folder is just as common as zipping its contents, and
    /// the wrapper directory has to come off or the tree lands one level deep.
    #[test]
    fn reads_a_wrapped_manifest_and_reports_the_prefix() {
        let zip = zip_of(&[
            ("renzora_lumen/Cargo.toml", MANIFEST),
            ("renzora_lumen/src/lib.rs", ""),
        ]);
        let (name, prefix) = plugin_crate_and_prefix(&zip).unwrap();
        assert_eq!(name, "renzora_lumen");
        assert_eq!(prefix, "renzora_lumen/");
    }

    /// The wrapper directory's name is irrelevant — the manifest names the crate.
    #[test]
    fn the_crate_name_comes_from_the_manifest_not_the_folder() {
        let zip = zip_of(&[
            ("some-download-folder/Cargo.toml", MANIFEST),
            ("some-download-folder/src/lib.rs", ""),
        ]);
        let (name, _) = plugin_crate_and_prefix(&zip).unwrap();
        assert_eq!(name, "renzora_lumen");
    }

    #[test]
    fn a_manifest_nested_deeper_than_one_level_is_not_the_plugins() {
        let zip = zip_of(&[("a/b/Cargo.toml", MANIFEST)]);
        assert!(plugin_crate_and_prefix(&zip).is_err());
    }

    #[test]
    fn rejects_an_archive_with_no_manifest() {
        let zip = zip_of(&[("src/lib.rs", ""), ("readme.md", "hi")]);
        assert!(plugin_crate_and_prefix(&zip).is_err());
    }

    #[test]
    fn rejects_a_crate_name_that_would_escape_the_plugins_directory() {
        for bad in ["../../evil", "a/b", "with space"] {
            let manifest = format!("[package]\nname = \"{bad}\"\n");
            let zip = zip_of(&[("Cargo.toml", manifest.as_str())]);
            assert!(plugin_crate_and_prefix(&zip).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn rejects_bytes_that_are_not_a_zip() {
        assert!(plugin_crate_and_prefix(b"definitely not a zip").is_err());
    }

    #[test]
    fn plugin_categories_match_the_servers() {
        assert!(is_plugin_category("plugins"));
        assert!(is_plugin_category("plugin"));
        assert!(!is_plugin_category("scripts"));
        // The directory mapping has to agree, or the restart prompt and the
        // install path would disagree about what a plugin is.
        assert_eq!(install_dir_for_category("plugins"), "plugins");
    }
}
