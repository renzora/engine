//! Blade scatter — turns a chunk's painted density mask into the per-blade
//! instance records the grass pipeline draws.
//!
//! This decides *where the blades are*, not what they look like: the blade's
//! geometry is rebuilt from `@builtin(vertex_index)` in `grass.wgsl`, so nothing
//! here emits a vertex. Each blade costs one 48-byte [`GrassInstance`].

use bevy::prelude::*;

use super::data::{FoliageDensityMap, FoliageType};
use super::instance::GrassInstance;

/// Painted weight at which a texel scatters at full density.
///
/// The brush walks a texel towards 1.0 over the frames the cursor spends on it,
/// so a moving stroke never gets near 1.0 — the ramp has to top out early or
/// ordinary painting would produce thin grass. The brush's accumulation rate is
/// tuned against this number (see `foliage_paint_system`), so the two move
/// together.
const FULL_COVERAGE_WEIGHT: f32 = 0.25;
/// Weights below this are treated as unpainted ground.
///
/// Deliberately small. Coverage is *proportional* to the painted weight with no
/// floor under it, which is what keeps a patch inside the brush: bilinear
/// sampling bleeds a texel's weight a texel outward, and a floor turned that
/// bleed into a ring of half-density grass sitting outside the gizmo.
const MIN_PAINTED_WEIGHT: f32 = 0.004;

/// Most blades one chunk may scatter.
///
/// Still a ceiling, but a far higher one than the baked-mesh scatter could
/// afford: a blade is 48 bytes of instance data rather than ~560 bytes of
/// vertices, so this is ~96 MB per chunk at the limit instead of well over a
/// gigabyte. Two million covers a 64 m chunk painted wall to wall at ~490
/// blades/m², which is denser than the defaults ask for — the budget is a
/// backstop against a pathological config, not a limit you should meet in
/// normal use. Past it the scatter thins uniformly, so density degrades rather
/// than frame time.
pub const MAX_BLADES_PER_CHUNK: usize = 2_000_000;

/// Deterministic hash for reproducible random from grid position.
fn hash_pos(x: u32, z: u32, seed: u32) -> f32 {
    let mut h = x
        .wrapping_mul(2654435761)
        .wrapping_add(z.wrapping_mul(2246822519))
        .wrapping_add(seed);
    h ^= h >> 13;
    h = h.wrapping_mul(1274126177);
    h ^= h >> 16;
    (h as f32) / (u32::MAX as f32)
}

/// What one pass over the paint mask tells the scatter loop.
struct MaskSurvey {
    /// Painted extent in UV, already grown by the texel that bilinear sampling
    /// reaches into.
    min_uv: Vec2,
    max_uv: Vec2,
    /// Σ of per-texel coverage — the painted area in texels, weighted by how
    /// densely each one scatters. Times the world area of a texel, times blades
    /// per square metre, this is how many blades the mask is about to ask for.
    coverage_texels: f32,
}

/// Survey the paint mask for one foliage type.
///
/// The scatter grid has ~200k cells across a 64 m chunk and a stroke usually
/// touches a corner of it, so walking the whole grid to find that out is the
/// wrong way round: the mask has far fewer texels, and one pass over it yields
/// both where to scan and how much to expect. Returns `None` if nothing is
/// painted.
fn survey_mask(density_map: &FoliageDensityMap, type_index: usize) -> Option<MaskSurvey> {
    let res = density_map.resolution;
    if res == 0 || type_index >= super::data::MAX_FOLIAGE_TYPES {
        return None;
    }
    let (mut min_x, mut min_z, mut max_x, mut max_z) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut coverage_texels = 0.0f32;
    for z in 0..res {
        for x in 0..res {
            // Indexed through `get`: a hand-edited or truncated scene can hand
            // us a weight vector that doesn't match `resolution`, and a survey
            // is not the place to panic over it.
            let Some(w) = density_map
                .density_weights
                .get((z * res + x) as usize)
                .map(|t| t[type_index])
            else {
                continue;
            };
            if w < MIN_PAINTED_WEIGHT {
                continue;
            }
            min_x = min_x.min(x);
            min_z = min_z.min(z);
            max_x = max_x.max(x);
            max_z = max_z.max(z);
            coverage_texels += (w / FULL_COVERAGE_WEIGHT).min(1.0);
        }
    }
    if min_x == u32::MAX {
        return None;
    }
    // Grow by one texel: a painted texel's weight bleeds that far under bilinear
    // sampling, and cropping the scan at the painted texels would shave the
    // patch's own soft edge off.
    let last = (res - 1) as f32;
    let min_uv = Vec2::new(
        min_x.saturating_sub(1) as f32 / last,
        min_z.saturating_sub(1) as f32 / last,
    );
    let max_uv = Vec2::new(
        (max_x + 1).min(res - 1) as f32 / last,
        (max_z + 1).min(res - 1) as f32 / last,
    );
    Some(MaskSurvey {
        min_uv,
        max_uv,
        coverage_texels,
    })
}

/// Scatter one chunk's blades for one foliage type.
///
/// Returns the instance records, in chunk-local space. `None` when the type is
/// off or nothing is painted — which is the common case per chunk, and the
/// reason the mask survey runs before anything else.
///
/// # Arguments
/// - `foliage_type`: Configuration (density, height/width ranges, etc.)
/// - `type_index`: Which foliage type slot in the density map.
/// - `density_map`: Per-chunk density weights.
/// - `heights`: Terrain chunk heightmap (normalized 0..1).
/// - `chunk_resolution`: Heightmap vertices per side.
/// - `chunk_size`: World-space size of the chunk.
/// - `min_height`: Terrain minimum height.
/// - `height_range`: max_height - min_height.
/// - `seed`: Deterministic seed for this chunk.
pub fn scatter_foliage_chunk(
    foliage_type: &FoliageType,
    type_index: usize,
    density_map: &FoliageDensityMap,
    heights: &[f32],
    chunk_resolution: u32,
    chunk_size: f32,
    min_height: f32,
    height_range: f32,
    seed: u32,
) -> Option<Vec<GrassInstance>> {
    if !foliage_type.enabled || foliage_type.density <= 0.0 {
        return None;
    }

    // `density` is the *scatter grid* — one cell per clump, not per blade. The
    // clump is what closes the gaps between cells: a lone blade on a 0.18 m grid
    // reads as scattered spikes however the blade itself is shaped, because real
    // grass grows in tufts rather than on a lattice.
    let per_clump = foliage_type.blades_per_clump.clamp(1, 16);
    let spacing = 1.0 / foliage_type.density.sqrt();
    let grid_count = (chunk_size / spacing).ceil() as u32;

    let survey = survey_mask(density_map, type_index)?;

    // Scan only the painted extent. Grid cells outside it can't produce a blade,
    // and at the default density there are ~200k of them per chunk to skip.
    let cell_range = |from: f32, to: f32| -> (u32, u32) {
        let lo = ((from * chunk_size / spacing).floor().max(0.0) as u32).min(grid_count);
        let hi = (((to * chunk_size / spacing).ceil() as u32) + 1).min(grid_count);
        (lo, hi)
    };
    let (gx_lo, gx_hi) = cell_range(survey.min_uv.x, survey.max_uv.x);
    let (gz_lo, gz_hi) = cell_range(survey.min_uv.y, survey.max_uv.y);

    // How many blades the mask is asking for, and how hard to thin if that is
    // more than a single mesh should carry. Estimated from the survey rather
    // than counted in a first pass over the grid — the estimate is unbiased and
    // a counting pass would double the cost of the whole generator.
    let texel_area = (chunk_size / density_map.resolution as f32).powi(2);
    let expected_blades =
        survey.coverage_texels * texel_area * foliage_type.density * per_clump as f32;
    let budget_keep = if expected_blades > MAX_BLADES_PER_CHUNK as f32 {
        MAX_BLADES_PER_CHUNK as f32 / expected_blades
    } else {
        1.0
    };

    let est_blades = (expected_blades.max(0.0) as usize).min(MAX_BLADES_PER_CHUNK);
    let mut blades: Vec<GrassInstance> = Vec::with_capacity(est_blades);

    let vert_spacing = chunk_size / (chunk_resolution - 1) as f32;

    for gz in gz_lo..gz_hi {
        for gx in gx_lo..gx_hi {
            let cell_seed = seed.wrapping_add(gx * 7919 + gz * 6271);

            for blade in 0..per_clump {
                // Each blade in a clump gets its own decorrelated stream, so the
                // clump is a tuft of independent blades rather than a stack of
                // identical ones.
                let seed_val = cell_seed.wrapping_add(blade.wrapping_mul(104_729));

                // Grid position with jitter
                let jitter_x = hash_pos(gx, gz, seed_val) - 0.5;
                let jitter_z = hash_pos(gz, gx, seed_val.wrapping_add(1)) - 0.5;
                let local_x = (gx as f32 + 0.5 + jitter_x * 0.9) * spacing;
                let local_z = (gz as f32 + 0.5 + jitter_z * 0.9) * spacing;

                if local_x < 0.0 || local_x >= chunk_size || local_z < 0.0 || local_z >= chunk_size
                {
                    continue;
                }

                // Sample density weight from the painted density map
                let uv_x = local_x / chunk_size;
                let uv_z = local_z / chunk_size;
                let weight = density_map.sample(uv_x, uv_z, type_index);
                if weight < MIN_PAINTED_WEIGHT {
                    continue;
                }

                // Coverage is proportional to how much paint is here, all the way
                // down to nothing. That is what makes a patch stop where the brush
                // stopped: the brush's own falloff leaves a weight gradient at the
                // rim, and this turns it into a density gradient instead of a
                // circle with a hard edge somewhere past the gizmo.
                let coverage = (weight / FULL_COVERAGE_WEIGHT).min(1.0);
                if hash_pos(gx * 83, gz * 89, seed_val.wrapping_add(9)) >= coverage {
                    continue;
                }

                // Chunk-wide budget. Applied per blade and uniformly, so thinning
                // costs density evenly rather than clipping the mesh at whichever
                // corner the loop happened to fill it from.
                if budget_keep < 1.0
                    && hash_pos(gx * 97, gz * 101, seed_val.wrapping_add(10)) >= budget_keep
                {
                    continue;
                }

                // Bilinear height interpolation
                let fx = local_x / vert_spacing;
                let fz = local_z / vert_spacing;
                let vx0 = (fx.floor() as u32).min(chunk_resolution - 1);
                let vz0 = (fz.floor() as u32).min(chunk_resolution - 1);
                let vx1 = (vx0 + 1).min(chunk_resolution - 1);
                let vz1 = (vz0 + 1).min(chunk_resolution - 1);
                let tx = fx.fract();
                let tz = fz.fract();

                let get_h = |x: u32, z: u32| -> f32 {
                    heights
                        .get((z * chunk_resolution + x) as usize)
                        .copied()
                        .unwrap_or(0.0)
                };
                let h_norm = get_h(vx0, vz0) * (1.0 - tx) * (1.0 - tz)
                    + get_h(vx1, vz0) * tx * (1.0 - tz)
                    + get_h(vx0, vz1) * (1.0 - tx) * tz
                    + get_h(vx1, vz1) * tx * tz;
                let y = min_height + h_norm * height_range;

                // Per-blade random attributes
                let h_rand = hash_pos(gx * 13, gz * 17, seed_val.wrapping_add(2));
                let blade_height = foliage_type.height_range.x
                    + (foliage_type.height_range.y - foliage_type.height_range.x) * h_rand;

                let w_rand = hash_pos(gx * 23, gz * 29, seed_val.wrapping_add(3));
                let blade_width = foliage_type.width_range.x
                    + (foliage_type.width_range.y - foliage_type.width_range.x) * w_rand;

                let phase =
                    hash_pos(gx * 37, gz * 41, seed_val.wrapping_add(4)) * std::f32::consts::TAU;

                let bend = ((blade_height - foliage_type.height_range.x)
                    / (foliage_type.height_range.y - foliage_type.height_range.x).max(0.01)
                    * 0.7
                    + hash_pos(gx * 47, gz * 53, seed_val.wrapping_add(6)).abs() * 0.3)
                    .clamp(0.0, 1.0);

                let lean_x = (hash_pos(gx * 59, gz * 61, seed_val.wrapping_add(7)) - 0.5) * 0.06;
                let lean_z = (hash_pos(gx * 67, gz * 71, seed_val.wrapping_add(8)) - 0.5) * 0.06;

                let color_var = (phase * 3.7).sin() * 0.12;

                // Y-axis rotation, stored resolved: the vertex shader would
                // otherwise run these two trig calls for every vertex of every
                // blade, and they are constant across the blade.
                let angle = phase * 2.5;

                blades.push(GrassInstance {
                    position_height: [local_x, y, local_z, blade_height],
                    width_phase_bend_var: [blade_width, phase, bend, color_var],
                    lean_rotation: [lean_x, lean_z, angle.sin(), angle.cos()],
                });
            }
        }
    }

    if blades.is_empty() {
        return None;
    }

    Some(blades)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted_map(res: u32, weight: f32) -> FoliageDensityMap {
        let mut m = FoliageDensityMap::new(res);
        for w in &mut m.density_weights {
            w[0] = weight;
        }
        m
    }

    fn gen(ft: &FoliageType, map: &FoliageDensityMap) -> Option<Vec<GrassInstance>> {
        scatter_foliage_chunk(ft, 0, map, &[0.0; 9 * 9], 9, 8.0, 0.0, 1.0, 1)
    }

    fn count(ft: &FoliageType, map: &FoliageDensityMap) -> usize {
        gen(ft, map).map_or(0, |b| b.len())
    }

    fn full_chunk(ft: &FoliageType, map: &FoliageDensityMap) -> Option<Vec<GrassInstance>> {
        scatter_foliage_chunk(ft, 0, map, &vec![0.0; 129 * 129], 129, 64.0, 0.0, 1.0, 1)
    }

    /// The clump is the coverage knob: raising it has to put more blades on the
    /// ground, not just re-seed the same ones.
    #[test]
    fn clumps_multiply_blade_count() {
        let map = painted_map(16, 1.0);
        let one = FoliageType {
            blades_per_clump: 1,
            ..default()
        };
        let four = FoliageType {
            blades_per_clump: 4,
            ..default()
        };
        let a = count(&one, &map);
        let b = count(&four, &map);
        assert!(b > a * 3, "4 blades/clump gave {b} blades vs {a} for 1");
    }

    /// Width and height reach the GPU as world metres, scaled by nothing else.
    /// The width used to be multiplied by the blade height as well, which is
    /// what made the grass read as sparse spikes.
    #[test]
    fn width_and_height_reach_the_instance_unscaled() {
        let ft = FoliageType {
            blades_per_clump: 1,
            width_range: Vec2::splat(0.5),
            height_range: Vec2::splat(0.1),
            ..default()
        };
        for blade in &gen(&ft, &painted_map(16, 1.0)).unwrap() {
            assert!((blade.position_height[3] - 0.1).abs() < 1e-5, "height");
            assert!((blade.width_phase_bend_var[0] - 0.5).abs() < 1e-5, "width");
        }
    }

    /// The rotation is baked as its sine and cosine so the vertex shader doesn't
    /// run trig per vertex. A pair that isn't a unit vector would shear or
    /// resize every blade it touches.
    #[test]
    fn rotation_is_stored_as_a_unit_sin_cos_pair() {
        for blade in &gen(&FoliageType::default(), &painted_map(16, 1.0)).unwrap() {
            let (s, c) = (blade.lean_rotation[2], blade.lean_rotation[3]);
            assert!((s * s + c * c - 1.0).abs() < 1e-4, "sin/cos not normalised");
        }
    }

    /// Unpainted ground has no grass at all. The scatter must return `None` so
    /// the chunk spawns no draw call rather than an empty one.
    #[test]
    fn unpainted_chunk_scatters_nothing() {
        assert!(gen(&FoliageType::default(), &painted_map(16, 0.0)).is_none());
    }

    /// Coverage tracks the painted weight in proportion, with nothing propping
    /// it up from below. The floor that used to be here is what let a patch
    /// spill past the brush: bilinear sampling bleeds weight a texel outward,
    /// and a floor turned that bleed into a ring of half-density grass sitting
    /// outside the gizmo.
    #[test]
    fn coverage_is_proportional_to_paint() {
        let ft = FoliageType {
            blades_per_clump: 4,
            ..default()
        };
        let full = count(&ft, &painted_map(16, FULL_COVERAGE_WEIGHT)) as f32;
        let quarter = count(&ft, &painted_map(16, FULL_COVERAGE_WEIGHT * 0.25)) as f32;
        let ratio = quarter / full;
        assert!(
            (0.15..0.35).contains(&ratio),
            "quarter the paint gave {ratio:.2} of the blades, expected ~0.25"
        );
    }

    /// Past `FULL_COVERAGE_WEIGHT` more paint changes nothing — the brush never
    /// gets a texel near 1.0 during a moving stroke, so the ramp has to top out
    /// well before it.
    #[test]
    fn coverage_saturates_before_full_weight() {
        let ft = FoliageType::default();
        assert_eq!(
            count(&ft, &painted_map(16, FULL_COVERAGE_WEIGHT)),
            count(&ft, &painted_map(16, 1.0))
        );
    }

    /// A chunk painted wall to wall must degrade in density, not in frame time.
    /// The budget is far higher than the baked-mesh scatter could afford, but it
    /// is still a budget.
    #[test]
    fn a_pathological_config_stays_inside_the_blade_budget() {
        let dense = FoliageType {
            density: 512.0,
            blades_per_clump: 16,
            ..default()
        };
        let blades = full_chunk(&dense, &painted_map(256, 1.0)).unwrap().len();
        assert!(
            blades <= MAX_BLADES_PER_CHUNK,
            "{blades} blades exceeds the {MAX_BLADES_PER_CHUNK} budget"
        );
        // The estimate drives the thinning, so it has to land near the cap
        // rather than merely under it — a wildly low estimate would thin a full
        // chunk to nothing.
        assert!(
            blades > MAX_BLADES_PER_CHUNK / 2,
            "{blades} blades is far under the budget — the estimate is off"
        );
    }

    /// The defaults have to fit a fully painted chunk without the budget cutting
    /// in. The budget is a backstop against a pathological config, not something
    /// normal use should ever meet.
    #[test]
    fn the_defaults_fit_a_full_chunk_unthinned() {
        let ft = FoliageType::default();
        let blades = full_chunk(&ft, &painted_map(256, 1.0)).unwrap().len();
        let asked_for = (64.0 * 64.0 * ft.density * ft.blades_per_clump as f32) as usize;
        assert!(
            blades < MAX_BLADES_PER_CHUNK,
            "the defaults ({blades}) already meet the budget"
        );
        // Within a tenth of what the density asks for; the shortfall is blades
        // whose jitter pushed them off the chunk edge.
        assert!(
            blades as f32 > asked_for as f32 * 0.9,
            "{blades} blades vs the {asked_for} the defaults ask for"
        );
    }

    /// Bilinear sampling, and the scan bounds derived from the painted texels,
    /// both have to agree that an isolated painted texel produces grass — and
    /// that it stays near that texel rather than tiling a square across the map.
    #[test]
    fn a_single_painted_texel_scatters_locally() {
        let mut map = FoliageDensityMap::new(64);
        map.density_weights[32 * 64 + 32][0] = 1.0;
        let blades =
            full_chunk(&FoliageType::default(), &map).expect("one texel should still grow grass");
        // The texel sits at the middle of a 64 m chunk and is 1 m across;
        // bilinear reach adds a texel either side, so nothing should land far
        // from centre.
        let texel = 64.0 / 64.0;
        for blade in &blades {
            let (x, z) = (blade.position_height[0], blade.position_height[2]);
            assert!(
                (x - 32.0).abs() < texel * 2.5 && (z - 32.0).abs() < texel * 2.5,
                "blade at ({x}, {z}) is nowhere near the painted texel"
            );
        }
    }
}
