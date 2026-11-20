//! Turning a terrain saved before landscape materials into one that has one.
//!
//! Two things a scene may hold that this replaces:
//!
//! * a `TerrainSurface` (the hand-written slope-and-height surface). It
//!   becomes a [`TerrainMaterial`] pointing at a landscape material generated
//!   from that surface's own layers: the same textures, tile sizes, tints and
//!   slope and height rules, in a project file of its own. A surface that was
//!   still exactly the default points at the shared Alpine Landscape instead.
//! * a `Painter` (layers painted as see-through overlay meshes). Each painted
//!   mask becomes weight for the landscape layer it best matches, by name and
//!   then by what its material was called, laid on in stack order exactly as
//!   the overlays stacked. A terrain with paint but no surface gets Alpine
//!   Landscape to paint into.
//!
//! Both are removed once converted, so a migrated scene saves only the new
//! form. Migration writes project files, so it runs in the editor only. A
//! shipped game holding a scene that was never opened in the editor since
//! still draws its surface with the old material (`crate::surface`), which is
//! kept for exactly that; its overlay paint is no longer drawn.

use bevy::prelude::*;
use renzora_shader::material::landscape::LandscapeBlend;
use renzora_shader::material::landscape_preset::{
    alpine_landscape_graph, alpine_spec, build_landscape_graph, current_default_texture, default_texture_path, Band,
    LandscapeLayerSpec, LandscapeSpec, ALPINE_LANDSCAPE_PATH,
};

use super::instance::TerrainMaterial;
use super::weights::{pack, push_toward, solid, unpack, weight_resolution, TerrainChunkWeights, Texel};
use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use crate::painter::{PaintLayer, Painter, PainterLayerMesh};
use crate::surface::{SurfaceBuild, TerrainSurface};

/// Height statistics of the engine's default textures, so a migrated surface
/// using them is height-blended the same way Alpine Landscape is.
fn known_height_stats(path: &str) -> Option<(f32, f32)> {
    let spec = alpine_spec();
    spec.layers.iter().find(|l| l.height == path).and_then(|l| l.height_stats)
}

/// The landscape spec that draws `surface` the way the old surface shader
/// did. Every layer height-blends (the old shader height-blended all of them);
/// the first layer is the ground and has no rule, as before.
pub fn spec_from_surface(surface: &TerrainSurface) -> LandscapeSpec {
    let layers = surface
        .layers
        .iter()
        .filter(|l| l.enabled)
        .take(crate::surface::MAX_SURFACE_LAYERS)
        .enumerate()
        .map(|(i, l)| {
            let slope = (i > 0 && (l.slope_min > 0.01 || l.slope_max < 89.99))
                .then_some(Band { min: l.slope_min, max: l.slope_max, blend: l.slope_blend.max(0.0) });
            let height_band = (i > 0 && (l.height_min > -99_999.0 || l.height_max < 99_999.0))
                .then_some(Band { min: l.height_min, max: l.height_max, blend: l.height_blend.max(0.0) });
            LandscapeLayerSpec {
                name: if l.name.trim().is_empty() { format!("Layer {}", i + 1) } else { l.name.clone() },
                // The engine's own maps by their baked, mipmapped copies.
                albedo: current_default_texture(&l.albedo),
                normal: current_default_texture(&l.normal),
                arm: current_default_texture(&l.arm),
                height: current_default_texture(&l.height),
                tint: [l.tint.x, l.tint.y, l.tint.z],
                tile_size: l.tile_size.max(0.01),
                triplanar: l.triplanar,
                height_stats: known_height_stats(&current_default_texture(&l.height)),
                slope,
                height_band,
                breakup: l.breakup.clamp(0.0, 1.0),
                blend: LandscapeBlend::Height,
            }
        })
        .collect();
    LandscapeSpec {
        layers,
        height_blending: surface.height_blending.clamp(0.0, 1.0),
        normal_strength: surface.normal_strength.max(0.0),
        macro_variation: surface.macro_variation.max(0.0),
        macro_size: surface.macro_size.max(1.0),
    }
}

/// Write `graph`, compiled, to `path` in the project.
fn write_material(root: &std::path::Path, path: &str, mut graph: renzora_shader::material::graph::MaterialGraph) -> Result<(), String> {
    let fs = root.join(path);
    if let Some(dir) = fs.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    let (json, report) = renzora_shader::material::precompiled::save_compiled_and_serialize(&mut graph, &fs)
        .map_err(|e| format!("could not compile {path}: {e}"))?;
    for e in &report.errors {
        warn!("[terrain] {path}: {e}");
    }
    std::fs::write(&fs, json).map_err(|e| format!("could not write {}: {e}", fs.display()))
}

/// Make sure the project has Alpine Landscape at [`ALPINE_LANDSCAPE_PATH`],
/// writing the engine's copy if it does not. Its textures are the editor's to
/// supply (they are files the engine carries, not code); see
/// `renzora_terrain_editor::surface_ui`.
pub fn ensure_alpine_material(project_root: &std::path::Path) -> Result<(), String> {
    if project_root.join(ALPINE_LANDSCAPE_PATH).exists() {
        return Ok(());
    }
    write_material(project_root, ALPINE_LANDSCAPE_PATH, alpine_landscape_graph())
}

/// A file name for a terrain's own material: its name, made safe.
fn material_path_for(name: &str, root: &std::path::Path) -> String {
    let stem: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    let stem = if stem.is_empty() { "terrain".to_string() } else { stem };
    for n in 1.. {
        let path = if n == 1 {
            format!("materials/landscape/{stem}_landscape.material")
        } else {
            format!("materials/landscape/{stem}_landscape_{n}.material")
        };
        if !root.join(&path).exists() {
            return path;
        }
    }
    unreachable!("the suffix loop only ends by returning")
}

/// The landscape layer a painted layer becomes: one of the same name, else
/// one its material's file name suggests, else the dirt or gravel layer (what
/// a layer with no material painted), else the second layer, else the first.
pub fn match_paint_layer(paint: &PaintLayer, layers: &[String]) -> usize {
    let lower: Vec<String> = layers.iter().map(|l| l.to_ascii_lowercase()).collect();
    let name = paint.name.trim().to_ascii_lowercase();
    if let Some(i) = lower.iter().position(|l| *l == name) {
        return i;
    }
    // Words that mean the same ground, grouped.
    const KINDS: &[&[&str]] = &[
        &["grass", "meadow", "moss", "lawn", "turf"],
        &["dirt", "gravel", "mud", "soil", "path", "earth", "ground"],
        &["rock", "stone", "cliff", "boulder", "granite"],
        &["snow", "ice", "frost"],
        &["sand", "beach", "dune"],
    ];
    let hint = format!(
        "{name} {}",
        paint
            .material_path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    );
    for kind in KINDS {
        if kind.iter().any(|w| hint.contains(w)) {
            if let Some(i) = lower.iter().position(|l| kind.iter().any(|w| l.contains(w))) {
                return i;
            }
        }
    }
    if paint.material_path.is_none() {
        if let Some(i) = lower.iter().position(|l| l.contains("dirt") || l.contains("gravel")) {
            return i;
        }
    }
    if layers.len() > 1 {
        1
    } else {
        0
    }
}

/// Bilinear read of a painter mask at map metres `p`.
fn sample_mask(layer: &PaintLayer, terrain: &TerrainData, p: Vec2) -> f32 {
    let g = layer.grid_size();
    if g < 2 || layer.mask.len() < (g * g) as usize {
        return 0.0;
    }
    let span = terrain.chunks_x.max(terrain.chunks_z) as f32 * terrain.chunk_size;
    let cell = span / (g - 1) as f32;
    let fx = (p.x / cell).clamp(0.0, (g - 1) as f32);
    let fz = (p.y / cell).clamp(0.0, (g - 1) as f32);
    let (x0, z0) = (fx.floor() as u32, fz.floor() as u32);
    let (x1, z1) = ((x0 + 1).min(g - 1), (z0 + 1).min(g - 1));
    let (tx, tz) = (fx - x0 as f32, fz - z0 as f32);
    let m = |x: u32, z: u32| layer.mask[(z * g + x) as usize];
    let top = m(x0, z0) + (m(x1, z0) - m(x0, z0)) * tx;
    let bottom = m(x0, z1) + (m(x1, z1) - m(x0, z1)) * tx;
    (top + (bottom - top) * tz).clamp(0.0, 1.0)
}

/// The weights of the chunk at `(cx, cz)` once `layers` are painted onto
/// `existing` (channel 0 everywhere when `None`), each in turn, mask value as
/// strength, into `channels[i]`. `None` when no texel changed.
pub fn paint_masks_into_chunk(
    terrain: &TerrainData,
    cx: u32,
    cz: u32,
    existing: Option<&[Texel]>,
    layers: &[(&PaintLayer, usize)],
    base: usize,
) -> Option<Vec<Texel>> {
    let res = weight_resolution(terrain);
    let cell = terrain.chunk_size / res as f32;
    let mut out: Vec<Texel> = match existing {
        Some(t) if t.len() == (res * res) as usize => t.to_vec(),
        _ => vec![solid(0); (res * res) as usize],
    };
    let mut changed = false;
    for z in 0..res {
        for x in 0..res {
            let p = Vec2::new(
                (cx * res + x) as f32 * cell + cell * 0.5,
                (cz * res + z) as f32 * cell + cell * 0.5,
            );
            let i = (z * res + x) as usize;
            let mut w = unpack(out[i]);
            let mut touched = false;
            for (layer, channel) in layers {
                let m = sample_mask(layer, terrain, p);
                if m > 0.002 {
                    push_toward(&mut w, *channel, 1.0, m, base);
                    touched = true;
                }
            }
            if touched {
                let packed = pack(w, base);
                if packed != out[i] {
                    out[i] = packed;
                    changed = true;
                }
            }
        }
    }
    changed.then_some(out)
}

/// Convert every old terrain in the scene. Exclusive: it writes files and
/// rewrites components across the terrain and its chunks.
pub(crate) fn migrate_old_terrains(world: &mut World) {
    let Some(project_root) = world.get_resource::<renzora::CurrentProject>().map(|p| p.path.clone()) else {
        return;
    };
    let mut surfaced = world.query_filtered::<(Entity, &TerrainSurface, Option<&Name>), (With<TerrainData>, Without<TerrainMaterial>)>();
    let surfaces: Vec<(Entity, TerrainSurface, String)> = surfaced
        .iter(world)
        .map(|(e, s, n)| (e, s.clone(), n.map(|n| n.as_str().to_string()).unwrap_or_default()))
        .collect();
    let mut painted = world.query_filtered::<(Entity, &Painter), With<TerrainData>>();
    let painters: Vec<(Entity, Painter)> = painted.iter(world).map(|(e, p)| (e, p.clone())).collect();
    if surfaces.is_empty() && painters.is_empty() {
        return;
    }

    // Layer names of the material each converted terrain now has, for the
    // paint below.
    let mut layer_names: std::collections::HashMap<Entity, Vec<String>> = Default::default();
    for (root, surface, name) in surfaces {
        let (path, spec) = if surface == TerrainSurface::default() {
            if let Err(e) = ensure_alpine_material(&project_root) {
                warn!("[terrain] {e}");
                continue;
            }
            (ALPINE_LANDSCAPE_PATH.to_string(), alpine_spec())
        } else {
            let spec = spec_from_surface(&surface);
            let path = material_path_for(&name, &project_root);
            let title = if name.is_empty() { "Terrain Landscape".to_string() } else { format!("{name} Landscape") };
            if let Err(e) = write_material(&project_root, &path, build_landscape_graph(&title, &spec)) {
                warn!("[terrain] {e}");
                continue;
            }
            (path, spec)
        };
        let names: Vec<String> = spec.layers.iter().map(|l| l.name.clone()).collect();
        info!("[terrain] converted the surface of {root} into the landscape material {path}");
        renzora::console_log::console_info("Terrain", format!("Converted the terrain surface into {path}"));
        world
            .entity_mut(root)
            .remove::<(TerrainSurface, SurfaceBuild)>()
            .insert(TerrainMaterial { path, channels: names.clone() });
        layer_names.insert(root, names);
    }

    for (root, painter) in painters {
        let live: Vec<&PaintLayer> = painter
            .layers
            .iter()
            .filter(|l| l.enabled && l.mask.iter().any(|&m| m > 0.002))
            .collect();
        if !live.is_empty() {
            let names = match layer_names.get(&root) {
                Some(n) => n.clone(),
                None => match world.get::<TerrainMaterial>(root).cloned() {
                    Some(m) if !m.channels.is_empty() => m.channels,
                    Some(_) | None => {
                        if let Err(e) = ensure_alpine_material(&project_root) {
                            warn!("[terrain] {e}");
                            continue;
                        }
                        let names: Vec<String> = alpine_spec().layers.iter().map(|l| l.name.clone()).collect();
                        world
                            .entity_mut(root)
                            .insert(TerrainMaterial { path: ALPINE_LANDSCAPE_PATH.into(), channels: names.clone() });
                        names
                    }
                },
            };
            let mapped: Vec<(&PaintLayer, usize)> = live.iter().map(|l| (*l, match_paint_layer(l, &names))).collect();
            let Some(terrain) = world.get::<TerrainData>(root).cloned() else { continue };
            let mut chunks = world.query::<(Entity, &TerrainChunkData, &TerrainChunkOf, Option<&TerrainChunkWeights>)>();
            let jobs: Vec<(Entity, u32, u32, Option<Vec<Texel>>)> = chunks
                .iter(world)
                .filter(|(_, _, of, _)| of.0 == root)
                .map(|(e, c, _, w)| (e, c.chunk_x, c.chunk_z, w.and_then(|w| decoded(w, &terrain))))
                .collect();
            let res = weight_resolution(&terrain);
            for (chunk, cx, cz, existing) in jobs {
                if let Some(texels) = paint_masks_into_chunk(&terrain, cx, cz, existing.as_deref(), &mapped, 0) {
                    let mut weights = world.get::<TerrainChunkWeights>(chunk).cloned().unwrap_or_default();
                    weights.set_texels(res, texels);
                    world.entity_mut(chunk).insert(weights);
                }
            }
            info!("[terrain] converted {} painted layers of {root} into layer weights", live.len());
        }
        // The overlays are not drawn any more; their entities go with the
        // component that owned them.
        let mut overlays = world.query::<(Entity, &PainterLayerMesh)>();
        let stale: Vec<Entity> = overlays.iter(world).filter(|(_, m)| m.painter == root).map(|(e, _)| e).collect();
        for e in stale {
            world.entity_mut(e).despawn();
        }
        world.entity_mut(root).remove::<Painter>();
    }
}

fn decoded(weights: &TerrainChunkWeights, terrain: &TerrainData) -> Option<Vec<Texel>> {
    let mut w = weights.clone();
    w.ensure_decoded(weight_resolution(terrain)).then_some(w.texels)
}

/// The texture paths a default surface used, which the editor supplies.
pub fn default_surface_textures() -> Vec<String> {
    ["grass", "dirt", "rock", "snow"]
        .iter()
        .flat_map(|f| ["albedo", "normal", "arm", "height"].map(|m| default_texture_path(f, m)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_shader::material::codegen;

    #[test]
    fn the_default_surface_converts_to_the_alpine_layers() {
        let spec = spec_from_surface(&TerrainSurface::default());
        let names: Vec<&str> = spec.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Grass", "Gravel", "Rock", "Snow"]);
        // Same textures, tiling and rules as the preset.
        let alpine = alpine_spec();
        for (a, b) in spec.layers.iter().zip(&alpine.layers) {
            assert_eq!((&a.albedo, a.tile_size, a.slope, a.height_band), (&b.albedo, b.tile_size, b.slope, b.height_band));
        }
        assert!(spec.layers[0].slope.is_none(), "the ground layer has no rule");
    }

    #[test]
    fn an_edited_surface_converts_to_a_graph_that_compiles() {
        let mut surface = TerrainSurface::default();
        surface.layers[1].tile_size = 9.0;
        surface.layers[3].enabled = false;
        surface.layers[2].albedo = String::new();
        let spec = spec_from_surface(&surface);
        assert_eq!(spec.layers.len(), 3);
        assert_eq!(spec.layers[1].tile_size, 9.0);
        let result = codegen::compile(&build_landscape_graph("Edited", &spec));
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.landscape_layers, ["Grass", "Gravel", "Rock"]);
    }

    fn paint_layer(name: &str, material: Option<&str>) -> PaintLayer {
        let mut l = PaintLayer::empty(name, 3);
        l.material_path = material.map(str::to_string);
        l
    }

    #[test]
    fn painted_layers_find_the_landscape_layer_they_meant() {
        let layers: Vec<String> = ["Grass", "Gravel", "Rock", "Snow"].iter().map(|s| s.to_string()).collect();
        assert_eq!(match_paint_layer(&paint_layer("Snow", None), &layers), 3);
        assert_eq!(match_paint_layer(&paint_layer("Layer 1", Some("materials/cliff_stone.material")), &layers), 2);
        assert_eq!(match_paint_layer(&paint_layer("Path", None), &layers), 1);
        // No material at all painted the default dirt.
        assert_eq!(match_paint_layer(&paint_layer("Layer 2", None), &layers), 1);
        assert_eq!(match_paint_layer(&paint_layer("Layer 3", Some("materials/lava.material")), &layers), 1);
    }

    #[test]
    fn a_painted_mask_becomes_weight_where_it_was_painted() {
        let terrain = TerrainData { chunks_x: 1, chunks_z: 1, chunk_size: 16.0, chunk_resolution: 17, ..default() };
        // A 3 x 3 mask over the 16 m terrain, painted fully at its +x edge.
        let mut layer = PaintLayer::empty("Rock", 3);
        for z in 0..3 {
            layer.mask[z * 3 + 2] = 1.0;
        }
        let texels = paint_masks_into_chunk(&terrain, 0, 0, None, &[(&layer, 2)], 0).expect("something was painted");
        let res = weight_resolution(&terrain);
        let at = |x: u32| texels[(8 * res + x) as usize];
        // The last texel's centre is half a texel inside the painted edge, so
        // the bilinear mask there is just under one.
        assert!(at(res - 1)[2] > 230, "rock at the painted edge: {:?}", at(res - 1));
        assert_eq!(at(0), solid(0), "untouched at the far edge");
        assert!(texels.iter().all(|t| t.iter().map(|&b| b as u32).sum::<u32>() == 255));
    }
}
