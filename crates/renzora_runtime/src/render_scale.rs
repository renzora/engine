//! Render-resolution scale for the runtime.
//!
//! The active game camera is redirected to render into an **offscreen image
//! sized `final_scale ×` the LOGICAL window**, and a window-facing blit camera
//! upscales that image to fill the OS window with the UI composited on top at
//! native resolution. `final_scale` is the project's `[rendering] render_scale`
//! (a fraction of the design/logical window) times the per-camera
//! [`renzora::core::CameraRenderResolution`] (Full/Half/Quarter).
//!
//! **Sizing off the LOGICAL window is deliberate — it makes the DPI fix free.**
//! At `render_scale = 1.0` the offscreen is the *design* resolution (e.g.
//! 1280×720). On a high-DPI display the physical framebuffer is larger (1920×1008
//! at 150%), so rendering at the design resolution is ~2× fewer pixels — undoing
//! HiDPI pixel-bloat with no per-machine tuning. On a 1.0-DPI display the design
//! resolution *equals* the physical one, so the "offscreen ≥ physical on both
//! axes" gate below renders straight to the window at zero overhead. Raising the
//! slider to/past the display's DPI factor likewise saturates to native — we
//! never super-sample on a weak GPU.
//!
//! Runtime-only (added under `!is_editor`); the editor uses its own per-slot
//! viewport render-scale and never reads `[rendering] render_scale`.
//!
//! Composition with [`super::viewport_stretch`]: that plugin redirects the game
//! `Camera2d` and blits at order 999 with a full-window black clear. The two must
//! never be co-active (999's clear would wipe our order-998 output, and in a 3D
//! scene they'd grab different cameras). So this plugin **stands fully down**
//! whenever `[viewport] stretch_mode` is not `Disabled` — hands its camera back to
//! the window and tears its blit down — leaving viewport-stretch as the sole
//! present pass.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::image::{Image, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::ui::IsDefaultUiCamera;
use bevy::window::{PrimaryWindow, WindowResized};

use renzora::core::{CameraRenderResolution, StretchMode};

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
    project: Option<Res<renzora::CurrentProject>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    // The requested 3D render scale (a fraction of the LOGICAL window) plus whether
    // viewport-stretch is inactive. Read once from the project config; an absent
    // config → native (1.0) with stretch inactive.
    let (mut render_scale, stretch_disabled) = project
        .as_ref()
        .map(|p| {
            (
                p.config.rendering.render_scale,
                p.config.viewport.stretch_mode == StretchMode::Disabled,
            )
        })
        .unwrap_or((1.0, true));
    // A malformed value (0 / negative / NaN from a hand-edited toml) would size a
    // degenerate offscreen; fall back to native, then clamp to a sane band.
    if !render_scale.is_finite() || render_scale <= 0.0 {
        render_scale = 1.0;
    }
    render_scale = render_scale.clamp(0.1, 2.0);

    // Stand fully down while `viewport_stretch` owns the present: it redirects the
    // game `Camera2d` and blits at order 999 with a full-window BLACK clear that
    // would wipe our order-998 output, and in a 3D scene it grabs a *different*
    // camera than we prefer (Camera3d) — so both would blit. Hand our camera back
    // to the window and tear our blit down so exactly one present pass is live.
    if !stretch_disabled {
        if let Some(prev) = state.cam {
            if cams.get(prev).is_ok() {
                commands.entity(prev).insert(RenderTarget::default());
            }
        }
        teardown_blit(&mut commands, &mut state);
        return;
    }

    let win = Vec2::new(window.width(), window.height());
    let phys = window.physical_size();
    if win.x < 1.0 || win.y < 1.0 || phys.x < 1 || phys.y < 1 {
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

    let Some((cam, cam_scale)) = chosen else {
        teardown_blit(&mut commands, &mut state);
        return;
    };

    // Effective scale = project `render_scale` × the per-camera
    // `CameraRenderResolution` (Full/Half/Quarter). Sized off the LOGICAL window,
    // so at `render_scale = 1.0` the offscreen is the *design* resolution
    // (e.g. 1280×720) — which on a high-DPI display is fewer pixels than the
    // physical framebuffer (1920×1008 at 150%). That's the DPI pixel-bloat fix,
    // automatic and free.
    let final_scale = (render_scale * cam_scale).clamp(0.05, 2.0);
    let desired = UVec2::new(
        ((win.x * final_scale).round() as u32).max(1),
        ((win.y * final_scale).round() as u32).max(1),
    );

    // Render straight to the window whenever the offscreen would be at least the
    // physical framebuffer on both axes — nothing is saved by an equal-or-larger
    // intermediate, and we deliberately never super-sample on a weak GPU. This is
    // the no-op for a 1.0-DPI display at `render_scale = 1.0` (logical == physical),
    // and the native saturation when the slider is raised to/past the DPI factor.
    if desired.x >= phys.x && desired.y >= phys.y {
        if state.cam == Some(cam) {
            commands.entity(cam).insert(RenderTarget::default());
        }
        teardown_blit(&mut commands, &mut state);
        return;
    }

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
                // Make the window-facing blit the game's default UI camera while
                // it's live, so the FPS overlay / game canvases render on the
                // WINDOW at native res instead of following the 3D camera into the
                // downscaled offscreen (they'd blur). Torn down at native/no-op, and
                // viewport-stretch can't be co-active, so the "exactly one
                // IsDefaultUiCamera" invariant holds.
                IsDefaultUiCamera,
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
