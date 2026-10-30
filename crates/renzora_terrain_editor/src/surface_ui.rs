//! The terrain's landscape material in the editor: the default textures it
//! ships with. Choosing the material is the Terrain component's material slot
//! (`panel::material_slot`).
//!
//! The default layer textures live in the engine (`assets/terrain/`) and are
//! carried by the editor, not the runtime: a shipped game that uses them gets
//! them as project files like any other texture, and one that doesn't pays
//! nothing. Whenever a terrain's material names one the project lacks, it is
//! baked into the project at the path the material names (see
//! `renzora_shader::material::landscape_preset::default_texture_path`), then
//! reloaded. The same goes for Alpine Landscape itself, which the runtime can
//! write from code (`renzora_terrain::landscape::migrate`).
//!
//! **Baked, not copied.** The engine carries JPEGs, which load with no mip
//! chain and uncompressed; tiled across a terrain they shimmer into grey
//! static at a distance (see `default_texture_path`). Each is baked to `.rmip`
//! (block compressed, full mips) on a background thread the first time a
//! project needs it, about a second a map. A material saved while the defaults
//! were JPEGs has its paths moved to the baked copies.

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use std::path::PathBuf;

use renzora::core::CurrentProject;
use renzora_rmip::bake::{bake_image, BakeParams, TextureRole};
use renzora_shader::material::landscape_preset::{
    default_texture_path, superseded_default_textures, ALPINE_LANDSCAPE_PATH,
};
use renzora_terrain::landscape::{migrate, TerrainMaterial};

/// The engine's default layer textures, by project path, with how each bakes.
fn default_textures() -> [(String, &'static [u8], TextureRole); 16] {
    macro_rules! set {
        ($folder:literal) => {
            [
                (
                    default_texture_path($folder, "height"),
                    include_bytes!(concat!("../../../assets/terrain/", $folder, "/height.jpg")) as &[u8],
                    TextureRole::LinearData,
                ),
                (
                    default_texture_path($folder, "albedo"),
                    include_bytes!(concat!("../../../assets/terrain/", $folder, "/albedo.jpg")) as &[u8],
                    TextureRole::Color,
                ),
                (
                    default_texture_path($folder, "normal"),
                    include_bytes!(concat!("../../../assets/terrain/", $folder, "/normal.jpg")) as &[u8],
                    TextureRole::NormalMap,
                ),
                (
                    default_texture_path($folder, "arm"),
                    include_bytes!(concat!("../../../assets/terrain/", $folder, "/arm.jpg")) as &[u8],
                    TextureRole::LinearData,
                ),
            ]
        };
    }
    let [a, b, c, d] = set!("grass");
    let [e, f, g, h] = set!("dirt");
    let [i, j, k, l] = set!("rock");
    let [m, n, o, p] = set!("snow");
    [a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p]
}

/// Default textures being baked: the project path each will be written to,
/// with the bake running on the compute pool.
#[derive(Resource, Default)]
pub(crate) struct DefaultTextureBakes(Vec<(String, PathBuf, Task<Result<Vec<u8>, String>>)>);

/// Give the project what a terrain's material needs from the engine: Alpine
/// Landscape itself when that is the material, and any default texture the
/// material names that is not there yet.
///
/// In `PreUpdate`, before the runtime reads the material and the resolver
/// compiles it, so a new terrain finds everything already in place (the
/// textures a moment later, as their bakes finish).
pub(crate) fn provide_default_textures(
    materials: Query<&TerrainMaterial, Changed<TerrainMaterial>>,
    project: Option<Res<CurrentProject>>,
    asset_server: Res<AssetServer>,
    mut bakes: ResMut<DefaultTextureBakes>,
) {
    let Some(project) = project else { return };
    if materials.is_empty() {
        return;
    }
    let defaults = default_textures();
    for material in &materials {
        if material.path.is_empty() {
            continue;
        }
        if material.path == ALPINE_LANDSCAPE_PATH {
            if let Err(e) = migrate::ensure_alpine_material(&project.path) {
                warn!("[terrain] {e}");
            }
        }
        let file = project.path.join(&material.path);
        let Ok(mut text) = std::fs::read_to_string(&file) else { continue };
        if upgrade_default_paths(&mut text) {
            match std::fs::write(&file, &text) {
                Ok(()) => {
                    info!("[terrain] {} now uses the mipmapped default textures", material.path);
                    asset_server.reload(material.path.clone());
                }
                Err(e) => warn!("[terrain] could not update {}: {e}", file.display()),
            }
        }
        // Any default texture the file mentions. Its JSON names each one as a
        // quoted path, so a substring test finds them without parsing a graph.
        for (path, bytes, role) in &defaults {
            let target = project.path.join(path);
            if !text.contains(path.as_str()) || target.exists() || bakes.0.iter().any(|(p, ..)| p == path) {
                continue;
            }
            let (bytes, role) = (*bytes, *role);
            // The web build has no block compressor; it still gets the mips.
            let params = BakeParams { role, compress: cfg!(not(target_arch = "wasm32")), ..default() };
            let task = AsyncComputeTaskPool::get().spawn(async move { bake_image(bytes, params) });
            bakes.0.push((path.clone(), target, task));
        }
    }
}

/// Write each finished bake into the project and reload it.
pub(crate) fn finish_default_texture_bakes(mut bakes: ResMut<DefaultTextureBakes>, asset_server: Res<AssetServer>) {
    if bakes.0.is_empty() {
        return;
    }
    bakes.0.retain_mut(|(path, target, task)| {
        let Some(result) = block_on(future::poll_once(task)) else { return true };
        let written = result.and_then(|bytes| {
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&*target, bytes).map_err(|e| e.to_string())
        });
        match written {
            Ok(()) => {
                info!("[terrain] added the default texture {path} to the project");
                asset_server.reload(path.clone());
            }
            Err(e) => warn!("[terrain] could not add {path}: {e}"),
        }
        false
    });
}

/// Point a material's text at the baked default textures in place of the
/// JPEGs earlier versions named. Returns whether anything changed.
fn upgrade_default_paths(text: &mut String) -> bool {
    let mut changed = false;
    for (old, new) in superseded_default_textures() {
        if text.contains(&old) {
            *text = text.replace(&old, &new);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_texture_alpine_landscape_names_is_bundled() {
        let bundled: Vec<String> = default_textures().iter().map(|(p, ..)| p.clone()).collect();
        let spec = renzora_shader::material::landscape_preset::alpine_spec();
        for path in renzora_shader::material::landscape_preset::spec_textures(&spec) {
            assert!(bundled.contains(&path), "{path} is named by Alpine Landscape but not bundled");
        }
        assert!(default_textures().iter().all(|(_, b, _)| b.starts_with(&[0xFF, 0xD8])), "every source is a JPEG");
        // And the old default surface named the same set, so a migrated one
        // finds its textures too.
        for path in migrate::default_surface_textures() {
            assert!(bundled.contains(&path), "{path}");
        }
    }

    #[test]
    fn a_material_naming_the_old_jpegs_moves_to_the_baked_copies() {
        let mut text = r#"{"texture":"textures/terrain/v2/grass/albedo.jpg","other":"textures/mine.png"}"#.to_string();
        assert!(upgrade_default_paths(&mut text));
        assert!(text.contains(&default_texture_path("grass", "albedo")), "{text}");
        assert!(text.contains("textures/mine.png"));
        assert!(!upgrade_default_paths(&mut text), "a second pass finds nothing");
    }

    /// Only the four roles bake the way the material reads them: a normal map
    /// as two channels, colour as sRGB, the rest as linear data.
    #[test]
    fn each_map_bakes_by_its_role() {
        for (path, _, role) in default_textures() {
            let want = if path.ends_with("normal.rmip") {
                TextureRole::NormalMap
            } else if path.ends_with("albedo.rmip") {
                TextureRole::Color
            } else {
                TextureRole::LinearData
            };
            assert_eq!(role, want, "{path}");
        }
    }
}
