//! Editor-side bevy_ui render target.
//!
//! Spawns a dedicated `Camera2d` that renders bevy_ui to an offscreen
//! image. The canvas tab displays this image directly — what the user
//! sees IS the bevy_ui render, not an egui simulation. The active
//! `UiCanvas`'s `UiTargetCamera` is pointed at this camera in editor
//! edit mode so layout, clipping, theme, and visibility all behave
//! identically to runtime.
//!
//! Architecture mirrors `UiCanvasPreview` (the 3D scene preview) but is
//! UI-only: no Camera3d, no skybox sync, just a 2D camera whose only job
//! is to render the active canvas to a texture.

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy_egui::{EguiTextureHandle, EguiUserTextures};

use crate::components::UiCanvas;

/// Resolution of the UI editor render-target image. Matches the design
/// resolution 1:1 — text and shapes raster at native size.
pub const UI_RENDER_WIDTH: u32 = 1280;
pub const UI_RENDER_HEIGHT: u32 = 720;

/// Resource holding the editor's UI render target — image, egui texture
/// id, and the camera entity that renders bevy_ui to it.
#[derive(Resource)]
pub struct UiCanvasRender {
    pub image_handle: Handle<Image>,
    pub texture_id: Option<bevy_egui::egui::TextureId>,
    pub camera_entity: Entity,
}

/// Marker component for the editor's dedicated UI render camera.
#[derive(Component)]
pub struct UiEditorRenderCamera;

/// Startup system — creates the render-target image, registers it with
/// egui, and spawns the dedicated 2D camera that renders bevy_ui to it.
pub fn setup_ui_canvas_render(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut user_textures: ResMut<EguiUserTextures>,
) {
    let size = Extent3d {
        width: UI_RENDER_WIDTH,
        height: UI_RENDER_HEIGHT,
        depth_or_array_layers: 1,
    };

    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0u8; 4],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    image.sampler = ImageSampler::linear();

    let image_handle = images.add(image);
    user_textures.add_image(EguiTextureHandle::Strong(image_handle.clone()));
    let texture_id = user_textures.image_id(image_handle.id());

    // Dedicated 2D camera. UI rendering hangs off any active camera with a
    // `Camera` + render target — Camera2d is the lightest setup that
    // satisfies bevy_ui's render graph requirements.
    //
    // Spawned `is_active = false`. `sync_viewport_camera_activation` in
    // `renzora_viewport` flips it on only while the Viewport panel is
    // mounted and its view is set to `ViewportView::Ui`. That's the
    // single source of truth for which of the three viewport-hosted
    // editor cameras (3D / 2D / UI) is rendering at any moment, so the
    // GPU doesn't pay for an unused canvas pass on every frame. The
    // `order = -10` keeps it draw-order-before the editor camera and
    // play-mode camera if they ever happen to be on simultaneously.
    let camera_entity = commands
        .spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgba(0.08, 0.08, 0.10, 0.0)),
                order: -10,
                is_active: false,
                ..default()
            },
            RenderTarget::Image(image_handle.clone().into()),
            UiEditorRenderCamera,
            renzora::IsolatedCamera,
            renzora::HideInHierarchy,
            renzora::EditorLocked,
            Name::new("UI Editor Render Camera"),
        ))
        .id();

    commands.insert_resource(UiCanvasRender {
        image_handle,
        texture_id,
        camera_entity,
    });
}

/// Sync system — keeps every `UiCanvas` pointed at the editor's UI render
/// camera while the editor is in edit mode. In play mode, the existing
/// `sync_ui_canvas_target_camera` system takes over and points canvases
/// at the active game camera instead. In standalone runtime, this system
/// doesn't exist (it's editor-only) and canvases use Bevy's default
/// camera-finding.
pub fn sync_canvases_to_editor_camera(
    mut commands: Commands,
    play_mode: Option<Res<renzora::PlayModeState>>,
    render: Option<Res<UiCanvasRender>>,
    canvases: Query<(Entity, Option<&bevy::ui::UiTargetCamera>), With<UiCanvas>>,
) {
    let in_play = play_mode.is_some_and(|p| p.is_in_play_mode());
    if in_play {
        // Play-mode handler owns target-camera assignment.
        return;
    }
    let Some(render) = render else {
        return;
    };
    let target = render.camera_entity;
    for (entity, existing) in &canvases {
        let needs_update = match existing {
            Some(tc) => tc.entity() != target,
            None => true,
        };
        if needs_update {
            commands
                .entity(entity)
                .insert(bevy::ui::UiTargetCamera(target));
        }
    }
}
