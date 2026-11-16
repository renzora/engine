//! Renzora Viewport — bevy_ui (ember) panel that displays the 3D game world.
//!
//! Creates an offscreen render target, wires it to the runtime camera,
//! and displays the result inside the native docking panel system.

pub mod camera_preview;
pub mod debug_material;
pub mod debug_viz;
pub mod effect_routing;
pub mod external_runtime;
pub mod glb_compat;
pub mod material_drop;
mod effect_drop;
mod gaussian_drop;
pub mod html_drop;
pub mod model_cameras;
pub mod model_drop;
pub mod model_flatten;
/// Render the viewport only when the scene changes.
pub mod redraw;
mod axis_gizmo;
mod camera_preview_panel;
mod drop_target;
pub mod game_panel;
mod game_view;
mod height_ruler;
mod lit_accent;
mod load_overlay;
mod modal_hud;
mod nav;
pub mod overlay;
mod shading;
mod overlay_2d;
mod panel;
mod pivot_label;
mod stats_overlay;
pub mod toolbar;
mod tool_shelf;
mod work_card;
pub mod particle_drop;
pub mod persistence;
pub mod play_mode;
pub mod render_systems;
pub mod scene_drop;
pub mod settings;
pub mod shape_drop;
pub mod sprite_drop;
mod tool_buttons;
mod world_ui_pointer;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bevy::asset::embedded_asset;
use bevy::pbr::{
    wireframe::{WireframeConfig, WireframePlugin},
    MaterialPlugin,
};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::ViewportRenderTarget;

pub use camera_preview::CameraPreviewState;
// Re-export all viewport types from core (they now live in renzora::viewport_types)
pub use renzora::core::viewport_types::{
    CameraOrbitSnapshot, CameraSettingsState, CollisionGizmoVisibility, NavOverlayState,
    ProjectionMode, RenderToggles, SnapSettings, ViewAngleCommand, ViewportSettings, ViewportState,
    VisualizationMode,
};

const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;

/// Plugin that creates the render-to-texture viewport and registers the panel.
#[derive(Default)]
pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ViewportPlugin");
        embedded_asset!(app, "viewport_debug.wesl");
        app.add_plugins(WireframePlugin::default())
            .add_plugins(MaterialPlugin::<debug_material::ViewportDebugMaterial>::default())
            // Post-tonemap debug visualization for normals + linear depth.
            // Bypasses tonemap/AE so the colors come out as authored.
            .add_plugins(debug_viz::DebugVizPlugin)
            .insert_resource(WireframeConfig {
                global: false,
                default_color: bevy::color::Color::WHITE,
                // 0.19: new fields.
                default_line_width: 2.0,
                default_topology: bevy::pbr::wireframe::WireframeTopology::default(),
                // 0.20: wireframes drawn through geometry. Off, as before.
                xray_mode: false,
            })
            .init_resource::<ViewportState>()
            .init_resource::<ViewportResizeRequest>()
            .init_resource::<NavOverlayState>()
            .init_resource::<ViewportSettings>()
            .init_resource::<redraw::ViewportRedraw>()
            .init_resource::<overlay::ViewportOverlays>()
            .init_resource::<renzora::core::viewport_types::ViewportOverlayChanged>()
            .init_resource::<render_systems::ShadowBudget>()
            .add_plugins(bevy::render::extract_resource::ExtractResourcePlugin::<
                render_systems::ShadowBudget,
            >::default())
            // Owned here because the shading modes are what set it; the sky
            // crates only read it, and take it as absent when nothing does.
            .register_type::<renzora::core::EnvironmentSuppressed>()
            .init_resource::<renzora::core::EnvironmentSuppressed>()
            .init_resource::<renzora::core::viewport_types::ViewportRenderResolution>()
            .init_resource::<CameraOrbitSnapshot>()
            // Filled in by whatever is filling the scene in (a new terrain);
            // shown by `work_card`.
            .init_resource::<renzora::core::viewport_types::ViewportWork>()
            .init_resource::<renzora::core::InputFocusState>()
            .init_resource::<renzora::core::PlayModeState>()
            .init_resource::<external_runtime::ExternalRuntime>()
            .init_resource::<bevy::winit::WinitSettings>()
            .init_resource::<render_systems::OriginalMaterialStates>()
            .init_resource::<render_systems::LastToggleState>()
            .init_resource::<render_systems::DefaultGridTexture>()
            // Per-material-type viz-swap state (one of each per registered type).
            .init_resource::<render_systems::LastVizState<StandardMaterial>>()
            .init_resource::<render_systems::DebugMaterialCache<StandardMaterial>>()
            .init_resource::<render_systems::LastVizState<renzora_terrain::material::TerrainCheckerboardMaterial>>()
            .init_resource::<render_systems::DebugMaterialCache<renzora_terrain::material::TerrainCheckerboardMaterial>>()
            .add_systems(PostStartup, (setup_viewport, camera_preview::setup_camera_preview))
            // Bring scene-loaded model instances onto the production
            // material-binding path the moment Bevy finishes spawning the
            // GLB hierarchy. Drag-drop entities short-circuit this via the
            // `Without<ImportedRoot>` filter inside the observer.
            .add_observer(model_drop::decorate_rehydrated_scene_on_ready)
            // Same signal, second consumer: clear the wrapper-hiding latch so
            // the scan re-runs against the finished subtree instead of the
            // bare mount point it saw first.
            .add_observer(model_flatten::rearm_wrapper_hiding_on_ready)
            .init_resource::<renzora::core::EffectRouting>()
            .init_resource::<model_drop::PendingGltfLoads>()
            .init_resource::<model_drop::ModelDragPreviewState>()
            .init_resource::<model_drop::ModelPlaceholderAssets>()
            .init_resource::<renzora_ui::ShapeDragState>()
            .init_resource::<renzora_ui::ShapeDragPreviewState>()
            .init_resource::<drop_target::ArmedViewportDrop>()
            .init_resource::<renzora::core::viewport_types::DropCursorRequest>()
            .init_resource::<BrushCursorHiddenByUs>()
            .add_systems(Update, (
                update_input_focus,
                // Grouped into one tuple slot to stay within Bevy's 20-element
                // `add_systems` tuple cap (compute must precede resolve).
                (
                    compute_viewport_render_resolution.before(resolve_viewport_slots),
                    resolve_viewport_slots,
                ),
                render_systems::update_render_toggles,
                (
                    render_systems::apply_visualization_mode_for::<StandardMaterial>,
                    render_systems::apply_visualization_mode_for_custom::<renzora_terrain::material::TerrainCheckerboardMaterial>,
                    render_systems::forget_removed_materials::<StandardMaterial>,
                    render_systems::forget_removed_materials::<renzora_terrain::material::TerrainCheckerboardMaterial>,
                    // Grass is not here: it draws through its own instanced
                    // pipeline rather than a `Material`, so there is no material
                    // asset to swap for a debug view.
                ),
                // After every camera's activation for this frame, so a camera
                // waking gets its lights' shadows back in the same frame.
                render_systems::update_shadow_settings
                    .after(sync_viewport_camera_activation)
                    .after(camera_preview::sync_camera_preview_activation),
                // The four exclusive systems in this tuple are each a sync point
                // for the whole schedule even on a frame they return at once,
                // which is nearly every frame. Each condition is the system's
                // own opening check.
                play_mode::handle_play_mode_transitions.run_if(play_mode::play_request_pending),
                (
                    external_runtime::poll_external_runtime,
                    play_mode::sync_game_launched,
                )
                    .chain(),
                external_runtime::advance_runtime_phase,
                effect_routing::update_effect_routing,
                (
                    model_drop::spawn_loaded_gltfs,
                    model_flatten::flatten_pending_scenes
                        .run_if(any_with_component::<model_flatten::PendingFlatten>),
                    // After flatten: any wrappers that survived (e.g. a
                    // multi-child RootNode that flatten couldn't collapse)
                    // get tagged HideInHierarchy. Ordered so we don't write
                    // to entities that flatten is in the middle of despawning.
                    model_flatten::hide_gltf_wrappers
                        .after(model_flatten::flatten_pending_scenes),
                    model_drop::bind_material_refs,
                    model_drop::auto_discover_animations,
                    model_cameras::add_imported_cameras
                        .run_if(any_with_component::<model_cameras::PendingImportedCameras>),
                    model_drop::align_models_to_ground,
                ),
                (
                    model_drop::track_model_drag_preview,
                    model_drop::update_model_drag_ghost,
                    // Native (bevy_ui) drop: promote the preview entity in
                    // place. Must run before cleanup despawns the ghost once
                    // the payload is removed, so we order against this system
                    // explicitly.
                    model_drop::native_model_drop,
                    // Cleanup must run after the editor's deferred-command
                    // queue has drained — the native drop handler pushes a
                    // deferred drop that locks the placement entity into the
                    // scene (clears `placement_entity` from state). If cleanup
                    // ran first, it would despawn the still-being-placed entity
                    // right out from under that handler.
                    model_drop::cleanup_model_drag_ghost
                        .after(renzora_editor_framework::drain_editor_commands_native),
                ).chain(),
                shape_drop::shape_drag_ground_tracking
                    .before(shape_drop::shape_drag_raycast_system),
                shape_drop::shape_drag_raycast_system
                    .before(shape_drop::update_shape_drag_preview),
                shape_drop::update_shape_drag_preview,
                (
                    // Must run after the preview: the drop commits the ghost's
                    // resolved position (surface offset + grid snap), so that
                    // has to be this frame's, not the previous frame's.
                    shape_drop::native_shape_drop
                        .after(shape_drop::update_shape_drag_preview),
                    html_drop::native_html_drop,
                    html_drop::open_ui_template_request,
                    // Native (bevy_ui) asset drops (material / scene / sprite).
                    // `arm` captures the hovering drop candidate each frame;
                    // `commit` fires it on release — see `drop_target` for why
                    // we can't read the payload at release.
                    (
                        drop_target::arm_viewport_drop,
                        drop_target::commit_viewport_drop,
                    )
                        .chain(),
                ),
                shape_drop::handle_shape_spawn.run_if(|s: Res<renzora_ui::ShapeDragState>| s.pending_drop.is_some()),
                handle_view_shortcuts,
                handle_play_shortcuts,
                hide_cursor_for_brushes,
                (
                    persistence::migrate_project_prefs,
                    persistence::save_on_change.after(persistence::migrate_project_prefs),
                ),
            ).run_if(in_state(renzora_editor_framework::SplashState::Editor)));

        // The viewport's own settings are per-user now, so they load once rather
        // than on every project load. `PreStartup` so they are in place before
        // anything reads a camera sensitivity or a grid size.
        app.add_systems(PreStartup, persistence::apply_saved_settings);

        // Always-on panel-visibility gates — toggle is_active on the offscreen
        // cameras when their panels are / are not in the current dock tree so
        // layouts that don't show a given panel don't pay for its render pass.
        app.add_systems(
            Update,
            (
                // These were chained behind `track_last_scene_view` so that the
                // frame the view flipped to UI, both camera systems already
                // agreed on which scene view it had come from. There is no UI
                // view to flip to now, so there is nothing to agree about.
                (
                    // Decides whether this frame renders, from what changed
                    // since the last one; activation then applies it.
                    redraw::detect_viewport_changes,
                    sync_viewport_camera_activation,
                    park_primary_3d_target,
                )
                    .chain(),
                camera_preview::sync_camera_preview_activation,
                // Each slot's overlay image and camera (see `overlay`), sized
                // after this frame's resize and shown or hidden after this
                // frame's activation.
                overlay::sync_overlay_targets.after(resolve_viewport_slots),
                overlay::sync_overlay_nodes.after(sync_viewport_camera_activation),
            )
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );
        app.add_systems(
            PostUpdate,
            overlay::follow_scene_projections
                .before(bevy::camera::CameraUpdateSystems)
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );
        app.add_systems(PostUpdate, log_active_cameras);

        // Camera-preview spawn/update logic only when its panel is mounted.
        app.add_systems(
            Update,
            (
                camera_preview::update_camera_preview,
                camera_preview::resize_camera_preview,
            )
                .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                .run_if(camera_preview::camera_preview_panel_mounted),
        );

        // Editor-viewport mouse → world-UI ray. Its own `add_systems` rather
        // than a slot in the big Update tuple above: that one is already at
        // Bevy's 20-element cap, and this needs its own set membership anyway.
        // Not gated on play mode — world panels stay clickable while authoring.
        app.add_systems(
            Update,
            world_ui_pointer::publish_viewport_mouse_ray
                .in_set(renzora::WorldUiPointerSet::Publish)
                .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
        );

        // After the window-chrome queue, which is where `WindowAction::Close`
        // becomes an `AppExit`. Both live in `Last`; unordered, this can read the
        // message buffer before the close is written, and then the fast exit is
        // missed entirely — the runner unwinds the whole `World` instead and the
        // window sits there for seconds. See
        // `renzora_ui::window_chrome::WindowActionSet`.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(
            Last,
            external_runtime::kill_on_app_exit
                .after(renzora_ui::window_chrome::WindowActionSet),
        );

        // The shadow budget's render-world half: see `ShadowBudget`.
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app.add_systems(
                bevy::render::Render,
                render_systems::apply_shadow_budget
                    .in_set(bevy::render::RenderSystems::CreateViews)
                    .before(bevy::pbr::prepare_lights),
            );
        }

        panel::register(app);
        game_view::register(app);
        game_panel::register(app);
        camera_preview_panel::register(app);
        modal_hud::register(app);
        pivot_label::register(app);
        load_overlay::register(app);
        overlay_2d::register(app);
        stats_overlay::register(app);
    }
}

/// Tracks whether [`hide_cursor_for_brushes`] is currently the owner of the
/// cursor-hidden state. Without this, we can't tell the difference between
/// "we hid it" and "modal transform hid it", and would stomp on each other.
#[derive(Resource, Default)]
struct BrushCursorHiddenByUs(bool);

/// Hide the OS cursor while a brush tool is active and the pointer is over
/// the viewport. Only acts on transitions we own — if someone else (e.g.
/// modal transform) has hidden the cursor, we don't touch it.
fn hide_cursor_for_brushes(
    active_tool: Option<Res<renzora_editor_framework::ActiveTool>>,
    viewport: Option<Res<renzora::core::viewport_types::ViewportState>>,
    mut cursor_options: Query<&mut bevy::window::CursorOptions>,
    mut ours: ResMut<BrushCursorHiddenByUs>,
) {
    use renzora_editor_framework::ActiveTool;
    let Ok(mut cursor) = cursor_options.single_mut() else {
        return;
    };
    let brush_active = matches!(
        active_tool.as_deref(),
        Some(ActiveTool::TerrainSculpt | ActiveTool::TerrainPaint | ActiveTool::FoliagePaint)
    );
    let hovered = viewport.as_deref().is_some_and(|v| v.hovered);
    let should_hide = brush_active && hovered;

    if should_hide && !ours.0 {
        cursor.visible = false;
        ours.0 = true;
    } else if !should_hide && ours.0 {
        cursor.visible = true;
        ours.0 = false;
    }
}

/// Atomically-writable resize request for one viewport slot's `ui()` method.
#[derive(Default)]
pub struct SlotResizeRequest {
    pub width: AtomicU32,
    pub height: AtomicU32,
    pub hovered: AtomicBool,
    pub screen_x: AtomicU32,
    pub screen_y: AtomicU32,
}

impl SlotResizeRequest {
    fn new() -> Self {
        Self {
            width: AtomicU32::new(DEFAULT_WIDTH),
            height: AtomicU32::new(DEFAULT_HEIGHT),
            hovered: AtomicBool::new(false),
            screen_x: AtomicU32::new(0),
            screen_y: AtomicU32::new(0),
        }
    }
}

/// One resize request per viewport slot. Each panel writes its slot's entry
/// from `&World`; [`resolve_viewport_slots`] reads them each frame.
#[derive(Resource)]
pub struct ViewportResizeRequest {
    pub slots: [SlotResizeRequest; renzora::core::viewport_types::VIEWPORT_COUNT],
}

impl Default for ViewportResizeRequest {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| SlotResizeRequest::new()),
        }
    }
}

/// Creates one offscreen render target per viewport slot. Slot 0's image is also
/// published as the shared `ViewportRenderTarget` (the UI-canvas backdrop /
/// recorder read from it) and mirrored into the focused-viewport `ViewportState`.
pub(crate) fn setup_viewport(
    mut images: ResMut<Assets<Image>>,
    mut render_target: ResMut<ViewportRenderTarget>,
    mut viewport_state: ResMut<ViewportState>,
    mut viewports: ResMut<renzora::core::viewport_types::Viewports>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;
    bevy::log::info!("[viewport] setup_viewport — creating {VIEWPORT_COUNT} render targets");

    for i in 0..VIEWPORT_COUNT {
        let image_handle = images.add(make_render_target(DEFAULT_WIDTH, DEFAULT_HEIGHT));

        viewports.slots[i].image = Some(image_handle.clone());
        viewports.slots[i].current_size = UVec2::new(DEFAULT_WIDTH, DEFAULT_HEIGHT);

        if i == 0 {
            render_target.image = Some(image_handle.clone());
            viewport_state.image_handle = Some(image_handle);
            viewport_state.current_size = UVec2::new(DEFAULT_WIDTH, DEFAULT_HEIGHT);
        }
    }
}

/// Build a blank off-screen render-target image suitable for a camera to draw
/// into and a UI `ImageNode` to display.
pub(crate) fn make_render_target(width: u32, height: u32) -> Image {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    // No CPU copy: only the GPU draws into or samples a render target, and
    // wgpu zero-fills one created without data. The render-host path swaps
    // its own frame buffer in as `data`, so it never needed this one either.
    let mut image = Image {
        data: None,
        ..default()
    };
    image.texture_descriptor.size = size;
    image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC
        | TextureUsages::RENDER_ATTACHMENT;
    image
}

/// Derives the editor viewport's render resolution from the relevant scene
/// camera each frame and stores it in [`ViewportRenderResolution`] (read by
/// [`resolve_viewport_slots`]).
///
/// Priority — in play mode: the active camera (default, else first). In the
/// editor: the selected entity if it is a scene camera, else the default, else
/// the first scene camera. A camera with no [`CameraRenderResolution`] (or no
/// camera at all) resolves to `Full`.
fn compute_viewport_render_resolution(
    selection: Option<Res<renzora_editor_framework::EditorSelection>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    cameras: Query<
        (
            Entity,
            Option<&renzora::core::DefaultCamera>,
            Option<&renzora::core::CameraRenderResolution>,
        ),
        With<renzora::core::SceneCamera>,
    >,
    mut out: ResMut<renzora::core::viewport_types::ViewportRenderResolution>,
) {
    use renzora::core::viewport_types::RenderResolution;

    let in_play = play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode());

    let res_of = |r: Option<&renzora::core::CameraRenderResolution>| r.map(|r| r.0).unwrap_or_default();
    let default_or_first = || -> Option<RenderResolution> {
        cameras
            .iter()
            .find(|(_, d, _)| d.is_some())
            .or_else(|| cameras.iter().next())
            .map(|(_, _, r)| res_of(r))
    };

    let resolved = if in_play {
        default_or_first()
    } else {
        selection
            .and_then(|s| s.get())
            .and_then(|e| cameras.get(e).ok())
            .map(|(_, _, r)| res_of(r))
            .or_else(default_or_first)
    }
    .unwrap_or_default();

    if out.0 != resolved {
        out.0 = resolved;
    }
}

/// Per-frame resolver: applies each slot's pending resize, tracks dock
/// membership + hover, picks the focused slot, and mirrors it into the
/// singleton [`ViewportState`] that the gizmo / picking / overlay stack reads.
fn resolve_viewport_slots(
    resize_req: Res<ViewportResizeRequest>,
    ember_dock: Option<Res<renzora_ember::dock::Dock>>,
    ember_fixed: Option<Res<renzora_ember::dock::FixedDock>>,
    ember_windows: Option<Res<renzora_ember::dock::DockWindows>>,
    modals: Query<(), With<renzora_ember::widgets::ModalSurface>>,
    resolution: Option<Res<renzora::core::viewport_types::ViewportRenderResolution>>,
    mut viewports: ResMut<renzora::core::viewport_types::Viewports>,
    mut viewport_state: ResMut<ViewportState>,
    mut images: ResMut<Assets<Image>>,
    resize_busy: Option<Res<renzora::core::resize::ResizeBusy>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mut hidden_frames: Local<[u32; renzora::core::viewport_types::VIEWPORT_COUNT]>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;

    // A splitter or window edge is being dragged. Every size a drag passes
    // through used to resize the target, and DLSS starts over at every new
    // size (35 to 40 ms each), so a drag ran at 15 fps. The target now keeps
    // its size until the button comes up; the panel stretches the last image
    // meanwhile, which is what it already does for the render scale.
    let resizing = renzora::core::resize::resize_in_flight(&resize_busy);

    // Render-resolution scale: the target is sized at this fraction of the
    // panel and upscaled on display. `screen_size` stays the full panel size so
    // pointer→render-target mapping (cursor picking, drops) divides it back out.
    // The scale is derived per-frame from the relevant scene camera by
    // `compute_viewport_render_resolution`.
    let render_scale = resolution
        .as_ref()
        .map(|r| r.0.scale())
        .unwrap_or(1.0);

    // A bevy_ui modal (settings overlay, search/add-component overlay, …) covers
    // the viewport and must swallow the wheel/pointer — otherwise scrolling over
    // the modal also zooms the 3D camera behind it.
    let modal_open = !modals.is_empty();

    let mut newly_hovered: Option<usize> = None;
    #[allow(clippy::needless_range_loop)] // `i` indexes several parallel arrays
    for i in 0..VIEWPORT_COUNT {
        let req = &resize_req.slots[i];
        // "Docked" = the slot's panel is visible in the live dock. A slot counts
        // only while its panel is some leaf's *active tab*, so hidden tabs and
        // viewport-less workspaces release their slot.
        //
        // Floating dock windows count as docked too: a viewport torn off onto
        // another monitor is still visible and must keep its camera. So does the
        // fixed bottom area, for the same reason.
        //
        // There used to be a `DockingState` fallback here for when the ember
        // dock was absent. Its own comment recorded the problem with it: seeded
        // from the boot layout and never updated, it reported the viewport as
        // docked forever, which kept the slot-0 camera rendering the whole scene
        // behind empty workspaces. With that gone the absent-dock case answers
        // `true`, which is what it answered before any dock existed.
        let shown = match ember_dock.as_ref() {
            Some(d) => renzora_ember::dock::panel_visible_anywhere(
                VIEWPORT_PANEL_IDS[i],
                Some(d),
                ember_fixed.as_deref(),
                ember_windows.as_deref(),
            ),
            None => true,
        };
        // A slot counts as gone only once it has stayed gone for a moment. The
        // dock rebuilds its tree when a panel is added, closed or moved, and
        // for that one frame the viewport is in no leaf: the target shrank to
        // the placeholder and grew back, and DLSS rebuilt itself twice, a 35 ms
        // frame each, for a viewport that never left the screen.
        hidden_frames[i] = if shown { 0 } else { hidden_frames[i].saturating_add(1) };
        let docked = shown || hidden_frames[i] < HIDE_GRACE_FRAMES;
        let hovered = req.hovered.load(Ordering::Relaxed) && docked && !modal_open;
        let screen_position = Vec2::new(
            f32::from_bits(req.screen_x.load(Ordering::Relaxed)),
            f32::from_bits(req.screen_y.load(Ordering::Relaxed)),
        );
        let w = req.width.load(Ordering::Relaxed).clamp(64, 7680);
        let h = req.height.load(Ordering::Relaxed).clamp(64, 4320);

        let slot = &mut viewports.slots[i];
        slot.docked = docked;
        slot.hovered = hovered;
        slot.screen_position = screen_position;
        slot.screen_size = Vec2::new(w as f32, h as f32);
        if hovered {
            newly_hovered = Some(i);
        }
    }

    // Slot 0's camera can render with no viewport on screen: the UI canvas
    // samples its image as a backdrop, and a running game still needs its
    // atmosphere/IBL probe (see `sync_viewport_camera_activation`).
    let any_docked = viewports.slots.iter().any(|s| s.docked);
    let primary_renders_unseen = !any_docked
        && (play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode())
            || renzora_ember::dock::panel_visible_anywhere(
                "ui_canvas",
                ember_dock.as_deref(),
                ember_fixed.as_deref(),
                ember_windows.as_deref(),
            ));

    #[allow(clippy::needless_range_loop)] // `i` indexes the requests and slots
    for i in 0..VIEWPORT_COUNT {
        let req = &resize_req.slots[i];
        let w = req.width.load(Ordering::Relaxed).clamp(64, 7680);
        let h = req.height.load(Ordering::Relaxed).clamp(64, 4320);
        let slot = &mut viewports.slots[i];
        let docked = slot.docked;

        // A hidden slot whose camera still renders shrinks its target to a
        // token size, so that unseen pass rasterizes almost nothing. Every
        // other hidden slot keeps its size, because its camera is idle and
        // costs nothing: shrinking it anyway meant growing it back on return,
        // and DLSS rebuilt itself for that (about 40 ms) on every workspace
        // switch that brought a viewport back.
        let placeholder = !docked && i == 0 && primary_renders_unseen;
        let requested = if docked {
            UVec2::new(
                ((w as f32 * render_scale).round() as u32).max(1),
                ((h as f32 * render_scale).round() as u32).max(1),
            )
        } else if placeholder || slot.current_size == UVec2::ZERO {
            UVec2::splat(UNDOCKED_TARGET_SIZE)
        } else {
            slot.current_size
        };
        // Held only while it already shows a real size: a slot coming back
        // from the placeholder mid-drag still needs one.
        let hold = resizing && docked && slot.current_size.x > UNDOCKED_TARGET_SIZE;
        if slot.current_size != requested && !hold {
            if let Some(mut image) = slot.image.as_ref().and_then(|h| images.get_mut(h)) {
                image.resize(Extent3d {
                    width: requested.x,
                    height: requested.y,
                    depth_or_array_layers: 1,
                });
                slot.current_size = requested;
            }
        }
    }

    // Focus follows the hovered viewport, and sticks when the pointer leaves
    // all of them so the gizmo/camera keep targeting the last-used view.
    if let Some(i) = newly_hovered {
        viewports.focused = i;
    }
    let mut focused = viewports.focused.min(VIEWPORT_COUNT - 1);
    // Sticking is only right while the view is still there to stick to. A
    // focused slot whose panel has since been closed, collapsed or switched away
    // from would otherwise hold the `EditorCamera` marker off-screen, and every
    // camera gesture would drive a view nobody can see — so hand focus to the
    // first slot that is actually showing.
    if !viewports.slots[focused].docked {
        if let Some(i) = (0..VIEWPORT_COUNT).find(|&i| viewports.slots[i].docked) {
            focused = i;
        }
    }
    viewports.focused = focused;

    // Mirror the focused slot into the singleton ViewportState.
    let slot = &viewports.slots[focused];
    viewport_state.image_handle = slot.image.clone();
    viewport_state.current_size = slot.current_size;
    viewport_state.hovered = slot.hovered;
    viewport_state.screen_position = slot.screen_position;
    viewport_state.screen_size = slot.screen_size;
    viewport_state.docked = slot.docked;
}

/// Render-target edge length for undocked slots. Slot 0 can keep rendering
/// while its own panel is closed (another viewport, the UI canvas or a game
/// still needs the atmosphere/IBL probe it hosts), so this bounds what that
/// costs per pixel.
const UNDOCKED_TARGET_SIZE: u32 = 64;

/// Frames a viewport panel must be missing from the dock before its slot is
/// treated as closed: long enough to cover a dock rebuild, short enough that a
/// workspace without a viewport stops paying for one within a quarter second.
const HIDE_GRACE_FRAMES: u32 = 15;

/// Dock panel id for each viewport slot. Slot 0 keeps the historical `"viewport"`
/// id so existing saved layouts and `contains_panel("viewport")` checks keep working.
const VIEWPORT_PANEL_IDS: [&str; renzora::core::viewport_types::VIEWPORT_COUNT] =
    ["viewport", "viewport-2", "viewport-3", "viewport-4"];

// ── Input focus tracking ─────────────────────────────────────────────────────

/// Sync keyboard / pointer focus state so keyboard shortcut systems can skip
/// when the user is typing in a text field, and so the gizmo box-select gesture
/// doesn't arm while the pointer is over a floating overlay.
fn update_input_focus(
    mut input_focus: ResMut<renzora::core::InputFocusState>,
    ember_inputs: Query<&renzora_ember::widgets::EmberTextInput>,
    drag_editing: Option<Res<renzora_ember::widgets::AnyDragValueEditing>>,
    code_editing: Option<Res<renzora_ember::widgets::AnyCodeEditorFocused>>,
    over_overlay: Option<Res<renzora_ember::widgets::PointerOverOverlay>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
) {
    // A focused bevy_ui (ember) text field "wants keyboard" — so editor
    // keybindings (G/R/S, Delete, …) don't fire while typing in the shell.
    // A drag-value field in keyboard-edit mode counts too: it's a separate
    // widget from EmberTextInput, so without this, typing digits into an
    // inspector number field would also trigger the numpad camera/view shortcuts.
    let ember_focused = ember_inputs.iter().any(|i| i.focused);
    let drag_editing = drag_editing.is_some_and(|r| r.0);
    // A focused code editor owns the keyboard the same way a text field does —
    // otherwise Ctrl+Z in the code panel ALSO fires the scene/hierarchy undo, and
    // typing (e.g. `x`, `s`) triggers gizmo/view shortcuts.
    let code_focused = code_editing.is_some_and(|r| r.0);
    // While simulating, scripts take over input: suppress every editor keyboard
    // shortcut (and the editor-camera WASD) so the running game owns the keys —
    // exactly the behaviour that lets a script's `is_key_*` see them. Stop is
    // still reachable via Esc (checked before this guard) and the Stop button.
    let simulating = play_mode.as_ref().is_some_and(|p| p.is_simulating());
    // A plugin's claim on the keyboard, taken rather than read: the field is a
    // per-frame claim that a plugin re-raises for as long as it wants the keys,
    // so clearing it here is what lets the claim lapse when the plugin stops
    // making it. See `InputFocusState::plugin_wants_keyboard`.
    let plugin_claim = std::mem::take(&mut input_focus.plugin_wants_keyboard);
    input_focus.ui_wants_keyboard =
        ember_focused || drag_editing || code_focused || simulating || plugin_claim;
    // "Pointer over UI" = the cursor is over a floating overlay (dropdown / menu
    // / popup). The viewport's own hover flag (which already excludes overlays)
    // is what gates per-viewport interaction, so this only needs to flag the
    // overlay case for the gizmo's box-select guard.
    input_focus.pointer_over_ui = over_overlay.is_some_and(|r| r.0);
}

// ── View toggle shortcuts ────────────────────────────────────────────────────

fn handle_view_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<renzora::core::InputFocusState>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mut settings: ResMut<ViewportSettings>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }
    if mouse_button.pressed(MouseButton::Right) {
        return;
    }

    if keybindings.just_pressed(EditorAction::ToggleWireframe, &keyboard) {
        settings.render_toggles.wireframe = !settings.render_toggles.wireframe;
    }
    if keybindings.just_pressed(EditorAction::ToggleLighting, &keyboard) {
        settings.render_toggles.lighting = !settings.render_toggles.lighting;
    }
    if keybindings.just_pressed(EditorAction::ToggleGrid, &keyboard) {
        settings.show_grid = !settings.show_grid;
    }
    if keybindings.just_pressed(EditorAction::CameraSpeedUp, &keyboard) {
        settings.camera.move_speed = (settings.camera.move_speed * 1.25).min(500.0);
    }
    if keybindings.just_pressed(EditorAction::CameraSpeedDown, &keyboard) {
        settings.camera.move_speed = (settings.camera.move_speed / 1.25).max(0.1);
    }
    if keybindings.just_pressed(EditorAction::ToggleSnap, &keyboard) {
        let any_on = settings.snap.translate_enabled
            || settings.snap.rotate_enabled
            || settings.snap.scale_enabled;
        let new_state = !any_on;
        settings.snap.translate_enabled = new_state;
        settings.snap.rotate_enabled = new_state;
        settings.snap.scale_enabled = new_state;
    }
    if keybindings.just_pressed(EditorAction::ToggleEdgeSnap, &keyboard) {
        settings.snap.translate_edge_snap = !settings.snap.translate_edge_snap;
    }
    if keybindings.just_pressed(EditorAction::ToggleScaleBottom, &keyboard) {
        settings.snap.scale_bottom_anchor = !settings.snap.scale_bottom_anchor;
    }
}

// ── Play mode shortcuts ──────────────────────────────────────────────────────

fn handle_play_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Res<renzora::core::InputFocusState>,
    mut play_mode: ResMut<renzora::core::PlayModeState>,
) {
    // Escape exits play OR simulate mode — always, regardless of focus state.
    // (Checked before the `ui_wants_keyboard` guard below, which Simulate forces
    // on, so Esc remains the keyboard escape hatch out of a running simulation.)
    if keyboard.just_pressed(KeyCode::Escape)
        && (play_mode.is_in_play_mode() || play_mode.is_simulating())
    {
        play_mode.request_stop = true;
        return;
    }

    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.ui_wants_keyboard {
        return;
    }

    if keybindings.just_pressed(EditorAction::PlayStop, &keyboard) {
        if play_mode.is_in_play_mode() {
            play_mode.request_stop = true;
        } else {
            play_mode.request_play = true;
        }
    }
}


// ── Axis orientation gizmo (top-right corner) ───────────────────────────────

pub(crate) const AXIS_GIZMO_SIZE: f32 = 100.0;
pub(crate) const AXIS_GIZMO_MARGIN: f32 = 24.0; // extra margin to clear the resolution text

// `LastSceneView` used to live here: the scene view (3D or 2D) the viewport
// most recently showed, remembered so the UI canvas's backdrop kept showing the
// 2D scene you were editing rather than snapping to the 3D render when
// selecting a UI entity auto-switched the viewport into UI view. None of that
// exists any more — the canvas is its own panel and samples whatever the
// viewport is currently rendering, so there is no "came from" to remember.

/// What [`log_active_cameras`] reads of each camera it lists.
type LoggedCamera = (
    Entity,
    &'static Camera,
    Option<&'static Name>,
    Has<Camera3d>,
    Has<renzora::core::ViewportRedrawIdle>,
);

/// Log the active cameras whenever that set changes.
///
/// Every active camera runs its whole render schedule each frame, and a camera
/// left on by a panel that is no longer visible is otherwise invisible: an idle
/// editor was measured rendering seven 3D views that no reading of the code could
/// account for. One line per change, never per frame.
///
/// The viewport overlay cameras are left out. They switch on for a frame or two
/// each time a gizmo handle lights up or a brush cursor moves, which would be a
/// pair of lines per hover; and one renders only lines, so it is not the
/// expensive view this log is for.
fn log_active_cameras(
    cameras: Query<LoggedCamera, Without<renzora::core::ViewportOverlayCamera>>,
    mut last: Local<Vec<Entity>>,
) {
    // A camera idled by render-on-change still counts as on: it wakes the
    // moment anything changes, and listing it as off would log twice per touch.
    let mut active: Vec<(Entity, String, bool)> = cameras
        .iter()
        .filter(|(_, cam, .., idle)| cam.is_active || *idle)
        .map(|(e, _, name, is_3d, _)| {
            (e, name.map_or_else(|| format!("{e}"), |n| n.to_string()), is_3d)
        })
        .collect();
    active.sort_unstable_by_key(|(e, ..)| e.to_bits());
    let entities: Vec<Entity> = active.iter().map(|(e, ..)| *e).collect();
    if *last == entities {
        return;
    }
    *last = entities;
    let names: Vec<String> = active
        .into_iter()
        .map(|(_, name, is_3d)| if is_3d { format!("{name} (3D)") } else { name })
        .collect();
    info!("[viewport] {} active cameras: {:?}", names.len(), names);
}

/// The overlay cameras, kept provably apart from the scene, 2D and UI canvas
/// camera queries beside them in [`sync_viewport_camera_activation`].
type OnlyOverlayCameras = (
    Without<renzora::core::ViewportCamera>,
    Without<renzora::core::ViewportCamera2d>,
    Without<renzora_ember_editor::game_ui::canvas_render::UiEditorRenderCamera>,
);

/// Toggles each viewport camera's `is_active` based on whether its panel is
/// docked. The primary (slot 0) camera additionally follows the shared 3D / 2D
/// / UI mode — it backs UI authoring and steps aside for the 2D camera — while
/// the extra slots are plain 3D views that render whenever their panel is open.
///
/// Cameras whose panels aren't docked stay off so unused views cost no GPU; the
/// later optional "freeze" toggle will additionally gate the non-focused live
/// views here.
#[allow(clippy::too_many_arguments)]
fn sync_viewport_camera_activation(
    settings: Option<Res<ViewportSettings>>,
    viewports: Res<renzora::core::viewport_types::Viewports>,
    mut cameras_3d: Query<
        (
            Entity,
            &mut Camera,
            &renzora::core::ViewportCamera,
            Has<renzora::core::ViewportRedrawIdle>,
        ),
        (
            Without<renzora::core::ViewportCamera2d>,
            Without<renzora_ember_editor::game_ui::canvas_render::UiEditorRenderCamera>,
        ),
    >,
    mut cameras_2d: Query<
        (
            Entity,
            &mut Camera,
            &renzora::core::ViewportCamera2d,
            Has<renzora::core::ViewportRedrawIdle>,
        ),
        (
            Without<renzora::core::ViewportCamera>,
            Without<renzora_ember_editor::game_ui::canvas_render::UiEditorRenderCamera>,
        ),
    >,
    mut cameras_ui: Query<
        &mut Camera,
        (
            With<renzora_ember_editor::game_ui::canvas_render::UiEditorRenderCamera>,
            Without<renzora::core::ViewportCamera>,
            Without<renzora::core::ViewportCamera2d>,
        ),
    >,
    mut cameras_overlay: Query<(&mut Camera, &renzora::core::ViewportOverlayCamera), OnlyOverlayCameras>,
    mut overlays: ResMut<overlay::ViewportOverlays>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    kind_2d: Query<(), With<bevy::camera::Camera2d>>,
    dock: Option<Res<renzora_ember::dock::Dock>>,
    fixed_dock: Option<Res<renzora_ember::dock::FixedDock>>,
    dock_windows: Option<Res<renzora_ember::dock::DockWindows>>,
    redraw: Option<Res<redraw::ViewportRedraw>>,
    mut commands: Commands,
) {
    use renzora::core::viewport_types::ViewportView;

    // Render-on-change (see `redraw`): with Realtime off, a camera that would
    // render sits out the frames where nothing in the scene changed.
    let realtime = settings.as_ref().is_some_and(|s| s.realtime);
    let render_ok = realtime || redraw.as_ref().is_none_or(|r| r.should_render());
    let overlay_ok = realtime || redraw.as_ref().is_none_or(|r| r.should_render_overlay());
    // Which slots' 3D scene cameras would render, before the redraw gate: their
    // overlays are on show exactly then.
    let mut scene_wants = [false; renzora::core::viewport_types::VIEWPORT_COUNT];

    let ui_canvas_visible = renzora_ember::dock::panel_visible_anywhere(
        "ui_canvas",
        dock.as_deref(),
        fixed_dock.as_deref(),
        dock_windows.as_deref(),
    );

    let view = settings.map(|s| s.viewport_view).unwrap_or_default();

    // 2D view owns the slot images, and so does in-panel play of a 2D game
    // (whatever view was selected when Play was pressed):
    // `drive_editor_camera_in_play` steers the focused 2D camera onto the game
    // camera's framing, so the 2D cameras must be the ones rendering the panels.
    //
    // "UI view entered from 2D" used to be a third case here — the canvas's
    // scene backdrop showed the slot-0 image and had to keep showing the 2D
    // scene rather than snapping to the 3D render. The canvas is its own panel
    // now and simply samples whatever the viewport is rendering, so there is
    // nothing to remember and no third case.
    let playing_2d_game = play_mode
        .as_ref()
        .is_some_and(|pm| pm.is_in_play_mode())
        && play_mode
            .as_ref()
            .and_then(|pm| pm.active_game_camera)
            .is_some_and(|e| kind_2d.get(e).is_ok());
    let two_d_active = view == ViewportView::Two || playing_2d_game;

    // Whether anything is looking at the primary camera's output: a viewport
    // panel, the UI canvas (it samples that image as its backdrop), or a running
    // game. With none of them the primary idles, exactly as render-on-change
    // idles it, and the scene costs nothing to draw.
    //
    // This replaced hiding every scene mesh while no viewport was on screen.
    // That did stop the draw, but showing the meshes again rebuilt virtual
    // geometry's scene and Solari's ray-tracing structures from nothing, 60 to
    // 120 ms every time a workspace switch brought a viewport back.
    let playing = play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode());
    let primary_seen =
        viewports.slots.iter().any(|s| s.docked) || ui_canvas_visible || playing;

    for (entity, mut camera, vc, idle) in cameras_3d.iter_mut() {
        let docked = viewports.slots.get(vc.0).is_some_and(|s| s.docked);
        let want = if vc.0 == 0 {
            // The primary camera owns the atmosphere + IBL probe. Bevy's
            // atmosphere environment bake panics if that probe exists with no
            // active atmosphere view, and the probe can't be added/removed at
            // runtime without a separate pipeline crash. So while the editor is
            // rendering (the external-runtime case already returned above), the
            // primary stays active as the atmosphere/IBL source — it renders to
            // its own offscreen image regardless of whether its panel is
            // docked or the 2D view is selected. While it renders with no
            // viewport on screen that image is shrunk to `UNDOCKED_TARGET_SIZE`
            // (see `resolve_viewport_slots`), so the unseen pass costs almost
            // nothing per pixel.
            //
            // The render-on-change gate below is the one exception. Idling the
            // primary is safe in Bevy 0.20 because the probe's bind group only
            // needs buffers that outlive a frame with no atmosphere view: the
            // atmosphere is extracted for every 3D camera, active or not, and a
            // zero-count write leaves last frame's per-view buffer in place.
            // That same safety is what lets it sit out while nothing shows it.
            primary_seen
        } else {
            // Secondary 3D views render only when their panel is docked AND we're
            // not in 2D view — in 2D the slot's own `Camera2d` sibling takes over
            // the image, so keeping the 3D camera live would just burn GPU on a
            // render the 2D camera immediately clears over.
            docked && !two_d_active
        };
        let would_render = want;
        if let Some(slot) = scene_wants.get_mut(vc.0) {
            *slot = would_render;
        }
        let want = gate_redraw(&mut commands, entity, idle, want, render_ok);
        if camera.is_active != want {
            camera.is_active = want;
            // Idling and waking for render-on-change is not news: it happens
            // on every touch, and a line each time would bury the log.
            if idle || (would_render && !want) {
                continue;
            }
            // Logged on the TRANSITION, never per frame: a line every frame
            // here would itself cost frames, which is the bug this is meant to
            // help find. The reason is spelled out because "4 cameras were
            // spawned" is what the boot log says, and that is not the same
            // question as which ones are rendering.
            info!(
                "[viewport] 3D camera slot {} -> {} ({})",
                vc.0,
                if want { "ACTIVE" } else { "inactive" },
                if vc.0 == 0 && want {
                    "primary: on while a viewport, the UI canvas or a game shows it"
                } else if vc.0 == 0 {
                    "primary: nothing on screen shows it"
                } else if !docked {
                    "its panel is not docked"
                } else if two_d_active {
                    "2D view owns the slot image"
                } else {
                    "its panel is docked"
                }
            );
        }
    }

    // Each slot's overlay camera (see `overlay`): on show while its scene camera
    // owns a docked slot image in the 3D view, and rendering only when the
    // overlay gate lets it. Slot 0's scene camera also renders for the UI
    // canvas and a running game with no viewport docked; nothing shows its
    // overlay then.
    for (mut camera, oc) in cameras_overlay.iter_mut() {
        let i = oc.0;
        let docked = viewports.slots.get(i).is_some_and(|s| s.docked);
        let shown = scene_wants.get(i).copied().unwrap_or(false) && docked && !two_d_active;
        if let Some(slot) = overlays.shown.get_mut(i) {
            if *slot != shown {
                *slot = shown;
            }
        }
        let want = shown && overlay_ok;
        if camera.is_active != want {
            camera.is_active = want;
        }
    }

    // Each slot's 2D camera activates when its panel is docked and 2D owns the
    // image — so viewports 2/3/4 render the 2D scene from their own framing,
    // just as their 3D cameras render it from their own angle.
    for (entity, mut camera, vc, idle) in cameras_2d.iter_mut() {
        let docked = viewports.slots.get(vc.0).is_some_and(|s| s.docked);
        let would_render = docked && two_d_active;
        let want = gate_redraw(&mut commands, entity, idle, would_render, render_ok);
        if camera.is_active != want {
            camera.is_active = want;
            if idle || (would_render && !want) {
                continue;
            }
            info!(
                "[viewport] 2D camera slot {} -> {} ({})",
                vc.0,
                if want { "ACTIVE" } else { "inactive" },
                if !docked {
                    "its panel is not docked"
                } else if !two_d_active {
                    "3D view owns the slot image"
                } else {
                    "its panel is docked and 2D owns the slot image"
                }
            );
        }
    }

    // The offscreen UI render runs while the `ui_canvas` panel is showing its
    // output — not while the viewport happens to be in a particular view. That
    // was the old gate, and it is exactly the coupling the panel undid: the UI
    // editor no longer lives in this panel and has no business being switched on
    // by what this panel is looking at.
    let want_ui = ui_canvas_visible && !playing_2d_game;
    for mut camera in cameras_ui.iter_mut() {
        if camera.is_active != want_ui {
            info!(
                "[viewport] UI canvas camera -> {} ({})",
                if want_ui { "ACTIVE" } else { "inactive" },
                if !ui_canvas_visible {
                    "the ui_canvas panel is not showing"
                } else if playing_2d_game {
                    "a 2D game is playing in-panel"
                } else {
                    "the ui_canvas panel is showing"
                }
            );
            camera.is_active = want_ui;
        }
    }
}

/// Apply the render-on-change gate to a camera that `would_render`: returns
/// whether it renders, and keeps the [`renzora::core::ViewportRedrawIdle`]
/// marker on exactly the cameras the gate is holding back.
fn gate_redraw(
    commands: &mut Commands,
    entity: Entity,
    idle: bool,
    would_render: bool,
    render_ok: bool,
) -> bool {
    let held_back = would_render && !render_ok;
    if held_back && !idle {
        commands.entity(entity).insert(renzora::core::ViewportRedrawIdle);
    } else if !held_back && idle {
        commands.entity(entity).remove::<renzora::core::ViewportRedrawIdle>();
    }
    would_render && render_ok
}

/// While the 2D editor camera owns the primary panel image, point the primary
/// 3D camera at a token-sized parking image instead — and back at the slot-0
/// image when 3D view returns.
///
/// The primary Camera3d can never be deactivated (it hosts the atmosphere/IBL
/// probe — see `sync_viewport_camera_activation`), and its heavy passes
/// (prepass, GI, bloom, TAA, auto-exposure, tonemapping, MSAA writeback) are
/// per-pixel. In 2D view it used to keep rendering the full 3D scene into the
/// shared image only for the 2D camera to clear over it — the entire 3D
/// pipeline burned for output nobody ever saw. Rendering into a
/// `UNDOCKED_TARGET_SIZE`² image instead is the same trick undocked slots use:
/// every pass still runs (pipelines stay alive, so no add/remove wgpu churn),
/// but rasterizes almost nothing.
fn park_primary_3d_target(
    settings: Option<Res<ViewportSettings>>,
    viewports: Res<renzora::core::viewport_types::Viewports>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    kind_2d: Query<(), With<bevy::camera::Camera2d>>,
    mut images: ResMut<Assets<Image>>,
    mut parking: Local<Option<Handle<Image>>>,
    mut primary: Query<
        &mut bevy::camera::RenderTarget,
        With<renzora::core::PrimaryViewportCamera>,
    >,
) {
    use renzora::core::viewport_types::ViewportView;

    // Same "2D owns the primary image" condition as `sync_viewport_camera_activation`.
    let view = settings.map(|s| s.viewport_view).unwrap_or_default();
    let playing_2d_game = play_mode
        .as_ref()
        .is_some_and(|pm| pm.is_in_play_mode())
        && play_mode
            .as_ref()
            .and_then(|pm| pm.active_game_camera)
            .is_some_and(|e| kind_2d.get(e).is_ok());
    let two_d_owns_image = view == ViewportView::Two || playing_2d_game;

    // `RenderTarget` only exists once `sync_viewport_camera_targets` has bound
    // the startup images, so this query is empty until then — fine, there is
    // nothing to park before the first bind.
    let Ok(mut target) = primary.single_mut() else {
        return;
    };

    let desired: Handle<Image> = if two_d_owns_image {
        parking
            .get_or_insert_with(|| {
                images.add(make_render_target(UNDOCKED_TARGET_SIZE, UNDOCKED_TARGET_SIZE))
            })
            .clone()
    } else {
        let Some(slot0) = viewports.slots.first().and_then(|s| s.image.clone()) else {
            return;
        };
        slot0
    };

    let already = matches!(&*target, bevy::camera::RenderTarget::Image(t) if t.handle == desired);
    if !already {
        *target = bevy::camera::RenderTarget::Image(desired.into());
    }
}

renzora::add!(ViewportPlugin, Editor);
