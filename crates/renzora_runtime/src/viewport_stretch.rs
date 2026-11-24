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
//!
//! All of `[viewport]` is live: turning stretching on or off, or changing
//! the resolution, rebuilds the pass while the game runs (see
//! [`sync_stretch`]), so an in-game settings menu can offer it.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::image::{Image, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::window::{PrimaryWindow, WindowResized};

use renzora_project_api::{AspectMode, StretchMode};

/// Render layer that holds the blit sprite + blit camera. Separated
/// from the default game world (layer 0) so the game camera doesn't
/// re-render the blit sprite on top of the gameplay it already drew.
/// Allocated in the contract crate's registry, alongside every other rig's.
use renzora_viewport_api::VIEWPORT_STRETCH_BLIT_LAYER as BLIT_RENDER_LAYER;

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

/// The MSAA a camera had before it was redirected into the offscreen image,
/// which forces it off; `None` when it had no `Msaa` component. Put back when
/// stretching is turned off, so a game that leaves stretch mode keeps its
/// anti-aliasing instead of silently losing it.
#[derive(Component, Clone, Copy)]
struct StretchedMsaa(Option<Msaa>);

pub struct ViewportStretchPlugin;

impl Plugin for ViewportStretchPlugin {
    fn build(&self, app: &mut App) {
        // `sync_stretch` runs at Startup so a stretched game's first frame is
        // already stretched (the scene's cameras are redirected by the observer
        // as they load), and again whenever `CurrentProject` changes, so
        // `[viewport] width`, `height` and `stretch_mode` apply while the game
        // runs, from an in-game settings menu as much as from the editor. They
        // used to be read once at Startup and never again.
        app.add_systems(Startup, sync_stretch)
            .add_systems(
                Update,
                (
                    sync_stretch.run_if(
                        resource_exists_and_changed::<renzora_project_api::CurrentProject>,
                    ),
                    redirect_game_cameras_to_offscreen,
                    update_blit_layout,
                )
                    .chain(),
            )
            .add_observer(on_camera_2d_added_redirect);
    }
}

/// What a `[viewport]` change asks of the stretch pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StretchAction {
    /// Already as configured.
    Keep,
    /// Stretching was off: create the offscreen image and the blit pass.
    Create(UVec2),
    /// Stretching stays on at a new resolution: resize the offscreen image.
    Resize(UVec2),
    /// Stretching was on and is now off: render to the window again.
    Remove,
}

/// The offscreen resolution `[viewport]` asks for, or `None` for no stretching.
/// A zero width or height is treated as off rather than as a zero-pixel target,
/// which wgpu would refuse.
fn wanted_stretch(cfg: &renzora_project_api::ViewportConfig) -> Option<UVec2> {
    (cfg.stretch_mode != StretchMode::Disabled && cfg.width > 0 && cfg.height > 0)
        .then(|| UVec2::new(cfg.width, cfg.height))
}

/// Compare the stretch pass that exists (`current`, its offscreen size) with
/// the one wanted.
fn stretch_action(current: Option<UVec2>, wanted: Option<UVec2>) -> StretchAction {
    match (current, wanted) {
        (None, None) => StretchAction::Keep,
        (None, Some(size)) => StretchAction::Create(size),
        (Some(_), None) => StretchAction::Remove,
        (Some(have), Some(want)) if have == want => StretchAction::Keep,
        (Some(_), Some(want)) => StretchAction::Resize(want),
    }
}

/// The blit sprite and blit camera, which go together.
type BlitPassFilter = Or<(With<BlitSprite>, With<BlitCamera>)>;
/// A camera's layers, its MSAA, and whether that MSAA was already recorded.
type CameraMsaa = (Option<&'static RenderLayers>, Option<&'static Msaa>, Has<StretchedMsaa>);
/// [`CameraMsaa`] plus the entity and where it draws.
type RedirectCamera = (
    Entity,
    Option<&'static RenderLayers>,
    &'static RenderTarget,
    Option<&'static Msaa>,
    Has<StretchedMsaa>,
);
/// Every camera but the blit camera, with where it draws and the MSAA it had
/// before stretching.
type GameCameraTargets<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static RenderTarget, Option<&'static StretchedMsaa>),
    (With<Camera>, Without<BlitCamera>),
>;

/// Bring the stretch pass in line with `[viewport]`. Disabled mode leaves no
/// `ViewportStretchImage`, so every downstream system can early-out on its
/// absence.
///
/// A resize replaces the image's contents under the same handle rather than
/// making a new one: every redirected camera targets that handle, so they all
/// follow without being touched, and the blit sprite is re-laid out by
/// `update_blit_layout` because the resource changed.
fn sync_stretch(
    project: Option<Res<renzora_project_api::CurrentProject>>,
    stretch: Option<ResMut<ViewportStretchImage>>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    blit_pass: Query<Entity, BlitPassFilter>,
    cameras: GameCameraTargets,
) {
    let Some(project) = project else {
        warn!("[viewport_stretch] No project, stretch disabled");
        return;
    };
    let cfg = &project.config.viewport;
    if cfg.stretch_mode != StretchMode::Disabled && (cfg.width == 0 || cfg.height == 0) {
        warn!("[viewport_stretch] viewport.width/height is 0, falling back to disabled");
    }
    let current = stretch.as_ref().map(|s| s.size);
    match stretch_action(current, wanted_stretch(cfg)) {
        StretchAction::Keep => {}
        StretchAction::Create(size) => {
            let handle = images.add(stretch_image(size));
            spawn_blit_pass(&mut commands, &handle, size);
            commands.insert_resource(ViewportStretchImage { image: handle, size });
            info!(
                "[viewport_stretch] stretch={:?} aspect={:?} viewport={}x{} (offscreen image created)",
                cfg.stretch_mode, cfg.aspect_mode, size.x, size.y
            );
        }
        StretchAction::Resize(size) => {
            let Some(mut stretch) = stretch else { return };
            if let Err(e) = images.insert(stretch.image.id(), stretch_image(size)) {
                warn!("[viewport_stretch] could not resize the offscreen image: {e}");
                return;
            }
            stretch.size = size;
            info!("[viewport_stretch] viewport resized to {}x{}", size.x, size.y);
        }
        StretchAction::Remove => {
            let Some(stretch) = stretch else { return };
            for entity in &blit_pass {
                commands.entity(entity).try_despawn();
            }
            // Point every camera that was drawing into the offscreen image back
            // at the window, with the MSAA it had before stretching forced it
            // off for the nearest-neighbour upscale.
            let window = RenderTarget::Window(bevy::window::WindowRef::Primary);
            for (entity, target, saved) in &cameras {
                if !matches!(target, RenderTarget::Image(t) if t.handle == stretch.image) {
                    continue;
                }
                crate::camera_target::retarget(&mut commands, entity, window.clone());
                let mut e = commands.entity(entity);
                match saved {
                    Some(StretchedMsaa(Some(msaa))) => {
                        e.insert(*msaa);
                    }
                    Some(StretchedMsaa(None)) => {
                        e.remove::<Msaa>();
                    }
                    // Redirected before this was recorded: Off is always valid.
                    None => {}
                }
                e.remove::<StretchedMsaa>();
            }
            commands.remove_resource::<ViewportStretchImage>();
            info!("[viewport_stretch] stretch disabled, rendering to the window");
        }
    }
}

/// The offscreen image the game cameras render into, at `size`.
fn stretch_image(size: UVec2) -> Image {
    let size = Extent3d {
        width: size.x,
        height: size.y,
        depth_or_array_layers: 1,
    };

    // Manually-built render target image. Needs RENDER_ATTACHMENT (the
    // game camera renders into it) plus TEXTURE_BINDING (the blit
    // camera samples it). COPY_DST mirrors what `Image::new_target_texture`
    // would set — keeps options open for tonemapping / postprocess
    // passes that might want to copy into the texture later. No CPU copy:
    // only the GPU draws into or samples it, and wgpu zero-fills a texture
    // created without data.
    let mut image = Image::new_uninit(
        size,
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    // Nearest-neighbour sampler: the whole point of viewport stretch
    // for pixel art. Linear here would smear every pixel block.
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());
    image
}

/// Spawn the second render pass that draws the offscreen image to the OS
/// window. A dedicated `Camera2d` on a high render layer views a sprite that
/// displays the offscreen image.
fn spawn_blit_pass(commands: &mut Commands, image: &Handle<Image>, size: UVec2) {
    // Sprite displaying the offscreen image. `custom_size` gets
    // updated each frame by `update_blit_layout` to letterbox/
    // pillarbox the window. Centred at world (0, 0) so the blit
    // camera can sit at origin too.
    commands.spawn((
        Sprite {
            image: image.clone(),
            custom_size: Some(size.as_vec2()),
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
    cameras: Query<RedirectCamera, (With<Camera2d>, Without<BlitCamera>)>,
) {
    let Some(stretch) = stretch else {
        return;
    };

    for (entity, layers, current_target, msaa, saved) in cameras.iter() {
        // Only redirect cameras that render the default game layer (0).
        // RenderLayers::default() is layer 0; treat missing component
        // the same way.
        let on_game_layer = layers.is_none_or(|l| l.intersects(&RenderLayers::default()));
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
        // on every camera that renders into the offscreen image, keeping
        // what it had for when stretching is turned off.
        crate::camera_target::retarget(
            &mut commands,
            entity,
            (RenderTarget::Image(stretch.image.clone().into()), Msaa::Off),
        );
        if !saved {
            commands.entity(entity).insert(StretchedMsaa(msaa.copied()));
        }
    }
}

/// Observer: catches Camera2d entities the moment they're inserted
/// (preset spawn, scene load) so they start pointing at the offscreen
/// image *before* the first frame renders. The polling system above
/// covers the case where this observer can't redirect (e.g. the
/// stretch resource doesn't exist yet at insert time, then the user
/// hot-reloads project config).
fn on_camera_2d_added_redirect(
    trigger: On<Insert<Camera2d>>,
    stretch: Option<Res<ViewportStretchImage>>,
    cameras: Query<CameraMsaa, With<Camera2d>>,
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
    let Ok((layers, msaa, saved)) = cameras.get(entity) else {
        return;
    };
    let on_game_layer = layers.is_none_or(|l| l.intersects(&RenderLayers::default()));
    if !on_game_layer {
        return;
    }
    crate::camera_target::retarget(
        &mut commands,
        entity,
        (RenderTarget::Image(stretch.image.clone().into()), Msaa::Off),
    );
    if !saved {
        commands.entity(entity).insert(StretchedMsaa(msaa.copied()));
    }
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
    project: Option<Res<renzora_project_api::CurrentProject>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut blit: Query<(&mut Sprite, &mut Transform), With<BlitSprite>>,
    mut resize_events: MessageReader<WindowResized>,
    mut force_first: Local<bool>,
) {
    // Only do work when something has actually changed: window
    // resize, project change (e.g. live-reload), the stretch pass being
    // created or resized, or the very first tick after spawn.
    let resized = !resize_events.is_empty();
    resize_events.clear();
    let project_changed = project.as_ref().is_some_and(|p| p.is_changed());
    let stretch_changed = stretch.as_ref().is_some_and(|s| s.is_changed());
    if !resized && !project_changed && !stretch_changed && *force_first {
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

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_project_api::ViewportConfig;

    fn viewport(stretch_mode: StretchMode, width: u32, height: u32) -> ViewportConfig {
        ViewportConfig { width, height, stretch_mode, ..ViewportConfig::default() }
    }

    #[test]
    fn disabled_or_zero_sized_wants_no_stretch() {
        assert_eq!(wanted_stretch(&viewport(StretchMode::Disabled, 320, 180)), None);
        assert_eq!(wanted_stretch(&viewport(StretchMode::Viewport, 0, 180)), None);
        assert_eq!(wanted_stretch(&viewport(StretchMode::Viewport, 320, 0)), None);
        assert_eq!(
            wanted_stretch(&viewport(StretchMode::Viewport, 320, 180)),
            Some(UVec2::new(320, 180))
        );
    }

    #[test]
    fn stretch_actions() {
        let small = UVec2::new(320, 180);
        let large = UVec2::new(640, 360);
        assert_eq!(stretch_action(None, None), StretchAction::Keep);
        assert_eq!(stretch_action(None, Some(small)), StretchAction::Create(small));
        assert_eq!(stretch_action(Some(small), Some(small)), StretchAction::Keep);
        assert_eq!(stretch_action(Some(small), Some(large)), StretchAction::Resize(large));
        assert_eq!(stretch_action(Some(small), None), StretchAction::Remove);
    }

    /// Stretching forces a camera's MSAA off; turning it off again gives the
    /// camera back what it had.
    #[test]
    fn leaving_stretch_mode_restores_the_cameras_msaa() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            // Bevy's window plugin registers this in a real game.
            .add_message::<WindowResized>()
            .insert_resource(renzora_project_api::CurrentProject {
                path: "game".into(),
                config: Default::default(),
            })
            .add_plugins(ViewportStretchPlugin);
        let camera = app.world_mut().spawn((Camera2d, Msaa::Sample4)).id();
        app.update();

        let set_stretch = |app: &mut App, mode: StretchMode| {
            let mut p = app.world_mut().resource_mut::<renzora_project_api::CurrentProject>();
            p.config.viewport = viewport(mode, 320, 180);
        };
        set_stretch(&mut app, StretchMode::Viewport);
        app.update();
        app.update();
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Off));
        assert!(matches!(app.world().get::<RenderTarget>(camera), Some(RenderTarget::Image(_))));

        set_stretch(&mut app, StretchMode::Disabled);
        app.update();
        assert_eq!(app.world().get::<Msaa>(camera), Some(&Msaa::Sample4));
        assert!(matches!(app.world().get::<RenderTarget>(camera), Some(RenderTarget::Window(_))));
        assert!(app.world().get::<StretchedMsaa>(camera).is_none());
    }
}
