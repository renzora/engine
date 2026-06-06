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
pub mod html_drop;
pub mod model_drop;
pub mod model_flatten;
mod native_axis_gizmo;
mod native_camera_preview;
mod native_drop;
mod native_header;
mod native_nav;
mod native_viewport;
pub mod persistence;
pub mod play_mode;
pub mod render_systems;
pub mod scene_drop;
pub mod settings;
pub mod shape_drop;
pub mod sprite_drop;

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
use renzora_editor::DockingState;

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
        embedded_asset!(app, "viewport_debug.wgsl");
        app.add_plugins(WireframePlugin::default())
            .add_plugins(MaterialPlugin::<debug_material::ViewportDebugMaterial>::default())
            // Post-tonemap debug visualization for normals + linear depth.
            // Bypasses tonemap/AE so the colors come out as authored.
            .add_plugins(debug_viz::DebugVizPlugin)
            .insert_resource(WireframeConfig {
                global: false,
                default_color: bevy::color::Color::WHITE,
            })
            .init_resource::<ViewportState>()
            .init_resource::<ViewportResizeRequest>()
            .init_resource::<NavOverlayState>()
            .init_resource::<ViewportSettings>()
            .init_resource::<CameraOrbitSnapshot>()
            .init_resource::<renzora::core::InputFocusState>()
            .init_resource::<renzora::core::PlayModeState>()
            .init_resource::<external_runtime::ExternalRuntime>()
            .init_resource::<external_runtime::PausedRenderState>()
            .init_resource::<bevy::winit::WinitSettings>()
            .init_resource::<render_systems::OriginalMaterialStates>()
            .init_resource::<render_systems::LastToggleState>()
            // Per-material-type viz-swap state (one of each per registered type).
            .init_resource::<render_systems::LastVizState<StandardMaterial>>()
            .init_resource::<render_systems::DebugMaterialCache<StandardMaterial>>()
            .init_resource::<render_systems::LastVizState<renzora_terrain::splatmap_material::TerrainSplatmapMaterial>>()
            .init_resource::<render_systems::DebugMaterialCache<renzora_terrain::splatmap_material::TerrainSplatmapMaterial>>()
            .init_resource::<render_systems::LastVizState<renzora_terrain::material::TerrainCheckerboardMaterial>>()
            .init_resource::<render_systems::DebugMaterialCache<renzora_terrain::material::TerrainCheckerboardMaterial>>()
            .init_resource::<render_systems::LastVizState<renzora_terrain::foliage::material::GrassMaterial>>()
            .init_resource::<render_systems::DebugMaterialCache<renzora_terrain::foliage::material::GrassMaterial>>()
            .add_systems(PostStartup, (setup_viewport, camera_preview::setup_camera_preview))
            // Bring scene-loaded model instances onto the production
            // material-binding path the moment Bevy finishes spawning the
            // GLB hierarchy. Drag-drop entities short-circuit this via the
            // `Without<ImportedRoot>` filter inside the observer.
            .add_observer(model_drop::decorate_rehydrated_scene_on_ready)
            .init_resource::<renzora::core::EffectRouting>()
            .init_resource::<model_drop::PendingGltfLoads>()
            .init_resource::<model_drop::ModelDragPreviewState>()
            .init_resource::<renzora_ui::ShapeDragState>()
            .init_resource::<renzora_ui::ShapeDragPreviewState>()
            .init_resource::<native_drop::ArmedViewportDrop>()
            .init_resource::<BrushCursorHiddenByUs>()
            .add_systems(Update, (
                update_input_focus,
                resolve_viewport_slots,
                render_systems::update_render_toggles,
                (
                    render_systems::apply_visualization_mode_for::<StandardMaterial>,
                    render_systems::apply_visualization_mode_for_custom::<renzora_terrain::splatmap_material::TerrainSplatmapMaterial>,
                    render_systems::apply_visualization_mode_for_custom::<renzora_terrain::material::TerrainCheckerboardMaterial>,
                    render_systems::apply_visualization_mode_for_custom::<renzora_terrain::foliage::material::GrassMaterial>,
                ),
                render_systems::update_shadow_settings,
                play_mode::handle_play_mode_transitions,
                external_runtime::poll_external_runtime,
                external_runtime::advance_runtime_phase,
                effect_routing::update_effect_routing,
                (
                    model_drop::spawn_loaded_gltfs,
                    model_flatten::flatten_pending_scenes,
                    // After flatten: any wrappers that survived (e.g. a
                    // multi-child RootNode that flatten couldn't collapse)
                    // get tagged HideInHierarchy. Ordered so we don't write
                    // to entities that flatten is in the middle of despawning.
                    model_flatten::hide_gltf_wrappers
                        .after(model_flatten::flatten_pending_scenes),
                    model_drop::bind_material_refs,
                    model_drop::auto_discover_animations,
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
                        .after(renzora_editor::drain_editor_commands_native),
                ).chain(),
                shape_drop::shape_drag_ground_tracking
                    .before(shape_drop::shape_drag_raycast_system),
                shape_drop::shape_drag_raycast_system
                    .before(shape_drop::update_shape_drag_preview),
                shape_drop::update_shape_drag_preview,
                (
                    shape_drop::native_shape_drop,
                    html_drop::native_html_drop,
                    // Native (bevy_ui) asset drops (material / scene / sprite).
                    // `arm` captures the hovering drop candidate each frame;
                    // `commit` fires it on release — see `native_drop` for why
                    // we can't read the payload at release.
                    (
                        native_drop::arm_viewport_drop,
                        native_drop::commit_viewport_drop,
                    )
                        .chain(),
                ),
                shape_drop::handle_shape_spawn,
                handle_view_shortcuts,
                handle_play_shortcuts,
                hide_cursor_for_brushes,
                (
                    persistence::apply_prefs_on_project_load,
                    persistence::save_on_change
                        .after(persistence::apply_prefs_on_project_load),
                ),
            ).run_if(in_state(renzora_editor::SplashState::Editor)));

        // Always-on panel-visibility gates — toggle is_active on the offscreen
        // cameras when their panels are / are not in the current dock tree so
        // layouts that don't show a given panel don't pay for its render pass.
        app.add_systems(
            Update,
            (
                sync_viewport_camera_activation,
                camera_preview::sync_camera_preview_activation,
            )
                .run_if(in_state(renzora_editor::SplashState::Editor)),
        );

        // Camera-preview spawn/update logic only when its panel is mounted.
        app.add_systems(
            Update,
            (
                camera_preview::update_camera_preview,
                camera_preview::resize_camera_preview,
            )
                .run_if(in_state(renzora_editor::SplashState::Editor))
                .run_if(camera_preview::camera_preview_panel_mounted),
        );

        app.add_systems(Last, external_runtime::kill_on_app_exit);

        // Throttle / restore the editor's render loop around external runs.
        // Not gated on `SplashState` so the restore always runs.
        app.add_systems(Update, external_runtime::apply_runtime_pause_render);

        native_viewport::register_native_viewport(app);
        native_camera_preview::register(app);
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
    active_tool: Option<Res<renzora_editor::ActiveTool>>,
    viewport: Option<Res<renzora::core::viewport_types::ViewportState>>,
    mut cursor_options: Query<&mut bevy::window::CursorOptions>,
    mut ours: ResMut<BrushCursorHiddenByUs>,
) {
    use renzora_editor::ActiveTool;
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
fn setup_viewport(
    mut images: ResMut<Assets<Image>>,
    mut render_target: ResMut<ViewportRenderTarget>,
    mut viewport_state: ResMut<ViewportState>,
    mut viewports: ResMut<renzora::core::viewport_types::Viewports>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;
    bevy::log::info!("[viewport] setup_viewport — creating {VIEWPORT_COUNT} render targets");

    for i in 0..VIEWPORT_COUNT {
        let size = Extent3d {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            depth_or_array_layers: 1,
        };

        let mut image = Image {
            data: Some(vec![0u8; (size.width * size.height * 4) as usize]),
            ..default()
        };
        image.texture_descriptor.size = size;
        image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::COPY_SRC
            | TextureUsages::RENDER_ATTACHMENT;

        let image_handle = images.add(image);

        viewports.slots[i].image = Some(image_handle.clone());
        viewports.slots[i].current_size = UVec2::new(DEFAULT_WIDTH, DEFAULT_HEIGHT);

        if i == 0 {
            render_target.image = Some(image_handle.clone());
            viewport_state.image_handle = Some(image_handle);
            viewport_state.current_size = UVec2::new(DEFAULT_WIDTH, DEFAULT_HEIGHT);
        }
    }
}

/// Per-frame resolver: applies each slot's pending resize, tracks dock
/// membership + hover, picks the focused slot, and mirrors it into the
/// singleton [`ViewportState`] that the gizmo / picking / overlay stack reads.
fn resolve_viewport_slots(
    resize_req: Res<ViewportResizeRequest>,
    docking: Option<Res<DockingState>>,
    ember_dock: Option<Res<renzora_ember::dock::Dock>>,
    modals: Query<(), With<renzora_ember::widgets::ModalSurface>>,
    mut viewports: ResMut<renzora::core::viewport_types::Viewports>,
    mut viewport_state: ResMut<ViewportState>,
    mut images: ResMut<Assets<Image>>,
) {
    use renzora::core::viewport_types::VIEWPORT_COUNT;

    // A bevy_ui modal (settings overlay, search/add-component overlay, …) covers
    // the viewport and must swallow the wheel/pointer — otherwise scrolling over
    // the modal also zooms the 3D camera behind it.
    let modal_open = !modals.is_empty();

    let mut newly_hovered: Option<usize> = None;
    #[allow(clippy::needless_range_loop)] // `i` indexes several parallel arrays
    for i in 0..VIEWPORT_COUNT {
        let req = &resize_req.slots[i];
        // "Docked" = the slot's panel is present in whichever dock is live. The
        // egui `DockingState` only knows about the egui dock, so when the
        // bevy_ui dock is active it reports the viewport as absent — which would
        // kill `hovered` (and with it camera nav + picking). OR in the ember
        // dock so the native viewport stays interactive.
        let egui_docked = docking
            .as_ref()
            .is_none_or(|d| d.tree.contains_panel(VIEWPORT_PANEL_IDS[i]));
        let ember_docked = ember_dock
            .as_ref()
            .is_some_and(|d| d.tree.is_active_tab(VIEWPORT_PANEL_IDS[i]));
        let docked = egui_docked || ember_docked;
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

        let requested = UVec2::new(w, h);
        if slot.current_size != requested {
            if let Some(image) = slot.image.as_ref().and_then(|h| images.get_mut(h)) {
                image.resize(Extent3d {
                    width: w,
                    height: h,
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
    let focused = viewports.focused.min(VIEWPORT_COUNT - 1);
    viewports.focused = focused;

    // Mirror the focused slot into the singleton ViewportState.
    let slot = &viewports.slots[focused];
    viewport_state.image_handle = slot.image.clone();
    viewport_state.current_size = slot.current_size;
    viewport_state.hovered = slot.hovered;
    viewport_state.screen_position = slot.screen_position;
    viewport_state.screen_size = slot.screen_size;
}

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
    over_overlay: Option<Res<renzora_ember::widgets::PointerOverOverlay>>,
) {
    // A focused bevy_ui (ember) text field "wants keyboard" — so editor
    // keybindings (G/R/S, Delete, …) don't fire while typing in the shell.
    let ember_focused = ember_inputs.iter().any(|i| i.focused);
    input_focus.egui_wants_keyboard = ember_focused;
    // "Pointer over UI" = the cursor is over a floating overlay (dropdown / menu
    // / popup). The viewport's own hover flag (which already excludes overlays)
    // is what gates per-viewport interaction, so this only needs to flag the
    // overlay case for the gizmo's box-select guard.
    input_focus.egui_has_pointer = over_overlay.is_some_and(|r| r.0);
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
    if input_focus.egui_wants_keyboard {
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
    // Escape exits play mode — always, regardless of focus state
    if keyboard.just_pressed(KeyCode::Escape) && play_mode.is_in_play_mode() {
        play_mode.request_stop = true;
        return;
    }

    if keybindings.rebinding.is_some() {
        return;
    }
    if input_focus.egui_wants_keyboard {
        return;
    }

    if keybindings.just_pressed(EditorAction::PlayStop, &keyboard) {
        if play_mode.is_in_play_mode() || play_mode.is_scripts_only() {
            play_mode.request_stop = true;
        } else {
            play_mode.request_play = true;
        }
    }
    if keybindings.just_pressed(EditorAction::PlayScriptsOnly, &keyboard) {
        if play_mode.is_in_play_mode() || play_mode.is_scripts_only() {
            play_mode.request_stop = true;
        } else {
            play_mode.request_scripts_only = true;
        }
    }
}


// ── Axis orientation gizmo (top-right corner) ───────────────────────────────

pub(crate) const AXIS_GIZMO_SIZE: f32 = 100.0;
pub(crate) const AXIS_GIZMO_MARGIN: f32 = 24.0; // extra margin to clear the resolution text


/// Toggles each viewport camera's `is_active` based on whether its panel is
/// docked. The primary (slot 0) camera additionally follows the shared 3D / 2D
/// / UI mode — it backs UI authoring and steps aside for the 2D camera — while
/// the extra slots are plain 3D views that render whenever their panel is open.
///
/// Cameras whose panels aren't docked stay off so unused views cost no GPU; the
/// later optional "freeze" toggle will additionally gate the non-focused live
/// views here.
fn sync_viewport_camera_activation(
    settings: Option<Res<ViewportSettings>>,
    viewports: Res<renzora::core::viewport_types::Viewports>,
    mut cameras_3d: Query<
        (&mut Camera, &renzora::core::ViewportCamera),
        (
            Without<renzora::core::EditorCamera2d>,
            Without<renzora_game_ui::canvas_render::UiEditorRenderCamera>,
        ),
    >,
    mut cameras_2d: Query<
        &mut Camera,
        (
            With<renzora::core::EditorCamera2d>,
            Without<renzora::core::ViewportCamera>,
            Without<renzora_game_ui::canvas_render::UiEditorRenderCamera>,
        ),
    >,
    mut cameras_ui: Query<
        &mut Camera,
        (
            With<renzora_game_ui::canvas_render::UiEditorRenderCamera>,
            Without<renzora::core::ViewportCamera>,
            Without<renzora::core::EditorCamera2d>,
        ),
    >,
    runtime: Option<Res<external_runtime::ExternalRuntime>>,
) {
    use renzora::core::viewport_types::ViewportView;

    // While an external runtime owns the screen the editor is paused behind a
    // full-screen overlay, so there's nothing to see through the offscreen
    // viewport cameras. Force them all inactive to skip their (expensive)
    // render passes and hand the GPU to the running game.
    let runtime_active = runtime
        .as_ref()
        .is_some_and(|r| r.phase() != external_runtime::RuntimePhase::Idle);
    if runtime_active {
        for (mut camera, _) in cameras_3d.iter_mut() {
            if camera.is_active {
                camera.is_active = false;
            }
        }
        for mut camera in cameras_2d.iter_mut().chain(cameras_ui.iter_mut()) {
            if camera.is_active {
                camera.is_active = false;
            }
        }
        return;
    }

    let view = settings.map(|s| s.viewport_view).unwrap_or_default();
    let primary_docked = viewports.slots.first().is_some_and(|s| s.docked);

    for (mut camera, vc) in cameras_3d.iter_mut() {
        let docked = viewports.slots.get(vc.0).is_some_and(|s| s.docked);
        let want = if vc.0 == 0 {
            // The primary camera owns the atmosphere + IBL probe. Bevy's
            // atmosphere environment bake panics if that probe exists with no
            // active atmosphere view, and the probe can't be added/removed at
            // runtime without a separate pipeline crash. So while the editor is
            // rendering (the external-runtime case already returned above), the
            // primary stays active as the atmosphere/IBL source — it renders to
            // its own offscreen image regardless of whether its panel is
            // docked or the 2D view is selected. Keeping it on is cheap next to
            // the crash it prevents.
            true
        } else {
            docked
        };
        if camera.is_active != want {
            camera.is_active = want;
        }
    }

    let want_2d = primary_docked && view == ViewportView::Two;
    let want_ui = primary_docked && view == ViewportView::Ui;
    for mut camera in cameras_2d.iter_mut() {
        if camera.is_active != want_2d {
            camera.is_active = want_2d;
        }
    }
    for mut camera in cameras_ui.iter_mut() {
        if camera.is_active != want_ui {
            camera.is_active = want_ui;
        }
    }
}

renzora::add!(ViewportPlugin, Editor);
