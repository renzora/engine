//! Shared ECS components & entity-tag markers.
//!
//! Split out of `core/mod.rs` to keep it manageable. Holds the boundary-crossing
//! components and marker types — entity tags, the editor/viewport/scene camera
//! markers + camera presets/exposure, render-phase routing, hierarchy/selection
//! flags, the extracted-PBR material mirror (`PbrAdvanced` et al.), mesh
//! primitives and `MeshInstanceData`. Re-exported from `core`
//! (`pub use components::*`) so every `renzora::Foo` path is unchanged.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::viewport_types;

/// A shared **group / class** label for an entity — the CSS-`class` analogue,
/// deliberately NOT an identity.
///
/// Many entities can carry the same group (e.g. `"inventory"`), which is exactly
/// how the `<for group="…">` markup repeat-list finds every member to render one
/// row each. This is separate from an entity's unique **id** (its [`Name`], see
/// [`crate::core::entity_id`]) — ids are one-per-entity, groups are many-to-one.
#[derive(Component, Default, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EntityGroup {
    pub group: String,
}

/// Marker component for the editor's scene-navigation camera.
///
/// This camera is used for orbit/pan/zoom during editing and renders to the
/// viewport texture. It is hidden from the hierarchy and cannot be deleted.
/// User-created scene cameras are separate entities.
#[derive(Component)]
pub struct EditorCamera;

/// Marker component for the editor's 2D scene-navigation camera.
///
/// Sibling of [`EditorCamera`]: orthographic, attached to the same viewport
/// render target, but only active when `ViewportSettings.viewport_view` is
/// [`ViewportView::Two`]. Pan with middle-mouse, zoom with scroll.
#[derive(Component)]
pub struct EditorCamera2d;

/// Identifies which of the multi-viewport slots a 3D editor camera belongs to.
///
/// There are [`viewport_types::VIEWPORT_COUNT`] of these cameras, one per
/// viewport panel (`viewport`, `viewport-2`, …). Each renders the same scene
/// from its own angle into its own render-target image. The *focused* slot's
/// camera additionally carries the [`EditorCamera`] marker so the existing
/// single-camera gizmo / picking / overlay systems all operate on whichever
/// viewport the user is interacting with — see `Viewports` in
/// [`viewport_types`].
#[derive(Component, Clone, Copy, Debug)]
pub struct ViewportCamera(pub usize);

/// Identifies which multi-viewport slot a *2D* editor camera belongs to.
///
/// The 2D sibling of [`ViewportCamera`]: there is one orthographic `Camera2d`
/// per viewport slot, each rendering the same 2D scene into its own slot image
/// with its own independent pan/zoom. Only active while the global
/// [`viewport_types::ViewportView`] is `Two`. The *focused* slot's 2D camera
/// additionally carries the [`EditorCamera2d`] marker so the 2D picker / grid /
/// overlays all operate on whichever viewport the user is interacting with —
/// the same focused-mirror trick [`ViewportCamera`] uses for 3D.
#[derive(Component, Clone, Copy, Debug)]
pub struct ViewportCamera2d(pub usize);

/// Marker for viewport slot 0's camera specifically. Unlike [`EditorCamera`]
/// (which follows focus), this never moves off slot 0 — used as the stable
/// "default focus" view.
#[derive(Component, Clone, Copy, Debug)]
pub struct PrimaryViewportCamera;

/// Marker component tagging an entity as a 2D scene node.
///
/// Currently semantically equivalent to a plain `Transform` parent, but
/// distinguished so the editor can: (a) auto-switch the viewport to 2D
/// view when one is selected, and (b) show a 2D-specific hierarchy icon
/// instead of the generic folder/circle.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Node2d;

/// Editor pick/selection half-extents for a spriteless 2D entity.
///
/// Spriteless 2D nodes normally pick against a small fixed marker box, which
/// is fine for point-like things (lights, occluders) but useless for entities
/// with real spatial extent and no sprite — a particle emitter spanning
/// hundreds of pixels would only be grabbable in a 20px square at its origin.
/// A system that knows the entity's true footprint (e.g. the hanabi sync from
/// the effect's emitter shape) inserts this, and the 2D picker/overlay use it
/// in place of the marker box. Derived data — deliberately NOT reflected, so
/// scene save never serializes it.
#[derive(Component, Clone, Copy, Debug)]
pub struct PickBounds2d {
    /// Half-extents of the pick box, in world units.
    pub half_extents: Vec2,
    /// Centre of the pick box in the entity's LOCAL frame. Non-zero when the
    /// footprint is asymmetric around the origin — a snow emitter's particles
    /// all travel below it, a fire plume rises above it — so the box tracks
    /// where the effect actually is instead of ballooning symmetrically.
    pub offset: Vec2,
}

/// Asset-relative path of the image bound to a `Sprite`.
///
/// Mirror of `UiImagePath` for sprites. Bevy's `Sprite.image` holds a
/// `Handle<Image>`, which doesn't survive scene save/load — handle IDs
/// are runtime-only and don't remap. This component stores the path so
/// a rehydration system can re-load the image and assign the handle on
/// scene load (or whenever the path changes via the inspector / a
/// drag-drop).
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SpriteImagePath(pub String);

/// Persisted mirror of `Sprite.custom_size` for a user-resized sprite.
///
/// Bevy's `Sprite` isn't `#[reflect(Serialize, Deserialize)]`, so scene save
/// drops the whole component — including the size set by the 2D resize
/// handles — and the rehydration path rebuilds a fresh `Sprite` at the
/// image's native dimensions. This component IS serializable; an editor-side
/// system keeps it in sync with `Sprite.custom_size` (present only while a
/// custom size is set), and the load path applies it back so a resized
/// sprite reopens at the size it was saved with.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SpriteCustomSize(pub Vec2);

/// Y-sort: derive this 2D entity's Z from its world Y every frame, so sprites
/// lower on screen draw in front — the classic top-down "walk behind the tree"
/// ordering.
///
/// Bevy's 2D transparent pass already sorts by Z, so no render-pipeline work is
/// involved: a runtime system just overwrites `Transform.translation.z` with
/// `z_base - (world_y + offset) * scale`. Z becomes a *derived* value for these
/// entities — whatever gets saved in the scene is harmlessly recomputed on load.
///
/// `offset` moves the sort point away from the sprite's center: sprites pivot
/// at their middle, but a tall tree should sort by its trunk base, so a tree
/// sprite uses a negative offset of roughly half its height. `z_base` is the
/// layer band the entity sorts within (±0.5 around it), letting y-sorted props
/// sit above a ground tilemap at z 0 and below UI/overlay content.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[reflect(Component, Default, Serialize, Deserialize)]
pub struct YSort {
    /// Center of the Z band this entity sorts within.
    pub z_base: f32,
    /// World units added to the entity's Y before sorting (negative = sort
    /// point below the sprite center, e.g. at a character's feet).
    pub offset: f32,
}

impl Default for YSort {
    fn default() -> Self {
        Self { z_base: 1.0, offset: 0.0 }
    }
}

/// Grid-based sprite-sheet cropping: slice a `Sprite`'s texture into
/// `hframes` columns × `vframes` rows and show one cell at a time.
///
/// This is the persistent, animatable side of frame cropping. Bevy's
/// `Sprite.rect` is the runtime truth, but it's pixel coordinates tied to
/// one specific image and (like the rest of `Sprite`) doesn't survive scene
/// save/load — so this component stores the *grid*, and an engine system
/// derives the rect from it plus the loaded image's dimensions every frame.
/// Deriving late also covers async image loads and texture swaps, where a
/// rect computed at insert time would be stale or unavailable.
///
/// `frame` is a row-major cell index (`frame = row * hframes + column`) and
/// wraps modulo the cell count, so a property-animation track can sweep
/// `0 → hframes*vframes` linearly for a looping flipbook. It lives here —
/// not in a separate component — so the animation panel's reflection picker
/// offers `SpriteSheet.frame` as an animatable field on any sprite that has
/// the component.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SpriteSheet {
    /// Number of columns the texture is sliced into (min 1).
    pub hframes: u32,
    /// Number of rows the texture is sliced into (min 1).
    pub vframes: u32,
    /// Row-major cell index to display; wraps modulo `hframes * vframes`.
    pub frame: u32,
}

impl Default for SpriteSheet {
    fn default() -> Self {
        Self { hframes: 1, vframes: 1, frame: 0 }
    }
}

/// A rectangular sub-region of an atlas image, in atlas cells — the persistent
/// crop for a multi-tile "object" drawn as a **single** sprite (e.g. a tree
/// stamped from a 3×3 tilemap-palette block is one entity, not nine tiles).
///
/// Sibling of [`SpriteSheet`]: where that picks *one* cell of a grid, this
/// selects a `w × h` block starting at cell `(col, row)`. Like `SpriteSheet`
/// it's the saved, image-independent description — Bevy's `Sprite.rect` is
/// runtime-only and dropped by scene save — and an engine system derives the
/// pixel rect from it every frame. The rect is pure arithmetic on the stored
/// `tile_px`, so (unlike `SpriteSheet`) it needs no loaded image: cell `c`
/// maps to pixel `c * tile_px`.
///
/// An entity has at most one of `SpriteSheet` / `SpriteAtlasRegion`; the two
/// derive systems key off their own component, so they never fight over
/// `Sprite.rect`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SpriteAtlasRegion {
    /// Top-left atlas cell column of the block.
    pub col: u32,
    /// Top-left atlas cell row of the block.
    pub row: u32,
    /// Block width in atlas cells (min 1).
    pub w: u32,
    /// Block height in atlas cells (min 1).
    pub h: u32,
    /// Pixel size of one atlas cell (square). Converts cells → pixels for the
    /// derived rect.
    pub tile_px: u32,
}

/// A reflection probe's authored environment source — the *persistent* side of a
/// parallax-corrected cubemap probe.
///
/// Bevy's `GeneratedEnvironmentMapLight` is the runtime GPU side: its filter
/// **runs the moment that component exists** and demands a **power-of-two cube**
/// texture, so attaching it with an unset (1×1 default) or equirectangular
/// handle spams GPU validation errors. To avoid that, a probe carries *this*
/// component instead, and `renzora_environment_map` only inserts
/// `GeneratedEnvironmentMapLight` **once a valid cube is ready** — loading the
/// `path`, reprojecting an equirect `.exr`/`.hdr` into a POT cube (or using a
/// `.ktx2`/`.dds` cube directly), and applying `intensity`. Only this component
/// persists in the scene; the cube is regenerated on load.
#[derive(Component, Reflect, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ReflectionProbeSource {
    /// Project-relative path to the source image (equirect HDR or cube container).
    pub path: String,
    /// Strength multiplier applied to the probe's reflections (cd/m²).
    pub intensity: f32,
}

impl Default for ReflectionProbeSource {
    fn default() -> Self {
        Self { path: String::new(), intensity: 1.0 }
    }
}

/// Marker component to hide an entity (and its children) from the hierarchy panel.
#[derive(Component)]
pub struct HideInHierarchy;

/// Set on an entity while a sound it owns is actually sounding.
///
/// Maintained by `renzora_audio` from its live-voice bookkeeping, and read by the
/// hierarchy panel to show a playing indicator. It lives here, in the contract
/// crate, precisely so the hierarchy does not have to link the audio crate to ask
/// a one-bit question — the pattern the plugin split exists to encourage.
///
/// Deliberately *not* serialized: it describes this instant, not the scene.
#[derive(Component)]
pub struct AudioEmitting;

/// Canonical render-pass ordering phases for the Bevy 0.19 `Core3d` schedule —
/// the centralized "render composition" pipeline (see `docs/render-composition.md`
/// and `renzora::postprocess`). Bevy deleted the render graph in 0.19 and moved
/// to system ordering; this enum is the single shared vocabulary so renzora's
/// many view-target passes (GI, reflections, post-process, …) slot into a known
/// order instead of each hardcoding `.before(some_other_system)`.
///
/// Phases are interleaved with bevy's own post-process systems, which act as
/// fixed anchors (the render-composition framework places these phases around
/// them in ONE place):
///
/// ```text
/// MainPass ─ Gi ─ [bevy TAA] ─ HdrPost ─ [bevy tonemapping] ─ LdrPost ─ [fxaa/smaa] ─ Overlay
/// ```
///
/// A render pass joins a phase with `.in_set(renzora::RenderPhase::Gi)` (for a
/// system pass) or by registering a handler in that phase (data-driven, for the
/// future node-graph pipeline editor) — and never references another pass.
#[derive(
    bevy::ecs::schedule::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum RenderPhase {
    /// HDR/linear, after the main 3D pass and BEFORE temporal AA: global
    /// illumination composite, screen-space reflections. Running before TAA is
    /// what puts GI in the temporal history (otherwise: SSGI flicker / SDF grey).
    Gi,
    /// HDR, after temporal AA: bloom, depth-of-field, motion blur.
    HdrPost,
    /// LDR, after tonemapping: color grading, vignette, and the rest of the
    /// unified post-process effects.
    LdrPost,
    /// Final overlays (debug visualizations, gizmo composites) — after AA.
    Overlay,
}

/// Editor viewport gate: this scene entity was force-hidden because no
/// viewport panel is visible, and the stored value is its *authored*
/// `Visibility` (the slot-0 editor camera must stay active for the
/// atmosphere/IBL probe, so hiding the scene is how a viewport-less workspace
/// stops paying for shadow maps, GI and mesh extraction — see
/// `renzora_viewport::gate_scene_visibility`).
///
/// Deliberately NOT `Reflect`: it must never serialize into a scene file.
/// Scene saves restore the stored value before extracting so the authored
/// visibility is what lands on disk (see `renzora_engine::scene_io`); the
/// hierarchy panel's eye icon reads it for the same reason.
#[derive(Component, Clone, Copy)]
pub struct ViewportGateHidden(pub Visibility);

/// Marker component — entity persists across scene loads (e.g. loader UI root).
/// `process_pending_scene_loads` and similar despawn-the-world logic must skip these.
///
/// Auto-applied to every entity spawned from an autoload scene (see
/// `renzora_engine::autoload`). The component is also reflected so users can
/// hand-tag arbitrary entities from the inspector if they ever need to.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Persistent;

/// Marker component — entity is locked from editing in the hierarchy.
#[derive(Component)]
pub struct EditorLocked;

/// Marker component — viewport picking stops at this entity instead of walking
/// past it to a higher-up named ancestor. Apply to compound entities (terrains,
/// prefab roots, etc.) that own many named children but should be selectable
/// as a unit.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct SelectionStop;

/// Marker component — camera should be excluded from scene-wide effects (skybox, post-processing).
#[derive(Component)]
pub struct IsolatedCamera;

/// Marks an entity as the root of a nested-scene instance.
///
/// The `source` field is an asset-relative path to the `.ron` scene file that
/// provides the instance's contents. In the host scene file, only this root
/// entity (with its transform + any host-level overrides) is serialized; the
/// instance's child entity tree lives in the referenced source file and is
/// expanded on load.
///
/// Edits to entities *inside* an instance tree autosave back to the source
/// file. Edits to the instance root's transform persist in the host scene as
/// per-instance placement overrides.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SceneInstance {
    /// Asset-relative path to the source `.ron` scene file.
    pub source: String,
    /// When `true`, the instance participates in world streaming: in a
    /// running game (and editor play/simulate) its contents expand only while
    /// the camera is within `load_radius` of the instance root, and collapse
    /// again beyond `unload_radius`. In editor *edit* mode streamed instances
    /// stay fully expanded so designers can see and edit them. All streaming
    /// fields are `#[reflect(default)]`/`#[serde(default)]` so scenes saved
    /// before they existed still deserialize.
    #[serde(default)]
    #[reflect(default)]
    pub streamed: bool,
    /// Camera distance (world units) at which a streamed instance loads.
    #[serde(default = "default_instance_load_radius")]
    #[reflect(default = "default_instance_load_radius")]
    pub load_radius: f32,
    /// Camera distance at which a streamed instance unloads. Kept above
    /// `load_radius` at evaluation time (hysteresis) so an instance sitting
    /// on the boundary doesn't thrash load/unload every frame.
    #[serde(default = "default_instance_unload_radius")]
    #[reflect(default = "default_instance_unload_radius")]
    pub unload_radius: f32,
}

fn default_instance_load_radius() -> f32 {
    150.0
}
fn default_instance_unload_radius() -> f32 {
    200.0
}

impl Default for SceneInstance {
    fn default() -> Self {
        Self {
            source: String::new(),
            streamed: false,
            load_radius: default_instance_load_radius(),
            unload_radius: default_instance_unload_radius(),
        }
    }
}

/// A 3D Gaussian-splat cloud instance (`.ply` / `.gcloud`).
///
/// Stores the project-relative *path* to the cloud file rather than an asset
/// `Handle` — the same path-in-component / handle-at-runtime split models,
/// audio, and particles use — so the component survives the reflection-driven
/// scene serializer. The `renzora_gaussian_splatting` distribution plugin
/// watches for added/changed components and resolves `source` into the live
/// `bevy_gaussian_splatting` cloud handle + `CloudSettings`; without that
/// plugin in `plugins/` the component is inert data.
///
/// Lives in the contract crate (not the plugin) because it crosses the dlopen
/// boundary: the viewport drop handler and inspector run in the host while the
/// sync system runs in the plugin, and both must agree on one `TypeId`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct GaussianSplat {
    /// Project-relative path to the `.ply` / `.gcloud` cloud file.
    pub source: String,
    /// Uniform opacity multiplier applied to every splat (0–1).
    #[serde(default = "default_splat_multiplier")]
    #[reflect(default = "default_splat_multiplier")]
    pub opacity: f32,
    /// Uniform size multiplier applied to every splat. This scales the
    /// individual gaussians, not the entity transform — use it to fatten or
    /// thin the splats without moving them.
    #[serde(default = "default_splat_multiplier")]
    #[reflect(default = "default_splat_multiplier")]
    pub splat_scale: f32,
}

fn default_splat_multiplier() -> f32 {
    1.0
}

impl Default for GaussianSplat {
    fn default() -> Self {
        Self {
            source: String::new(),
            opacity: default_splat_multiplier(),
            splat_scale: default_splat_multiplier(),
        }
    }
}

/// Contract between the editor and the XR layer for in-editor VR play.
///
/// Lives in the contract crate because the two sides never link each other:
/// the editor (viewport play-mode code) flips `requested` when the "VR
/// Headset" play target starts/stops, and the statically-linked `renzora_xr`
/// session controller — present only when the engine booted XR-capable —
/// creates/ends the OpenXR session to match and reports back through
/// `active`. Booting XR-capable requires an OpenXR runtime at launch; when
/// none was found this resource simply never gets inserted, which is how the
/// editor knows to tell the user VR is unavailable.
#[derive(Resource, Default, Debug)]
pub struct VrPlayState {
    /// Editor wants the headset session up (set on VR play, cleared on stop).
    pub requested: bool,
    /// The OpenXR session is running and rendering to the headset.
    pub active: bool,
}

/// One device's pointing ray at a world-space UI panel this frame.
///
/// `id` names the producer and must be unique across them — see
/// [`WorldUiPointers`] for the assignments. It also seeds the `bevy_picking`
/// pointer id the panel system spawns, so it has to stay stable frame to
/// frame or the pointer would be torn down and rebuilt every tick.
///
/// `trigger` is an analog 0..1 press (a VR trigger's pull, or 0/1 for a mouse
/// button) rather than a bool, because the panel system applies press/release
/// hysteresis to it — a trigger resting near its threshold would otherwise
/// chatter clicks.
#[derive(Clone, Copy, Debug)]
pub struct WorldUiPointerRay {
    /// Producer id — unique per source, stable across frames.
    pub id: u8,
    /// Where the device is pointing, in world space.
    pub ray: bevy::math::Ray3d,
    /// Analog press, 0..1.
    pub trigger: f32,
}

/// Ordering for the [`WorldUiPointers`] producers and their consumer.
///
/// The producers live in crates that never link the consumer, so nothing else
/// can order them: without this, whether a click reached a panel on the frame
/// it happened or the frame after would depend on system-insertion order. The
/// consumer configures `Publish` before `Consume`; producers only declare
/// which side they're on.
#[derive(
    bevy::ecs::schedule::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum WorldUiPointerSet {
    /// Refill this frame's rays (wands, mouse).
    Publish,
    /// Turn them into `bevy_picking` pointers.
    Consume,
}

/// Every device currently pointing into the world, refilled each frame.
///
/// The contract behind world-space game UI: several crates that never link
/// each other produce rays, and one consumer (ember's world-panel system)
/// turns them into `bevy_picking` pointers. Producers can't just clear the
/// list — they'd delete each other's entries depending on system order — so
/// each owns fixed ids and retains away only its own before pushing:
///
/// | id | Producer |
/// |----|----------|
/// | 0, 1 | `renzora_xr` — left / right wand |
/// | 2 | the mouse: `renzora_viewport` in an editor session, ember's own publisher in the shipped game (each gates itself off in the other case, so only one writes) |
///
/// A producer that has nothing to publish (cursor left the viewport, VR play
/// stopped) simply retains its ids away and pushes nothing. The consumer
/// treats an absent id as "this device is gone" and releases whatever its
/// pointer was holding, so a button can't be left stuck down.
#[derive(Resource, Default, Debug)]
pub struct WorldUiPointers(pub Vec<WorldUiPointerRay>);

/// Serializable marker for a scene camera entity.
///
/// Stored alongside `Camera3d` so the camera can be recreated on scene load
/// (since `Camera3d` itself is not serializable).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SceneCamera;

/// Marks a camera as the default game camera for preview and play mode.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DefaultCamera;

/// Per-camera render-resolution scale (Full / Half / Quarter).
///
/// Sizes this camera's render target at a fraction of the display size and
/// upscales it. In the editor, the viewport reflects the resolution of the
/// relevant scene camera (selected → default → first); in play mode the active
/// camera's resolution drives the game render target. Absent ⇒ Full.
#[derive(Component, Clone, Copy, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CameraRenderResolution(pub viewport_types::RenderResolution);

/// One named camera angle — a captured world-space pose.
///
/// Stored in a [`CameraPresets`] list on a camera entity so the angle persists
/// in the scene RON and can be jumped to from scripting (`goto_camera_preset`)
/// or the inspector's "Camera Presets" section.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
pub struct CameraPreset {
    /// Lookup key used by scripting (`goto_camera_preset("name")`) and shown in
    /// the inspector list. Not required to be unique, but `goto` matches the
    /// first by name.
    pub name: String,
    /// World-space translation of the camera at capture time.
    pub translation: Vec3,
    /// World-space orientation of the camera at capture time.
    pub rotation: Quat,
}

impl CameraPreset {
    /// Build a preset from a name and a world-space transform.
    pub fn from_transform(name: impl Into<String>, transform: &Transform) -> Self {
        Self {
            name: name.into(),
            translation: transform.translation,
            rotation: transform.rotation,
        }
    }

    /// The pose as a [`Transform`] (scale left at one — camera scale is ignored).
    pub fn to_transform(&self) -> Transform {
        Transform {
            translation: self.translation,
            rotation: self.rotation,
            scale: Vec3::ONE,
        }
    }
}

/// A list of named camera angles attached to a camera entity.
///
/// Authored in the inspector ("Camera Presets" section → *Capture current
/// view*) and serialized into the scene. A script on the same entity can jump
/// the camera to any preset by name with `goto_camera_preset("name")`, or query
/// the list with `camera_preset_count()` / `camera_preset_name(i)`.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CameraPresets {
    pub presets: Vec<CameraPreset>,
}

impl CameraPresets {
    /// Find a preset by name (first match).
    pub fn get(&self, name: &str) -> Option<&CameraPreset> {
        self.presets.iter().find(|p| p.name == name)
    }
}

/// Live scene EV-100, written each frame by `renzora_auto_exposure`'s
/// GPU luminance readback system. `0.0` until the first readback completes
/// (or if auto-exposure isn't enabled). Read by scripting / debug HUDs.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct CameraExposureState {
    pub ev100: f32,
}

/// Maps each rendering camera to its effect source entities.
///
/// Each route is `(target_camera, [source_entities])`. For a given Settings
/// type the **first** source entity that has it wins.
///
/// Updated each frame by the routing system (editor: viewport crate,
/// runtime: renzora_engine). Read by per-crate sync systems.
#[derive(Resource, Default, Debug)]
pub struct EffectRouting {
    pub routes: Vec<(Entity, Vec<Entity>)>,
}

impl EffectRouting {
    /// Iterate all routes.
    pub fn iter(&self) -> impl Iterator<Item = &(Entity, Vec<Entity>)> {
        self.routes.iter()
    }
}

/// Serializable shape ID — stored alongside `Mesh3d` so the shape can be recreated on scene load.
///
/// The string must match a shape registered in the [`ShapeRegistry`].
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshPrimitive(pub String);

/// Serialized geometry override written by the mesh editor (Edit / Sculpt
/// modes) whenever an entity's mesh is modified.
///
/// `MeshPrimitive` / `MeshInstanceData` rehydrate their meshes from the
/// shape registry / source glTF on scene load, which would silently discard
/// user edits — this component persists the edited geometry in the scene and
/// wins over both (its rehydrate system runs after `rehydrate_meshes` and
/// replaces the `Mesh3d` handle). Arrays are flat (`xyzxyz…`) because
/// scene reflection round-trips flat `Vec<f32>` reliably.
///
/// `face_vertices` + `face_vertex_counts` carry the editor's *topology* —
/// the bounded face boundaries that the render-time `indices` (a flat
/// triangle list) cannot recover on its own. Bevy's `Mesh` only stores
/// triangles, so `to_mesh` discards the topology at exit. Without
/// persisted topology, `EditMesh::from_mesh` would have to guess which
/// triangles belong to which face — and once the user has extruded
/// adjacent coplanar quads, that guess is ambiguous and produces
/// diagonal-boundary faces on re-entry. Storing the topology here lets
/// the editor rebuild `EditMesh.faces` exactly on re-entry.
///
/// Both fields are `#[serde(default)]` so old scene files (and old
/// `from_mesh`-only snapshots) still load through the
/// triangle-import + `merge_coplanar_triangle_pairs` fallback.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EditedMesh {
    /// Vertex positions, 3 floats per vertex.
    pub positions: Vec<f32>,
    /// Vertex normals, 3 floats per vertex.
    pub normals: Vec<f32>,
    /// UV coordinates, 2 floats per vertex.
    pub uvs: Vec<f32>,
    /// Triangle list indices.
    pub indices: Vec<u32>,
    /// Flattened vertex IDs for every editable face, in perimeter order.
    /// Each face contributes `face_vertex_counts[i]` consecutive entries
    /// here. Empty when the topology is absent (older snapshots, imported
    /// triangle-only meshes that have not yet been baked through the
    /// editor).
    #[serde(default)]
    pub face_vertices: Vec<u32>,
    /// Number of vertices belonging to each editable face. Pairs with
    /// `face_vertices` to delimit the per-face windows. Empty when
    /// `face_vertices` is empty.
    #[serde(default)]
    pub face_vertex_counts: Vec<u32>,
}

impl EditedMesh {
    /// Snapshot a triangle-list `Mesh`'s geometry. Returns `None` when the
    /// mesh is missing positions or indices. This is the *triangle-only*
    /// path: it produces no editable topology. The editor uses it as a
    /// fallback when no `face_vertices` are present (older snapshots,
    /// fresh imports).
    pub fn from_mesh(mesh: &Mesh) -> Option<Self> {
        use bevy::mesh::VertexAttributeValues;
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
            VertexAttributeValues::Float32x3(v) => v.iter().flatten().copied().collect(),
            _ => return None,
        };
        let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(VertexAttributeValues::Float32x3(v)) => v.iter().flatten().copied().collect(),
            _ => Vec::new(),
        };
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(v)) => v.iter().flatten().copied().collect(),
            _ => Vec::new(),
        };
        let indices = match mesh.indices()? {
            bevy::mesh::Indices::U16(v) => v.iter().map(|&i| i as u32).collect(),
            bevy::mesh::Indices::U32(v) => v.clone(),
        };
        Some(Self {
            positions,
            normals,
            uvs,
            indices,
            face_vertices: Vec::new(),
            face_vertex_counts: Vec::new(),
        })
    }

    /// Snapshot an `EditMesh`-style topology alongside geometry. Takes
    /// the post-weld geometry arrays (positions / normals / uvs /
    /// indices) plus the editor's face boundaries, which mirror the
    /// fields `EditMesh::bake_to_mesh` writes.
    ///
    /// `face_perimeters` is one slice of vertex IDs per face, in
    /// perimeter order. The contract-crate boundary stays clean — the
    /// caller (`renzora_mesh_edit`) is the only place that knows
    /// `EditMesh::faces` exists; this function takes plain `Vec<u32>`
    /// data so the contract crate has no dependency on the editor
    /// crate.
    ///
    /// Faces with fewer than 3 vertex IDs are skipped — they have no
    /// perimeter to record and would corrupt the topology on re-entry.
    pub fn from_edit_mesh(
        positions: &[f32],
        normals: &[f32],
        uvs: &[f32],
        indices: &[u32],
        face_perimeters: &[Vec<u32>],
    ) -> Self {
        let total: usize = face_perimeters.iter().map(|f| f.len()).sum();
        let count = face_perimeters
            .iter()
            .filter(|f| f.len() >= 3)
            .count();
        let mut face_vertices: Vec<u32> = Vec::with_capacity(total);
        let mut face_vertex_counts: Vec<u32> = Vec::with_capacity(count);
        for face in face_perimeters {
            if face.len() < 3 {
                continue;
            }
            face_vertex_counts.push(face.len() as u32);
            face_vertices.extend(face.iter().copied());
        }

        Self {
            positions: positions.to_vec(),
            normals: normals.to_vec(),
            uvs: uvs.to_vec(),
            indices: indices.to_vec(),
            face_vertices,
            face_vertex_counts,
        }
    }

    /// True when this snapshot carries editable topology that can be
    /// trusted on re-entry. False for triangle-only snapshots from the
    /// `from_mesh` path.
    pub fn has_face_topology(&self) -> bool {
        !self.face_vertices.is_empty() && !self.face_vertex_counts.is_empty()
    }

    /// Validate persisted face topology against the geometry. Returns
    /// `true` when every face has at least 3 vertices, every vertex ID
    /// is in range, and the flattened length matches the counts.
    /// Callers should fall back to triangle-import when this returns
    /// `false`.
    pub fn face_topology_is_valid(&self) -> bool {
        if self.face_vertices.is_empty() && self.face_vertex_counts.is_empty() {
            return false; // empty topology — use the import fallback
        }
        if self.face_vertices.len()
            != self.face_vertex_counts.iter().map(|&c| c as usize).sum::<usize>()
        {
            return false;
        }
        let vert_count = self.positions.len() / 3;
        let mut offset = 0usize;
        for &count in &self.face_vertex_counts {
            if count < 3 {
                return false;
            }
            for i in 0..count as usize {
                let v = self.face_vertices[offset + i];
                if v as usize >= vert_count {
                    return false;
                }
            }
            offset += count as usize;
        }
        true
    }

    /// Rebuild a renderable `Mesh` from the stored geometry. The
    /// topology fields are editor-only and don't appear in the render
    /// output.
    pub fn to_mesh(&self) -> Mesh {
        use bevy::asset::RenderAssetUsages;
        use bevy::mesh::{Indices, PrimitiveTopology};
        let n = self.positions.len() / 3;
        let positions: Vec<[f32; 3]> = self
            .positions
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        let normals: Vec<[f32; 3]> = if self.normals.len() == self.positions.len() {
            self.normals
                .chunks_exact(3)
                .map(|c| [c[0], c[1], c[2]])
                .collect()
        } else {
            vec![[0.0, 1.0, 0.0]; n]
        };
        let uvs: Vec<[f32; 2]> = if self.uvs.len() == n * 2 {
            self.uvs.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
        } else {
            vec![[0.0, 0.0]; n]
        };
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(self.indices.clone()));
        mesh
    }
}

/// Non-serialized marker: this entity's `Mesh3d` already reflects its
/// [`EditedMesh`]. The editor inserts it when writing `EditedMesh`; the
/// scene-load rehydrator only applies `EditedMesh` where it's absent, so
/// live editor baking isn't redone (or worse, re-allocated) every frame.
#[derive(Component)]
pub struct EditedMeshApplied;

/// Event fired when a model importer has pulled PBR material data out of a
/// source file and needs somewhere to persist it as a `.material` graph.
/// Importers (the import dialog and the viewport drop pipeline) trigger this
/// per extracted material; an observer in `renzora_shader::material` writes
/// a node-graph `.material` file. Both sides communicate only through this
/// type — no sibling crate deps.
#[derive(Event, Debug, Clone)]
pub struct PbrMaterialExtracted {
    /// Human-friendly name for the material; becomes the `.material` filename.
    pub name: String,
    /// Absolute path of the directory to write the `.material` file into.
    pub output_dir: std::path::PathBuf,
    /// Absolute path of the project root. Subscribers compute the
    /// project-relative `wgsl_path` link saved into the `.material` from
    /// this; left empty when there's no project context.
    pub project_root: std::path::PathBuf,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// glTF emissive factor (RGB linear). Multiplied with `emissive_texture`
    /// when present; used as a constant when not.
    pub emissive: [f32; 3],
    /// Asset-relative URIs to the corresponding textures, e.g.
    /// `"models/car/textures/body_albedo.png"`. `None` if absent.
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
    /// glTF metallic-roughness map. Channels: G = roughness, B = metallic.
    pub metallic_roughness_texture: Option<String>,
    /// Standalone roughness map (`r` → roughness) for sources that don't pack
    /// metallic-roughness into one image (OBJ `map_Pr`, USD).
    pub roughness_texture: Option<String>,
    /// Standalone metallic map (`r` → metallic).
    pub metallic_texture: Option<String>,
    pub emissive_texture: Option<String>,
    /// Ambient occlusion map (R channel only).
    pub occlusion_texture: Option<String>,
    /// glTF spec-gloss `specularGlossinessTexture` (RGB = specular color,
    /// A = glossiness). The material observer routes its inverted alpha
    /// channel into the `roughness` pin so per-pixel glossiness survives
    /// the spec-gloss → metal-rough conversion. `None` for metal-rough
    /// materials.
    pub specular_glossiness_texture: Option<String>,
    /// Standalone opacity/alpha map with no glTF metal-rough equivalent
    /// (legacy FBX `TransparentColor` / `TransparencyFactor`). The material
    /// observer samples its `r` channel into the `alpha` pin so cloud shells
    /// and cutouts that drive transparency through a dedicated grayscale mask
    /// punch through.
    pub opacity_texture: Option<String>,
    /// Standalone specular/reflectivity mask (legacy FBX `SpecularColor` /
    /// `ReflectionColor`). Routed into `metallic` (and its inverse into
    /// `roughness`) to approximate a pre-PBR specular map.
    pub specular_texture: Option<String>,
    /// Extended PBR channels (clearcoat, transmission, anisotropy, ior, …)
    /// from glTF `KHR_materials_*` / modern FBX / USD. Default for sources that
    /// only author base metallic-roughness.
    pub advanced: PbrAdvanced,
    /// glTF alpha behavior. The graph resolver maps this onto Bevy's
    /// `AlphaMode` so transparency renders correctly.
    pub alpha_mode: PbrAlphaMode,
    /// Alpha discard threshold for `Mask` mode. Ignored otherwise.
    pub alpha_cutoff: f32,
    /// `doubleSided` flag — render both faces (glass, foliage, fabric).
    pub double_sided: bool,
}

/// Mirrors glTF 2.0 `alphaMode`. Lives in core so the import event and the
/// material graph use a single shared enum without crate-cycle gymnastics.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[derive(Default)]
pub enum PbrAlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}

/// Extended/advanced PBR channels beyond the base metallic-roughness model —
/// the union of what Bevy's `StandardMaterial` can shade and what glTF
/// `KHR_materials_*` extensions (and modern FBX/USD) author. Importers fill in
/// whatever the source provides; the graph builder seeds the matching
/// `output/surface` pins and samples any textures into them.
///
/// Defaults mirror the glTF 2.0 spec so a material that omits a channel renders
/// identically to one that never had it (e.g. `ior = 1.5`, no clearcoat).
#[derive(Clone, Debug)]
pub struct PbrAdvanced {
    /// `KHR_materials_clearcoat` clearcoatFactor — strength of the lacquer layer.
    pub clearcoat: f32,
    pub clearcoat_roughness: f32,
    pub clearcoat_texture: Option<String>,
    pub clearcoat_roughness_texture: Option<String>,
    pub clearcoat_normal_texture: Option<String>,
    /// `KHR_materials_transmission` transmissionFactor → `specular_transmission`.
    pub specular_transmission: f32,
    pub transmission_texture: Option<String>,
    /// Bevy diffuse transmission (translucent thin surfaces — leaves, paper).
    pub diffuse_transmission: f32,
    /// `KHR_materials_volume` — thickness of the refractive volume + textures.
    pub thickness: f32,
    pub thickness_texture: Option<String>,
    /// `KHR_materials_ior` — index of refraction (glass ≈ 1.5, water ≈ 1.33).
    pub ior: f32,
    /// `KHR_materials_volume` attenuation: how far light travels before being
    /// tinted by `attenuation_color`.
    pub attenuation_distance: f32,
    pub attenuation_color: [f32; 3],
    /// `KHR_materials_anisotropy` — brushed-metal directional highlight.
    pub anisotropy_strength: f32,
    pub anisotropy_rotation: f32,
    pub anisotropy_texture: Option<String>,
    /// `KHR_materials_specular` specularFactor → dielectric `reflectance`.
    pub reflectance: f32,
    /// `KHR_materials_unlit` — bypass lighting entirely (emissive-style flat
    /// shading). The graph builder switches to the unlit output when set.
    pub unlit: bool,
}

impl Default for PbrAdvanced {
    fn default() -> Self {
        Self {
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
            clearcoat_texture: None,
            clearcoat_roughness_texture: None,
            clearcoat_normal_texture: None,
            specular_transmission: 0.0,
            transmission_texture: None,
            diffuse_transmission: 0.0,
            thickness: 0.0,
            thickness_texture: None,
            ior: 1.5,
            // Large finite sentinel rather than f32::INFINITY: the graph
            // serializes to JSON, which has no infinity literal and would
            // emit `null` — corrupting the value on reload. 1e37 is
            // effectively "no attenuation" for any real scene.
            attenuation_distance: 1.0e37,
            attenuation_color: [1.0, 1.0, 1.0],
            anisotropy_strength: 0.0,
            anisotropy_rotation: 0.0,
            anisotropy_texture: None,
            reflectance: 0.5,
            unlit: false,
        }
    }
}

impl PbrAdvanced {
    /// Returns `true` when no extended channel deviates from its default, so
    /// callers can skip emitting advanced nodes for plain metal-rough materials.
    pub fn is_default(&self) -> bool {
        self.clearcoat == 0.0
            && self.clearcoat_texture.is_none()
            && self.clearcoat_roughness_texture.is_none()
            && self.clearcoat_normal_texture.is_none()
            && self.specular_transmission == 0.0
            && self.transmission_texture.is_none()
            && self.diffuse_transmission == 0.0
            && self.thickness == 0.0
            && self.thickness_texture.is_none()
            && self.ior == 1.5
            && self.anisotropy_strength == 0.0
            && self.anisotropy_texture.is_none()
            && self.reflectance == 0.5
            && !self.unlit
    }

    /// Produce a copy with every texture path mapped through `f` — used by the
    /// import bridges to rewrite model-relative URIs to project-relative ones.
    pub fn rewrite_textures(&self, f: impl Fn(&Option<String>) -> Option<String>) -> Self {
        Self {
            clearcoat_texture: f(&self.clearcoat_texture),
            clearcoat_roughness_texture: f(&self.clearcoat_roughness_texture),
            clearcoat_normal_texture: f(&self.clearcoat_normal_texture),
            transmission_texture: f(&self.transmission_texture),
            thickness_texture: f(&self.thickness_texture),
            anisotropy_texture: f(&self.anisotropy_texture),
            attenuation_color: self.attenuation_color,
            ..self.clone()
        }
    }
}


/// Event fired when a file or folder is renamed/moved inside the project's
/// asset tree. Subscribers should patch any stored asset-relative references
/// from `old` to `new` (and, when `old` is a folder, any paths prefixed by it).
/// Paths are asset-relative (no leading project root, forward slashes).
#[derive(Event, Debug, Clone)]
pub struct AssetPathChanged {
    pub old: String,
    pub new: String,
    /// `true` when the moved item was a directory — consumers should perform
    /// prefix matching on stored paths. `false` matches the exact path.
    pub is_dir: bool,
}

impl AssetPathChanged {
    /// If `path` references the moved asset (or something under it when
    /// `is_dir`), return the rewritten path. Otherwise `None`.
    pub fn rewrite(&self, path: &str) -> Option<String> {
        if self.is_dir {
            if let Some(rest) = path.strip_prefix(&self.old) {
                let sep = rest.starts_with('/') || rest.is_empty();
                if sep {
                    return Some(format!("{}{}", self.new, rest));
                }
            }
            None
        } else if path == self.old {
            Some(self.new.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod asset_path_changed_tests {
    use super::AssetPathChanged;

    fn file(old: &str, new: &str) -> AssetPathChanged {
        AssetPathChanged { old: old.into(), new: new.into(), is_dir: false }
    }
    fn dir(old: &str, new: &str) -> AssetPathChanged {
        AssetPathChanged { old: old.into(), new: new.into(), is_dir: true }
    }

    #[test]
    fn file_rename_is_exact_match_only() {
        let ev = file("scenes/main.bsn", "scenes/level1.bsn");
        assert_eq!(ev.rewrite("scenes/main.bsn").as_deref(), Some("scenes/level1.bsn"));
        assert_eq!(ev.rewrite("scenes/other.bsn"), None);
    }

    /// A file rename must not act as a prefix match, or renaming `main.bsn`
    /// would also rewrite `main.bsn.bak` — and these paths are written straight
    /// back into `project.toml`.
    #[test]
    fn file_rename_does_not_match_by_prefix() {
        let ev = file("scenes/main", "scenes/level1");
        assert_eq!(ev.rewrite("scenes/main.bsn"), None);
    }

    #[test]
    fn dir_rename_rewrites_contents() {
        let ev = dir("scenes", "levels");
        assert_eq!(ev.rewrite("scenes/main.bsn").as_deref(), Some("levels/main.bsn"));
        assert_eq!(ev.rewrite("scenes").as_deref(), Some("levels"));
    }

    /// `scenes2/` is not inside `scenes/`, so a folder rename must leave it be.
    #[test]
    fn dir_rename_respects_path_boundaries() {
        let ev = dir("scenes", "levels");
        assert_eq!(ev.rewrite("scenes2/main.bsn"), None);
    }
}

/// Serializable marker for an imported 3D model (GLTF/GLB).
///
/// Stored on the parent entity; the actual `SceneRoot` is a child.
/// On scene load, the runtime rehydrates by re-loading the model from `model_path`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshInstanceData {
    /// Asset-relative path to the GLB/GLTF file (e.g. `models/chair.glb`).
    pub model_path: Option<String>,
}

/// Distance-LOD configuration for a [`MeshInstanceData`] model.
///
/// The exporter bakes simplified variants beside each model
/// (`models/chair_lod1.glb`, `_lod2`, …). When variants exist for a model —
/// packed in the rpak or loose on disk — the runtime spawns them as sibling
/// subtrees and tags every mesh with a `VisibilityRange` band, so Bevy
/// crossfades between detail levels by camera distance. This component tunes
/// the bands; models without it use the defaults below. Every field is
/// `#[reflect(default)]` so scenes saved before a field existed still load.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshLod {
    /// Master switch — `false` renders only the base model, full detail at any
    /// distance, even when LOD files exist.
    #[serde(default = "default_true")]
    #[reflect(default = "default_true")]
    pub enabled: bool,
    /// Outer edge of each detail band in world units: the base mesh shows up
    /// to `distances[0]`, LOD1 up to `distances[1]`, and so on. When more LOD
    /// files exist than entries here, extra bands extend geometrically past
    /// the last entry.
    #[serde(default = "default_lod_distances")]
    #[reflect(default = "default_lod_distances")]
    pub distances: Vec<f32>,
    /// Width of the dithered crossfade between adjacent bands, in world
    /// units. `0` switches levels abruptly — one atomic swap, computed
    /// CPU-side. (A flash once blamed on this dither turned out to be the
    /// texture tier streamer writing not-yet-loaded images into materials;
    /// with those swaps load-gated, the crossfade default is back.)
    #[serde(default = "default_lod_crossfade")]
    #[reflect(default = "default_lod_crossfade")]
    pub crossfade: f32,
    /// Distance beyond which the model vanishes entirely. `0` = never cull —
    /// the lowest LOD stays visible to the horizon.
    #[serde(default)]
    #[reflect(default)]
    pub cull_distance: f32,
}

fn default_true() -> bool {
    true
}
fn default_lod_distances() -> Vec<f32> {
    vec![40.0, 100.0, 220.0]
}
fn default_lod_crossfade() -> f32 {
    5.0
}

impl Default for MeshLod {
    fn default() -> Self {
        Self {
            enabled: true,
            distances: default_lod_distances(),
            crossfade: default_lod_crossfade(),
            cull_distance: 0.0,
        }
    }
}

