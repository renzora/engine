//! Terrain operations for crates that must not link the terrain crate.
//!
//! The MCP server (a native plugin, which links only this crate and Bevy)
//! needs to do what a person does with the terrain brushes: find the ground
//! under a point, sculpt along a path, run the generator, erode, build a
//! terrain graph. Clicking the
//! brushes through the UI works, but it is guesswork about where a screen point
//! lands on a hill, and nothing in the UI answers "how high is the ground
//! here?", which is the question every placed tree and rock asks.
//!
//! So the terrain editor installs a [`TerrainTools`] resource of plain
//! function pointers taking `&mut World`, and a caller runs them from an
//! exclusive system. No resource means no terrain editor in the build; a caller
//! says so rather than guessing.
//!
//! Units: world metres and degrees throughout. Points on the ground are given
//! as `x`/`z` world coordinates; the terrain under them is found, whichever it
//! is and wherever it was moved to.

use bevy::prelude::*;

/// What lies under a world `x`/`z`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundHit {
    pub terrain: Entity,
    /// The surface point, in world space.
    pub point: Vec3,
    /// The surface normal, in world space.
    pub normal: Vec3,
}

impl GroundHit {
    /// Steepness in degrees: 0 is level, 90 a wall.
    pub fn slope_degrees(&self) -> f32 {
        self.normal.y.clamp(-1.0, 1.0).acos().to_degrees()
    }
}

/// One sculpt stroke: a brush dragged along `path`.
#[derive(Clone, Debug)]
pub struct TerrainSculpt {
    /// The terrain to sculpt; `None` means the one under the first point.
    pub terrain: Option<Entity>,
    /// A brush name as the Sculpt tab lists it: raise, lower, smooth, flatten,
    /// set_height, noise, terrace, erosion, hydro, relax, pinch, ramp, retop,
    /// erase.
    pub brush: String,
    /// World `x`/`z` points; one point is a single dab.
    pub path: Vec<Vec2>,
    /// Brush radius in metres.
    pub radius: f32,
    /// For raise, lower and noise: metres of change at the centre of the
    /// stroke. For every other brush: 0..1, how far one pass goes toward its
    /// result.
    pub strength: f32,
    /// 0..1: how much of the radius fades out (0 is a hard disc).
    pub falloff: f32,
    /// World height for flatten, set_height and ramp; `None` takes the
    /// ground height at the first point.
    pub target_height: Option<f32>,
}

/// One run of the generator.
#[derive(Clone, Debug, Default)]
pub struct TerrainGenerate {
    pub terrain: Option<Entity>,
    /// eroded, fbm, ridged, billow, warped or hybrid (default eroded).
    pub noise: Option<String>,
    /// Metres per unit of noise: the size of the biggest features.
    pub scale: Option<f32>,
    /// Peak-to-floor amplitude in metres.
    pub height: Option<f32>,
    /// World height of the floor.
    pub base: Option<f32>,
    pub octaves: Option<u32>,
    /// Above 1 makes peaks with valley floors between them; below 1 plateaus.
    pub exponent: Option<f32>,
    pub seed: Option<u32>,
    /// replace, add, subtract, max or min (default replace).
    pub blend: Option<String>,
    /// A world `x`/`z` rectangle, `(min, max)`; `None` is the whole terrain.
    pub region: Option<(Vec2, Vec2)>,
    /// 0..1 edge feather of the region.
    pub feather: Option<f32>,
}

/// One erosion pass.
#[derive(Clone, Debug)]
pub struct TerrainErode {
    pub terrain: Option<Entity>,
    /// Raindrops for hydraulic erosion; 0 skips it.
    pub droplets: u32,
    /// Thermal weathering passes; 0 skips it.
    pub thermal_iterations: u32,
    /// Slope loose ground rests at, for thermal weathering.
    pub repose_degrees: f32,
    /// Limit to a disc, `(world x/z centre, radius in metres)`; `None` is the
    /// whole terrain.
    pub area: Option<(Vec2, f32)>,
    pub seed: u32,
}

impl Default for TerrainErode {
    fn default() -> Self {
        Self {
            terrain: None,
            droplets: 60_000,
            thermal_iterations: 8,
            repose_degrees: 38.0,
            area: None,
            seed: 1,
        }
    }
}

/// One build of a terrain graph (a `.tgraph` file) into a terrain.
///
/// The build replaces the terrain's generator layer and keeps its sculpt
/// layer, so hand edits survive it. It runs to completion before the call
/// returns, at the terrain's full resolution.
#[derive(Clone, Debug, Default)]
pub struct TerrainGraphBuild {
    /// The terrain to build; `None` means the only one in the scene.
    pub terrain: Option<Entity>,
    /// Project-relative `.tgraph` to build from. `None` keeps the graph the
    /// terrain's generator already names. Given, it is stored on the terrain,
    /// so the next build (and the saved scene) uses it too.
    pub graph: Option<String>,
    /// The seed to build with; `None` keeps the terrain's. Stored like `graph`.
    pub seed: Option<u64>,
}

/// The terrain editor's operations. Each edit is one entry on the scene's
/// undo stack and answers with a one-line summary of what it did.
#[derive(Resource, Clone, Copy)]
pub struct TerrainTools {
    /// The ground under world `x`/`z`, on any terrain.
    pub ground: fn(&mut World, Vec2) -> Option<GroundHit>,
    pub sculpt: fn(&mut World, &TerrainSculpt) -> Result<String, String>,
    pub generate: fn(&mut World, &TerrainGenerate) -> Result<String, String>,
    pub erode: fn(&mut World, &TerrainErode) -> Result<String, String>,
    /// Build a terrain graph into a terrain. See [`TerrainGraphBuild`].
    pub build_graph: fn(&mut World, &TerrainGraphBuild) -> Result<String, String>,
    /// Paint a landscape layer along a path. See [`TerrainPaint`].
    pub paint: fn(&mut World, &TerrainPaint) -> Result<String, String>,
    /// Give a terrain (`None`: the only one) a landscape `.material`,
    /// project-relative. An empty path is Alpine Landscape, the engine's
    /// default, which is copied into the project if it is not there.
    pub set_material: fn(&mut World, Option<Entity>, &str) -> Result<String, String>,
    /// The layers a terrain's material offers, for [`TerrainPaint::layer`].
    pub layers: fn(&mut World, Option<Entity>) -> Result<Vec<TerrainLayerInfo>, String>,
}

/// One stroke of the landscape weight brush: a terrain layer painted along
/// `path`.
///
/// Painting raises the named layer and lowers the others in proportion, so a
/// point's layers always add up to the whole; erase does the reverse, smooth
/// blurs every layer together and fill lays the layer straight down at the
/// target weight.
#[derive(Clone, Debug, Default)]
pub struct TerrainPaint {
    /// The terrain to paint; `None` means the one under the first point.
    pub terrain: Option<Entity>,
    /// The layer, by the name the terrain's landscape material gives it (see
    /// [`TerrainTools::layers`]).
    pub layer: String,
    /// paint, erase, smooth or fill (empty is paint).
    pub mode: String,
    /// World `x`/`z` points; one point is a single dab.
    pub path: Vec<Vec2>,
    /// Brush radius in metres.
    pub radius: f32,
    /// 0..1: how far one pass goes toward the result.
    pub strength: f32,
    /// 0..1: the weight paint and fill bring the layer to (`None` is 1).
    pub target: Option<f32>,
    /// 0..1: how much of the radius fades out (0 is a hard disc).
    pub falloff: f32,
    /// Only where the ground's slope is within this range, in degrees.
    pub slope_range: Option<Vec2>,
    /// Only where the ground's world height is within this range, in metres.
    pub height_range: Option<Vec2>,
}

/// One layer of a terrain's landscape material, as [`TerrainTools::layers`]
/// reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainLayerInfo {
    pub name: String,
    /// Weight, Height or Alpha: how the material mixes it.
    pub blend: String,
    /// 0..1 of the terrain it covers by painted weight.
    pub coverage: f32,
}
