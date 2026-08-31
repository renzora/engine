//! What a file *is*, by path: its icon, its accent colour and its type label.
//!
//! One table, because a `.lua` should look the same everywhere it appears — in
//! the asset browser's grid, in a folder picker listing files to attach, in a
//! document tab. It lived in `renzora_asset_browser` and the picker could not
//! reach it (ember cannot depend on a crate that depends on ember), which is
//! exactly the shape that ends in two tables drifting apart.
//!
//! Colours are deliberately **theme-independent**: these are *type* identities,
//! not chrome, so a material stays the same green when the palette changes.

use std::path::Path;

/// A theme is a folder of files, so a `.toml` inside one is not a config — it
/// is the theme. Recognised by name or by living under a `themes/` ancestor.
pub fn is_theme_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.eq_ignore_ascii_case("theme.toml") {
        return true;
    }
    let is_toml = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("toml"));
    is_toml
        && path.ancestors().skip(1).any(|a| {
            a.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("themes"))
        })
}

/// Phosphor icon *name* for `path`. `is_dir` short-circuits to a folder, so a
/// caller that already knows does not pay a `stat`.
pub fn icon_for(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    if is_theme_file(path) {
        return "swatches";
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "ktx2" | "dds" | "bmp" | "tga" => "image",
        "ttf" | "otf" | "woff" | "woff2" => "text-aa",
        "glb" | "gltf" | "obj" | "fbx" => "cube",
        "material" => "palette",
        "wgsl" | "glsl" | "vert" | "frag" => "graphics-card",
        "lua" | "rs" | "py" | "js" | "ts" => "code",
        "scene" | "bsn" | "ron" | "scn" => "film-slate",
        "wav" | "ogg" | "mp3" | "flac" => "speaker-high",
        "particle" => "sparkle",
        "ply" | "gcloud" | "sog" | "ssog" => "cloud",
        "blueprint" | "bp" => "blueprint",
        "html" => "browser",
        "toml" => "brackets-curly",
        _ => "file",
    }
}

/// Accent colour + human type label for `path`.
pub fn type_info(path: &Path) -> ((u8, u8, u8), &'static str) {
    if is_theme_file(path) {
        return ((255, 170, 210), "Theme");
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "ttf" | "otf" | "woff" | "woff2" => ((140, 200, 255), "Font"),
        "material" => ((0, 200, 130), "Material"),
        "blueprint" | "bp" => ((100, 180, 255), "Blueprint"),
        "lua" => ((120, 170, 255), "Lua Script"),
        "wgsl" | "glsl" | "vert" | "frag" => ((220, 120, 255), "Shader"),
        "rs" => ((230, 140, 90), "Rust Script"),
        "png" | "jpg" | "jpeg" | "webp" | "ktx2" | "dds" | "bmp" | "tga" => {
            ((150, 210, 120), "Texture")
        }
        "glb" | "gltf" | "obj" | "fbx" => ((255, 170, 100), "Model"),
        "bsn" | "ron" | "scn" | "scene" => ((115, 191, 242), "Scene"),
        "particle" => ((230, 160, 90), "Particle"),
        "ply" | "gcloud" | "sog" | "ssog" => ((190, 150, 255), "Gaussian Splat"),
        "wav" | "ogg" | "mp3" | "flac" => ((200, 130, 230), "Audio"),
        "html" => ((230, 120, 90), "UI Template"),
        "" => ((150, 155, 170), "File"),
        other => ((150, 155, 170), uppercase_ext(other)),
    }
}

/// Just the colour, for callers that do not want the label.
pub fn color_for(path: &Path) -> (u8, u8, u8) {
    type_info(path).0
}

/// Leak a small set of uppercased extension labels for unknown types so a
/// subtitle can read e.g. "TXT" / "JSON". Bounded: only the handful of distinct
/// unknown extensions actually present in a project are ever leaked.
fn uppercase_ext(ext: &str) -> &'static str {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut map = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
    if let Some(s) = map.get(ext) {
        return s;
    }
    let leaked: &'static str = Box::leak(ext.to_uppercase().into_boxed_str());
    map.insert(ext.to_string(), leaked);
    leaked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_toml_beats_the_plain_toml_arm() {
        assert_eq!(icon_for(Path::new("themes/dark/theme.toml"), false), "swatches");
        assert_eq!(icon_for(Path::new("project.toml"), false), "brackets-curly");
    }

    #[test]
    fn a_directory_is_a_folder_whatever_its_name_looks_like() {
        // `models/tree.glb/` would otherwise read as a model.
        assert_eq!(icon_for(Path::new("models/tree.glb"), true), "folder");
    }

    #[test]
    fn unknown_extensions_label_as_themselves() {
        assert_eq!(type_info(Path::new("notes.md")).1, "MD");
        assert_eq!(type_info(Path::new("LICENSE")).1, "File");
    }
}
