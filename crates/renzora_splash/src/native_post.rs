//! Real full-frame post-process for the splash.
//!
//! The splash background (sky shader + terrain render) is rendered to an offscreen
//! image by a dedicated `Camera2d` (mirroring `renzora_game_ui_editor`'s
//! `canvas_render`). A fullscreen [`PostView`] node on the main camera then samples
//! that image through `post.wgsl`, which does genuine bloom + chromatic aberration
//! + scanlines + vignette — effects a UI overlay can't do because it can't read
//! what's behind it. The interactive launcher UI stays on the main camera, on top
//! of the post result, so it remains crisp and clickable.
//!
//! The post camera is `is_active`-gated to the [`SplashState::Splash`] state and
//! carries the editor's isolation markers, so it costs nothing and doesn't disturb
//! the editor outside the splash.

use bevy::asset::{Asset, RenderAssetUsages};
use bevy::camera::RenderTarget;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::shader::ShaderRef;
use bevy::time::Real;
use bevy::ui::{ComputedNode, FocusPolicy};
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;
use bevy::window::PrimaryWindow;

use crate::SplashState;

const INIT_W: u32 = 1920;
const INIT_H: u32 = 1080;

/// Marker for the fullscreen node (on the main camera) that displays the
/// post-processed background.
#[derive(Component)]
pub(crate) struct PostView;

/// Marker for the splash post camera.
#[derive(Component)]
struct PostCamera;

/// Handle to the offscreen background image + the camera that renders it.
#[derive(Resource)]
pub(crate) struct SplashPost {
    pub image: Handle<Image>,
    pub camera: Entity,
    size: UVec2,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct PostMaterial {
    /// x = time, y = width(px), z = height(px).
    #[uniform(0)]
    params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    image: Option<Handle<Image>>,
}

impl UiMaterial for PostMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_splash/post.wgsl".into()
    }
}

pub(crate) fn register(app: &mut App) {
    bevy::asset::embedded_asset!(app, "post.wgsl");
    bevy::asset::embedded_asset!(app, "tvoff.wgsl");
    bevy::asset::embedded_asset!(app, "matrix.wgsl");
    app.add_plugins(UiMaterialPlugin::<PostMaterial>::default());
    app.add_plugins(UiMaterialPlugin::<TvOffMaterial>::default());
    app.add_plugins(UiMaterialPlugin::<MatrixMaterial>::default());
    app.add_systems(Startup, setup_post);
    app.add_systems(OnEnter(SplashState::Editor), start_editor_intro);
    app.add_systems(
        Update,
        (
            gate_post_camera,
            resize_post_target,
            attach_post_view,
            sync_post,
            attach_tvoff_view,
            sync_tvoff,
            attach_editor_intro,
            tick_editor_intro,
            attach_matrix,
            sync_matrix,
        ),
    );
}

/// Build the offscreen image + the dedicated 2D camera that renders the splash
/// background into it. Spawned inactive; [`gate_post_camera`] turns it on only in
/// the splash state.
fn setup_post(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let size = Extent3d { width: INIT_W, height: INIT_H, depth_or_array_layers: 1 };
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
    let image = images.add(image);

    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::NONE),
                order: -20,
                is_active: false,
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            PostCamera,
            renzora::IsolatedCamera,
            renzora::EditorLocked,
            renzora::HideInHierarchy,
            Name::new("Splash Post Camera"),
        ))
        .id();

    commands.insert_resource(SplashPost { image, camera, size: UVec2::new(INIT_W, INIT_H) });
}

/// Only render the background pass while the splash is showing.
fn gate_post_camera(state: Res<State<SplashState>>, mut cam: Query<&mut Camera, With<PostCamera>>) {
    let want = matches!(state.get(), SplashState::Splash);
    for mut c in &mut cam {
        if c.is_active != want {
            c.is_active = want;
        }
    }
}

/// Keep the offscreen image sized to the window so the post pass is 1:1.
fn resize_post_target(
    mut post: ResMut<SplashPost>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(window) = windows.single() else { return };
    let w = (window.physical_width()).clamp(1, 7680);
    let h = (window.physical_height()).clamp(1, 4320);
    let requested = UVec2::new(w, h);
    if post.size == requested {
        return;
    }
    if let Some(mut image) = images.get_mut(&post.image) {
        image.resize(Extent3d { width: w, height: h, depth_or_array_layers: 1 });
        post.size = requested;
    }
}

fn attach_post_view(
    mut commands: Commands,
    post: Res<SplashPost>,
    mut materials: ResMut<Assets<PostMaterial>>,
    views: Query<Entity, (With<PostView>, Without<MaterialNode<PostMaterial>>)>,
) {
    for e in &views {
        let handle = materials.add(PostMaterial { params: Vec4::ZERO, image: Some(post.image.clone()) });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn sync_post(
    time: Res<Time>,
    post: Res<SplashPost>,
    mut materials: ResMut<Assets<PostMaterial>>,
    views: Query<&MaterialNode<PostMaterial>, With<PostView>>,
) {
    let t = time.elapsed_secs();
    for mat in &views {
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.params = Vec4::new(t, post.size.x as f32, post.size.y as f32, 0.0);
        }
    }
}

// ── CRT turn-off overlay ───────────────────────────────────────────────────────

/// Marker for the fullscreen CRT turn-off node (on the main camera, above the UI).
#[derive(Component)]
pub(crate) struct TvOffView;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct TvOffMaterial {
    /// x = progress 0..1, y = active (0/1).
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for TvOffMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_splash/tvoff.wgsl".into()
    }
}

fn attach_tvoff_view(
    mut commands: Commands,
    mut materials: ResMut<Assets<TvOffMaterial>>,
    views: Query<Entity, (With<TvOffView>, Without<MaterialNode<TvOffMaterial>>)>,
) {
    for e in &views {
        let handle = materials.add(TvOffMaterial { params: Vec4::ZERO });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn sync_tvoff(
    tvoff: Option<Res<crate::TvOff>>,
    mut materials: ResMut<Assets<TvOffMaterial>>,
    views: Query<&MaterialNode<TvOffMaterial>, With<TvOffView>>,
) {
    let (active, progress) = match tvoff {
        Some(tv) => (1.0, (tv.timer / crate::TVOFF_DURATION).clamp(0.0, 1.0)),
        None => (0.0, 0.0),
    };
    for mat in &views {
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.params = Vec4::new(progress, active, 0.0, 0.0);
        }
    }
}

// ── Matrix rain (loading screen background) ────────────────────────────────────

/// Marker for the fullscreen matrix-rain node behind the loading terminal.
#[derive(Component)]
pub(crate) struct MatrixView;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct MatrixMaterial {
    /// x = time, y = width(px), z = height(px).
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for MatrixMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_splash/matrix.wgsl".into()
    }
}

fn attach_matrix(
    mut commands: Commands,
    mut materials: ResMut<Assets<MatrixMaterial>>,
    views: Query<Entity, (With<MatrixView>, Without<MaterialNode<MatrixMaterial>>)>,
) {
    for e in &views {
        let handle = materials.add(MatrixMaterial { params: Vec4::ZERO });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn sync_matrix(
    time: Res<Time>,
    mut materials: ResMut<Assets<MatrixMaterial>>,
    views: Query<(&ComputedNode, &MaterialNode<MatrixMaterial>), With<MatrixView>>,
) {
    let t = time.elapsed_secs();
    for (cn, mat) in &views {
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let size = cn.size();
            m.params = Vec4::new(t, size.x, size.y, 0.0);
        }
    }
}

// ── Editor power-on intro ──────────────────────────────────────────────────────

/// Marker for the editor power-on overlay (runs the CRT effect in reverse).
#[derive(Component)]
struct EditorIntroView;

#[derive(Resource, Default)]
struct EditorIntro {
    /// Time held fully black, waiting for the editor to finish loading.
    hold: f32,
    /// Time spent in the reveal animation once started.
    reveal: f32,
    revealing: bool,
}

/// Minimum black hold (lets the editor begin), the max wait before revealing
/// anyway, and the reveal length.
const EDITOR_INTRO_HOLD_MIN: f32 = 0.3;
const EDITOR_INTRO_HOLD_MAX: f32 = 8.0;
const EDITOR_INTRO_REVEAL: f32 = 0.45;

/// On entering the editor, drop a black overlay that quickly powers on (CRT
/// reveal: dot → line → full) so the editor doesn't pop in abruptly.
fn start_editor_intro(mut commands: Commands) {
    commands.insert_resource(EditorIntro::default());
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            ..default()
        },
        // Black until the material attaches (avoids a one-frame editor flash).
        BackgroundColor(Color::BLACK),
        GlobalZIndex(12000),
        FocusPolicy::Pass,
        EditorIntroView,
        Name::new("editor-intro"),
    ));
}

fn attach_editor_intro(
    mut commands: Commands,
    mut materials: ResMut<Assets<TvOffMaterial>>,
    views: Query<Entity, (With<EditorIntroView>, Without<MaterialNode<TvOffMaterial>>)>,
) {
    for e in &views {
        // Start fully closed (progress 1 = black), active.
        let handle = materials.add(TvOffMaterial { params: Vec4::new(1.0, 1.0, 0.0, 0.0) });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

/// Hold the screen black until the editor has finished loading (its tab-decode
/// overlay is no longer active, after a short minimum, or a max timeout), then
/// power on (progress 1 → 0) so the reveal blends into a ready editor.
fn tick_editor_intro(
    time: Res<Time<Real>>,
    intro: Option<ResMut<EditorIntro>>,
    overlay: Option<Res<crate::EditorLoadingOverlayActive>>,
    mut commands: Commands,
    mut materials: ResMut<Assets<TvOffMaterial>>,
    views: Query<(Entity, Option<&MaterialNode<TvOffMaterial>>), With<EditorIntroView>>,
) {
    let Some(mut intro) = intro else { return };
    let dt = time.delta_secs();

    let progress = if !intro.revealing {
        intro.hold += dt;
        let editor_busy = overlay.is_some_and(|o| o.0);
        let ready = (intro.hold >= EDITOR_INTRO_HOLD_MIN && !editor_busy)
            || intro.hold >= EDITOR_INTRO_HOLD_MAX;
        if ready {
            intro.revealing = true;
        }
        1.0 // stay fully black while waiting
    } else {
        intro.reveal += dt;
        (1.0 - intro.reveal / EDITOR_INTRO_REVEAL).clamp(0.0, 1.0)
    };

    for (_, mat) in &views {
        if let Some(m) = mat {
            if let Some(mut mm) = materials.get_mut(&m.0) {
                mm.params = Vec4::new(progress, 1.0, 0.0, 0.0);
            }
        }
    }

    if intro.revealing && intro.reveal >= EDITOR_INTRO_REVEAL {
        for (e, _) in &views {
            commands.entity(e).try_despawn();
        }
        commands.remove_resource::<EditorIntro>();
    }
}
