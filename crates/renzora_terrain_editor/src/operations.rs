//! The terrain editor's side of [`renzora::terrain_tools`]: ground lookups,
//! sculpt strokes, the generator, erosion and terrain graph builds, callable
//! with `&mut World` by a
//! crate that cannot link this one (the MCP server).
//!
//! Each edit runs the same code the brushes and the Generate panel run, and
//! records one entry on the scene's undo stack, so an edit made this way is
//! undone with Ctrl+Z like any other.

use bevy::prelude::*;

use renzora::terrain_tools::{GroundHit, TerrainErode, TerrainGenerate, TerrainGraphBuild, TerrainSculpt, TerrainTools};
use renzora_terrain::data::{
    compute_brush_falloff, NoiseMode, StampBlendMode, TerrainBrushType, TerrainChunkData, TerrainChunkOf, TerrainData,
    TerrainSculptState, TerrainSettings,
};
use renzora_terrain::generate::TerrainGenSettings;
use renzora_terrain::sculpt;

pub(crate) fn install(app: &mut App) {
    app.insert_resource(TerrainTools {
        ground,
        sculpt: sculpt_stroke,
        generate,
        erode,
        build_graph,
        paint: crate::landscape_ops::paint,
        set_material: crate::landscape_ops::set_material,
        layers: crate::landscape_ops::layers,
    });
}

// ── Lookup ───────────────────────────────────────────────────────────────────

fn ray_down(world: &mut World, xz: Vec2) -> Option<renzora_terrain::raycast::TerrainRayHit> {
    let mut terrains = world.query::<(Entity, &TerrainData, &GlobalTransform)>();
    let mut chunks = world.query::<(Entity, &TerrainChunkData, &TerrainChunkOf)>();
    let chunk_list: Vec<_> = chunks.iter(world).collect();
    let ray = Ray3d::new(Vec3::new(xz.x, 1.0e5, xz.y), Dir3::NEG_Y);
    renzora_terrain::raycast::raycast_terrains(ray, terrains.iter(world), chunk_list)
}

fn ground(world: &mut World, xz: Vec2) -> Option<GroundHit> {
    let hit = ray_down(world, xz)?;
    // The normal from the slope across a metre either side: the heights are
    // a triangle surface, so the exact normal of one triangle would flicker
    // between neighbours' for a point on an edge.
    let h = |world: &mut World, d: Vec2| ray_down(world, xz + d).map_or(hit.point.y, |h| h.point.y);
    let (l, r) = (h(world, Vec2::new(-0.5, 0.0)), h(world, Vec2::new(0.5, 0.0)));
    let (d, u) = (h(world, Vec2::new(0.0, -0.5)), h(world, Vec2::new(0.0, 0.5)));
    let normal = Vec3::new(l - r, 1.0, d - u).normalize();
    Some(GroundHit { terrain: hit.terrain, point: hit.point, normal })
}

/// The terrain to edit: the one named, or the one under `at`.
pub(crate) fn pick_terrain(world: &mut World, named: Option<Entity>, at: Option<Vec2>) -> Result<Entity, String> {
    if let Some(e) = named {
        return world
            .get::<TerrainData>(e)
            .map(|_| e)
            .ok_or_else(|| format!("{e} is not a terrain"));
    }
    if let Some(hit) = at.and_then(|p| ray_down(world, p)) {
        return Ok(hit.terrain);
    }
    let mut q = world.query_filtered::<Entity, With<TerrainData>>();
    let all: Vec<Entity> = q.iter(world).collect();
    match all.as_slice() {
        [one] => Ok(*one),
        [] => Err("there is no terrain in the scene; spawn_preset terrain makes one".into()),
        _ => Err("there are several terrains and the point is on none of them; name one".into()),
    }
}

/// World `x`/`z` to the terrain's local metres (0..total width).
pub(crate) fn to_local(world: &World, terrain: Entity, xz: Vec2) -> Vec2 {
    let data = world.get::<TerrainData>(terrain);
    let pos = world.get::<GlobalTransform>(terrain).map(|t| t.translation()).unwrap_or_default();
    let half = data.map_or(Vec2::ZERO, |d| Vec2::new(d.total_width(), d.total_depth()) / 2.0);
    xz - pos.xz() + half
}

// ── Undo ─────────────────────────────────────────────────────────────────────

/// Base heights of one terrain's chunks, by chunk entity.
#[derive(Clone)]
struct Heights(Vec<(Entity, Vec<f32>)>);

fn owned_chunks(world: &mut World, terrain: Entity) -> Vec<Entity> {
    let mut q = world.query::<(Entity, &TerrainChunkOf)>();
    q.iter(world).filter(|(_, of)| of.0 == terrain).map(|(e, _)| e).collect()
}

fn snapshot(world: &World, chunks: &[Entity]) -> Heights {
    Heights(
        chunks
            .iter()
            .filter_map(|e| world.get::<TerrainChunkData>(*e).map(|c| (*e, c.base_heights.clone())))
            .collect(),
    )
}

fn restore(world: &mut World, entry: &Heights) {
    for (entity, heights) in &entry.0 {
        if let Some(mut chunk) = world.get_mut::<TerrainChunkData>(*entity) {
            chunk.base_heights = heights.clone();
            chunk.dirty = true;
        }
    }
}

/// Run `edit` over the terrain's chunks as one undo step labelled `label`.
/// Returns whether anything changed.
fn edit_chunks(
    world: &mut World,
    terrain: Entity,
    label: &str,
    edit: impl FnOnce(&TerrainData, &mut [&mut TerrainChunkData]),
) -> Result<bool, String> {
    let data = world.get::<TerrainData>(terrain).cloned().ok_or("not a terrain")?;
    let owned = owned_chunks(world, terrain);
    let before = snapshot(world, &owned);
    {
        let mut q = world.query::<(Entity, &mut TerrainChunkData)>();
        let mut chunks: Vec<&mut TerrainChunkData> = q
            .iter_mut(world)
            .filter(|(e, _)| owned.contains(e))
            .map(|(_, c)| c.into_inner())
            .collect();
        edit(&data, &mut chunks);
    }
    let after = snapshot(world, &owned);
    let changed = before.0.iter().zip(&after.0).any(|(a, b)| a.1 != b.1);
    if changed {
        renzora_undo::record(
            world,
            renzora_undo::UndoContext::Scene,
            Box::new(renzora_undo::SnapshotCmd {
                label: label.to_string(),
                before,
                after,
                restore,
                merge_key: None,
            }),
        );
        renzora_undo::seal(world, &renzora_undo::UndoContext::Scene);
    }
    Ok(changed)
}

// ── Sculpt ───────────────────────────────────────────────────────────────────

fn brush_named(name: &str) -> Option<TerrainBrushType> {
    let wanted = name.to_ascii_lowercase().replace([' ', '-'], "_");
    Some(match wanted.as_str() {
        "raise" => TerrainBrushType::Raise,
        "lower" => TerrainBrushType::Lower,
        "sculpt" => TerrainBrushType::Sculpt,
        "smooth" => TerrainBrushType::Smooth,
        "flatten" => TerrainBrushType::Flatten,
        "set_height" | "seth" => TerrainBrushType::SetHeight,
        "erase" => TerrainBrushType::Erase,
        "noise" => TerrainBrushType::Noise,
        "erosion" | "thermal" => TerrainBrushType::Erosion,
        "hydro" | "hydraulic" => TerrainBrushType::Hydro,
        "terrace" => TerrainBrushType::Terrace,
        "pinch" => TerrainBrushType::Pinch,
        "relax" => TerrainBrushType::Relax,
        "cliff" => TerrainBrushType::Cliff,
        "retop" => TerrainBrushType::Retop,
        "ramp" => TerrainBrushType::Ramp,
        _ => return None,
    })
}

/// Dabs every quarter radius along the path, so a stroke is as even as a
/// dragged brush however far apart its points are.
pub(crate) fn dabs(path: &[Vec2], radius: f32) -> Vec<Vec2> {
    let step = (radius * 0.25).max(0.25);
    let mut out = vec![path[0]];
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let n = (a.distance(b) / step).ceil().max(1.0) as usize;
        out.extend((1..=n).map(|i| a.lerp(b, i as f32 / n as f32)));
    }
    out
}

/// The largest summed falloff weight any dab centre receives from all the
/// dabs: how many dabs' worth of change the most-brushed point of the
/// stroke takes. One for a single dab; about 5.6 along a long straight
/// stroke at the default falloff; more where a path doubles back.
fn peak_overlap(points: &[Vec2], radius: f32, settings: &TerrainSettings) -> f32 {
    points
        .iter()
        .map(|p| {
            points
                .iter()
                .map(|q| compute_brush_falloff(p.distance(*q) / radius, settings.falloff, settings.falloff_type))
                .sum::<f32>()
        })
        .fold(1.0, f32::max)
}

fn sculpt_stroke(world: &mut World, stroke: &TerrainSculpt) -> Result<String, String> {
    let brush = brush_named(&stroke.brush).ok_or_else(|| {
        format!(
            "no brush called {}; use raise, lower, smooth, flatten, set_height, noise, terrace, erosion, hydro, relax, pinch, ramp, retop, cliff or erase",
            stroke.brush
        )
    })?;
    if stroke.path.is_empty() {
        return Err("the stroke needs at least one point".into());
    }
    let radius = stroke.radius.clamp(0.5, 2000.0);
    let terrain = pick_terrain(world, stroke.terrain, stroke.path.first().copied())?;
    let data = world.get::<TerrainData>(terrain).cloned().ok_or("not a terrain")?;
    let local: Vec<Vec2> = stroke.path.iter().map(|p| to_local(world, terrain, *p)).collect();
    let points = dabs(&local, radius);

    // Flatten, set height and ramp aim at a height: the one given, or the
    // ground under the first point, in the terrain's normalized units.
    let terrain_y = world.get::<GlobalTransform>(terrain).map_or(0.0, |t| t.translation().y);
    let target_world = match stroke.target_height {
        Some(h) => Some(h),
        None => ground(world, stroke.path[0]).map(|g| g.point.y),
    };
    let normalized = |h: f32| ((h - terrain_y - data.min_height) / data.height_range()).clamp(0.0, 1.0);
    let target = target_world.map(normalized);

    // A ramp runs from that height to the ground under the last point. The
    // Ramp brush has no end point of its own (it pulls toward the stroke's
    // start, harder at the brush centre, which flattened instead of sloping),
    // so a ramp here is a flatten whose target slides along the path.
    let ramp_end = (brush == TerrainBrushType::Ramp)
        .then(|| stroke.path.last().and_then(|p| ground(world, *p)))
        .flatten()
        .map(|g| normalized(g.point.y));
    let brush = if ramp_end.is_some() { TerrainBrushType::Flatten } else { brush };
    let along: Vec<f32> = {
        let mut run = 0.0;
        let mut out = vec![0.0];
        for pair in points.windows(2) {
            run += pair[0].distance(pair[1]);
            out.push(run);
        }
        out.iter().map(|d| if run > 0.0 { d / run } else { 0.0 }).collect()
    };

    // Raise, lower, sculpt and noise move by metres: each dab moves a vertex
    // `2 * dt * falloff` metres, and the dabs overlap, so dt is the asked-for
    // change divided by the most overlap any point of the stroke gets. That
    // overlap was once taken to be four everywhere, which overshot a long
    // stroke by half again (eight dabs reach a point, not four) and made a
    // single dab a quarter as strong as asked. The rest blend toward a result
    // by a fraction per dab.
    let metres = matches!(
        brush,
        TerrainBrushType::Raise | TerrainBrushType::Lower | TerrainBrushType::Sculpt | TerrainBrushType::Noise
    );
    let mut settings = TerrainSettings {
        brush_type: brush,
        brush_radius: radius,
        falloff: stroke.falloff.clamp(0.0, 1.0),
        target_height: target.unwrap_or(0.5),
        ..default()
    };
    let dt = if metres {
        settings.brush_strength = 1.0;
        stroke.strength.abs() / 2.0 / peak_overlap(&points, radius, &settings)
    } else {
        settings.brush_strength = stroke.strength.clamp(0.0, 1.0);
        0.5
    };
    let shift = metres && stroke.strength < 0.0;
    let label = match ramp_end {
        Some(_) => "Sculpt: Ramp".to_string(),
        None => format!("Sculpt: {}", brush.display_name()),
    };
    let seed = points.len() as u32;

    let changed = edit_chunks(world, terrain, &label, |data, chunks| {
        for (i, p) in points.iter().enumerate() {
            match brush {
                TerrainBrushType::Erosion | TerrainBrushType::Hydro => {
                    let (droplets, thermal) = if brush == TerrainBrushType::Hydro {
                        ((settings.brush_strength * 3000.0) as u32 + 1, 0)
                    } else {
                        (0, (settings.brush_strength * 30.0).ceil() as u32)
                    };
                    renzora_terrain::erosion::erode(data, chunks, Some((*p, radius)), droplets, thermal, 34.0, seed + i as u32);
                }
                _ => {
                    let state = TerrainSculptState {
                        flatten_start_height: match (ramp_end, target) {
                            (Some(end), Some(start)) => Some(start + (end - start) * along[i]),
                            _ => target,
                        },
                        ..default()
                    };
                    sculpt::apply_brush_dab(data, chunks, &settings, &state, p.x, p.y, dt, shift);
                }
            }
        }
    })?;
    let name = if ramp_end.is_some() { "Ramp" } else { brush.display_name() };
    Ok(if changed {
        format!("{name} stroke of {} dabs, radius {radius} m, on {terrain}", points.len())
    } else {
        "the stroke changed nothing (off the terrain, or already at its target)".into()
    })
}

// ── Generate ─────────────────────────────────────────────────────────────────

fn generate(world: &mut World, request: &TerrainGenerate) -> Result<String, String> {
    let at = request.region.map(|(a, b)| (a + b) / 2.0);
    let terrain = pick_terrain(world, request.terrain, at)?;
    let previous = world.get_resource::<TerrainGenSettings>().cloned().unwrap_or_default();
    let mut settings = previous.clone();
    if let Some(name) = &request.noise {
        settings.mode = match name.to_ascii_lowercase().as_str() {
            "fbm" => NoiseMode::Fbm,
            "ridge" | "ridged" => NoiseMode::Ridge,
            "billow" => NoiseMode::Billow,
            "warped" => NoiseMode::Warped,
            "hybrid" => NoiseMode::Hybrid,
            "eroded" | "alpine" => NoiseMode::Eroded,
            other => return Err(format!("no noise called {other}; use eroded, fbm, ridged, billow, warped or hybrid")),
        };
    }
    if let Some(blend) = &request.blend {
        settings.blend = match blend.to_ascii_lowercase().as_str() {
            "replace" => StampBlendMode::Replace,
            "add" => StampBlendMode::Add,
            "subtract" => StampBlendMode::Subtract,
            "max" => StampBlendMode::Max,
            "min" => StampBlendMode::Min,
            other => return Err(format!("no blend called {other}; use replace, add, subtract, max or min")),
        };
    }
    let terrain_y = world.get::<GlobalTransform>(terrain).map_or(0.0, |t| t.translation().y);
    settings.scale = request.scale.unwrap_or(settings.scale).max(1.0);
    settings.height = request.height.unwrap_or(settings.height);
    settings.base = request.base.map_or(settings.base, |b| b - terrain_y);
    settings.octaves = request.octaves.unwrap_or(settings.octaves).clamp(1, 10);
    settings.exponent = request.exponent.unwrap_or(settings.exponent);
    settings.seed = request.seed.unwrap_or(settings.seed);
    settings.feather = request.feather.unwrap_or(settings.feather).clamp(0.0, 1.0);
    settings.source = renzora_terrain::generate::GenSource::Noise;
    match request.region {
        Some((a, b)) => {
            let (a, b) = (to_local(world, terrain, a), to_local(world, terrain, b));
            settings.whole_terrain = false;
            settings.region_min = a.min(b);
            settings.region_max = a.max(b);
        }
        None => settings.whole_terrain = true,
    }

    // `generate_now` reads the panel's settings; run it with these, then put
    // the panel's back so a call does not rearrange what the user had set.
    world.insert_resource(settings.clone());
    crate::generate_tool::generate_now(world, terrain);
    world.insert_resource(previous);
    Ok(format!(
        "generated {:?} noise on {terrain}: scale {} m, height {} m, seed {}",
        settings.mode, settings.scale, settings.height, settings.seed
    ))
}

// ── Erode ────────────────────────────────────────────────────────────────────

fn erode(world: &mut World, request: &TerrainErode) -> Result<String, String> {
    let terrain = pick_terrain(world, request.terrain, request.area.map(|a| a.0))?;
    let area = request.area.map(|(c, r)| (to_local(world, terrain, c), r.max(1.0)));
    let mut moved = 0;
    let changed = edit_chunks(world, terrain, "Erode Terrain", |data, chunks| {
        moved = renzora_terrain::erosion::erode(
            data,
            chunks,
            area,
            request.droplets,
            request.thermal_iterations,
            request.repose_degrees,
            request.seed,
        );
    })?;
    Ok(if changed {
        format!("eroded {terrain}: {} raindrops, {} weathering passes, {moved} vertices moved", request.droplets, request.thermal_iterations)
    } else {
        "erosion changed nothing".into()
    })
}

// ── Terrain graph ────────────────────────────────────────────────────────────

/// Build a terrain graph into a terrain on this thread, storing the graph
/// and seed asked for on the terrain's generator first so the saved scene
/// (and the next build) uses them. The editor's Build button runs the same
/// commit from a background task instead.
fn build_graph(world: &mut World, request: &TerrainGraphBuild) -> Result<String, String> {
    use renzora_terrain_graph::TerrainGenerator;
    let terrain = pick_terrain(world, request.terrain, None)?;
    let mut generator = world.get::<TerrainGenerator>(terrain).cloned().unwrap_or_default();
    if let Some(graph) = &request.graph {
        generator.graph = graph.clone();
    }
    if let Some(seed) = request.seed {
        generator.seed = seed;
    }
    if generator.graph.is_empty() {
        return Err("the terrain has no graph yet; name a .tgraph".into());
    }
    if world.get::<TerrainGenerator>(terrain) != Some(&generator) {
        world.entity_mut(terrain).insert(generator.clone());
    }
    let summary = renzora_terrain_graph::world::build_now(world, terrain)?;
    Ok(format!("{} on {terrain}: {summary}", generator.graph))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_brush_the_sculpt_tab_lists_has_a_name() {
        for brush in TerrainBrushType::all() {
            if *brush == TerrainBrushType::Stamp {
                continue;
            }
            assert_eq!(brush_named(brush.display_name()), Some(*brush), "{}", brush.display_name());
        }
        assert_eq!(brush_named("nope"), None);
    }

    #[test]
    fn dabs_are_spaced_a_quarter_radius_apart() {
        let d = dabs(&[Vec2::ZERO, Vec2::new(10.0, 0.0)], 8.0);
        assert_eq!(d.len(), 6);
        assert!(d.windows(2).all(|w| w[0].distance(w[1]) <= 2.0 + 1e-4));
        assert_eq!(dabs(&[Vec2::ONE], 4.0), vec![Vec2::ONE]);
    }

    #[test]
    fn overlap_is_one_for_a_dab_and_about_five_and_a_half_along_a_stroke() {
        let settings = TerrainSettings { falloff: 0.6, ..default() };
        assert_eq!(peak_overlap(&dabs(&[Vec2::ZERO], 20.0), 20.0, &settings), 1.0);
        let long = dabs(&[Vec2::ZERO, Vec2::new(400.0, 0.0)], 20.0);
        let overlap = peak_overlap(&long, 20.0, &settings);
        assert!((5.0..6.2).contains(&overlap), "{overlap}");
    }
}
