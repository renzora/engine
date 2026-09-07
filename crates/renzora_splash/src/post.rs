//! Real full-frame post-process for the splash.
//!
//! The splash background (the Light Chamber render — see `chamber.rs`) is
//! rendered to an offscreen image by a dedicated `Camera2d` (mirroring
//! `renzora_game_ui_editor`'s `canvas_render`). A fullscreen [`PostView`] node on
//! the main camera then samples that image through `post.wgsl`, which does the
//! lens/film grade — halation, lateral chromatic aberration, vignette, grain —
//! effects a UI overlay can't do because it can't read what's behind it. The
//! interactive launcher UI stays on the main camera, on top of the post result, so
//! it remains crisp and clickable.
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
use bevy::ui::ComputedNode;
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
    bevy::asset::embedded_asset!(app, "haze.wgsl");
    app.add_plugins(UiMaterialPlugin::<PostMaterial>::default());
    app.add_plugins(UiMaterialPlugin::<HazeMaterial>::default());
    app.add_systems(Startup, setup_post);
    app.add_systems(
        Update,
        (
            gate_post_camera,
            resize_post_target,
            attach_post_view,
            sync_post,
            attach_haze,
            sync_haze,
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

/// Only render the background pass while the splash is showing — and never on an
/// integrated GPU.
///
/// The cinematic is a full-window, multi-pass post chain (a volumetric light
/// chamber → spectral/film shaders) rendered at the display's physical resolution.
/// That is precisely the fill-rate-bound workload an integrated adapter is worst at,
/// and it is the *first* thing a user sees — so on a weak GPU the engine's opening
/// impression is a stuttering animation before the editor has even loaded. It is
/// decorative, so it is not worth paying for; the splash UI itself is unaffected.
///
/// `chamber::manage_chamber` gates the 3D scene on the same condition, so on
/// an integrated adapter nothing is rendered *or* displayed.
///
/// With the camera inactive the offscreen target simply keeps its initial clear
/// (`Color::NONE`), so the backdrop reads as flat rather than broken.
fn gate_post_camera(
    state: Res<State<SplashState>>,
    integrated: Option<Res<renzora::GpuIsIntegrated>>,
    mut cam: Query<&mut Camera, With<PostCamera>>,
) {
    let cinematic_ok = !integrated.is_some_and(|g| g.yes);
    let want = matches!(state.get(), SplashState::Splash) && cinematic_ok;
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

// ── Drifting haze (loading screen background) ──────────────────────────────────

/// Marker for the fullscreen haze node behind the loading terminal — the chamber's
/// shafts and dust carried through to the screen that follows the splash.
#[derive(Component)]
pub(crate) struct HazeView;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct HazeMaterial {
    /// x = time, y = width(px), z = height(px).
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for HazeMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_splash/haze.wgsl".into()
    }
}

fn attach_haze(
    mut commands: Commands,
    mut materials: ResMut<Assets<HazeMaterial>>,
    views: Query<Entity, (With<HazeView>, Without<MaterialNode<HazeMaterial>>)>,
) {
    for e in &views {
        let handle = materials.add(HazeMaterial { params: Vec4::ZERO });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn sync_haze(
    time: Res<Time>,
    mut materials: ResMut<Assets<HazeMaterial>>,
    views: Query<(&ComputedNode, &MaterialNode<HazeMaterial>), With<HazeView>>,
) {
    let t = time.elapsed_secs();
    for (cn, mat) in &views {
        if let Some(mut m) = materials.get_mut(&mat.0) {
            let size = cn.size();
            m.params = Vec4::new(t, size.x, size.y, 0.0);
        }
    }
}
