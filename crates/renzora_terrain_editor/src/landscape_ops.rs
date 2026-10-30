//! The landscape half of [`renzora::terrain_tools`]: painting layer weights,
//! choosing a terrain's material and listing its layers, for a caller that
//! cannot link the terrain crates (the MCP server).
//!
//! Painting runs the same brush the Paint tab does
//! (`renzora_terrain::landscape::weights::apply_weight_dab`), dab by dab along
//! the path, and records one undo entry for the whole stroke.

use bevy::prelude::*;

use renzora::terrain_tools::{TerrainLayerInfo, TerrainPaint};
use renzora_shader::material::landscape_preset::ALPINE_LANDSCAPE_PATH;
use renzora_terrain::data::{BrushFalloffType, BrushShape, TerrainChunkData, TerrainChunkOf, TerrainData};
use renzora_terrain::landscape::weights::{apply_weight_dab, weight_resolution, Texel, WeightTarget};
use renzora_terrain::landscape::{migrate, LandscapeStatus, TerrainChunkWeights, TerrainLandscape, TerrainMaterial, WeightDab};
use renzora_terrain::paint::{PaintBrushType, PAINT_RADIUS_RANGE};

use crate::operations::{dabs, pick_terrain, to_local};

fn mode_named(name: &str) -> Option<PaintBrushType> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "" | "paint" => PaintBrushType::Paint,
        "erase" => PaintBrushType::Erase,
        "smooth" | "blur" => PaintBrushType::Smooth,
        "fill" => PaintBrushType::Fill,
        _ => return None,
    })
}

/// Layer weights of chunks, by entity: an undo payload.
#[derive(Clone)]
struct Weights(Vec<(Entity, u32, Vec<Texel>)>);

fn snapshot(world: &World, chunks: &[Entity]) -> Weights {
    Weights(
        chunks
            .iter()
            .filter_map(|e| world.get::<TerrainChunkWeights>(*e).map(|w| (*e, w.resolution, w.texels.clone())))
            .collect(),
    )
}

fn restore(world: &mut World, entry: &Weights) {
    for (entity, res, texels) in &entry.0 {
        if let Some(mut weights) = world.get_mut::<TerrainChunkWeights>(*entity) {
            let res = (*res).max(weights.resolution).max(1);
            let texels = if texels.is_empty() {
                vec![renzora_terrain::landscape::weights::solid(0); (res * res) as usize]
            } else {
                texels.clone()
            };
            weights.set_texels(res, texels);
        }
    }
}

pub(crate) fn paint(world: &mut World, request: &TerrainPaint) -> Result<String, String> {
    let mode = mode_named(&request.mode)
        .ok_or_else(|| format!("no paint mode called {}; use paint, erase, smooth or fill", request.mode))?;
    if request.path.is_empty() {
        return Err("the stroke needs at least one point".into());
    }
    let terrain = pick_terrain(world, request.terrain, request.path.first().copied())?;
    let landscape = world
        .get::<TerrainLandscape>(terrain)
        .cloned()
        .ok_or("the terrain has no landscape material yet; give it one first")?;
    if landscape.layers.is_empty() {
        return Err(match landscape.status {
            LandscapeStatus::Loading => "the terrain's material is still loading; try again in a moment".into(),
            LandscapeStatus::Failed(why) => why,
            _ => "the terrain's material has no layers to paint".into(),
        });
    }
    let layer = landscape
        .layers
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(request.layer.trim()))
        .ok_or_else(|| {
            let names: Vec<&str> = landscape.layers.iter().map(|l| l.name.as_str()).collect();
            format!("the material has no layer called {}; it has {}", request.layer, names.join(", "))
        })?;
    let data = world.get::<TerrainData>(terrain).cloned().ok_or("not a terrain")?;
    let root_y = world.get::<GlobalTransform>(terrain).map_or(0.0, |t| t.translation().y);
    let radius = request.radius.clamp(PAINT_RADIUS_RANGE.0, PAINT_RADIUS_RANGE.1);
    let local: Vec<Vec2> = request.path.iter().map(|p| to_local(world, terrain, *p)).collect();
    let points = dabs(&local, radius);

    // Every chunk of the terrain gets weights to paint into, even on the frame
    // its material was given.
    let mut q = world.query_filtered::<(Entity, &TerrainChunkOf), With<TerrainChunkData>>();
    let owned: Vec<Entity> = q.iter(world).filter(|(_, of)| of.0 == terrain).map(|(e, _)| e).collect();
    let res = weight_resolution(&data);
    for &chunk in &owned {
        match world.get_mut::<TerrainChunkWeights>(chunk) {
            Some(mut w) => {
                w.ensure_decoded(res);
            }
            None => {
                world.entity_mut(chunk).insert(TerrainChunkWeights::default());
            }
        }
    }

    let before = snapshot(world, &owned);
    let mut changed = false;
    {
        let mut q = world.query::<(Entity, &TerrainChunkData, &mut TerrainChunkWeights)>();
        let mut targets: Vec<WeightTarget> = q
            .iter_mut(world)
            .filter(|(e, _, _)| owned.contains(e))
            .map(|(_, chunk, weights)| WeightTarget { chunk, weights: weights.into_inner() })
            .collect();
        for p in &points {
            let dab = WeightDab {
                mode,
                center: *p,
                radius,
                amount: request.strength.clamp(0.0, 1.0),
                target: request.target.unwrap_or(1.0).clamp(0.0, 1.0),
                falloff: request.falloff.clamp(0.0, 1.0),
                falloff_type: BrushFalloffType::Smooth,
                shape: BrushShape::Circle,
                channel: layer.channel,
                base: landscape.base_channel(),
                slope_mask: request.slope_range,
                height_mask: request.height_range,
                root_y,
            };
            changed |= apply_weight_dab(&data, &mut targets, &dab);
        }
    }
    if !changed {
        return Ok("the stroke changed nothing (off the terrain, outside its masks, or already at its target)".into());
    }
    let after = snapshot(world, &owned);
    renzora_undo::record(
        world,
        renzora_undo::UndoContext::Scene,
        Box::new(renzora_undo::SnapshotCmd {
            label: format!("Paint Terrain: {}", layer.name),
            before,
            after,
            restore,
            merge_key: None,
        }),
    );
    renzora_undo::seal(world, &renzora_undo::UndoContext::Scene);
    Ok(format!(
        "painted {} with {} dabs of radius {radius} m on {terrain}",
        layer.name,
        points.len()
    ))
}

#[derive(Clone)]
struct MaterialChoice(Entity, Option<TerrainMaterial>);

fn restore_material(world: &mut World, entry: &MaterialChoice) {
    let Ok(mut e) = world.get_entity_mut(entry.0) else { return };
    match &entry.1 {
        Some(m) => {
            e.insert(m.clone());
        }
        None => {
            e.remove::<TerrainMaterial>();
        }
    }
}

pub(crate) fn set_material(world: &mut World, terrain: Option<Entity>, path: &str) -> Result<String, String> {
    let terrain = pick_terrain(world, terrain, None)?;
    let path = if path.trim().is_empty() { ALPINE_LANDSCAPE_PATH } else { path.trim() };
    if !path.ends_with(".material") {
        return Err(format!("{path} is not a .material"));
    }
    let root = world.get_resource::<renzora::CurrentProject>().map(|p| p.path.clone());
    if path == ALPINE_LANDSCAPE_PATH {
        if let Some(root) = &root {
            migrate::ensure_alpine_material(root)?;
        }
    } else if let Some(root) = &root {
        if !root.join(path).is_file() {
            return Err(format!("no file at {path} in the project"));
        }
    }
    let before = MaterialChoice(terrain, world.get::<TerrainMaterial>(terrain).cloned());
    renzora_terrain::landscape::assign_material(world, terrain, path)?;
    let after = MaterialChoice(terrain, world.get::<TerrainMaterial>(terrain).cloned());
    renzora_undo::record(
        world,
        renzora_undo::UndoContext::Scene,
        Box::new(renzora_undo::SnapshotCmd {
            label: "Terrain Material".to_string(),
            before,
            after,
            restore: restore_material,
            merge_key: None,
        }),
    );
    renzora_undo::seal(world, &renzora_undo::UndoContext::Scene);
    Ok(format!("{terrain} now wears {path}; its layers load over the next frames"))
}

pub(crate) fn layers(world: &mut World, terrain: Option<Entity>) -> Result<Vec<TerrainLayerInfo>, String> {
    let terrain = pick_terrain(world, terrain, None)?;
    let landscape = world
        .get::<TerrainLandscape>(terrain)
        .ok_or("the terrain has no landscape material yet; give it one first")?;
    match &landscape.status {
        LandscapeStatus::Ready => Ok(landscape
            .layers
            .iter()
            .map(|l| TerrainLayerInfo { name: l.name.clone(), blend: l.blend.display_name().to_string(), coverage: l.coverage })
            .collect()),
        LandscapeStatus::Loading => Err("the terrain's material is still loading".into()),
        LandscapeStatus::Failed(why) => Err(why.clone()),
        LandscapeStatus::Plain => Ok(Vec::new()),
        LandscapeStatus::None => Err("the terrain has no landscape material yet; give it one first".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_paint_mode_has_a_name() {
        for (name, mode) in [
            ("paint", PaintBrushType::Paint),
            ("Erase", PaintBrushType::Erase),
            ("smooth", PaintBrushType::Smooth),
            ("fill", PaintBrushType::Fill),
            ("", PaintBrushType::Paint),
        ] {
            assert_eq!(mode_named(name), Some(mode), "{name}");
        }
        assert_eq!(mode_named("spray"), None);
    }
}
