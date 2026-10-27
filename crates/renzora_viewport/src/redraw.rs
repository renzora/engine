//! Render the viewport only when the scene changes.
//!
//! # Why
//!
//! The editor UI and the 3D viewport are one Bevy app: the same frame renders
//! both, on the same GPU queue. A scene that takes 150 ms to render (Solari on a
//! big level does) therefore held every panel, every hover and every keystroke
//! to 6 FPS, even while nothing in the scene was moving.
//!
//! With [`ViewportSettings::realtime`] off (the default) the viewport cameras
//! render for [`SETTLE_FRAMES`] after anything changes, then switch off. The
//! panel keeps showing the last image, and the UI runs at its own frame rate
//! until the next change. It does not make a frame that moves any cheaper.
//!
//! # What counts as a change
//!
//! Erring on the side of rendering. A false alarm only costs what every frame
//! cost before; a missed change leaves a stale picture on screen, which is the
//! failure that actually matters. So this scans broadly and only ignores what
//! is known to churn:
//!
//! - **Any component changed, added or removed on a scene entity** (anything
//!   with a `GlobalTransform` that is not UI and is visible to the viewport),
//!   minus [`ignored_components`], which Bevy rewrites every frame on its own.
//! - **The viewport cameras moving**: transform, projection, render target only.
//!   Their effect components are rewritten every frame (Bevy writes
//!   `reset = false` into TAA, DLSS and Solari through a mutable reference), and
//!   the settings behind them live on the World Environment, which the scene
//!   scan already watches.
//! - **Mesh, material and texture assets** changing or finishing loading.
//! - **Editor state**: viewport settings, selection, effect routing, the clear
//!   colour, gizmo config, and a viewport panel resizing or docking.
//! - **The mouse or keyboard over a viewport**, for hover highlights and gizmo
//!   handles, which change without anything in the scene changing.
//! - **Play and Simulate**, which render every frame.
//!
//! Shader animation (wind, water, clouds) and GPU particles move without the
//! scene changing, so they hold still while idle. Display > Render > Realtime
//! is for when that matters.
//!
//! # Why settle frames
//!
//! TAA, Solari's ReSTIR and DLSS all build their image up over several frames.
//! Stopping on the first frame after a change would freeze a noisy, half
//! converged picture.

use std::any::TypeId;

use bevy::asset::AssetEvent;
use bevy::camera::primitives::{Aabb, CascadesFrusta, CubemapFrusta, Frustum};
use bevy::camera::visibility::{
    CascadesVisibleEntities, CubemapVisibleEntities, InheritedVisibility, RenderLayers,
    ViewVisibility, VisibleEntities, VisibleMeshEntities,
};
use bevy::camera::RenderTarget;
use bevy::ecs::component::ComponentId;
use bevy::ecs::message::{MessageCursor, Messages};
use bevy::ecs::query::QueryState;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::light::cluster::Clusters;
use bevy::light::Cascades;
use bevy::pbr::PreviousGlobalTransform;
use bevy::prelude::*;
use bevy::transform::components::TransformTreeChanged;
use renzora::core::viewport_types::{ViewportSettings, Viewports};
use renzora::core::{IsolatedCamera, ViewportCamera, ViewportCamera2d, ViewportRedrawIdle};
use renzora_editor_framework::EditorSelection;
use renzora_shader::material::GraphMaterial;

/// Frames the viewport keeps rendering after the last change. Enough for TAA
/// and Solari's temporal reuse to converge; at 60 FPS it is a third of a second.
pub(crate) const SETTLE_FRAMES: u32 = 20;

/// Frames of rendering without a single idle frame before saying why. At 60
/// FPS that is ten seconds, long enough that it is not a drag in progress.
const NEVER_IDLE_FRAMES: u32 = 600;

/// Whether the viewport cameras may render this frame.
#[derive(Resource)]
pub struct ViewportRedraw {
    frames_left: u32,
    /// Frames rendered since the viewport last idled.
    awake_for: u32,
    /// Whether the "never idles" line has been printed for this stretch.
    reported: bool,
    /// The last thing that woke it, for that line.
    last_reason: String,
}

impl Default for ViewportRedraw {
    fn default() -> Self {
        Self {
            frames_left: SETTLE_FRAMES,
            awake_for: 0,
            reported: false,
            last_reason: String::new(),
        }
    }
}

impl ViewportRedraw {
    /// Whether the viewport cameras should render this frame.
    pub fn should_render(&self) -> bool {
        self.frames_left > 0
    }
}

/// The scans and message cursors, kept between frames.
#[derive(Default)]
pub(crate) struct DetectState {
    scene: Option<QueryState<EntityRef<'static>, SceneFilter>>,
    cameras: Option<QueryState<EntityRef<'static>, CameraFilter>>,
    /// Entity and archetype signature of the scene, so spawns, despawns and
    /// component removals count, which change ticks cannot see.
    signature: u64,
    /// Size and docking of every viewport slot.
    slots: u64,
    motion: MessageCursor<MouseMotion>,
    wheel: MessageCursor<MouseWheel>,
    meshes: MessageCursor<AssetEvent<Mesh>>,
    standard: MessageCursor<AssetEvent<StandardMaterial>>,
    graph: MessageCursor<AssetEvent<GraphMaterial>>,
    images: MessageCursor<AssetEvent<Image>>,
}

/// Scene entities: anything placed in the world that is not UI and not a
/// viewport camera. UI has no `GlobalTransform` in Bevy 0.20; the filter on
/// `Node` is belt and braces.
type SceneFilter = (
    With<GlobalTransform>,
    Without<Node>,
    Without<ViewportCamera>,
    Without<ViewportCamera2d>,
    Without<IsolatedCamera>,
);

type CameraFilter = Or<(With<ViewportCamera>, With<ViewportCamera2d>)>;

/// Components Bevy (or the editor) writes every frame whether or not anything
/// changed. A missing entry here does no harm beyond the viewport never
/// idling, and [`NEVER_IDLE_FRAMES`] says which entity is doing it.
fn ignored_components() -> [TypeId; 14] {
    [
        TypeId::of::<ViewVisibility>(),
        TypeId::of::<InheritedVisibility>(),
        TypeId::of::<VisibleEntities>(),
        TypeId::of::<VisibleMeshEntities>(),
        TypeId::of::<CascadesVisibleEntities>(),
        TypeId::of::<CubemapVisibleEntities>(),
        TypeId::of::<Frustum>(),
        TypeId::of::<CascadesFrusta>(),
        TypeId::of::<CubemapFrusta>(),
        TypeId::of::<Cascades>(),
        TypeId::of::<Clusters>(),
        TypeId::of::<Aabb>(),
        TypeId::of::<PreviousGlobalTransform>(),
        TypeId::of::<TransformTreeChanged>(),
    ]
}

/// The only viewport-camera components that mean the picture moved.
fn camera_components() -> [TypeId; 3] {
    [
        TypeId::of::<GlobalTransform>(),
        TypeId::of::<Projection>(),
        TypeId::of::<RenderTarget>(),
    ]
}

/// Decide whether the viewport renders this frame. Exclusive, because it reads
/// change ticks on every component of every scene entity, which no typed query
/// can express.
pub(crate) fn detect_viewport_changes(world: &mut World, mut state: Local<DetectState>) {
    let state = &mut *state;
    // An exclusive system sees its own last run as the world's last change tick.
    let last_run = world.last_change_tick();
    let this_run = world.read_change_tick();

    let realtime = world
        .get_resource::<ViewportSettings>()
        .is_some_and(|s| s.realtime);
    let playing = world
        .get_resource::<renzora::core::PlayModeState>()
        .is_some_and(|p| !p.is_editing());

    // A renderer feature can ask for longer settling, or for every frame (see
    // `ViewportRedrawDemand`).
    let demand = world.get_resource::<renzora::core::ViewportRedrawDemand>().copied().unwrap_or_default();
    let settle_frames = demand.settle_frames.max(SETTLE_FRAMES);

    let reason = if realtime {
        None
    } else if playing {
        Some("play mode".to_string())
    } else if let Some(why) = demand.continuous {
        Some(why.to_string())
    } else {
        find_change(world, state, last_run, this_run)
    };

    let mut redraw = world.resource_mut::<ViewportRedraw>();
    if realtime {
        // Realtime renders every frame through `sync_viewport_camera_activation`
        // reading the setting directly; keep the window full so switching it off
        // settles rather than cutting to a half-converged frame.
        redraw.frames_left = settle_frames;
        redraw.awake_for = 0;
        redraw.reported = false;
        return;
    }
    match reason {
        Some(reason) => {
            redraw.frames_left = settle_frames;
            redraw.last_reason = reason;
        }
        None => redraw.frames_left = redraw.frames_left.saturating_sub(1),
    }
    if redraw.frames_left == 0 {
        redraw.awake_for = 0;
        redraw.reported = false;
    } else {
        redraw.awake_for += 1;
        if redraw.awake_for >= NEVER_IDLE_FRAMES && !redraw.reported && !playing && demand.continuous.is_none() {
            redraw.reported = true;
            info!(
                "[viewport] rendering on change, but something has changed every frame for {NEVER_IDLE_FRAMES} frames, so the viewport never idles. Last change: {}",
                redraw.last_reason
            );
        }
    }
}

/// The first thing found that changed this frame, described for the log.
fn find_change(
    world: &mut World,
    state: &mut DetectState,
    last_run: bevy::ecs::change_detection::Tick,
    this_run: bevy::ecs::change_detection::Tick,
) -> Option<String> {
    let changed = |ticks: Option<bevy::ecs::change_detection::ComponentTicks>| {
        ticks.is_some_and(|t| t.is_changed(last_run, this_run))
    };

    // Input over a viewport. Read every cursor every frame so a stale backlog
    // never wakes it later.
    let hovered = world
        .get_resource::<Viewports>()
        .is_some_and(|v| v.slots.iter().any(|s| s.hovered && s.docked));
    let moved = world
        .get_resource::<Messages<MouseMotion>>()
        .is_some_and(|m| state.motion.read(m).count() > 0);
    let scrolled = world
        .get_resource::<Messages<MouseWheel>>()
        .is_some_and(|m| state.wheel.read(m).count() > 0);
    let buttons = world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|b| b.get_pressed().next().is_some() || b.get_just_released().next().is_some());
    let keys = world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|k| k.get_pressed().next().is_some() || k.get_just_released().next().is_some());
    let mut assets_changed = None;
    if let Some(m) = world.get_resource::<Messages<AssetEvent<Mesh>>>() {
        if state.meshes.read(m).count() > 0 {
            assets_changed = Some("a mesh asset");
        }
    }
    if let Some(m) = world.get_resource::<Messages<AssetEvent<StandardMaterial>>>() {
        if state.standard.read(m).count() > 0 {
            assets_changed = Some("a material asset");
        }
    }
    if let Some(m) = world.get_resource::<Messages<AssetEvent<GraphMaterial>>>() {
        if state.graph.read(m).count() > 0 {
            assets_changed = Some("a graph material asset");
        }
    }
    if let Some(m) = world.get_resource::<Messages<AssetEvent<Image>>>() {
        if state.images.read(m)
            .any(|e| !matches!(e, AssetEvent::Unused { .. } | AssetEvent::Removed { .. }))
        {
            assets_changed = Some("a texture");
        }
    }

    if hovered && (moved || scrolled || buttons || keys) {
        return Some("input over the viewport".into());
    }
    if let Some(what) = assets_changed {
        return Some(what.into());
    }

    // Editor state that changes the picture without touching the scene.
    if changed(world.get_resource_change_ticks::<ViewportSettings>())
        || changed(world.get_resource_change_ticks::<EditorSelection>())
        || changed(world.get_resource_change_ticks::<renzora::EffectRouting>())
        || changed(world.get_resource_change_ticks::<ClearColor>())
        || changed(world.get_resource_change_ticks::<GizmoConfigStore>())
    {
        return Some("editor settings or selection".into());
    }

    // Viewport panels resizing or docking. `Viewports` itself is written every
    // frame (hover, screen position), so hash only what the render depends on.
    let slots = world.get_resource::<Viewports>().map_or(0, |v| {
        v.slots.iter().fold(0u64, |h, s| {
            mix(mix(h, ((s.current_size.x as u64) << 32) | s.current_size.y as u64), s.docked as u64)
        })
    });
    if slots != state.slots {
        state.slots = slots;
        return Some("a viewport resized or docked".into());
    }

    // The viewport cameras: only what moves the picture.
    let watched: Vec<ComponentId> = camera_components()
        .iter()
        .filter_map(|t| world.components().get_id(*t))
        .collect();
    let cameras = state
        .cameras
        .get_or_insert_with(|| world.query_filtered::<EntityRef, CameraFilter>());
    for camera in cameras.iter(world) {
        let active = camera.get::<Camera>().is_some_and(|c| c.is_active)
            || camera.contains::<ViewportRedrawIdle>();
        if active && watched.iter().any(|id| changed(camera.get_change_ticks_by_id(*id))) {
            return Some("the viewport camera moved".into());
        }
    }

    // The scene. Everything not on the ignore list counts.
    let ignored: Vec<ComponentId> = ignored_components()
        .iter()
        .filter_map(|t| world.components().get_id(*t))
        .collect();
    // What the primary viewport camera can see, so offscreen rigs (material
    // preview, thumbnails) on their own render layers don't keep it awake.
    let mut primary_layers = world.query_filtered::<
        Option<&RenderLayers>,
        With<renzora::core::PrimaryViewportCamera>,
    >();
    let view_layers = primary_layers
        .iter(world)
        .next()
        .flatten()
        .cloned()
        .unwrap_or_default();
    let scene = state
        .scene
        .get_or_insert_with(|| world.query_filtered::<EntityRef, SceneFilter>());
    let mut signature = 0u64;
    let mut found = None;
    for entity in scene.iter(world) {
        let layers = entity.get::<RenderLayers>().cloned().unwrap_or_default();
        if !layers.intersects(&view_layers) {
            continue;
        }
        signature = signature.wrapping_add(mix(
            entity.id().to_bits(),
            entity.archetype().id().index() as u64,
        ));
        if found.is_some() {
            continue;
        }
        let hit = entity.archetype().components().iter().any(|id| {
            !ignored.contains(id) && changed(entity.get_change_ticks_by_id(*id))
        });
        if hit {
            let name = entity
                .get::<Name>()
                .map_or_else(|| format!("{}", entity.id()), |n| n.to_string());
            found = Some(format!("a component on '{name}'"));
        }
    }
    if signature != state.signature {
        state.signature = signature;
        return Some(found.unwrap_or_else(|| "an entity was added, removed or changed shape".into()));
    }
    found
}

/// Cheap 64-bit mix for the signatures. Only compared with itself, never stored.
fn mix(h: u64, v: u64) -> u64 {
    (h ^ v.wrapping_mul(0x9E37_79B9_7F4A_7C15)).rotate_left(27).wrapping_mul(0x94D0_49BB_1331_11EB)
}
