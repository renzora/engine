//! Per-camera render-resolution downscale for the runtime.
//!
//! Honors [`renzora::core::CameraRenderResolution`] on the active game camera:
//! when it asks for Half / Quarter, the camera is redirected to render into an
//! **offscreen image sized at that fraction of the window**, and a second pass
//! upscales that image to fill the OS window. Full resolution (or no component)
//! leaves the camera rendering straight to the window — zero overhead.
//!
//! This is the runtime counterpart to the editor's viewport render-scale (which
//! resizes the editor's own offscreen viewport image). It is only added for
//! shipped builds (`!is_editor`).
//!
//! Composition with [`super::viewport_stretch`]: that plugin redirects the game
//! `Camera2d` to its own offscreen image in `Viewport` stretch mode. When that
//! has happened the camera no longer targets the window, so this plugin leaves
//! it alone — render-scale and viewport-stretch don't stack (yet). In the
//! default `Disabled` stretch mode (camera → window) render-scale applies to
//! both 2D and 3D cameras.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::image::{Image, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::window::{PrimaryWindow, WindowResized};

use renzora::core::CameraRenderResolution;

/// Render layer for the upscale blit sprite + camera. Distinct from
/// `viewport_stretch`'s layer (31) so the two present passes never collide.
const RS_BLIT_LAYER: usize = 30;

/// Marker on the sprite that displays the downscaled offscreen image.
#[derive(Component)]
struct RsBlitSprite;

/// Marker on the camera that upscales [`RsBlitSprite`] to the window.
#[derive(Component)]
struct RsBlitCamera;

/// Tracks the offscreen target + blit entities + which game camera we redirected.
#[derive(Resource, Default)]
struct RenderScaleState {
    image: Option<Handle<Image>>,
    size: UVec2,
    sprite: Option<Entity>,
    blit_cam: Option<Entity>,
    /// The game camera currently redirected into `image` (so we can restore it).
    cam: Option<Entity>,
}

pub struct RenderScalePlugin;

impl Plugin for RenderScalePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderScaleState>()
            .add_systems(Update, apply_render_scale);
    }
}

fn on_game_layer(layers: Option<&RenderLayers>) -> bool {
    layers.is_none_or(|l| l.intersects(&RenderLayers::default()))
}

fn make_image(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    let ext = Extent3d {
        width: size.x,
        height: size.y,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new_fill(
        ext,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    // Linear sampling: render-scale is a smooth-upscale perf knob, not a
    // pixel-art workflow (that's `viewport_stretch`, which uses nearest).
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    images.add(image)
}

/// Despawn the blit pass and forget the offscreen target. Does *not* restore the
/// game camera's render target — the caller handles that where it has the entity.
fn teardown_blit(commands: &mut Commands, state: &mut RenderScaleState) {
    if let Some(s) = state.sprite.take() {
        commands.entity(s).despawn();
    }
    if let Some(c) = state.blit_cam.take() {
        commands.entity(c).despawn();
    }
    state.image = None;
    state.size = UVec2::ZERO;
    state.cam = None;
}

#[allow(clippy::type_complexity)]
fn apply_render_scale(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut state: ResMut<RenderScaleState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<
        (
            Entity,
            &Camera,
            Option<&CameraRenderResolution>,
            Option<&RenderLayers>,
            &RenderTarget,
        ),
        (
            Without<RsBlitCamera>,
            Or<(With<Camera3d>, With<Camera2d>)>,
        ),
    >,
    mut blit: Query<(&mut Sprite, &mut Transform), With<RsBlitSprite>>,
    mut resize_events: MessageReader<WindowResized>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let win = Vec2::new(window.width(), window.height());
    if win.x < 1.0 || win.y < 1.0 {
        return;
    }
    let resized = !resize_events.is_empty();
    resize_events.clear();

    // Pick the active game camera we should manage: it must render the default
    // layer and currently target the window (or already be the one we redirected
    // — tracked via `state.cam`). Cameras pointed at some *other* image (editor
    // target, viewport-stretch) are intentionally left alone.
    let mut chosen: Option<(Entity, f32)> = None;
    for (e, cam, res, layers, rt) in cams.iter() {
        if !cam.is_active || !on_game_layer(layers) {
            continue;
        }
        let ours = state.cam == Some(e);
        let on_window = matches!(rt, RenderTarget::Window(_));
        if !ours && !on_window {
            continue;
        }
        chosen = Some((e, res.map(|r| r.0.scale()).unwrap_or(1.0)));
        break;
    }
    let chosen_e = chosen.map(|(e, _)| e);

    // If we previously redirected a different (or now-inactive) camera, hand it
    // back to the window before doing anything else.
    if let Some(prev) = state.cam {
        if Some(prev) != chosen_e && cams.get(prev).is_ok() {
            commands.entity(prev).insert(RenderTarget::default());
        }
    }

    let Some((cam, scale)) = chosen else {
        teardown_blit(&mut commands, &mut state);
        return;
    };

    // Full resolution: make sure the camera renders straight to the window and
    // nothing lingers.
    if scale >= 1.0 {
        if state.cam == Some(cam) {
            commands.entity(cam).insert(RenderTarget::default());
        }
        teardown_blit(&mut commands, &mut state);
        return;
    }

    let desired = UVec2::new(
        ((win.x * scale).round() as u32).max(1),
        ((win.y * scale).round() as u32).max(1),
    );

    // Ensure the offscreen image exists at the right size.
    if state.image.is_none() {
        let handle = make_image(&mut images, desired);
        state.image = Some(handle);
        state.size = desired;
    } else if state.size != desired {
        if let Some(mut img) = state.image.as_ref().and_then(|h| images.get_mut(h)) {
            img.resize(Extent3d {
                width: desired.x,
                height: desired.y,
                depth_or_array_layers: 1,
            });
        }
        state.size = desired;
    }
    let image = state.image.clone().expect("just ensured");

    // Redirect the camera into the offscreen image if we haven't already.
    if state.cam != Some(cam) {
        commands
            .entity(cam)
            .insert(RenderTarget::Image(image.clone().into()));
        state.cam = Some(cam);
    }

    // Ensure the blit pass exists.
    if state.sprite.is_none() {
        let sprite = commands
            .spawn((
                Sprite {
                    image: image.clone(),
                    custom_size: Some(win),
                    ..default()
                },
                Transform::from_xyz(win.x * 0.5, -win.y * 0.5, 0.0),
                RenderLayers::layer(RS_BLIT_LAYER),
                RsBlitSprite,
                Name::new("Render Scale Blit Sprite"),
            ))
            .id();
        let blit_cam = commands
            .spawn((
                Camera2d,
                Camera {
                    // After the game camera (0), before viewport_stretch's blit (999).
                    order: 998,
                    clear_color: ClearColorConfig::Custom(Color::BLACK),
                    ..default()
                },
                RenderLayers::layer(RS_BLIT_LAYER),
                RsBlitCamera,
                Name::new("Render Scale Blit Camera"),
            ))
            .id();
        state.sprite = Some(sprite);
        state.blit_cam = Some(blit_cam);
    } else if resized {
        // Keep the upscaled sprite filling the window. The blit Camera2d shares
        // the engine-wide viewport_origin (0, 1) convention (set by the Camera2d
        // observer in renzora_engine), so the visible region is (0, -win) to
        // (win, 0); centre the sprite within it.
        if let Ok((mut sprite, mut transform)) = blit.single_mut() {
            sprite.custom_size = Some(win);
            transform.translation.x = win.x * 0.5;
            transform.translation.y = -win.y * 0.5;
        }
    }
}
