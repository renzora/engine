//! Renzora Viewport — editor panel that displays the 3D game world.
//!
//! Creates an offscreen render target, wires it to the runtime camera,
//! and displays the result as an egui image inside the docking panel system.

pub mod camera_preview;
pub mod effect_routing;
pub mod header;
pub mod play_mode;
pub mod render_systems;
pub mod settings;
pub mod toolbar;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use egui_phosphor::regular;
use renzora_editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_runtime::ViewportRenderTarget;
use renzora_theme::ThemeManager;

pub use camera_preview::CameraPreviewState;
pub use settings::{
    CameraOrbitSnapshot, CameraSettingsState, CollisionGizmoVisibility, ProjectionMode,
    RenderToggles, SnapSettings, ViewAngleCommand, ViewportSettings, VisualizationMode,
};

const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;

/// Plugin that creates the render-to-texture viewport and registers the panel.
pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WireframePlugin::default())
            .insert_resource(WireframeConfig {
                global: false,
                default_color: bevy::color::Color::WHITE,
            })
            .init_resource::<ViewportState>()
            .init_resource::<ViewportResizeRequest>()
            .init_resource::<ViewportSettings>()
            .init_resource::<CameraOrbitSnapshot>()
            .init_resource::<renzora_runtime::PlayModeState>()
            .init_resource::<render_systems::OriginalMaterialStates>()
            .init_resource::<render_systems::LastRenderState>()
            .add_systems(PostStartup, (setup_viewport, camera_preview::setup_camera_preview))
            .init_resource::<renzora_core::EffectRouting>()
            .add_systems(Update, (
                handle_viewport_resize,
                render_systems::update_render_toggles,
                render_systems::update_shadow_settings,
                camera_preview::update_camera_preview,
                play_mode::handle_play_mode_transitions,
                effect_routing::update_effect_routing,
            ).run_if(in_state(renzora_editor::SplashState::Editor)));

        app.register_panel(ViewportPanel);
        app.register_panel(CameraPreviewPanel);
    }
}

/// Tracks the render target image and current resolution.
#[derive(Resource)]
pub struct ViewportState {
    pub image_handle: Option<Handle<Image>>,
    pub current_size: UVec2,
    /// Whether the mouse cursor is currently over the viewport.
    pub hovered: bool,
    /// Screen-space position of the viewport panel (top-left corner).
    pub screen_position: Vec2,
    /// Screen-space size of the viewport panel.
    pub screen_size: Vec2,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            image_handle: None,
            current_size: UVec2::new(DEFAULT_WIDTH, DEFAULT_HEIGHT),
            hovered: false,
            screen_position: Vec2::ZERO,
            screen_size: Vec2::new(DEFAULT_WIDTH as f32, DEFAULT_HEIGHT as f32),
        }
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
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;

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
            let is_hovered = ui.rect_contains_pointer(rect);
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

        // Overlay: vertical tool bar (gizmo modes, terrain tools, play button)
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

        // Overlay: axis orientation gizmo
        let show_axis = world
            .get_resource::<ViewportSettings>()
            .map_or(true, |s| s.show_axis_gizmo);
        let play_mode = world.get_resource::<renzora_runtime::PlayModeState>();
        let in_play = play_mode.map_or(false, |p| p.is_in_play_mode());
        if show_axis && !in_play {
            if let Some(orbit) = world.get_resource::<CameraOrbitSnapshot>() {
                render_axis_gizmo(ui, orbit, rect);
            }
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
            world.get::<renzora_runtime::DefaultCamera>(e).is_some()
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

// ── On-screen console log overlay (play mode) ──────────────────────────────

const LOG_MAX_VISIBLE: usize = 12;
const LOG_DISPLAY_DURATION: f64 = 5.0;
const LOG_FADE_DURATION: f64 = 1.0;

fn render_viewport_logs(ui: &mut egui::Ui, world: &World, viewport_rect: egui::Rect) {
    use renzora_console::state::ConsoleState;

    // Only show during play mode
    let Some(play_mode) = world.get_resource::<renzora_runtime::PlayModeState>() else { return };
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

const AXIS_GIZMO_SIZE: f32 = 100.0;
const AXIS_GIZMO_MARGIN: f32 = 24.0; // extra margin to clear the resolution text

fn render_axis_gizmo(
    ui: &mut egui::Ui,
    orbit: &CameraOrbitSnapshot,
    viewport_rect: egui::Rect,
) {
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

    let painter = ui.painter();

    for &(depth, offset, color, label, _yaw, _pitch, is_positive) in &projected {
        let end = egui::Pos2::new(center.x + offset.x, center.y + offset.y);

        // Fade axes pointing away
        let alpha = if depth < -0.1 { 100 } else { 255 };
        let c = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);

        let line_width = if is_positive {
            if depth < -0.1 { 2.0 } else { 3.0 }
        } else {
            if depth < -0.1 { 1.0 } else { 1.5 }
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
                egui::FontId::proportional(11.0),
                egui::Color32::WHITE,
            );
        } else {
            painter.circle_stroke(end, cap_size, egui::Stroke::new(2.0, c));
        }
    }

    // Center dot
    painter.circle_filled(center, 3.0, egui::Color32::from_rgb(180, 180, 180));
}
