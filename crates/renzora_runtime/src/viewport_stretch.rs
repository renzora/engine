//! Godot-style viewport stretch for the runtime.
//!
//! When `project.config.viewport.stretch_mode == StretchMode::Viewport`,
//! the game camera renders to an **offscreen image** at
//! `viewport.width × viewport.height`, and a second pass blits that
//! image to the OS window with nearest-neighbour sampling. This is the
//! pixel-art workflow: a 320×180 game looks crisp on a 1920×1080
//! monitor because each game-pixel becomes a 6×6 block of screen
//! pixels with no smoothing.
//!
//! Aspect-mode `Keep` letterboxes (or pillarboxes) when the window
//! aspect doesn't match the viewport's. Other modes are placeholders
//! for now — `Expand` / `KeepWidth` / `KeepHeight` round-trip through
//! the config but use the same uniform-fit math as `Keep` until we
//! wire them up.
//!
//! Disabled mode is the no-op default — render goes straight to the
//! window, identical behaviour to before this module existed.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::image::{Image, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::window::{PrimaryWindow, WindowResized};

use renzora::core::{AspectMode, StretchMode};

/// Render layer that holds the blit sprite + blit camera. Separated
/// from the default game world (layer 0) so the game camera doesn't
/// re-render the blit sprite on top of the gameplay it already drew.
const BLIT_RENDER_LAYER: usize = 31;

/// Resource: the offscreen image the game camera renders to. Only
/// inserted when stretch mode is `Viewport`. Other systems read this
/// to redirect the game `Camera2d`'s `RenderTarget`.
#[derive(Resource, Clone)]
pub struct ViewportStretchImage {
    pub image: Handle<Image>,
    pub size: UVec2,
}

/// Marker on the offscreen-image-displaying sprite — the thing the
/// blit camera renders. Resize logic finds it via this marker.
#[derive(Component)]
struct BlitSprite;

/// Marker on the camera that draws `BlitSprite` to the OS window.
#[derive(Component)]
struct BlitCamera;

pub struct ViewportStretchPlugin;

impl Plugin for ViewportStretchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_stretch.before(spawn_blit_pass))
            .add_systems(Startup, spawn_blit_pass)
            .add_systems(
                Update,
                (redirect_game_cameras_to_offscreen, update_blit_layout),
            )
            .add_observer(on_camera_2d_added_redirect);
    }
}

/// Startup: if the project asks for viewport stretching, create the
/// offscreen image and stash it in `ViewportStretchImage`. Disabled
/// mode is a no-op — the resource never gets inserted, so every
/// downstream system can early-out by checking for its absence.
fn setup_stretch(
    project: Option<Res<renzora::CurrentProject>>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(project) = project else {
        warn!("[viewport_stretch] No project — stretch disabled");
        return;
    };
    let cfg = &project.config.viewport;
    if cfg.stretch_mode == StretchMode::Disabled {
        return;
    }
    if cfg.width == 0 || cfg.height == 0 {
        warn!("[viewport_stretch] viewport.width/height is 0 — falling back to disabled");
        return;
    }

    let size = Extent3d {
        width: cfg.width,
        height: cfg.height,
        depth_or_array_layers: 1,
    };

    // Manually-built render target image. Needs RENDER_ATTACHMENT (the
    // game camera renders into it) plus TEXTURE_BINDING (the blit
    // camera samples it). COPY_DST mirrors what `Image::new_target_texture`
    // would set — keeps options open for tonemapping / postprocess
    // passes that might want to copy into the texture later.
    let mut image = Image::new_fill(
        size,
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    // Nearest-neighbour sampler: the whole point of viewport stretch
    // for pixel art. Linear here would smear every pixel block.
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());

    let handle = images.add(image);
    commands.insert_resource(ViewportStretchImage {
        image: handle,
        size: UVec2::new(cfg.width, cfg.height),
    });

    info!(
        "[viewport_stretch] stretch={:?} aspect={:?} viewport={}x{} (offscreen image created)",
        cfg.stretch_mode, cfg.aspect_mode, cfg.width, cfg.height
    );
}

/// Startup: spawn the second render pass that draws the offscreen
/// image to the OS window. A dedicated `Camera2d` on a high render
/// layer views a sprite that displays the offscreen image. Only runs
/// when `ViewportStretchImage` exists (i.e. stretch mode is Viewport).
fn spawn_blit_pass(stretch: Option<Res<ViewportStretchImage>>, mut commands: Commands) {
    let Some(stretch) = stretch else {
        return;
    };

    // Sprite displaying the offscreen image. `custom_size` gets
    // updated each frame by `update_blit_layout` to letterbox/
    // pillarbox the window. Centred at world (0, 0) so the blit
    // camera can sit at origin too.
    commands.spawn((
        Sprite {
            image: stretch.image.clone(),
            custom_size: Some(stretch.size.as_vec2()),
            ..default()
        },
        Transform::default(),
        RenderLayers::layer(BLIT_RENDER_LAYER),
        BlitSprite,
        Name::new("Viewport Blit Sprite"),
    ));

    // Blit camera: renders to window, only sees the blit layer.
    // Order=999 puts it after every reasonable game camera. Black
    // clear colour fills the letterbox bars.
    commands.spawn((
        Camera2d,
        Camera {
            order: 999,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Transform::default(),
        RenderLayers::layer(BLIT_RENDER_LAYER),
        BlitCamera,
        Name::new("Viewport Blit Camera"),
    ));
}

/// Run each frame: any `Camera2d` that's still pointed at the primary
/// window when stretch mode is on gets redirected to the offscreen
/// image. The user-authored `Camera 2D` is loaded from the scene with
/// `RenderTarget::Window(Primary)`, so we can't catch it at scene-
/// deserialization time without racing the load. A polling system is
/// the simplest correct fix; it's a no-op once everyone is pointed at
/// the right target. Exclude the blit camera and any camera with
/// RenderLayers that *don't* include the default game layer (so we
/// don't accidentally retarget UI cameras).
fn redirect_game_cameras_to_offscreen(
    stretch: Option<Res<ViewportStretchImage>>,
    mut commands: Commands,
    cameras: Query<
        (Entity, Option<&RenderLayers>, &RenderTarget),
        (With<Camera2d>, Without<BlitCamera>),
    >,
) {
    let Some(stretch) = stretch else {
        return;
    };

    for (entity, layers, current_target) in cameras.iter() {
        // Only redirect cameras that render the default game layer (0).
        // RenderLayers::default() is layer 0; treat missing component
        // the same way.
        let on_game_layer = layers.map_or(true, |l| l.intersects(&RenderLayers::default()));
        if !on_game_layer {
            continue;
        }
        if matches!(current_target, RenderTarget::Image(_)) {
            continue;
        }
        // Bevy's default Camera2d carries `Msaa::Sample4`. With nearest-
        // upscaling the AA-blended edge pixels in the offscreen image
        // get blown up into full-size dark borders around every sprite
        // — exactly what kills the look for pixel-art games. Force Off
        // on every camera that renders into the offscreen image.
        commands
            .entity(entity)
            .insert((RenderTarget::Image(stretch.image.clone().into()), Msaa::Off));
    }
}

/// Observer: catches Camera2d entities the moment they're inserted
/// (preset spawn, scene load) so they start pointing at the offscreen
/// image *before* the first frame renders. The polling system above
/// covers the case where this observer can't redirect (e.g. the
/// stretch resource doesn't exist yet at insert time, then the user
/// hot-reloads project config).
fn on_camera_2d_added_redirect(
    trigger: On<Insert, Camera2d>,
    stretch: Option<Res<ViewportStretchImage>>,
    cameras: Query<Option<&RenderLayers>, With<Camera2d>>,
    blit_cameras: Query<(), With<BlitCamera>>,
    mut commands: Commands,
) {
    let Some(stretch) = stretch else {
        return;
    };
    let entity = trigger.entity;
    if blit_cameras.get(entity).is_ok() {
        return;
    }
    let Ok(layers) = cameras.get(entity) else {
        return;
    };
    let on_game_layer = layers.map_or(true, |l| l.intersects(&RenderLayers::default()));
    if !on_game_layer {
        return;
    }
    commands
        .entity(entity)
        .insert((RenderTarget::Image(stretch.image.clone().into()), Msaa::Off));
}

/// Resize the blit sprite each frame to fit the current window with
/// the configured aspect mode. Letterbox/pillarbox bars are whatever's
/// outside the sprite — the blit camera's clear colour fills them.
///
/// The blit camera shares the engine's Godot-style `viewport_origin`
/// (0, 1) (set globally by the Camera2d observer in `renzora_engine`),
/// so its visible world region is `(0, -win_h)` to `(win_w, 0)`. We
/// position the blit sprite at the *centre* of that region — anchor
/// stays Center — so the scaled offscreen image always lands inside
/// the visible region with letterbox/pillarbox gaps filled by the
/// blit camera's clear colour.
fn update_blit_layout(
    stretch: Option<Res<ViewportStretchImage>>,
    project: Option<Res<renzora::CurrentProject>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut blit: Query<(&mut Sprite, &mut Transform), With<BlitSprite>>,
    mut resize_events: MessageReader<WindowResized>,
    mut force_first: Local<bool>,
) {
    // Only do work when something has actually changed: window
    // resize, project change (e.g. live-reload), or the very first
    // tick after spawn.
    let resized = !resize_events.is_empty();
    resize_events.clear();
    let project_changed = project.as_ref().map_or(false, |p| p.is_changed());
    if !resized && !project_changed && *force_first {
        return;
    }
    *force_first = true;

    let Some(stretch) = stretch else {
        return;
    };
    let Some(project) = project else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((mut sprite, mut transform)) = blit.single_mut() else {
        return;
    };

    let win_w = window.width();
    let win_h = window.height();
    if win_w <= 0.0 || win_h <= 0.0 {
        return;
    }

    let vp_w = stretch.size.x as f32;
    let vp_h = stretch.size.y as f32;
    if vp_w <= 0.0 || vp_h <= 0.0 {
        return;
    }

    // Compute the displayed sprite size in window pixels.
    let aspect_mode = project.config.viewport.aspect_mode;
    let (out_w, out_h) = match aspect_mode {
        AspectMode::Expand => (win_w, win_h),
        AspectMode::KeepWidth => {
            let scale = win_w / vp_w;
            (win_w, vp_h * scale)
        }
        AspectMode::KeepHeight => {
            let scale = win_h / vp_h;
            (vp_w * scale, win_h)
        }
        AspectMode::Keep => {
            let scale = (win_w / vp_w).min(win_h / vp_h);
            (vp_w * scale, vp_h * scale)
        }
    };
    sprite.custom_size = Some(Vec2::new(out_w, out_h));

    // Centre the sprite in the visible region. With viewport_origin
    // (0, 1) on the blit camera (per the engine-wide convention), the
    // visible region is (0, -win_h) to (win_w, 0); its centre is at
    // (win_w/2, -win_h/2). Sprite anchor is Center by default, so the
    // sprite extends symmetrically around this point.
    transform.translation.x = win_w * 0.5;
    transform.translation.y = -win_h * 0.5;
}
