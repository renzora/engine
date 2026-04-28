//! Renzora Viewport — editor panel that displays the 3D game world.
//!
//! Creates an offscreen render target, wires it to the runtime camera,
//! and displays the result as an egui image inside the docking panel system.

pub mod camera_preview;
pub mod debug_material;
pub mod effect_routing;
pub mod glb_compat;
pub mod header;
pub mod material_drop;
pub mod model_drop;
pub mod model_flatten;
pub mod play_mode;
pub mod scene_drop;
pub mod shape_drop;
pub mod render_systems;
pub mod settings;
pub mod persistence;
pub mod toolbar;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::asset::embedded_asset;
use bevy::pbr::{MaterialPlugin, wireframe::{WireframeConfig, WireframePlugin}};
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui;
use bevy_egui::{EguiContexts, EguiTextureHandle, EguiUserTextures};
use egui_phosphor::regular;
use renzora_editor::{AppEditorExt, DockingState, EditorPanel, PanelLocation};
use renzora::core::EditorCamera;
use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::ViewportRenderTarget;
use renzora_theme::ThemeManager;

pub use camera_preview::CameraPreviewState;
// Re-export all viewport types from core (they now live in renzora::viewport_types)
pub use renzora::core::viewport_types::{
    CameraOrbitSnapshot, CameraSettingsState, CollisionGizmoVisibility, NavOverlayState,
    ProjectionMode, RenderToggles, SnapSettings, ViewAngleCommand, ViewportSettings,
    ViewportState, VisualizationMode,
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
            .init_resource::<renzora::core::EffectRouting>()
            .init_resource::<model_drop::PendingGltfLoads>()
            .init_resource::<model_drop::ModelDragPreviewState>()
            .init_resource::<renzora_ui::ShapeDragState>()
            .init_resource::<renzora_ui::ShapeDragPreviewState>()
            .init_resource::<BrushCursorHiddenByUs>()
            .add_systems(Update, (
                update_input_focus,
                handle_viewport_resize,
                render_systems::update_render_toggles,
                (
                    render_systems::apply_visualization_mode_for::<StandardMaterial>,
                    render_systems::apply_visualization_mode_for_custom::<renzora_terrain::splatmap_material::TerrainSplatmapMaterial>,
                    render_systems::apply_visualization_mode_for_custom::<renzora_terrain::material::TerrainCheckerboardMaterial>,
                    render_systems::apply_visualization_mode_for_custom::<renzora_terrain::foliage::material::GrassMaterial>,
                ),
                render_systems::update_shadow_settings,
                play_mode::handle_play_mode_transitions,
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
                    model_drop::apply_ghost_material_override,
                    model_drop::cleanup_model_drag_ghost,
                ).chain(),
                shape_drop::shape_drag_ground_tracking
                    .before(shape_drop::shape_drag_raycast_system),
                shape_drop::shape_drag_raycast_system
                    .before(shape_drop::update_shape_drag_preview),
                shape_drop::update_shape_drag_preview,
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
        app.add_systems(Update, (
            sync_viewport_camera_activation,
            camera_preview::sync_camera_preview_activation,
        ).run_if(in_state(renzora_editor::SplashState::Editor)));

        // Camera-preview spawn/update logic only when its panel is mounted.
        app.add_systems(
            Update,
            camera_preview::update_camera_preview
                .run_if(in_state(renzora_editor::SplashState::Editor))
                .run_if(camera_preview::camera_preview_panel_mounted),
        );

// Register the crosshair overlay so the cursor goes to Crosshair
        // whenever the pointer is over the viewport rect.
        app.world_mut()
            .resource_mut::<renzora_editor::ViewportOverlayRegistry>()
            .register(150, draw_viewport_cursor_overlay);

        app.register_panel(ViewportPanel);
        app.register_panel(CameraPreviewPanel);
    }
}

/// Egui overlay that sets the viewport cursor to a crosshair whenever the
/// pointer is inside the viewport rect. Brush tools and modal transforms
/// separately hide the OS cursor, so the crosshair is only actually seen
/// in the "normal" gizmo-tool states, which is what we want.
fn draw_viewport_cursor_overlay(
    ui: &mut bevy_egui::egui::Ui,
    world: &World,
    rect: bevy_egui::egui::Rect,
) {
    use bevy_egui::egui::CursorIcon;
    use renzora_editor::ActiveTool;

    // Brushes hide the cursor entirely; don't fight them with a crosshair icon.
    if let Some(tool) = world.get_resource::<ActiveTool>() {
        if matches!(
            *tool,
            ActiveTool::TerrainSculpt | ActiveTool::TerrainPaint | ActiveTool::FoliagePaint
        ) {
            return;
        }
    }

    // Only show the crosshair when the pointer is actually over the viewport
    // with nothing on top. `is_pointer_over_area` reports true when the pointer
    // is over any floating egui Area — dropdowns, popups, context menus,
    // tooltips, and the vertical toolbar / nav / play overlays. The viewport
    // itself is drawn into a panel (not an Area), so this cleanly excludes
    // overlays without excluding the viewport.
    let ctx = ui.ctx();
    let pointer_in = ctx.pointer_hover_pos().map_or(false, |p| rect.contains(p));
    let obstructed = ctx.is_pointer_over_area() || ctx.wants_pointer_input();
    if pointer_in && !obstructed {
        ctx.set_cursor_icon(CursorIcon::Crosshair);
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
    let Ok(mut cursor) = cursor_options.single_mut() else { return };
    let brush_active = matches!(
        active_tool.as_deref(),
        Some(ActiveTool::TerrainSculpt | ActiveTool::TerrainPaint | ActiveTool::FoliagePaint)
    );
    let hovered = viewport.as_deref().map_or(false, |v| v.hovered);
    let should_hide = brush_active && hovered;

    if should_hide && !ours.0 {
        cursor.visible = false;
        ours.0 = true;
    } else if !should_hide && ours.0 {
        cursor.visible = true;
        ours.0 = false;
    }
}

/// Atomically-writable resize request from the panel's `ui()` method.
///
/// The panel writes the desired size here (from `&World`), and an `Update`
/// system reads it to resize the render texture when needed.
#[derive(Resource)]
pub struct ViewportResizeRequest {
    pub width: AtomicU32,
    pub height: AtomicU32,
    pub hovered: AtomicBool,
    pub screen_x: AtomicU32,
    pub screen_y: AtomicU32,
}

impl Default for ViewportResizeRequest {
    fn default() -> Self {
        Self {
            width: AtomicU32::new(DEFAULT_WIDTH),
            height: AtomicU32::new(DEFAULT_HEIGHT),
            hovered: AtomicBool::new(false),
            screen_x: AtomicU32::new(0),
            screen_y: AtomicU32::new(0),
        }
    }
}

/// Creates the offscreen render target and wires it to the runtime camera.
fn setup_viewport(
    mut images: ResMut<Assets<Image>>,
    mut render_target: ResMut<ViewportRenderTarget>,
    mut user_textures: ResMut<EguiUserTextures>,
    mut viewport_state: ResMut<ViewportState>,
) {
    bevy::log::info!("[viewport] setup_viewport running — creating render target image");
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

    // Register with egui so the panel can display it
    user_textures.add_image(EguiTextureHandle::Strong(image_handle.clone()));

    // Tell the runtime camera to render here
    render_target.image = Some(image_handle.clone());

    // Store for the panel and resize system
    viewport_state.image_handle = Some(image_handle);
    viewport_state.current_size = UVec2::new(DEFAULT_WIDTH, DEFAULT_HEIGHT);
}

/// Checks if the panel requested a resize and updates the render texture.
fn handle_viewport_resize(
    resize_req: Res<ViewportResizeRequest>,
    mut viewport_state: ResMut<ViewportState>,
    mut images: ResMut<Assets<Image>>,
) {
    // Sync hover state and screen position
    viewport_state.hovered = resize_req.hovered.load(Ordering::Relaxed);
    viewport_state.screen_position = Vec2::new(
        f32::from_bits(resize_req.screen_x.load(Ordering::Relaxed)),
        f32::from_bits(resize_req.screen_y.load(Ordering::Relaxed)),
    );

    let w = resize_req.width.load(Ordering::Relaxed);
    let h = resize_req.height.load(Ordering::Relaxed);

    // Clamp to reasonable bounds
    let w = w.max(64).min(7680);
    let h = h.max(64).min(4320);

    viewport_state.screen_size = Vec2::new(w as f32, h as f32);

    let requested = UVec2::new(w, h);
    if viewport_state.current_size == requested {
        return;
    }

    if let Some(ref handle) = viewport_state.image_handle {
        if let Some(image) = images.get_mut(handle) {
            image.resize(Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            });
            viewport_state.current_size = requested;
        }
    }
}

// ── Viewport Panel ──────────────────────────────────────────────────────────

/// Editor panel that displays the 3D game world rendered by the runtime camera.
pub struct ViewportPanel;

impl EditorPanel for ViewportPanel {
    fn id(&self) -> &str {
        "viewport"
    }

    fn title(&self) -> &str {
        "Viewport"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::MONITOR)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        // Header bar with toggles and dropdowns
        header::viewport_header(ui, world);

        let rect = ui.available_rect_before_wrap();

        // Request resize to match panel dimensions + track hover
        if let Some(req) = world.get_resource::<ViewportResizeRequest>() {
            let w = (rect.width().max(1.0)) as u32;
            let h = (rect.height().max(1.0)) as u32;
            req.width.store(w, Ordering::Relaxed);
            req.height.store(h, Ordering::Relaxed);
            req.screen_x.store(rect.min.x.to_bits(), Ordering::Relaxed);
            req.screen_y.store(rect.min.y.to_bits(), Ordering::Relaxed);
            // Treat the viewport as NOT hovered while any egui widget is
            // being dragged (panel resize handle, tab undock, hierarchy
            // drag, etc.) so the gizmo's box-select gesture doesn't arm
            // and viewport-only systems sleep until the drag releases.
            let egui_dragging = ui.ctx().dragged_id().is_some()
                || ui.ctx().is_using_pointer();
            let is_hovered = ui.rect_contains_pointer(rect) && !egui_dragging;
            req.hovered.store(is_hovered, Ordering::Relaxed);
        }

        // Look up the egui texture ID for our render target
        let texture_id = world
            .get_resource::<ViewportState>()
            .and_then(|vs| vs.image_handle.as_ref())
            .and_then(|handle| {
                world
                    .get_resource::<EguiUserTextures>()
                    .and_then(|ut| ut.image_id(handle.id()))
            });

        if let Some(texture_id) = texture_id {
            let size = egui::vec2(rect.width(), rect.height());
            ui.put(
                rect,
                egui::Image::new(egui::load::SizedTexture::new(texture_id, size)),
            );

            // CPU-projected overlays (grid, gizmos) paint on top of the 3D
            // image, bypassing the Bevy render pipeline entirely.
            if let Some(overlay) = world.get_resource::<renzora_editor::ViewportOverlayRegistry>() {
                overlay.draw_all(ui, world, rect);
            }
        } else {
            // Fallback while render target is being set up
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 25));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Initializing viewport...",
                egui::FontId::proportional(14.0),
                egui::Color32::from_white_alpha(80),
            );
        }

        // Check for model asset drops on the viewport
        model_drop::check_viewport_model_drop(ui, world, rect);

        // Check for .material asset drops on the viewport
        material_drop::check_viewport_material_drop(ui, world, rect);

        // Check for .ron scene drops (spawns a SceneInstance)
        scene_drop::check_viewport_scene_drop(ui, world, rect);

        // Check for shape library drops on the viewport
        shape_drop::check_viewport_shape_drop(ui, world, rect);

        // Overlay: modal transform HUD (scale circle, mode text, axis info)
        render_modal_transform_hud(ui.ctx(), world, rect);

        // Overlay: horizontal tool bar under the header (gizmo modes, terrain tools)
        toolbar::render_tool_overlay(ui.ctx(), world, rect);

        // Overlay: on-screen console logs during play mode
        render_viewport_logs(ui, world, rect);

        // Overlay: resolution indicator
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| &tm.active_theme);
        let info_color = theme
            .map(|t| t.text.muted.to_color32())
            .unwrap_or(egui::Color32::from_white_alpha(50));

        if let Some(vs) = world.get_resource::<ViewportState>() {
            ui.painter().text(
                egui::Pos2::new(rect.max.x - 8.0, rect.min.y + 6.0),
                egui::Align2::RIGHT_TOP,
                format!("{} x {}", vs.current_size.x, vs.current_size.y),
                egui::FontId::proportional(10.0),
                info_color,
            );
        }

        // Overlay: model load progress (mesh-only ghost + textured drops)
        render_model_load_progress(ui, world, rect);

        // Overlay: axis orientation gizmo
        let show_axis = world
            .get_resource::<ViewportSettings>()
            .map_or(true, |s| s.show_axis_gizmo);
        let play_mode = world.get_resource::<renzora::core::PlayModeState>();
        let in_play = play_mode.map_or(false, |p| p.is_in_play_mode());
        if show_axis && !in_play {
            render_axis_gizmo(ui.ctx(), world, rect);
        }

        // Overlay: nav pan/zoom buttons (right side, below axis gizmo)
        if !in_play {
            toolbar::render_nav_overlay(ui.ctx(), world, rect);
        }
    }

    fn closable(&self) -> bool {
        false
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Camera Preview Panel ────────────────────────────────────────────────────

pub struct CameraPreviewPanel;

impl EditorPanel for CameraPreviewPanel {
    fn id(&self) -> &str {
        "camera_preview"
    }

    fn title(&self) -> &str {
        "Camera Preview"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::APERTURE)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let preview = world.get_resource::<CameraPreviewState>();
        let user_textures = world.get_resource::<EguiUserTextures>();

        let has_preview = preview.as_ref().map_or(false, |p| p.previewing.is_some());

        if !has_preview {
            let theme = world
                .get_resource::<ThemeManager>()
                .map(|tm| &tm.active_theme);
            let text_color = theme
                .map(|t| t.text.muted.to_color32())
                .unwrap_or(egui::Color32::from_white_alpha(80));

            ui.centered_and_justified(|ui| {
                ui.label(egui::RichText::new("No cameras in scene").color(text_color));
            });
            return;
        }

        // Camera name overlay
        let previewing_entity = preview.as_ref().and_then(|p| p.previewing);
        let camera_name = previewing_entity.and_then(|e| {
            world.get::<Name>(e).map(|n| n.as_str().to_string())
        }).unwrap_or_else(|| "Camera".to_string());

        let is_default = previewing_entity.map_or(false, |e| {
            world.get::<renzora::core::DefaultCamera>(e).is_some()
        });

        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| &tm.active_theme);
        let muted_color = theme
            .map(|t| t.text.muted.to_color32())
            .unwrap_or(egui::Color32::from_white_alpha(80));

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&camera_name).size(11.0).color(muted_color));
            if is_default {
                ui.label(egui::RichText::new(regular::STAR).size(10.0).color(egui::Color32::from_rgb(255, 200, 80)));
            }
        });

        let available_width = ui.available_width();
        let preview_height = available_width * (9.0 / 16.0);

        let texture_id = preview.and_then(|p| {
            p.texture_id.or_else(|| {
                user_textures.and_then(|ut| ut.image_id(p.image_handle.id()))
            })
        });

        if let Some(texture_id) = texture_id {
            ui.add(egui::Image::new(egui::load::SizedTexture::new(
                texture_id,
                [available_width, preview_height],
            )));
        } else {
            let bg = theme
                .map(|t| t.surfaces.faint.to_color32())
                .unwrap_or(egui::Color32::from_gray(30));
            let text_color = theme
                .map(|t| t.text.disabled.to_color32())
                .unwrap_or(egui::Color32::from_white_alpha(50));

            egui::Frame::new().fill(bg).show(ui, |ui| {
                ui.set_min_size(egui::Vec2::new(available_width, preview_height));
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Preview loading...").color(text_color));
                });
            });
        }
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }
}

// ── Input focus tracking ─────────────────────────────────────────────────────

/// Sync egui keyboard focus state so keyboard shortcut systems can skip
/// when the user is typing in a text field.
fn update_input_focus(
    mut ctx: EguiContexts,
    mut input_focus: ResMut<renzora::core::InputFocusState>,
) {
    if let Ok(c) = ctx.ctx_mut() {
        input_focus.egui_wants_keyboard = c.wants_keyboard_input();
        input_focus.egui_has_pointer = c.wants_pointer_input() || c.is_pointer_over_area();
    }
}

// ── Modal transform HUD overlay ──────────────────────────────────────────────

fn render_modal_transform_hud(ctx: &egui::Context, world: &World, viewport_rect: egui::Rect) {
    let Some(hud) = world.get_resource::<renzora::core::ModalTransformHud>() else { return };
    if !hud.active {
        return;
    }

    let axis_color = egui::Color32::from_rgba_unmultiplied(
        hud.axis_color[0], hud.axis_color[1], hud.axis_color[2], hud.axis_color[3],
    );

    // Scale mode: draw circle at pivot + line to cursor + dots
    if hud.is_scale {
        if let Some(pivot) = hud.pivot {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("modal_scale_overlay"),
            ));

            let pivot_pos = egui::Pos2::new(pivot[0], pivot[1]);
            let cursor_pos = egui::Pos2::new(hud.cursor[0], hud.cursor[1]);

            const CIRCLE_RADIUS: f32 = 30.0;
            painter.circle_stroke(pivot_pos, CIRCLE_RADIUS, egui::Stroke::new(1.5, axis_color));
            painter.line_segment([pivot_pos, cursor_pos], egui::Stroke::new(1.5, axis_color));
            painter.circle_filled(pivot_pos, 3.0, axis_color);
            painter.circle_filled(cursor_pos, 3.0, axis_color);
        }
    }

    // HUD bar at bottom of viewport
    let hud_height = 60.0;
    let hud_width = 300.0;
    let hud_rect = egui::Rect::from_min_size(
        egui::Pos2::new(
            viewport_rect.center().x - hud_width / 2.0,
            viewport_rect.max.y - hud_height - 10.0,
        ),
        egui::Vec2::new(hud_width, hud_height),
    );

    egui::Area::new(egui::Id::new("modal_transform_hud"))
        .fixed_pos(hud_rect.min)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter();

            // Background
            painter.rect_filled(
                hud_rect,
                8.0,
                egui::Color32::from_rgba_unmultiplied(30, 30, 35, 230),
            );
            painter.rect_stroke(
                hud_rect,
                8.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 70)),
                egui::StrokeKind::Outside,
            );

            // Mode text
            painter.text(
                egui::Pos2::new(hud_rect.center().x, hud_rect.min.y + 16.0),
                egui::Align2::CENTER_CENTER,
                hud.mode,
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );

            // Axis constraint
            if !hud.axis_name.is_empty() {
                painter.text(
                    egui::Pos2::new(hud_rect.center().x, hud_rect.min.y + 32.0),
                    egui::Align2::CENTER_CENTER,
                    format!("Axis: {}", hud.axis_name),
                    egui::FontId::proportional(12.0),
                    axis_color,
                );
            }

            // Numeric input
            if !hud.numeric_display.is_empty() {
                painter.text(
                    egui::Pos2::new(hud_rect.center().x, hud_rect.min.y + 44.0),
                    egui::Align2::CENTER_CENTER,
                    format!("Value: {}", hud.numeric_display),
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_rgb(100, 200, 255),
                );
            }

            // Help text
            painter.text(
                egui::Pos2::new(hud_rect.center().x, hud_rect.max.y - 8.0),
                egui::Align2::CENTER_CENTER,
                "X/Y/Z axis | Enter confirm | Esc cancel",
                egui::FontId::proportional(10.0),
                egui::Color32::from_rgb(120, 120, 130),
            );
        });
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
    if play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode()) { return; }
    if keybindings.rebinding.is_some() { return; }
    if input_focus.egui_wants_keyboard { return; }
    if mouse_button.pressed(MouseButton::Right) { return; }

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

    if keybindings.rebinding.is_some() { return; }
    if input_focus.egui_wants_keyboard { return; }

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

// ── Model load progress overlay ────────────────────────────────────────────

fn render_model_load_progress(ui: &mut egui::Ui, world: &World, viewport_rect: egui::Rect) {
    let entries = model_drop::collect_model_load_progress(world);
    if entries.is_empty() {
        return;
    }

    let theme = match world.get_resource::<ThemeManager>() {
        Some(tm) => tm.active_theme.clone(),
        None => return,
    };

    let panel_width: f32 = 240.0;
    let row_height: f32 = 32.0;
    let padding: f32 = 8.0;
    let total_height = padding * 2.0 + row_height * entries.len() as f32;
    let pos = egui::Pos2::new(
        viewport_rect.min.x + 12.0,
        viewport_rect.max.y - total_height - 12.0,
    );

    egui::Area::new(egui::Id::new("viewport_model_load_progress"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ui.ctx(), |ui| {
            let frame = egui::Frame::NONE
                .fill(theme.surfaces.panel.to_color32())
                .stroke(egui::Stroke::new(
                    1.0,
                    theme.widgets.border.to_color32(),
                ))
                .inner_margin(egui::Margin::symmetric(8, 8))
                .corner_radius(egui::CornerRadius::same(4));
            frame.show(ui, |ui| {
                ui.set_width(panel_width);
                for (name, frac) in entries {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        renzora_ui::widgets::spinner(ui, 12.0, &theme);
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(name)
                                    .font(egui::FontId::proportional(10.0))
                                    .color(theme.text.primary.to_color32()),
                            );
                            renzora_ui::widgets::progress_bar(
                                ui,
                                frac.unwrap_or(0.4),
                                4.0,
                                &theme,
                            );
                        });
                    });
                }
            });
        });
    ui.ctx().request_repaint();
}

// ── On-screen console log overlay (play mode) ──────────────────────────────

const LOG_MAX_VISIBLE: usize = 12;
const LOG_DISPLAY_DURATION: f64 = 5.0;
const LOG_FADE_DURATION: f64 = 1.0;

fn render_viewport_logs(ui: &mut egui::Ui, world: &World, viewport_rect: egui::Rect) {
    use renzora_console::state::ConsoleState;

    // Only show during play mode
    let Some(play_mode) = world.get_resource::<renzora::core::PlayModeState>() else { return };
    if !play_mode.is_in_play_mode() && !play_mode.is_scripts_only() { return; }

    let Some(console) = world.get_resource::<ConsoleState>() else { return };
    let current_time = world.resource::<Time>().elapsed_secs_f64();

    // Collect recent entries (within display duration)
    let recent: Vec<_> = console.entries.iter().rev()
        .filter(|e| {
            let age = current_time - e.timestamp;
            age < LOG_DISPLAY_DURATION && e.timestamp > 0.0
        })
        .take(LOG_MAX_VISIBLE)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    if recent.is_empty() { return; }

    let painter = ui.painter();
    let mut y = viewport_rect.min.y + 10.0;
    let x = viewport_rect.min.x + 12.0;
    let font = egui::FontId::monospace(12.0);

    for entry in &recent {
        let age = current_time - entry.timestamp;
        let fade_start = LOG_DISPLAY_DURATION - LOG_FADE_DURATION;
        let alpha = if age > fade_start {
            ((LOG_DISPLAY_DURATION - age) / LOG_FADE_DURATION) as f32
        } else {
            1.0
        }.clamp(0.0, 1.0);

        if alpha <= 0.0 { continue; }

        let [r, g, b] = entry.level.color();
        let color = egui::Color32::from_rgba_unmultiplied(r, g, b, (alpha * 255.0) as u8);
        let shadow_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, (alpha * 180.0) as u8);

        let prefix = if entry.category.is_empty() {
            String::new()
        } else {
            format!("[{}] ", entry.category)
        };
        let text = format!("{}{}", prefix, entry.message);

        // Drop shadow
        painter.text(
            egui::Pos2::new(x + 1.0, y + 1.0),
            egui::Align2::LEFT_TOP,
            &text,
            font.clone(),
            shadow_color,
        );
        // Foreground
        painter.text(
            egui::Pos2::new(x, y),
            egui::Align2::LEFT_TOP,
            &text,
            font.clone(),
            color,
        );

        y += 16.0;
    }
}

// ── Axis orientation gizmo (top-right corner) ───────────────────────────────

pub(crate) const AXIS_GIZMO_SIZE: f32 = 100.0;
pub(crate) const AXIS_GIZMO_MARGIN: f32 = 24.0; // extra margin to clear the resolution text

fn render_axis_gizmo(
    ctx: &egui::Context,
    world: &World,
    viewport_rect: egui::Rect,
) {
    let Some(orbit) = world.get_resource::<CameraOrbitSnapshot>() else { return };
    let Some(nav) = world.get_resource::<NavOverlayState>() else { return };
    let Some(cmds) = world.get_resource::<renzora_editor::EditorCommands>() else { return };
    let center = egui::Pos2::new(
        viewport_rect.max.x - AXIS_GIZMO_SIZE / 2.0 - AXIS_GIZMO_MARGIN,
        viewport_rect.min.y + AXIS_GIZMO_SIZE / 2.0 + AXIS_GIZMO_MARGIN,
    );

    let cos_yaw = orbit.yaw.cos();
    let sin_yaw = orbit.yaw.sin();
    let cos_pitch = orbit.pitch.cos();
    let sin_pitch = orbit.pitch.sin();

    // Axes: (world dir, color, label, target_yaw, target_pitch, is_positive)
    let axes: [(Vec3, egui::Color32, &str, f32, f32, bool); 6] = [
        (Vec3::X,  egui::Color32::from_rgb(237, 76, 92),   "X",  std::f32::consts::FRAC_PI_2, 0.0, true),
        (Vec3::Y,  egui::Color32::from_rgb(139, 201, 63),  "Y",  0.0, std::f32::consts::FRAC_PI_2, true),
        (Vec3::Z,  egui::Color32::from_rgb(68, 138, 255),  "Z",  0.0, 0.0, true),
        (-Vec3::X, egui::Color32::from_rgb(150, 50, 60),   "-X", -std::f32::consts::FRAC_PI_2, 0.0, false),
        (-Vec3::Y, egui::Color32::from_rgb(80, 120, 40),   "-Y", 0.0, -std::f32::consts::FRAC_PI_2, false),
        (-Vec3::Z, egui::Color32::from_rgb(40, 80, 150),   "-Z", std::f32::consts::PI, 0.0, false),
    ];

    let axis_length = AXIS_GIZMO_SIZE / 2.0 - 12.0;

    // Project each axis to screen space, collecting (depth, offset, color, label, yaw, pitch, positive)
    let mut projected: Vec<(f32, egui::Vec2, egui::Color32, &str, f32, f32, bool)> = axes
        .iter()
        .map(|(dir, color, label, yaw, pitch, positive)| {
            // Rotate by yaw
            let r = Vec3::new(
                dir.x * cos_yaw + dir.z * sin_yaw,
                dir.y,
                -dir.x * sin_yaw + dir.z * cos_yaw,
            );
            // Rotate by pitch
            let v = Vec3::new(
                r.x,
                r.y * cos_pitch + r.z * sin_pitch,
                -r.y * sin_pitch + r.z * cos_pitch,
            );
            let offset = egui::Vec2::new(v.x * axis_length, -v.y * axis_length);
            (v.z, offset, *color, *label, *yaw, *pitch, *positive)
        })
        .collect();

    // Sort back-to-front
    projected.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let gizmo_rect = egui::Rect::from_center_size(center, egui::vec2(AXIS_GIZMO_SIZE, AXIS_GIZMO_SIZE));

    egui::Area::new(egui::Id::new("viewport_axis_gizmo"))
        .fixed_pos(gizmo_rect.min)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let resp = ui.interact(
                gizmo_rect,
                egui::Id::new("axis_gizmo_interact"),
                egui::Sense::click_and_drag(),
            );

            if resp.drag_started() {
                nav.orbit_dragging.store(true, Ordering::Relaxed);
            }
            if resp.drag_stopped() {
                nav.orbit_dragging.store(false, Ordering::Relaxed);
            }

            if resp.dragged() {
                let d = resp.drag_delta();
                nav.orbit_delta_x.fetch_add((d.x * 1000.0) as i32, Ordering::Relaxed);
                nav.orbit_delta_y.fetch_add((d.y * 1000.0) as i32, Ordering::Relaxed);
            }

            if resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let local_pos = pos - center;
                    
                    // Find closest axis endpoint
                    let mut closest_axis = None;
                    let mut min_dist = 15.0; // Click radius

                    for &(_depth, offset, _color, _label, yaw, pitch, _is_positive) in &projected {
                        let dist = (local_pos - offset).length();
                        if dist < min_dist {
                            min_dist = dist;
                            closest_axis = Some((yaw, pitch));
                        }
                    }

                    if let Some((yaw, pitch)) = closest_axis {
                        cmds.push(move |w: &mut World| {
                            if let Some(mut settings) = w.get_resource_mut::<ViewportSettings>() {
                                settings.pending_view_angle = Some(ViewAngleCommand { yaw, pitch });
                            }
                        });
                    }
                }
            }

            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            let painter = ui.painter();

            // Draw background sphere highlight
            let is_active = nav.orbit_dragging.load(Ordering::Relaxed);
            if resp.hovered() || is_active {
                let theme_mgr = world.get_resource::<renzora_theme::ThemeManager>();
                let theme = theme_mgr.map(|tm| &tm.active_theme);
                
                let bg_color = if is_active {
                    theme.map(|t| t.semantic.accent.to_color32().gamma_multiply(0.2))
                        .unwrap_or(egui::Color32::from_rgba_unmultiplied(100, 100, 255, 40))
                } else {
                    theme.map(|t| t.widgets.hovered_bg.to_color32().gamma_multiply(0.3))
                        .unwrap_or(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20))
                };
                painter.circle_filled(center, AXIS_GIZMO_SIZE / 2.0, bg_color);

                if is_active {
                    let stroke_color = theme.map(|t| t.semantic.accent.to_color32())
                        .unwrap_or(egui::Color32::from_rgba_unmultiplied(100, 100, 255, 180));
                    painter.circle_stroke(center, AXIS_GIZMO_SIZE / 2.0, egui::Stroke::new(1.0, stroke_color));
                }
            }

            for &(_depth, offset, color, label, _yaw, _pitch, is_positive) in &projected {
                let end = center + offset;

                // Fade axes pointing away
                let alpha = if _depth < -0.1 { 100 } else { 255 };
                let c = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);

                let line_width = if is_positive {
                    if _depth < -0.1 { 2.0 } else { 3.0 }
                } else {
                    if _depth < -0.1 { 1.0 } else { 1.5 }
                };

                if is_positive {
                    painter.line_segment([center, end], egui::Stroke::new(line_width, c));
                }

                let cap_size = if is_positive { 9.0 } else { 6.0 };
                if is_positive {
                    painter.circle_filled(end, cap_size, c);
                    painter.text(
                        end,
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(10.0),
                        egui::Color32::WHITE,
                    );
                } else {
                    painter.circle_stroke(end, cap_size, egui::Stroke::new(line_width, c));
                }
            }
            
            // Center dot
            painter.circle_filled(center, 3.0, egui::Color32::from_rgb(180, 180, 180));
        });
}

/// Toggles the Editor Camera's `is_active` based on whether any panel that
/// displays its output is currently mounted in the dock tree. Today that's
/// the Viewport panel and the UI Canvas panel (the canvas draws game UI on
/// top of the editor scene render). If neither is mounted there's no point
/// running the editor scene render pass.
fn sync_viewport_camera_activation(
    docking: Option<Res<DockingState>>,
    mut cameras: Query<&mut Camera, With<EditorCamera>>,
) {
    let mounted = docking.map_or(true, |d| {
        d.tree.contains_panel("viewport") || d.tree.contains_panel("ui_canvas")
    });
    for mut camera in cameras.iter_mut() {
        if camera.is_active != mounted {
            camera.is_active = mounted;
        }
    }
}
