//! Renzora Viewport — editor panel that displays the 3D game world.
//!
//! Creates an offscreen render target, wires it to the runtime camera,
//! and displays the result as an egui image inside the docking panel system.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_egui::egui;
use bevy_egui::{EguiTextureHandle, EguiUserTextures};
use egui_phosphor::regular;
use renzora_editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_runtime::ViewportRenderTarget;
use renzora_theme::ThemeManager;

const DEFAULT_WIDTH: u32 = 1280;
const DEFAULT_HEIGHT: u32 = 720;

/// Plugin that creates the render-to-texture viewport and registers the panel.
pub struct ViewportPlugin;

impl Plugin for ViewportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewportState>()
            .init_resource::<ViewportResizeRequest>()
            .add_systems(PostStartup, setup_viewport)
            .add_systems(Update, handle_viewport_resize);

        app.register_panel(ViewportPanel);
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
    }

    fn closable(&self) -> bool {
        false
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}
