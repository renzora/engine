//! The `.vfx` file: a versioned RON document holding one effect.
//!
//! An effect is a list of emitters, and an emitter's behaviour is a node graph
//! ([`VfxGraph`]). The graph is stored in the same shape as a material graph
//! (nodes with an id, a `node_type` string, a canvas position and inline input
//! values, plus a flat connection list) so the editor can put it in ember's node
//! graph widget without a translation layer. Everything that is not behaviour
//! (texture, blend, facing) lives in [`RenderSettings`] beside the graph,
//! because it selects a pipeline rather than computing a value per particle.
//!
//! ## Short files
//!
//! Almost every field is `#[serde(default)]`, so a hand-written effect states
//! only what differs from the defaults. A file that is just
//! `(emitters: [(graph: (nodes: [(id: 1, node_type: "output/particle")])) ])`
//! loads and emits.
//!
//! ## Versions
//!
//! `version` is read first, on its own, before the document is parsed as a
//! whole. A file from a newer engine is refused with a message saying so rather
//! than half-parsed: serde would otherwise ignore the fields it does not know
//! and hand back an effect that silently lost them. Older files go through
//! [`migrate`], which applies each step from the file's version up to
//! [`VFX_VERSION`] on the typed asset. Additive changes need no step at all,
//! because the new field defaults; a step exists for anything whose *meaning*
//! changed.

use std::collections::BTreeMap;

use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::curve::{CurvePoint, GradientStop};

/// The version this engine writes. Bump it together with a [`MIGRATIONS`] entry
/// when the meaning of an existing field changes.
pub const VFX_VERSION: u32 = 1;

/// The file extension the loader claims.
pub const VFX_EXTENSION: &str = "vfx";

/// Node ids are plain integers, as in the material graph, so the editor can
/// share its id allocator and selection code.
pub type NodeId = u64;

// ── Effect ──────────────────────────────────────────────────────────────────

/// One particle effect, as loaded from a `.vfx` file.
#[derive(Asset, TypePath, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VfxAsset {
    pub version: u32,
    pub name: String,
    pub dimension: Dimension,
    /// Length of one cycle in seconds. Emission stops after it unless the effect
    /// loops; tracks run over it and the timeline scrubs over it. 0 means "no end": a
    /// non-looping effect with no duration emits for as long as it plays.
    pub duration: f32,
    pub looping: bool,
    pub emitters: Vec<Emitter>,
    /// Keyed parameters over the effect's own time (see [`crate::params`]).
    /// Additive, so no version step: a file without the field has none, and
    /// skipped when empty so an effect nobody keyed saves exactly as before.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<crate::params::ParamTrack>,
}

impl Default for VfxAsset {
    fn default() -> Self {
        Self {
            version: VFX_VERSION,
            name: String::new(),
            dimension: Dimension::ThreeD,
            duration: 2.0,
            looping: true,
            emitters: Vec::new(),
            tracks: Vec::new(),
        }
    }
}

/// Which renderer draws the effect. A property of the whole effect rather than
/// of an emitter, because it decides the camera it is seen through.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum Dimension {
    TwoD,
    #[default]
    ThreeD,
}

/// Where particles live once spawned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum SimulationSpace {
    /// Particles move with the entity: a torch's flame follows the torch.
    #[default]
    Local,
    /// Particles are released into the world where they spawn: a moving
    /// emitter leaves a trail.
    World,
}

/// One emitter: a capacity, a seed, a behaviour graph and how to draw it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Emitter {
    pub name: String,
    pub enabled: bool,
    /// Most particles alive at once. Spawns past it are dropped and counted.
    pub capacity: u32,
    pub space: SimulationSpace,
    /// Seeds every per-particle random, so a replay is identical.
    pub seed: u64,
    pub graph: VfxGraph,
    pub render: RenderSettings,
    /// Real scene light cast by the emitter's particles ([`ParticleLight`]).
    /// Additive and skipped while off, so an effect without one saves exactly
    /// as it did before the field existed.
    #[serde(skip_serializing_if = "is_default")]
    pub light: ParticleLight,
    /// Particles of other emitters spawned by this one's particles
    /// ([`SubEmitter`]). Skipped when empty, like `light`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sub_emitters: Vec<SubEmitter>,
    /// Where the emitter simulates ([`Backend`]). Skipped while `Auto`, so an
    /// effect saved before the GPU backend existed re-saves unchanged.
    #[serde(skip_serializing_if = "is_default")]
    pub backend: Backend,
    /// `World` space only: the share of the entity's world velocity each
    /// particle adds to its own at birth, so sparks thrown from a moving cart
    /// keep its momentum. A `Local` emitter already carries its particles
    /// with the entity, and inheriting on top would run them ahead of it, so
    /// it ignores this. The graph's **Emitter Velocity** input does the same
    /// per particle; this is the one-number version. Skipped while 0.
    #[serde(skip_serializing_if = "is_default")]
    pub inherit_velocity: f32,
}

/// Which simulation runs an emitter.
///
/// The graph means the same on both: the GPU backend is compiled from the
/// same IR as the CPU interpreter (see `crate::gpu`). What differs is cost
/// and coverage. The GPU runs hundreds of thousands of particles a frame but
/// needs compute shaders and does not yet cover every feature (per-particle
/// lights, scene collisions, project materials), so the choice is per emitter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Backend {
    /// The GPU when the device has compute, the emitter is large enough to be
    /// worth it ([`crate::gpu::AUTO_GPU_MIN_CAPACITY`]), and it uses nothing
    /// the GPU backend lacks; the CPU otherwise. An effect never fails to play
    /// because of where it runs.
    #[default]
    Auto,
    /// Always the CPU interpreter, which runs everywhere.
    Cpu,
    /// The GPU whatever the capacity. An emitter that cannot run there falls
    /// back to the CPU with a Console warning saying why.
    Gpu,
}

/// Capacity a new emitter gets, and what an unversioned file's zero means.
pub const DEFAULT_CAPACITY: u32 = 256;

impl Default for Emitter {
    fn default() -> Self {
        Self {
            name: "emitter".to_string(),
            enabled: true,
            capacity: DEFAULT_CAPACITY,
            space: SimulationSpace::Local,
            seed: 1,
            graph: VfxGraph::default(),
            render: RenderSettings::default(),
            light: ParticleLight::default(),
            sub_emitters: Vec::new(),
            backend: Backend::Auto,
            inherit_velocity: 0.0,
        }
    }
}

impl Emitter {
    /// The velocity share a newborn takes from the entity, as the simulation
    /// applies it: 0 for a `Local` emitter (see [`Self::inherit_velocity`])
    /// and for a value that is not a number.
    pub fn inherited_velocity(&self) -> f32 {
        if self.space == SimulationSpace::World && self.inherit_velocity.is_finite() {
            self.inherit_velocity
        } else {
            0.0
        }
    }
}

// ── Light ───────────────────────────────────────────────────────────────────

/// How an emitter lights the scene.
///
/// Particles themselves emit no light onto the scene. A fire that does not light
/// the ground beside it reads as a sticker, so an emitter can carry real Bevy
/// `PointLight`s whose colour and brightness follow its live particles: a fire
/// that dies down dims its light with it, and one that flares brightens it.
/// The lights are hidden children of the effect, never saved, and they reach
/// Solari exactly as a hand-placed `PointLight` does (see `crate::light`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightMode {
    #[default]
    Off,
    /// One light at the emitter (plus [`ParticleLight::offset`]), coloured by
    /// the particles' average colour and as bright as all of them together.
    PerEmitter,
    /// A light on each of the `n` brightest particles, which follow them. Up
    /// to [`MAX_PARTICLE_LIGHTS`]; each is a real clustered light, so a few
    /// is the right number.
    BrightestN(u32),
}

/// The most lights one emitter may own in [`LightMode::BrightestN`]. Every
/// light is a clustered light (and a Solari emissive proxy), so this caps what
/// a typo in an effect file can cost a frame.
pub const MAX_PARTICLE_LIGHTS: u32 = 16;

/// The count [`LightMode::BrightestN`] gets when switched on in the editor.
pub const DEFAULT_PARTICLE_LIGHTS: u32 = 4;

/// An emitter's light settings. See [`LightMode`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParticleLight {
    pub mode: LightMode,
    /// Linear RGB tint multiplied into the particles' average colour. A
    /// procedural flame's colour is an HDR tint with its heat in alpha, not
    /// the colour it glows, so a fire wants a warm tint here.
    pub color: [f32; 3],
    /// Lumens per fully bright particle: a particle whose colour has a
    /// luminance of 1 and an alpha of 1. The light is the sum over the live
    /// particles, so its brightness follows how many there are and how bright
    /// each is right now.
    pub intensity: f32,
    /// Metres the light reaches.
    pub range: f32,
    /// The light's own size in metres: softer highlights and shadows, and the
    /// radius of the sphere Solari stands it in as.
    pub radius: f32,
    /// Shadow maps cost a cube render per light per frame, so they are off
    /// unless asked for.
    pub shadows: bool,
    /// Seconds for the light to settle on a change. A fire spawns and kills
    /// particles every frame; without smoothing its light strobes with them.
    pub smoothing: f32,
    /// 0..1: how far a noise of time dims the light below its value, for a
    /// fire's flicker. 0 is steady.
    pub flicker: f32,
    /// Flicker speed in Hz.
    pub flicker_rate: f32,
    /// [`LightMode::PerEmitter`] only: where the light sits relative to the
    /// entity, in its local space. A log fire's light belongs at flame height,
    /// not on the ground it would then only graze.
    pub offset: [f32; 3],
}

impl Default for ParticleLight {
    fn default() -> Self {
        Self {
            mode: LightMode::Off,
            color: [1.0, 1.0, 1.0],
            intensity: 50.0,
            range: 6.0,
            radius: 0.1,
            shadows: false,
            smoothing: 0.12,
            flicker: 0.0,
            flicker_rate: 6.0,
            offset: [0.0, 0.0, 0.0],
        }
    }
}

impl ParticleLight {
    /// How many light entities this setting needs.
    pub fn light_count(&self) -> usize {
        match self.mode {
            LightMode::Off => 0,
            LightMode::PerEmitter => 1,
            LightMode::BrightestN(n) => n.clamp(1, MAX_PARTICLE_LIGHTS) as usize,
        }
    }
}

// ── Sub-emitters ────────────────────────────────────────────────────────────

/// When a [`SubEmitter`] fires for a particle of its emitter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SubEmitterTrigger {
    /// When the particle is born.
    OnBirth,
    /// When it dies: its lifetime runs out, or a collider kills it.
    OnDeath,
    /// Every `seconds` of the particle's life, from `seconds` after birth.
    Interval(f32),
}

/// Spawn particles of another emitter of the same effect from this emitter's
/// particles: sparks that burst into embers when they die, a rocket that drops
/// smoke every tenth of a second.
///
/// The target emitter's own spawn pins are not used once anything targets it:
/// it emits only what its parents ask for, which is what a sub-emitter is.
/// Its init program still runs for every particle (lifetime, velocity, size,
/// colour), and the parent's position is added to whatever position it gives,
/// so a shape node spreads the children around the parent.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubEmitter {
    pub trigger: SubEmitterTrigger,
    /// Index of the emitter to spawn, in [`VfxAsset::emitters`].
    pub target: usize,
    /// Particles per trigger.
    pub count: u32,
    /// 0..1: how much of the parent's velocity each child adds to its own.
    pub inherit_velocity: f32,
    /// Multiply each child's colour by the parent's.
    pub inherit_color: bool,
}

impl Default for SubEmitter {
    fn default() -> Self {
        Self {
            trigger: SubEmitterTrigger::OnDeath,
            target: 0,
            count: 8,
            inherit_velocity: 0.0,
            inherit_color: false,
        }
    }
}

// ── Graph ───────────────────────────────────────────────────────────────────

/// An emitter's behaviour graph. Same shape as `renzora_shader`'s
/// `MaterialGraph` minus the material-only fields.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VfxGraph {
    pub nodes: Vec<VfxNode>,
    pub connections: Vec<Connection>,
    /// The editor's comment boxes. Purely visual (the compiler never reads
    /// them), and the contract crate's type so ember's graph view draws them
    /// the way it draws a material's. Skipped when empty so a graph nobody
    /// annotated saves exactly as it did before the field existed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<renzora::GraphComment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VfxNode {
    pub id: NodeId,
    /// Catalog key such as `"math/multiply"`; see [`crate::nodes`].
    pub node_type: String,
    /// Canvas position for the editor. Ignored by the compiler.
    #[serde(default)]
    pub position: [f32; 2],
    /// Inline values for unconnected input pins, and the node's properties
    /// (curve keys, gradient stops, flags). A `BTreeMap` rather than the
    /// material graph's `HashMap` so a saved file lists them in a stable order
    /// and a re-save of an unchanged effect produces no diff.
    #[serde(default)]
    pub input_values: BTreeMap<String, PinValue>,
}

impl VfxNode {
    pub fn new(id: NodeId, node_type: &str) -> Self {
        Self {
            id,
            node_type: node_type.to_string(),
            position: [0.0, 0.0],
            input_values: BTreeMap::new(),
        }
    }

    /// Builder used by the tests and the built-in effects.
    pub fn with(mut self, pin: &str, value: PinValue) -> Self {
        self.input_values.insert(pin.to_string(), value);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub from_node: NodeId,
    pub from_pin: String,
    pub to_node: NodeId,
    pub to_pin: String,
}

impl Connection {
    pub fn new(from_node: NodeId, from_pin: &str, to_node: NodeId, to_pin: &str) -> Self {
        Self {
            from_node,
            from_pin: from_pin.to_string(),
            to_node,
            to_pin: to_pin.to_string(),
        }
    }
}

/// An inline value. Pins are `Float`/`Vec2`/`Vec3`/`Color`; the rest are node
/// properties that are not connectable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PinValue {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    /// Linear RGBA.
    Color([f32; 4]),
    Bool(bool),
    Curve(Vec<CurvePoint>),
    Gradient(Vec<GradientStop>),
    /// A text property: a Parameter node's name. Last, so every file written
    /// before it existed still parses to the same values.
    Text(String),
}

// ── Render settings ─────────────────────────────────────────────────────────

/// How an emitter's particles are drawn. Everything here selects a pipeline or
/// a texture, which is why it is not part of the graph.
///
/// The fields after `z_order` were added with the procedural shading modes and
/// are skipped when they hold their default, so an effect that uses none of
/// them saves exactly as it did before they existed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderSettings {
    /// Project-relative image path. `None` draws a soft round dot.
    pub texture: Option<String>,
    pub sprite_sheet: SpriteSheet,
    pub blend: BlendMode,
    pub facing: Facing,
    /// 2D only: draw order relative to the entity, added to its z.
    pub z_order: f32,
    /// What the fragment shader draws for each particle.
    #[serde(skip_serializing_if = "is_default")]
    pub shading: Shading,
    /// `VelocityAligned` only: seconds of travel added to the quad's height,
    /// so a particle moving at 2 m/s with a stretch of 0.1 is 0.2 m longer
    /// than its `size`. A spark's streak and a flame tongue's lick both come
    /// from speed, which a fixed size cannot follow.
    #[serde(skip_serializing_if = "is_default")]
    pub stretch: f32,
    /// Crossfade between a sprite sheet's frame and the next by the fractional
    /// frame, instead of cutting. A 16-frame flame sheet stepped over a
    /// one-second life visibly strobes; blended it reads as continuous motion.
    #[serde(skip_serializing_if = "is_default")]
    pub frame_blend: bool,
    /// 0..1: dissolve the particle with scrolling noise as it ages, so it
    /// breaks up into wisps rather than fading uniformly. Applies to every
    /// shading mode; the procedural ones already erode on their own and add
    /// this on top.
    #[serde(skip_serializing_if = "is_default")]
    pub erosion: f32,
    /// 3D only, metres: fade the particle where it comes within this distance
    /// of the opaque scene behind it, so a quad crossing the ground or a log
    /// shows no hard line. 0 is off. Needs the camera's depth prepass, which
    /// every engine camera has; a camera without one draws hard edges.
    #[serde(skip_serializing_if = "is_default")]
    pub soft_distance: f32,
    /// [`Facing::Ribbon`] only: what the strip runs through and how it tapers.
    #[serde(skip_serializing_if = "is_default")]
    pub ribbon: Ribbon,
    /// 3D, `Alpha` and `Premultiplied` only: draw the emitter's particles back
    /// to front from the camera. Blending is order dependent, so overlapping
    /// smoke drawn in buffer order shows a far puff over a near one; sorted it
    /// does not. Off by default because it costs a sort per frame (on the GPU
    /// a bitonic sort of the whole pool), and additive blending, the common
    /// case for fire and sparks, does not care about order at all.
    ///
    /// Kept as written for every file saved before [`Self::draw_order`]
    /// existed, which it means `DrawOrder::ViewDepth` in; the editor writes
    /// both, so a file it saves still reads the same in an older engine.
    #[serde(skip_serializing_if = "is_default")]
    pub sort: bool,
    /// The order the particles are drawn in ([`DrawOrder`]); read through
    /// [`Self::draw_order`], which folds the old `sort` in and drops an order
    /// the blend cannot show. Skipped while `Index`.
    #[serde(skip_serializing_if = "is_default")]
    pub draw_order: DrawOrder,
    /// 3D only: draw each particle as this mesh instead of a quad: debris,
    /// leaves, coins, shell casings. `None` (the default) draws quads. While a
    /// mesh is set, `facing` does not apply and `orient` turns the mesh.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<ParticleMesh>,
    /// How a [`Self::mesh`] particle is turned. See [`MeshOrient`].
    #[serde(skip_serializing_if = "is_default")]
    pub orient: MeshOrient,
    /// 3D only: shade the particles with the scene's lights instead of
    /// drawing their colour as is. See [`Lighting`].
    #[serde(skip_serializing_if = "is_default")]
    pub lighting: Lighting,
    /// 3D only: a project `.material` the particles wear instead of the
    /// particle shader, on quads, ribbons and meshes alike, so debris can be
    /// the same rock the level is built from and a spell's sparks can run a
    /// node graph. The particle's colour reaches it as the vertex colour, and
    /// its age, seed and velocity through the material graph's Particle
    /// nodes. Drawn by `render::surface` on both backends: the material keeps
    /// its own fragment shader and the particle vertex shader places it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// [`BlendMode::AlphaMask`] and [`BlendMode::AlphaToCoverage`] only: the
    /// alpha below which a fragment is thrown away (or, with coverage, where
    /// its edge sits).
    #[serde(skip_serializing_if = "is_default_cutoff", default = "default_alpha_cutoff")]
    pub alpha_cutoff: f32,
    /// The particle's size is in pixels on screen rather than in world units,
    /// so it stays the same size at any distance: a marker, a sparkle that
    /// must read from across a map. Physical pixels, as Hanabi measured them,
    /// so a converted effect keeps its look.
    #[serde(skip_serializing_if = "is_default")]
    pub screen_space_size: bool,
    /// 3D only: whether the particles cast shadows. See [`ParticleShadows`].
    /// Last, and skipped while `Auto`, so every file written before it
    /// existed loads and saves unchanged.
    #[serde(skip_serializing_if = "is_default")]
    pub shadows: ParticleShadows,
    /// 3D, depth-written blends only: whether the particles draw through the
    /// deferred G-buffer or the forward pass. See [`RenderMethod`]. Skipped
    /// while `Auto`, so every file written before it existed loads and saves
    /// unchanged.
    #[serde(skip_serializing_if = "is_default")]
    pub render_method: RenderMethod,
}

/// Which renderer a solid particle draws through on a 3D camera.
///
/// Only the depth-written blends (`Opaque`, `AlphaMask`, `AlphaToCoverage`)
/// have a choice. A blended particle is composited over what is already
/// drawn, which a G-buffer (one surface per pixel) cannot hold, so every
/// transparent blend draws forward, after the deferred lighting, as it does
/// in every deferred engine; [`RenderSettings::effective_render_method`]
/// says `Forward` for them whatever this holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RenderMethod {
    /// What the project renders opaque geometry with: the G-buffer on a
    /// deferred project, whose cameras all carry a deferred prepass, and the
    /// forward pass otherwise. Bevy's `OpaqueRendererMethod::Auto`, the same
    /// route a `StandardMaterial` takes, so a solid particle is lit, reflected
    /// (SSR) and occluded (SSAO) like the rock beside it.
    #[default]
    Auto,
    /// Always the forward pass, even on a deferred camera: a particle that
    /// should keep the particle shader's own forward lighting.
    Forward,
    /// The G-buffer whenever every 3D camera has a deferred prepass, even on
    /// a forward project. Bevy cannot draw a deferred material on a camera
    /// without one (it leaves it out of the main pass, and its prepass queue
    /// has no deferred phase to put it in), so while any camera lacks it the
    /// particle draws forward instead; see `render::material::RenderRoute`.
    Deferred,
}

/// Which particles cast shadows into the scene's shadow maps.
///
/// Solid particles are geometry and should shadow like it: a burst of opaque
/// debris with no shadow floats. Blended ones are a judgement call. A puff of
/// smoke drawn as a soft quad casts a hard-edged square-ish shadow (shadow
/// maps hold depth, not opacity, so its alpha can only be thresholded), which
/// is wrong far more often than it is wanted, so they stay out unless asked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParticleShadows {
    /// Depth-written blends (`Opaque`, `AlphaMask`, `AlphaToCoverage`) cast;
    /// blended ones do not.
    #[default]
    Auto,
    /// Every particle casts, blended ones too, cut at `alpha_cutoff`.
    On,
    /// None cast, whatever the blend.
    Off,
}

/// The cutoff a new [`BlendMode::AlphaMask`] starts with, and what a file
/// that never set one means.
pub const DEFAULT_ALPHA_CUTOFF: f32 = 0.5;

fn default_alpha_cutoff() -> f32 {
    DEFAULT_ALPHA_CUTOFF
}

fn is_default_cutoff(v: &f32) -> bool {
    *v == DEFAULT_ALPHA_CUTOFF
}

/// The mesh a mesh particle is drawn as.
///
/// The built-in shapes are unit sized (they fit a 1 m cube centred on the
/// origin), so a particle's `size` is its size in metres whichever it is.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParticleMesh {
    Cube,
    Sphere,
    Cylinder,
    Capsule,
    Cone,
    Torus,
    Tetrahedron,
    /// A one-sided square facing +Z, for a leaf or a card that tumbles in 3D
    /// rather than turning to the camera.
    Quad,
    /// A model in the project: a `.glb` / `.gltf` path, its first mesh
    /// unless the path names one (`rock.glb#Mesh2/Primitive0`).
    Asset(String),
}

impl ParticleMesh {
    /// The built-in shapes, in the order the editor lists them.
    pub const BUILT_IN: [ParticleMesh; 8] = [
        ParticleMesh::Cube,
        ParticleMesh::Sphere,
        ParticleMesh::Cylinder,
        ParticleMesh::Capsule,
        ParticleMesh::Cone,
        ParticleMesh::Torus,
        ParticleMesh::Tetrahedron,
        ParticleMesh::Quad,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ParticleMesh::Cube => "Cube",
            ParticleMesh::Sphere => "Sphere",
            ParticleMesh::Cylinder => "Cylinder",
            ParticleMesh::Capsule => "Capsule",
            ParticleMesh::Cone => "Cone",
            ParticleMesh::Torus => "Torus",
            ParticleMesh::Tetrahedron => "Tetrahedron",
            ParticleMesh::Quad => "Quad",
            ParticleMesh::Asset(_) => "Model",
        }
    }
}

/// How a mesh particle is turned. Each mode is a full 3D orientation built
/// from data every particle already has (its rotation angle, velocity and
/// per-particle seed), so no graph needs a new output to use one, and the GPU
/// pool keeps its layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum MeshOrient {
    /// A random orientation per particle, spun about a random axis by the
    /// particle's rotation: debris, rubble, popcorn. Give the particles an
    /// angular velocity and they tumble as they fly.
    #[default]
    Tumble,
    /// The mesh's +Y along the velocity, rolled about it by the rotation:
    /// arrows, shell casings, raindrops.
    Velocity,
    /// Spun about a fixed axis (in emitter space) by the rotation: a coin
    /// standing on edge, a leaf that stays level as it turns.
    Axis([f32; 3]),
}

/// Scene lighting for an emitter. Unlit is how particles have always drawn:
/// fire and magic are light sources, not surfaces, and lighting them would
/// only dim them. Smoke in sunlight and debris are surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lighting {
    /// Shade with Bevy's PBR lighting: the directional light and its shadows,
    /// point and spot lights, ambient and the environment map. A quad is lit
    /// as a flat card facing the camera; a mesh by its own normals.
    pub lit: bool,
    /// 0 a mirror, 1 chalk.
    pub roughness: f32,
    /// 0 a dielectric, 1 a metal.
    pub metallic: f32,
}

impl Default for Lighting {
    fn default() -> Self {
        Self { lit: false, roughness: 0.7, metallic: 0.0 }
    }
}

/// What a [`Facing::Ribbon`] strip is drawn through.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RibbonKind {
    /// Each particle trails its own last positions: a comet, a sword swing's
    /// sparks, a firework's streak. The simulation keeps a short history per
    /// particle for it.
    #[default]
    Trail,
    /// One strip through the live particles in spawn order, oldest at the
    /// tail: the wake behind a moving `World` emitter. Particles are dealt
    /// round-robin into `groups` separate strips. This is how the old Hanabi
    /// ribbons were drawn, so a converted ribbon effect keeps its look.
    Connect,
}

/// Ribbon settings. Skipped in a file while they hold their defaults.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ribbon {
    pub kind: RibbonKind,
    /// `Trail`: positions kept per particle, the live one not counted. More is
    /// a longer, smoother trail, at two vertices each. Clamped to
    /// [`MAX_TRAIL_POINTS`].
    pub points: u32,
    /// `Trail`: metres (pixels in 2D) a particle must move before its next
    /// position is kept, so a slow particle's trail is not all one point and
    /// a fast one's covers ground. 0 keeps one every step.
    pub min_distance: f32,
    /// `Connect`: how many separate strips.
    pub groups: u32,
    /// The strip's width at its tail, as a fraction of the width at its head
    /// (the particle's size x).
    pub tail_width: f32,
    /// The colour's alpha at the tail, as a fraction of the head's.
    pub tail_alpha: f32,
    /// A flat camera-facing strip, or a round tube ([`RibbonShape`]). Last
    /// and skipped while `Flat`, so every ribbon saved before tubes existed
    /// re-saves unchanged.
    #[serde(skip_serializing_if = "is_default")]
    pub shape: RibbonShape,
    /// [`RibbonShape::Tube`] only: vertices around the tube, clamped to
    /// [`MIN_TUBE_SEGMENTS`]..=[`MAX_TUBE_SEGMENTS`].
    #[serde(skip_serializing_if = "is_default_tube_segments", default = "default_tube_segments")]
    pub radial_segments: u32,
}

/// What cross-section a ribbon has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RibbonShape {
    /// A strip turned toward the camera along its length: two vertices a
    /// point, and from the side it reads as a line of light.
    #[default]
    Flat,
    /// A round tube of `radial_segments` around, its diameter the strip's
    /// width: real geometry, so it is lit by its own normals and has the
    /// same thickness from every side (a rope, a tentacle, a smoke ring).
    /// 3D only; a 2D effect draws it flat.
    Tube,
}

/// The longest trail a particle may keep.
pub const MAX_TRAIL_POINTS: u32 = 64;
/// The fewest vertices around a tube: three is a triangular prism, the least
/// that still encloses anything.
pub const MIN_TUBE_SEGMENTS: u32 = 3;
/// The most vertices around a tube. Every point of every trail pays for each
/// one, so this bounds what a typo can cost.
pub const MAX_TUBE_SEGMENTS: u32 = 32;
/// What a tube gets when it is switched on, and what a file that never said
/// means.
pub const DEFAULT_TUBE_SEGMENTS: u32 = 8;

fn default_tube_segments() -> u32 {
    DEFAULT_TUBE_SEGMENTS
}

fn is_default_tube_segments(v: &u32) -> bool {
    *v == DEFAULT_TUBE_SEGMENTS
}

impl Default for Ribbon {
    fn default() -> Self {
        Self {
            kind: RibbonKind::Trail,
            points: 16,
            min_distance: 0.05,
            groups: 1,
            tail_width: 0.0,
            tail_alpha: 0.0,
            shape: RibbonShape::Flat,
            radial_segments: DEFAULT_TUBE_SEGMENTS,
        }
    }
}

impl Ribbon {
    /// The trail history length, clamped.
    pub fn trail_points(&self) -> usize {
        self.points.clamp(1, MAX_TRAIL_POINTS) as usize
    }

    /// Vertices around one ring of a tube, clamped.
    pub fn tube_segments(&self) -> u32 {
        self.radial_segments.clamp(MIN_TUBE_SEGMENTS, MAX_TUBE_SEGMENTS)
    }
}

/// The order an emitter's particles are drawn in, which decides what shows
/// on top where alpha-blended particles overlap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DrawOrder {
    /// However the backend holds them, at no cost: the right choice whenever
    /// the order cannot be seen (additive light, a depth-written blend) or
    /// does not matter.
    #[default]
    Index,
    /// The oldest particles first, so the newest draw over them: a fresh
    /// puff of smoke over the haze it rose from.
    OldestFirst,
    /// The newest first, so older particles draw over them: a trail of
    /// decals whose oldest layer must stay on top.
    YoungestFirst,
    /// Back to front from the camera, which is what alpha blending wants
    /// when particles cross each other in depth. 3D only. The old `sort:
    /// true` is this.
    ViewDepth,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            texture: None,
            sprite_sheet: SpriteSheet::default(),
            blend: BlendMode::Alpha,
            facing: Facing::CameraBillboard,
            z_order: 0.0,
            shading: Shading::Sprite,
            stretch: 0.0,
            frame_blend: false,
            erosion: 0.0,
            soft_distance: 0.0,
            ribbon: Ribbon::default(),
            sort: false,
            draw_order: DrawOrder::Index,
            mesh: None,
            orient: MeshOrient::Tumble,
            lighting: Lighting::default(),
            material: None,
            alpha_cutoff: DEFAULT_ALPHA_CUTOFF,
            screen_space_size: false,
            shadows: ParticleShadows::Auto,
            render_method: RenderMethod::Auto,
        }
    }
}

impl RenderSettings {
    /// Whether this emitter's particles are sorted back to front: asked for,
    /// in 3D (a 2D effect has no depth to sort by), with a blend that depends
    /// on order (not additive, and not the depth-written opaque and masked
    /// modes, which the depth buffer orders for free), and not a ribbon (a
    /// strip is drawn in trail order).
    pub fn sorts(&self, dimension: Dimension) -> bool {
        self.draw_order(dimension) == DrawOrder::ViewDepth
    }

    /// The draw order the renderer applies: [`Self::draw_order`]'s field, or
    /// `ViewDepth` for an old file's `sort: true`, and `Index` wherever the
    /// order cannot show: additive light adds up the same in any order, the
    /// depth-written blends are ordered by the depth buffer, a ribbon is
    /// drawn along its strip, and a 2D effect has no depth to sort by (the
    /// age orders still apply there).
    pub fn draw_order(&self, dimension: Dimension) -> DrawOrder {
        let wanted = match (self.draw_order, self.sort) {
            (DrawOrder::Index, true) => DrawOrder::ViewDepth,
            (order, _) => order,
        };
        if !matches!(self.blend, BlendMode::Alpha | BlendMode::Premultiplied) || self.draws_ribbon(dimension) {
            return DrawOrder::Index;
        }
        if wanted == DrawOrder::ViewDepth && dimension != Dimension::ThreeD {
            return DrawOrder::Index;
        }
        wanted
    }

    /// Set the draw order the way the editor saves it: the new field, and the
    /// old `sort` flag kept in step so an older engine still sorts by depth.
    pub fn set_draw_order(&mut self, order: DrawOrder) {
        self.draw_order = order;
        self.sort = order == DrawOrder::ViewDepth;
    }

    /// Whether the ribbon is drawn as a tube: asked for, and in 3D.
    pub fn draws_tube(&self, dimension: Dimension) -> bool {
        self.draws_ribbon(dimension) && self.ribbon.shape == RibbonShape::Tube && dimension == Dimension::ThreeD
    }

    /// Whether the particles are meshes: a mesh is set and the effect is 3D.
    /// A 2D effect ignores the mesh (the 2D renderer draws quads only).
    pub fn draws_mesh(&self, dimension: Dimension) -> bool {
        self.mesh.is_some() && dimension == Dimension::ThreeD
    }

    /// Whether the particles are ribbons. A mesh wins over the Ribbon facing,
    /// since facing does not apply to a mesh at all.
    pub fn draws_ribbon(&self, dimension: Dimension) -> bool {
        self.facing == Facing::Ribbon && !self.draws_mesh(dimension)
    }

    /// The project material the particles wear, when they wear one: 3D and a
    /// non-blank path. Quads, ribbons and meshes all take one; a 2D effect
    /// draws through `Material2d`, which a 3D `.material` cannot run in.
    pub fn material_path(&self, dimension: Dimension) -> Option<&str> {
        if dimension != Dimension::ThreeD {
            return None;
        }
        self.material.as_deref().map(str::trim).filter(|p| !p.is_empty())
    }

    /// [`Self::material_path`] for mesh particles only, which is all a
    /// material could be worn by before quads and ribbons took one.
    pub fn mesh_material(&self, dimension: Dimension) -> Option<&str> {
        self.material_path(dimension).filter(|_| self.draws_mesh(dimension))
    }

    /// The render method the 3D material asks for: [`Self::render_method`]
    /// for a depth-written blend, `Forward` for every transparent one, which
    /// no G-buffer can hold (see [`RenderMethod`]).
    pub fn effective_render_method(&self) -> RenderMethod {
        if self.blend.writes_depth() {
            self.render_method
        } else {
            RenderMethod::Forward
        }
    }

    /// Whether the particles cast shadows: 3D, and per [`ParticleShadows`].
    pub fn casts_shadows(&self, dimension: Dimension) -> bool {
        dimension == Dimension::ThreeD
            && match self.shadows {
                ParticleShadows::Auto => self.blend.writes_depth(),
                ParticleShadows::On => true,
                ParticleShadows::Off => false,
            }
    }

    /// The facing the renderer works with. For quads it is `facing`; for a
    /// mesh it is what `orient` needs from the vertex: the velocity for
    /// [`MeshOrient::Velocity`] (so the stretch applies along it, as for a
    /// velocity-aligned quad), the axis for [`MeshOrient::Axis`], and nothing
    /// for a tumble. Both backends already deliver exactly those per facing,
    /// which is why the mesh path needs no data of its own.
    pub fn effective_facing(&self, dimension: Dimension) -> Facing {
        if !self.draws_mesh(dimension) {
            return self.facing;
        }
        match self.orient {
            MeshOrient::Tumble => Facing::CameraBillboard,
            MeshOrient::Velocity => Facing::VelocityAligned,
            MeshOrient::Axis(a) => Facing::FixedAxis(a),
        }
    }
}

/// What a particle looks like inside its quad.
///
/// The two procedural modes exist because a convincing flame needs a texture
/// most projects do not have: a flipbook rendered from a fluid sim. Both draw
/// from noise in the fragment shader, seeded per particle and animated by the
/// clock, so every particle is a different shape and none of them repeats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Shading {
    /// The texture (or the soft dot without one), tinted by the colour.
    #[default]
    Sprite,
    /// A flame tongue: a teardrop stretched along the quad's height, its edges
    /// eroded by rising noise, its temperature mapped through a blackbody
    /// ramp. Draw it `Additive` and `VelocityAligned`; the colour's rgb is an
    /// HDR tint and its alpha is the heat (1 = white-hot core, 0.5 = orange).
    ProceduralFlame,
    /// A soft billowing puff with eroded edges and a top-lit look. Draw it
    /// `Alpha`; the colour is the smoke's colour and opacity.
    ProceduralSmoke,
}

/// A texture split into a `columns` x `rows` grid of frames, read left to
/// right, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpriteSheet {
    pub columns: u32,
    pub rows: u32,
    pub mode: SheetMode,
}

impl Default for SpriteSheet {
    fn default() -> Self {
        Self {
            columns: 1,
            rows: 1,
            mode: SheetMode::OverLifetime,
        }
    }
}

impl SpriteSheet {
    pub fn frame_count(&self) -> u32 {
        self.columns.max(1) * self.rows.max(1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SheetMode {
    /// Play the whole sheet once across each particle's life.
    OverLifetime,
    /// Play at a fixed rate, looping.
    Fps(f32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    /// Ordinary alpha blending: smoke, confetti.
    #[default]
    Alpha,
    /// Light adds up: fire, sparks, magic.
    Additive,
    /// The texture's colour is already multiplied by its alpha.
    Premultiplied,
    /// Solid where the alpha is at least `alpha_cutoff`, absent elsewhere:
    /// leaves, grass, chain-link. Writes depth and draws in the alpha-mask
    /// phase, so it sorts with the scene and costs no particle sort.
    AlphaMask,
    /// Fully solid: debris, coins. Writes depth and draws in the opaque phase.
    Opaque,
    /// Darkens what is behind by the colour, faded by the alpha: soot, scorch
    /// marks, shadows of a swarm. Order independent (multiplication commutes),
    /// so it never needs a sort. Bevy's `AlphaMode::Multiply` in 3D.
    Multiply,
    /// A mask whose edge is antialiased by the camera's MSAA: the alpha
    /// becomes sample coverage, so foliage cut-outs lose their stair steps
    /// and still write depth. Without MSAA it is an alpha mask. Bevy's
    /// `AlphaMode::AlphaToCoverage` in 3D; a mask in 2D, which has no such
    /// mode.
    AlphaToCoverage,
}

impl BlendMode {
    /// Whether the mode writes depth and draws with the scene's opaque
    /// geometry rather than in the transparent pass.
    pub fn writes_depth(self) -> bool {
        matches!(self, BlendMode::AlphaMask | BlendMode::Opaque | BlendMode::AlphaToCoverage)
    }

    /// Whether the mode reads `alpha_cutoff`.
    pub fn uses_cutoff(self) -> bool {
        matches!(self, BlendMode::AlphaMask | BlendMode::AlphaToCoverage)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Facing {
    /// The quad always faces the camera.
    #[default]
    CameraBillboard,
    /// The quad's height runs along the velocity and its face turns toward the
    /// camera around it: streaks, sparks, rain.
    VelocityAligned,
    /// The quad's height runs along a fixed axis (in emitter space) and its
    /// face turns toward the camera around it: a cylindrical billboard.
    FixedAxis([f32; 3]),
    /// A camera-facing strip instead of a quad, through each particle's trail
    /// or through the particles themselves (see [`Ribbon`]). UVs run along
    /// it, head to tail, so `ProceduralFlame` draws a flame licking back
    /// along the trail and a texture stretches down its length.
    Ribbon,
}

// ── Loading, versions and migrations ────────────────────────────────────────

/// Why a `.vfx` could not be read.
#[derive(Debug)]
pub enum VfxLoadError {
    Io(std::io::Error),
    Utf8(std::str::Utf8Error),
    Ron(ron::error::SpannedError),
    /// Written by a newer engine than this one.
    TooNew { found: u32, supported: u32 },
}

impl std::fmt::Display for VfxLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VfxLoadError::Io(e) => write!(f, "could not read the effect: {e}"),
            VfxLoadError::Utf8(e) => write!(f, "the effect is not UTF-8 text: {e}"),
            VfxLoadError::Ron(e) => write!(f, "the effect is not valid RON: {e}"),
            VfxLoadError::TooNew { found, supported } => write!(
                f,
                "the effect is version {found} but this engine reads up to version {supported}; \
                 update the engine to open it"
            ),
        }
    }
}

impl std::error::Error for VfxLoadError {}

impl From<std::io::Error> for VfxLoadError {
    fn from(e: std::io::Error) -> Self {
        VfxLoadError::Io(e)
    }
}

impl From<ron::error::SpannedError> for VfxLoadError {
    fn from(e: ron::error::SpannedError) -> Self {
        VfxLoadError::Ron(e)
    }
}

/// Only the version, read before anything else. Unknown fields are skipped, so
/// this parses any effect from any engine.
#[derive(Deserialize)]
struct VersionProbe {
    #[serde(default)]
    version: u32,
}

/// One migration step: upgrades an asset that was written as `from` to
/// `from + 1`. Runs on the typed asset, after serde has filled in defaults.
pub struct Migration {
    pub from: u32,
    pub apply: fn(&mut VfxAsset),
}

/// Every step, in order. Version 0 is a file with no `version` field at all:
/// written by hand before the format was versioned, or by someone who left it
/// out. Its one difference from version 1 is that an omitted or zero capacity
/// meant "the default" rather than "nothing", so it gets one.
pub const MIGRATIONS: &[Migration] = &[Migration {
    from: 0,
    apply: |asset| {
        for emitter in &mut asset.emitters {
            if emitter.capacity == 0 {
                emitter.capacity = DEFAULT_CAPACITY;
            }
        }
    },
}];

/// Bring an asset written as `from` up to [`VFX_VERSION`].
pub fn migrate(asset: &mut VfxAsset, from: u32) {
    for step in MIGRATIONS {
        if step.from >= from && step.from < VFX_VERSION {
            (step.apply)(asset);
        }
    }
    asset.version = VFX_VERSION;
}

/// Parse a `.vfx` document, migrating it to the current version.
pub fn parse_vfx(text: &str) -> Result<VfxAsset, VfxLoadError> {
    let probe: VersionProbe = ron::from_str(text)?;
    if probe.version > VFX_VERSION {
        return Err(VfxLoadError::TooNew {
            found: probe.version,
            supported: VFX_VERSION,
        });
    }
    let mut asset: VfxAsset = ron::from_str(text)?;
    // A missing field deserializes to the struct default, which is the
    // CURRENT version; the probe is what knows the file really said nothing.
    migrate(&mut asset, probe.version);
    Ok(asset)
}

/// Serialize an effect the way the editor will save it.
pub fn write_vfx(asset: &VfxAsset) -> Result<String, ron::Error> {
    let config = ron::ser::PrettyConfig::new()
        .depth_limit(16)
        .struct_names(false)
        .indentor("  ".to_string());
    ron::ser::to_string_pretty(asset, config)
}

/// Bevy loader for `.vfx`. Hot reload comes from the asset server: the project
/// watcher reports the change, Bevy reloads the asset and emits
/// `AssetEvent::Modified`, and the runtime rebuilds every instance of it.
#[derive(Default, TypePath)]
pub struct VfxAssetLoader;

impl AssetLoader for VfxAssetLoader {
    type Asset = VfxAsset;
    type Settings = ();
    type Error = VfxLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let text = std::str::from_utf8(&bytes).map_err(VfxLoadError::Utf8)?;
        parse_vfx(text)
    }

    fn extensions(&self) -> &[&str] {
        &[VFX_EXTENSION]
    }
}
