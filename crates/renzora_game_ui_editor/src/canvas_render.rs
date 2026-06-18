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

use renzora_game_ui::components::UiCanvas;

/// *Initial* resolution of the UI editor render-target image (the default
/// canvas reference). The target is resized to follow the active canvas's
/// reference resolution by [`sync_render_target_to_reference`], so the render
/// is always 1:1 with design space — one design pixel = one texture pixel.
pub const UI_RENDER_WIDTH: u32 = 1280;
pub const UI_RENDER_HEIGHT: u32 = 720;

/// Resource holding the editor's UI render target — the offscreen image and
/// the camera entity that renders bevy_ui to it. Consumers display the render
/// via `image_handle` (a bevy_ui `ImageNode`).
#[derive(Resource)]
pub struct UiCanvasRender {
    pub image_handle: Handle<Image>,
    pub camera_entity: Entity,
    /// Current pixel size of `image_handle`, tracked so the resize system only
    /// reallocates the texture when the reference resolution actually changes.
    pub current_size: UVec2,
}

/// Marker component for the editor's dedicated UI render camera.
#[derive(Component)]
pub struct UiEditorRenderCamera;

/// Startup system — creates the render-target image and spawns the dedicated
/// 2D camera that renders bevy_ui to it.
pub fn setup_ui_canvas_render(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
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
        camera_entity,
        current_size: UVec2::new(UI_RENDER_WIDTH, UI_RENDER_HEIGHT),
    });
}

/// Keep the offscreen render target sized to the **active canvas's reference
/// resolution** so the canvas renders 1:1 — one design pixel maps to one
/// texture pixel — with the global `UiScale` left at its default `1.0`.
///
/// This replaces the old `sync_ui_scale_to_canvas_reference`, which fit a
/// non-default reference (say 1920×1080) into a *fixed* 1280×720 texture by
/// writing the global `UiScale`. Because the editor shell is itself native
/// bevy_ui under the same global `UiScale`, that scaled the entire editor
/// chrome (ribbon, panels, popups) — see issue #55. Sizing the target to the
/// reference instead keeps design-space coordinates lined up everywhere
/// (geometry, overlay handles, drag math all already work in reference space ×
/// zoom) and never touches the chrome.
///
/// The texture is reused via [`Image::resize`]; it only reallocates when the
/// reference changes, which is a rare design-time edit. The display frame
/// stretches this texture to `reference × zoom`, so any size renders cleanly.
pub(crate) fn sync_render_target_to_reference(
    state: Res<crate::NativeCanvasState>,
    render: Option<ResMut<UiCanvasRender>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut render) = render else {
        return;
    };
    // Clamp to the same bounds the `Ref Width`/`Ref Height` inspector fields
    // allow (1..=7680 × 1..=4320), guarding against a degenerate texture size.
    let w = (state.canvas_width.round() as i64).clamp(1, 7680) as u32;
    let h = (state.canvas_height.round() as i64).clamp(1, 4320) as u32;
    let requested = UVec2::new(w, h);
    if render.current_size == requested {
        return;
    }
    if let Some(image) = images.get_mut(&render.image_handle) {
        image.resize(Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        });
        render.current_size = requested;
    }
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
