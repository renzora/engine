//! The terrain the New Environment starter makes: a broad flat floor to build
//! on, in a basin of low, soft hills.
//!
//! A new terrain is otherwise one flat tile, which shows nothing of what the
//! terrain tools can do until you have generated or sculpted it yourself. This
//! starts shaped, and it is still ordinary terrain: sculpt it or regenerate it
//! like any other.
//!
//! The floor is the point, and the hills are a horizon. A meadow ringed by
//! 300 m mountains was tried first: a striking picture, but the mountains
//! stood close enough to fill the view, a low sun behind them turned them to
//! silhouettes, and there was less flat ground than a blockout wants.
//!
//! No material: it wears the blockout grid like every new terrain, and a
//! landscape material is a choice made in the Terrain component. Loading one
//! here would also write it and its textures into the project as a side
//! effect of starting a scene.
//!
//! Fixed seeds, so every new environment starts from the same landscape.

use bevy::prelude::*;

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use crate::mesh::spawn_terrain_chunks;
use crate::sculpt::fbm;

/// Chunks per side: 2 km across at the default 64 m chunk. 512 m was tried
/// first and read as a toy: the floor was a clearing and the hills a wall
/// standing right in front of the camera. Chunks stream, so the size costs
/// little while you work in one part of it.
const CHUNKS: u32 = 32;
/// The floor, in metres. Above the editor grid, so the two never fight over
/// the same plane.
const FLOOR: f32 = 1.0;
/// How far from the centre the floor stays dead flat, in metres, before the
/// hills start; the rim wanders in and out of this by [`RIM_WANDER`].
const BASIN: f32 = 430.0;
/// Where the hills reach their full height: the far corners.
const RIM: f32 = 1400.0;
/// The hills' height at [`RIM`], before [`Shape::swell`] raises or lowers it:
/// about 90 m at the middle of each edge and 120 m or so in the corners.
const HILLS: f32 = 130.0;
/// How far the rim's foot wanders from a circle, in metres either way, so
/// the basin is a valley with bays and spurs rather than a bowl.
const RIM_WANDER: f32 = 250.0;
/// Small rises and dips laid over the hills, in metres either way.
const DETAIL: f32 = 4.0;

/// The heightfield, in terrain-local metres.
struct Shape {
    half: Vec2,
}

impl Shape {
    fn new(terrain: &TerrainData) -> Self {
        Self { half: Vec2::new(terrain.total_width(), terrain.total_depth()) * 0.5 }
    }

    /// Low-frequency noise, `-1..1`, at metres `p` from the centre.
    fn wobble(p: Vec2, scale: f32, octaves: u32, seed: u32) -> f32 {
        let q = p / scale;
        (fbm(q.x, q.y, octaves, 2.0, 0.5, seed) - 0.5) * 2.0
    }

    /// How tall the hills stand here, against [`HILLS`]: some stretches of
    /// the rim are a low swell, some twice as high.
    fn swell(p: Vec2) -> f32 {
        (1.0 + Self::wobble(p, 450.0, 3, 11) * 0.8).max(0.25)
    }

    /// Height in metres at terrain-local `(x, z)`.
    fn height(&self, _terrain: &TerrainData, x: f32, z: f32) -> f32 {
        let p = Vec2::new(x, z) - self.half;
        // Distance out from the basin's edge as a share of the way to the
        // rim, its edge bent by noise.
        let r = p.length() + Self::wobble(p, 520.0, 3, 7) * RIM_WANDER;
        let t = ((r - BASIN) / (RIM - BASIN)).clamp(0.0, 1.0);
        // Rises quickly off the floor and levels out towards the corners,
        // eased in at the foot so the floor meets the slope without a crease.
        let foot = (t / 0.12).min(1.0);
        let rise = (1.0 - (1.0 - t).powf(1.3)) * foot * foot * (3.0 - 2.0 * foot);
        let detail = Self::wobble(p, 200.0, 3, 13) * DETAIL * foot;
        FLOOR + HILLS * rise * Self::swell(p) + detail
    }
}

/// Where the starting camera stands, metres from the centre across the
/// floor: well inside the basin, with a long stretch of it in front.
const VIEW_EYE: Vec2 = Vec2::new(158.0, -196.0);
/// How far above the ground the starting camera stands: eye height.
const VIEW_HEIGHT: f32 = 2.6;
/// Which way it looks across the floor, as a compass bearing in degrees
/// (0 is towards -Z), and how far down.
const VIEW_BEARING: f32 = 230.0;
const VIEW_PITCH: f32 = 2.8;

/// Where a new environment's camera starts, as world-space `(eye, target)`:
/// standing on the floor at eye height, looking almost level across it at
/// the hills. The target is where that line meets the ground, so the orbit
/// pivot lands on something. Up above the rim looking down was tried and
/// read as a map rather than a place.
pub fn starter_view() -> (Vec3, Vec3) {
    let data = starter_data();
    let shape = Shape::new(&data);
    // World x/z to the shape's terrain-local metres: the terrain is centred
    // on the origin.
    let ground = |x: f32, z: f32| shape.height(&data, x + shape.half.x, z + shape.half.y);
    let eye = Vec3::new(VIEW_EYE.x, ground(VIEW_EYE.x, VIEW_EYE.y) + VIEW_HEIGHT, VIEW_EYE.y);
    let (bearing, pitch) = (VIEW_BEARING.to_radians(), VIEW_PITCH.to_radians());
    let ahead = Vec3::new(bearing.sin() * pitch.cos(), -pitch.sin(), -bearing.cos() * pitch.cos());
    let mut target = eye + ahead * 1000.0;
    for step in 1..=1000 {
        let p = eye + ahead * step as f32;
        if p.y <= ground(p.x, p.z) {
            target = p;
            break;
        }
    }
    (eye, target)
}

fn starter_data() -> TerrainData {
    TerrainData {
        chunks_x: CHUNKS,
        chunks_z: CHUNKS,
        min_height: -10.0,
        max_height: FLOOR + HILLS * 2.5 + DETAIL + 20.0,
        ..default()
    }
}

/// A starter chunk still waiting for its heights. Nothing of its terrain gets
/// a mesh or a stand-in until [`shape_pending_chunks`] has filled in every
/// chunk: a chunk's edges are lit from its neighbours, so one built beside a
/// still-flat neighbour kept a seam (see `rehydrate_terrain_chunks`).
#[derive(Component)]
pub struct PendingShape;

/// Most chunks shaped in one frame. Each is about 17 thousand vertices of
/// eroded noise; the whole 2 km starter took a second of every core, which as
/// one frame was a freeze on clicking the tile. In batches the editor keeps
/// drawing, and the progress card moves.
const SHAPES_PER_FRAME: usize = 48;

/// Spawn the starter terrain, centred on the origin. Its chunks start flat
/// and [`PendingShape`]; [`shape_pending_chunks`] gives them their heights
/// over the next frames, nearest the camera first.
pub fn spawn_starter_terrain(world: &mut World) -> Entity {
    let data = starter_data();
    let terrain = world
        .spawn((
            Name::new("Terrain"),
            // At the origin: chunks are laid out centred on their terrain
            // (`TerrainData::chunk_world_origin`), so this puts the floor
            // where a new scene's camera looks. Offsetting by half the size
            // as well once moved the whole landscape a kilometre into one
            // corner and left the camera over nothing.
            Transform::default(),
            Visibility::default(),
            data.clone(),
            renzora::SelectionStop,
        ))
        .id();
    for chunk in spawn_terrain_chunks(world, terrain, &data, None) {
        world.entity_mut(chunk).insert(PendingShape);
    }
    terrain
}

/// One chunk's heights, normalized to the terrain's range.
fn chunk_heights(shape: &Shape, data: &TerrainData, chunk_x: u32, chunk_z: u32) -> Vec<f32> {
    let resolution = data.chunk_resolution;
    let spacing = data.vertex_spacing();
    let range = data.height_range();
    let origin = Vec2::new(chunk_x as f32, chunk_z as f32) * data.chunk_size;
    (0..resolution * resolution)
        .map(|i| {
            let x = origin.x + (i % resolution) as f32 * spacing;
            let z = origin.y + (i / resolution) as f32 * spacing;
            ((shape.height(data, x, z) - data.min_height) / range).clamp(0.0, 1.0)
        })
        .collect()
}

/// Shape a batch of [`PendingShape`] chunks, the nearest to the streaming
/// focus first, in parallel.
///
/// The heights go into both the sculpt layer and the composed buffer, with
/// nothing marked dirty: a fresh terrain has no height layers, so composing
/// would only copy one into the other, and flagging it rebuilt every mesh a
/// second time right after it was first built.
pub fn shape_pending_chunks(
    mut commands: Commands,
    focus: Option<Res<crate::streaming::StreamFocus>>,
    terrains: Query<(&TerrainData, &GlobalTransform)>,
    mut chunks: Query<(Entity, &mut TerrainChunkData, &TerrainChunkOf), With<PendingShape>>,
) {
    if chunks.is_empty() {
        return;
    }
    let focus = focus.and_then(|f| f.0);
    let mut queue: Vec<(f32, Entity, Entity, u32, u32)> = chunks
        .iter()
        .filter_map(|(entity, chunk, of)| {
            let (data, root) = terrains.get(of.0).ok()?;
            let half = data.chunk_size * 0.5;
            let centre =
                root.transform_point(data.chunk_world_origin(chunk.chunk_x, chunk.chunk_z) + Vec3::new(half, 0.0, half));
            let distance = focus.map_or(0.0, |f| f.xz().distance_squared(centre.xz()));
            Some((distance, entity, of.0, chunk.chunk_x, chunk.chunk_z))
        })
        .collect();
    queue.sort_by(|a, b| a.0.total_cmp(&b.0));
    queue.truncate(SHAPES_PER_FRAME);

    // One shape per terrain in the batch; there is almost always one.
    let mut shapes: Vec<(Entity, &TerrainData, Shape)> = Vec::new();
    for &(_, _, terrain, ..) in &queue {
        if shapes.iter().all(|(t, ..)| *t != terrain) {
            if let Ok((data, _)) = terrains.get(terrain) {
                shapes.push((terrain, data, Shape::new(data)));
            }
        }
    }
    let shaped: Vec<(Entity, Vec<f32>)> = bevy::tasks::ComputeTaskPool::get().scope(|scope| {
        for &(_, entity, terrain, x, z) in &queue {
            let Some((_, data, shape)) = shapes.iter().find(|(t, ..)| *t == terrain) else { continue };
            scope.spawn(async move { (entity, chunk_heights(shape, data, x, z)) });
        }
    });

    for (entity, heights) in shaped {
        let Ok((_, mut chunk, _)) = chunks.get_mut(entity) else { continue };
        chunk.heights = heights.clone();
        chunk.base_heights = heights;
        chunk.dirty = false;
        commands.entity(entity).remove::<PendingShape>();
    }
}

/// How the viewport's progress card describes a terrain filling in.
const WORK_ID: &str = "terrain_build";

/// Report terrain that is still shaping or building to the viewport's
/// progress card ([`ViewportWork`](renzora::core::viewport_types::ViewportWork)).
///
/// Counts the chunks still waiting, out of the most that were waiting since
/// the card appeared, so the bar only moves forwards. Ordinary streaming as
/// the camera moves brings in a few chunks at a time, which is over before a
/// card could be read, so it only appears for a real backlog.
pub fn report_terrain_progress(
    work: Option<ResMut<renzora::core::viewport_types::ViewportWork>>,
    shaping: Query<(), With<PendingShape>>,
    building: Query<
        (),
        (
            With<TerrainChunkData>,
            Without<Mesh3d>,
            Without<crate::data::TerrainChunkStreamedOut>,
            Without<PendingShape>,
        ),
    >,
    mut peak: Local<u32>,
) {
    /// Fewer chunks than this waiting does not bring the card up.
    const BACKLOG: u32 = 24;
    let Some(mut work) = work else { return };
    let shaping = shaping.iter().count() as u32;
    let waiting = shaping + building.iter().count() as u32;
    if waiting == 0 || (*peak == 0 && waiting < BACKLOG) {
        *peak = 0;
        if work.get(WORK_ID).is_some() {
            work.set(WORK_ID, None);
        }
        return;
    }
    *peak = (*peak).max(waiting);
    let label = if shaping > 0 { "Shaping terrain" } else { "Building terrain" };
    let next = renzora::core::viewport_types::WorkItem {
        id: WORK_ID,
        label: label.to_string(),
        done: *peak - waiting,
        total: *peak,
    };
    if work.differs(WORK_ID, Some(&next)) {
        work.set(WORK_ID, Some(next));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Heights on a 65 x 65 grid over the whole terrain, with the spacing.
    fn survey() -> (Vec<Vec<f32>>, f32) {
        let data = starter_data();
        let shape = Shape::new(&data);
        let size = data.total_width();
        let n = 65;
        let step = size / (n - 1) as f32;
        let grid = (0..n)
            .map(|j| (0..n).map(|i| shape.height(&data, i as f32 * step, j as f32 * step)).collect())
            .collect();
        (grid, step)
    }

    /// A flat floor in the middle, hills rising off it to about 50 m at
    /// 700 m out and 90 m by the middle of each edge, highest in the corners,
    /// and an uneven rim rather than a ring.
    #[test]
    fn the_starter_is_a_flat_floor_in_a_basin_of_hills() {
        let data = starter_data();
        let (h, step) = survey();
        let n = h.len();
        let c = (n / 2) as f32;
        let mut rings: Vec<Vec<f32>> = vec![Vec::new(); 16];
        for (j, row) in h.iter().enumerate() {
            for (i, &y) in row.iter().enumerate() {
                let r = Vec2::new(i as f32 - c, j as f32 - c).length() * step;
                rings[(r / 100.0) as usize].push(y);
            }
        }
        let mean = |b: usize| rings[b].iter().sum::<f32>() / rings[b].len() as f32;
        let spread = |b: usize| {
            let (lo, hi) = rings[b].iter().fold((f32::MAX, f32::MIN), |(lo, hi), &y| (lo.min(y), hi.max(y)));
            hi - lo
        };
        for b in 0..3 {
            assert!(rings[b].iter().all(|&y| y == FLOOR), "the floor is not flat {}00 m out", b);
        }
        assert!((30.0..70.0).contains(&mean(7)), "700 m out averages {} m", mean(7));
        assert!((70.0..115.0).contains(&mean(10)), "the edges average {} m", mean(10));
        assert!(mean(13) > mean(10), "the corners ({} m) are not the highest", mean(13));
        assert!(spread(7) > 60.0, "the rim is a ring, {} m from lowest to highest", spread(7));
        let highest = h.iter().flatten().fold(f32::MIN, |a, &y| a.max(y));
        assert!((110.0..220.0).contains(&highest), "the highest hill is {highest} m");
        assert!(highest < data.max_height, "the hills clip at {highest} m");
    }

    /// Soft hills: mostly gentle slopes, nothing like a cliff.
    #[test]
    fn the_hills_are_soft() {
        let (h, step) = survey();
        let mut slopes = Vec::new();
        for j in 1..h.len() - 1 {
            for i in 1..h.len() - 1 {
                if h[j][i] > FLOOR + 5.0 {
                    let dx = h[j][i + 1] - h[j][i - 1];
                    let dz = h[j + 1][i] - h[j - 1][i];
                    slopes.push((dx.hypot(dz) / (2.0 * step)).atan().to_degrees());
                }
            }
        }
        slopes.sort_by(f32::total_cmp);
        let median = slopes[slopes.len() / 2];
        let steepest = slopes[slopes.len() - 1];
        assert!((5.0..16.0).contains(&median), "the median slope is {median} degrees");
        assert!(steepest < 38.0, "the steepest slope is {steepest} degrees");
    }

    /// The starting camera stands on the floor at eye height and looks
    /// almost level across it, with the pivot on the ground in front.
    #[test]
    fn the_starting_camera_stands_on_the_floor() {
        let data = starter_data();
        let shape = Shape::new(&data);
        let (eye, target) = starter_view();
        let ground = |p: Vec3| shape.height(&data, p.x + shape.half.x, p.z + shape.half.y);
        assert_eq!(ground(eye), FLOOR, "the camera is not over the floor");
        assert!((eye.y - ground(eye) - VIEW_HEIGHT).abs() < 1e-3);
        let run = eye.xz().distance(target.xz());
        assert!((20.0..200.0).contains(&run), "the pivot is {run} m ahead");
        assert!((target.y - ground(target)).abs() < 1.5, "the pivot is off the ground");
    }
}
