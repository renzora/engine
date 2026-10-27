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
        // The shared list, so a format the engine learns to load (SVG was the
        // last) gets its icon without this table being found and edited.
        e if renzora::is_image_extension(e) => "image",
        "ttf" | "otf" | "woff" | "woff2" => "text-aa",
        "rvg" | "glb" | "gltf" | "obj" | "fbx" => "cube",
        "material" => "palette",
        "wgsl" | "wesl" | "glsl" | "vert" | "frag" => "graphics-card",
        "lua" | "rs" | "py" | "js" | "ts" => "code",
        "scene" | "bsn" | "ron" | "scn" => "film-slate",
        // The same runner the hierarchy puts on an animated entity, so a clip on
        // disk and the thing that plays it read as one feature.
        "anim" => "person-simple-run",
        "animsm" => "tree-structure",
        "wav" | "ogg" | "mp3" | "flac" => "speaker-high",
        // The old effect format, still played (and converted) but no longer
        // edited as itself.
        "particle" => "sparkle",
        // The node-graph effect. Its own glyph so a project holding both kinds
        // tells them apart at a glance; same family as the old sparkle.
        "vfx" => "magic-wand",
        "ply" | "gcloud" | "sog" | "ssog" => "cloud",
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
        "lua" => ((120, 170, 255), "Lua Script"),
        "wgsl" | "wesl" | "glsl" | "vert" | "frag" => ((220, 120, 255), "Shader"),
        "rs" => ((230, 140, 90), "Rust Script"),
        // Same colour as a raster texture: it loads as one and fits every slot
        // one does. The label says vector so the scalability is discoverable.
        e if renzora::is_vector_image_extension(e) => ((150, 210, 120), "Vector Image"),
        e if renzora::is_image_extension(e) => ((150, 210, 120), "Texture"),
        "rvg" | "glb" | "gltf" | "obj" | "fbx" => ((255, 170, 100), "Model"),
        "bsn" | "ron" | "scn" | "scene" => ((115, 191, 242), "Scene"),
        "anim" => ((90, 215, 205), "Animation"),
        "animsm" => ((90, 215, 205), "State Machine"),
        "particle" => ((230, 160, 90), "Particle"),
        "vfx" => ((255, 140, 70), "Particle Effect"),
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
    fn animation_clips_are_not_plain_files() {
        // `.anim` is RON inside, but it must not fall through to the scene arm
        // (which matches `ron`) nor to the generic "ANIM" label.
        assert_eq!(icon_for(Path::new("models/character/Idle.anim"), false), "person-simple-run");
        assert_eq!(type_info(Path::new("models/character/Idle.anim")).1, "Animation");
        assert_eq!(icon_for(Path::new("locomotion.animsm"), false), "tree-structure");
    }

    #[test]
    fn node_graph_effects_have_their_own_identity() {
        assert_eq!(icon_for(Path::new("effects/fire.vfx"), false), "magic-wand");
        assert_eq!(type_info(Path::new("effects/fire.vfx")).1, "Particle Effect");
    }

    #[test]
    fn svg_is_an_image() {
        assert_eq!(icon_for(Path::new("ui/icons/heart.svg"), false), "image");
        assert_eq!(icon_for(Path::new("ui/icons/heart.SVGZ"), false), "image");
        assert_eq!(type_info(Path::new("ui/icons/heart.svg")).1, "Vector Image");
        assert_eq!(type_info(Path::new("tex/wall.png")).1, "Texture");
        // Its settings sidecar is a TOML file, not a second image.
        assert_eq!(icon_for(Path::new("ui/icons/heart.svg.toml"), false), "brackets-curly");
    }

    #[test]
    fn unknown_extensions_label_as_themselves() {
        assert_eq!(type_info(Path::new("notes.md")).1, "MD");
        assert_eq!(type_info(Path::new("LICENSE")).1, "File");
    }
}
