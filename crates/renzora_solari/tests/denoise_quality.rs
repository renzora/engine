//! How close Solari's built-in denoiser gets to the true image, measured on a
//! real ray-tracing GPU.
//!
//! A sunlit room is rendered three ways: by the realtime path with the denoiser,
//! by the realtime path without it, and by Solari's own offline path tracer run
//! long enough to converge, which is the reference. Each image is scored against
//! the reference, so a change to the denoiser is judged by a number rather than
//! by eye, and a regression shows up as one.
//!
//! The scene is the case the denoiser exists for: an interior lit mostly by
//! sunlight bouncing off the floor through a window, where nearly everything the
//! camera sees is indirect light, the noisiest thing Solari computes.
//!
//! Opt-in like the engine's other GPU tests: `RENZORA_GPU_TESTS=1`, and it needs
//! a GPU that can ray trace. `RENZORA_GPU_TEST_OUT=<dir>` writes every image as a
//! PPM for looking at. Run with `--nocapture` to see the scores.

use std::sync::Mutex;

use bevy::asset::RenderAssetUsages;
use bevy::camera::{CameraMainTextureUsages, Exposure, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
use bevy::render::RenderPlugin;
use bevy::solari::pathtracer::{Pathtracer, PathtracingPlugin};
use bevy::solari::prelude::{RaytracingMesh3d, SolariLighting, SolariPlugins};
use bevy::solari::realtime::{SolariDebug, SolariDebugOutput, SolariDebugStats, SolariDebugView};
use bevy::solari::scene::RaytracingScenePlugin;
use bevy::window::{ExitCondition, WindowPlugin};

/// The image size: small by default so a run takes seconds, or
/// `RENZORA_SOLARI_TEST_SIZE=<w>x<h>` to test at a real viewport's size, where
/// every pixel-sized radius in the denoiser and in ReSTIR covers less of the
/// scene. The width must be a multiple of 32, so a row of half floats fills
/// whole 256-byte readback rows.
fn size() -> (u32, u32) {
    std::env::var("RENZORA_SOLARI_TEST_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((320, 192))
}

/// Frames the realtime path runs before its image is taken: about two seconds
/// at 60 fps, long enough for every temporal stage to settle.
const REALTIME_FRAMES: usize = 120;

/// Samples per pixel the reference accumulates. The path tracer reaches the
/// sky only by bounce rays that leave through the window, so it needs many.
const REFERENCE_FRAMES: usize = 16384;

/// One renderer at a time; each test stands up a full one.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Where the camera stands and what it looks at, at the end of a run.
#[derive(Clone, Copy, Debug)]
struct Pose {
    eye: Vec3,
    target: Vec3,
}

/// The view the regression tests use: across the room towards the window.
const DEFAULT_POSE: Pose = Pose {
    eye: Vec3::new(-2.4, 1.5, 2.1),
    target: Vec3::new(1.5, 1.1, -1.2),
};

/// The room's interior, which the benchmark's cameras are placed in.
const ROOM_MIN: Vec3 = Vec3::new(-2.8, 0.3, -2.3);
const ROOM_MAX: Vec3 = Vec3::new(2.8, 2.8, 2.3);

/// `count` camera poses spread through the room, from a fixed seed so every
/// run measures the same ones. Random rather than chosen, so the denoiser is
/// not tuned to whatever one view happens to show: each stands anywhere in the
/// room and looks at a point at least a metre and a half away, so between them
/// they cover the window, walls, floor and furniture from near and far, high
/// and low.
fn benchmark_poses(count: usize) -> Vec<Pose> {
    let mut state = 0x9E3779B9u32;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    let mut point = move || ROOM_MIN + (ROOM_MAX - ROOM_MIN) * Vec3::new(random(), random(), random());
    let mut poses = Vec::with_capacity(count);
    while poses.len() < count {
        let (eye, target) = (point(), point());
        if eye.distance(target) >= 1.5 {
            poses.push(Pose { eye, target });
        }
    }
    poses
}

#[derive(Clone, Copy, PartialEq)]
enum Renderer {
    /// The realtime path, denoised or not, with or without TAA (which the
    /// editor's viewport runs whenever DLSS does not), and with or without
    /// DLSS Ray Reconstruction.
    Realtime { denoise: bool, taa: bool, dlss: Dlss },
    /// The offline path tracer, the reference.
    Reference,
}

/// Whether DLSS Ray Reconstruction runs. Only takes effect in a build with the
/// `dlss` feature.
#[derive(Clone, Copy, PartialEq)]
enum Dlss {
    Off,
    /// For the whole run, at DLAA (full resolution), as the editor's default.
    On,
    /// For the first half of the run, then taken away, the way choosing the
    /// built-in denoiser in the inspector does.
    #[cfg_attr(not(feature = "dlss"), expect(dead_code, reason = "only the dlss build's test switches DLSS off"))]
    ThenOff,
}

#[cfg(feature = "dlss")]
type RayReconstruction = bevy::anti_alias::dlss::Dlss<bevy::anti_alias::dlss::DlssRayReconstructionFeature>;

/// The realtime path without DLSS.
fn realtime(denoise: bool, taa: bool) -> Renderer {
    Renderer::Realtime { denoise, taa, dlss: Dlss::Off }
}

/// How the camera behaves while the realtime path runs.
#[derive(Clone, Copy, PartialEq)]
enum Motion {
    Still,
    /// Drifts sideways into its final place over the whole run, the way a
    /// player strafes: every frame is a reprojection, never a still.
    Strafe,
}

/// A linear HDR image, three floats a pixel.
struct Frame {
    rgb: Vec<f32>,
}

fn gpu_enabled() -> bool {
    std::env::var("RENZORA_GPU_TESTS").unwrap_or_default() == "1"
}

fn app(renderer: Renderer) -> App {
    let mut app = App::new();
    // With the `dlss` feature, `DefaultPlugins` holds Bevy's DLSS plugin, which
    // panics at build without a project id. The engine's own, from
    // `renzora_runtime`.
    #[cfg(feature = "dlss")]
    app.insert_resource(bevy::anti_alias::dlss::DlssProjectId(
        bevy::asset::uuid::Uuid::from_u128(0x0364273c_a645_40f3_b774_74a7377b0c10),
    ));
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<bevy::log::LogPlugin>()
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                    backends: Some(Backends::VULKAN),
                    features: SolariPlugins::required_wgpu_features(),
                    ..default()
                })),
                ..default()
            }),
    );
    // The path tracer runs on its own: the realtime plugin switches every
    // material to deferred, and a camera without a deferred prepass then fails.
    // Any GPU error fails the test. Bevy's default only logs one, and a test
    // could then pass over a dispatch that would break a real session.
    app.insert_resource(bevy::render::error_handler::RenderErrorHandler(fail_on_gpu_error));
    match renderer {
        Renderer::Realtime { .. } => {
            // The world cache's cell count is published as a render
            // diagnostic, which only this plugin records.
            app.add_plugins((SolariPlugins, bevy::render::diagnostic::RenderDiagnosticsPlugin));
        }
        Renderer::Reference => {
            app.add_plugins((RaytracingScenePlugin, PathtracingPlugin));
        }
    }
    app.finish();
    app.cleanup();
    app
}

fn fail_on_gpu_error(
    error: &bevy::render::error_handler::RenderError,
    _main_world: &mut World,
    _render_world: &mut World,
) -> bevy::render::error_handler::RenderErrorPolicy {
    panic!("GPU error ({:?}): {}", error.ty, error.description);
}

fn plain(color: Color, roughness: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: roughness,
        ..default()
    }
}

/// A box with everything Solari's BLAS builder demands: 32-bit indices and
/// exactly position, normal, UV and tangent.
fn traced_box(world: &mut World, size: Vec3, at: Vec3, material: &Handle<StandardMaterial>) -> Entity {
    let mut mesh = Mesh::from(Cuboid::from_size(size));
    mesh.asset_usage = RenderAssetUsages::default();
    mesh.generate_tangents().expect("a cuboid has UVs");
    if let Some(bevy::mesh::Indices::U16(indices)) = mesh.indices() {
        let wide = indices.iter().map(|&i| i as u32).collect();
        mesh.insert_indices(bevy::mesh::Indices::U32(wide));
    }
    let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
    world
        .spawn((
            Mesh3d(mesh.clone()),
            RaytracingMesh3d(mesh),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at),
        ))
        .id()
}

/// A tiling normal map of small random bumps, like sprayed plaster or a woven
/// fabric: the normal changes from one pixel to the next, which is what makes
/// a denoiser's normal edge-stop refuse to blur.
fn bumpy_normal_map(images: &mut Assets<Image>) -> Handle<Image> {
    noise_texture(images, 0x2545F491, |random| {
        // Tilted up to about 20 degrees in a random direction.
        let (x, y) = ((random() - 0.5) * 0.7, (random() - 0.5) * 0.7);
        let n = Vec3::new(x, y, 1.0).normalize();
        [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5]
    })
}

/// A finished floor's textures: a normal map tilting each texel a few
/// degrees, and a metallic-roughness map (roughness in green) varying the
/// gloss from texel to texel, as the apartment's wood and marble do. The
/// changes are texture detail, not edges, and a denoiser that treats them as
/// edges leaves a glossy floor grainy.
fn floor_textures(images: &mut Assets<Image>) -> (Handle<Image>, Handle<Image>) {
    let normals = noise_texture(images, 0x68E31DA4, |random| {
        let (x, y) = ((random() - 0.5) * 0.18, (random() - 0.5) * 0.18);
        let n = Vec3::new(x, y, 1.0).normalize();
        [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5]
    });
    let roughness = noise_texture(images, 0xB5297A4D, |random| [0.0, 0.5 + 0.5 * random(), 0.0]);
    (normals, roughness)
}

/// A tiling 256 x 256 linear texture, `texel` giving each texel's RGB.
fn noise_texture(images: &mut Assets<Image>, seed: u32, mut texel: impl FnMut(&mut dyn FnMut() -> f32) -> [f32; 3]) -> Handle<Image> {
    use bevy::render::render_resource::{Extent3d, TextureDimension};
    const SIZE: u32 = 256;
    let mut state = seed;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for _ in 0..SIZE * SIZE {
        let rgb = texel(&mut random);
        data.extend(rgb.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8));
        data.push(255);
    }
    let mut image = Image::new(
        Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::Repeat,
        address_mode_v: bevy::image::ImageAddressMode::Repeat,
        ..bevy::image::ImageSamplerDescriptor::linear()
    });
    images.add(image)
}

/// A closed 6 x 3 x 5 m room with a window in the +x wall, a sofa and a table,
/// and the sun shining in through the window onto the floor. The ceiling is
/// glossy and the back wall bumpy, like the editor's apartment test scene.
fn room(world: &mut World) {
    let bumps = bumpy_normal_map(&mut world.resource_mut::<Assets<Image>>());
    let (floor_normals, floor_roughness) = floor_textures(&mut world.resource_mut::<Assets<Image>>());
    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
    let wall = materials.add(plain(Color::srgb(0.62, 0.57, 0.52), 0.9));
    let bumpy_wall = materials.add(StandardMaterial {
        normal_map_texture: Some(bumps),
        ..plain(Color::srgb(0.62, 0.57, 0.52), 0.9)
    });
    let ceiling = materials.add(plain(Color::srgb(0.8, 0.8, 0.78), 0.36));
    // Glossy, like the apartment's polished floor: the reflection of the
    // window in it is what the denoiser smooths least.
    let floor = materials.add(StandardMaterial {
        normal_map_texture: Some(floor_normals),
        metallic_roughness_texture: Some(floor_roughness),
        ..plain(Color::srgb(0.32, 0.22, 0.16), 0.25)
    });
    let fabric = materials.add(plain(Color::srgb(0.38, 0.38, 0.40), 0.85));
    let wood = materials.add(plain(Color::srgb(0.20, 0.13, 0.09), 0.4));

    let t = 0.2;
    // Floor, ceiling, and the three solid walls.
    traced_box(world, Vec3::new(6.4, t, 5.4), Vec3::new(0.0, -t / 2.0, 0.0), &floor);
    traced_box(world, Vec3::new(6.4, t, 5.4), Vec3::new(0.0, 3.0 + t / 2.0, 0.0), &ceiling);
    traced_box(world, Vec3::new(t, 3.0, 5.4), Vec3::new(-3.0 - t / 2.0, 1.5, 0.0), &wall);
    traced_box(world, Vec3::new(6.4, 3.0, t), Vec3::new(0.0, 1.5, -2.5 - t / 2.0), &bumpy_wall);
    traced_box(world, Vec3::new(6.4, 3.0, t), Vec3::new(0.0, 1.5, 2.5 + t / 2.0), &wall);
    // The window wall at +x, built around an opening from z -1.2..1.2 and
    // y 0.8..2.5.
    let x = 3.0 + t / 2.0;
    traced_box(world, Vec3::new(t, 3.0, 1.3), Vec3::new(x, 1.5, -1.85), &wall);
    traced_box(world, Vec3::new(t, 3.0, 1.3), Vec3::new(x, 1.5, 1.85), &wall);
    traced_box(world, Vec3::new(t, 0.8, 2.4), Vec3::new(x, 0.4, 0.0), &wall);
    traced_box(world, Vec3::new(t, 0.5, 2.4), Vec3::new(x, 2.75, 0.0), &wall);
    // Furniture.
    traced_box(world, Vec3::new(0.9, 0.8, 2.4), Vec3::new(-2.4, 0.4, -0.3), &fabric);
    traced_box(world, Vec3::new(1.2, 0.4, 0.7), Vec3::new(-0.6, 0.2, -0.3), &wood);
    traced_box(world, Vec3::new(0.7, 0.9, 0.7), Vec3::new(0.9, 0.45, 1.6), &fabric);
    // Thin posts, like chair legs and a lamp stand: silhouettes a pixel or two
    // wide, where camera jitter keeps changing which surface a pixel sees and
    // the denoiser's history is most often thrown away.
    for (x, z) in [(-0.2, 0.6), (0.1, 0.6), (-0.2, 0.9), (0.1, 0.9), (1.8, -1.6), (2.2, -0.4)] {
        traced_box(world, Vec3::new(0.035, 1.6, 0.035), Vec3::new(x, 0.8, z), &wood);
    }

    world.spawn((
        DirectionalLight {
            illuminance: 40_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(8.0, 6.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

impl Pose {
    /// The camera `frame` frames into a run of `frames`.
    fn at(self, motion: Motion, frame: usize, frames: usize) -> Transform {
        let offset = match motion {
            Motion::Still => Vec3::ZERO,
            // Half a metre to the camera's left at the start, arriving on the
            // last frame.
            Motion::Strafe => {
                let right = (self.target - self.eye).cross(Vec3::Y).normalize_or(Vec3::X);
                -right * 0.5 * (1.0 - frame as f32 / (frames - 1) as f32)
            }
        };
        Transform::from_translation(self.eye + offset).looking_at(self.target + offset, Vec3::Y)
    }
}

/// Render `frames` frames and keep the last two, with `debug`'s diagnostics on
/// the camera when given, and their figures once they arrive.
fn render_with(
    renderer: Renderer,
    motion: Motion,
    frames: usize,
    debug: Option<SolariDebug>,
) -> (Frame, Frame, Option<SolariDebugStats>) {
    render_at(DEFAULT_POSE, renderer, motion, frames, debug)
}

/// [`render_with`] from any camera pose.
fn render_at(
    pose: Pose,
    renderer: Renderer,
    motion: Motion,
    frames: usize,
    debug: Option<SolariDebug>,
) -> (Frame, Frame, Option<SolariDebugStats>) {
    render_in(&mut room, pose, renderer, motion, frames, debug)
}

/// [`render_at`] in any scene, which `scene` spawns under the same sky.
fn render_in(
    scene: &mut dyn FnMut(&mut World),
    pose: Pose,
    renderer: Renderer,
    motion: Motion,
    frames: usize,
    debug: Option<SolariDebug>,
) -> (Frame, Frame, Option<SolariDebugStats>) {
    let (mut kept, stats) = render_frames(scene, pose, renderer, motion, frames, debug, &|frame| frame + 2 >= frames, &mut |_, _| {});
    let image = kept.pop().expect("the last frame is kept");
    let previous = kept.pop().expect("the frame before it is kept");
    (previous, image, stats)
}

/// [`render_in`], keeping every frame `keep` picks, in order, and calling
/// `each_frame` with the world before each counted frame renders, for a test
/// that changes the scene partway through.
#[allow(clippy::too_many_arguments)]
fn render_frames(
    scene: &mut dyn FnMut(&mut World),
    pose: Pose,
    renderer: Renderer,
    motion: Motion,
    frames: usize,
    debug: Option<SolariDebug>,
    keep: &dyn Fn(usize) -> bool,
    each_frame: &mut dyn FnMut(&mut World, usize),
) -> (Vec<Frame>, Option<SolariDebugStats>) {
    let mut app = app(renderer);
    let world = app.world_mut();

    let (width, height) = size();
    let mut target = Image::new_target_texture(width, height, TextureFormat::Rgba16Float, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = world.resource_mut::<Assets<Image>>().add(target);

    scene(world);
    // A daylight sky, seen only through the window: Solari reaches it by
    // bounce rays that happen to leave through the opening, which makes it the
    // hardest light in the room to sample.
    let sky = EnvironmentMapLight {
        intensity: 4000.0,
        ..EnvironmentMapLight::solid_color(&mut world.resource_mut::<Assets<Image>>(), Color::srgb(0.75, 0.85, 1.0))
    };
    // The same sky drawn behind everything, as the editor's atmosphere is.
    // Solari leaves pixels with no surface to whatever draws the background,
    // while the reference traces the sky there, so without it every pixel of
    // open sky in view counted as error.
    let skybox = bevy::core_pipeline::Skybox {
        image: Some(sky.specular_map.clone()),
        brightness: sky.intensity,
        ..default()
    };
    let mut camera = world.spawn((
        sky,
        skybox,
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderTarget::Image(target.clone().into()),
        Msaa::Off,
        // Linear radiance straight into the float target, so the scores are
        // taken on light, not on a tonemapper's curve.
        Tonemapping::None,
        Exposure { ev100: 8.0 },
        CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING),
        pose.at(motion, 0, frames),
    ));
    match renderer {
        Renderer::Realtime { denoise, taa, dlss } => {
            // DLSS anti-aliases the image itself, so the editor takes TAA away
            // while it runs; so does this.
            if taa && dlss != Dlss::On {
                camera.insert(bevy::anti_alias::taa::TemporalAntiAliasing::default());
            }
            #[cfg(feature = "dlss")]
            if dlss != Dlss::Off {
                camera.insert(RayReconstruction {
                    perf_quality_mode: bevy::anti_alias::dlss::DlssPerfQualityMode::Dlaa,
                    ..default()
                });
            }
            #[cfg(not(feature = "dlss"))]
            let _ = dlss;
            camera.insert(SolariLighting {
                restir: true,
                denoise,
                // The editor's Ultra preset, the one the comparison with DLSS
                // was made on.
                primary_di_samples: 16,
                secondary_di_samples: 8,
                max_bounces: 4,
                world_cache_max_temporal_samples: 64.0,
                world_cache_direct_light_sample_count: 64,
                world_cache_cell_updates_soft_target: 80_000,
                ..default()
            });
        }
        Renderer::Reference => {
            camera.insert(Pathtracer::default());
        }
    }
    if let Some(debug) = debug {
        camera.insert(debug);
    }
    let camera = camera.id();


    // Pipelines compile asynchronously and Solari draws nothing until they
    // exist, so the scene is given time to come up before the run is counted:
    // until nothing is left compiling, then a little more for the BLASes.
    for _ in 0..60 {
        app.update();
    }
    for _ in 0..6000 {
        let waiting = app
            .sub_app(bevy::render::RenderApp)
            .world()
            .resource::<bevy::render::render_resource::PipelineCache>()
            .waiting_pipelines()
            .count();
        if waiting == 0 {
            break;
        }
        app.update();
    }
    for _ in 0..30 {
        app.update();
    }
    // A pipeline that failed to build never waits, and the pass it belongs
    // to then silently does nothing: fail on it here instead.
    for pipeline in app
        .sub_app(bevy::render::RenderApp)
        .world()
        .resource::<bevy::render::render_resource::PipelineCache>()
        .pipelines()
    {
        if let bevy::render::render_resource::CachedPipelineState::Err(error) = &pipeline.state {
            panic!("a pipeline failed to build: {error}");
        }
    }
    if let Some(mut lighting) = app.world_mut().get_mut::<SolariLighting>(camera) {
        lighting.reset = true;
    }
    // The path tracer's reset is not one-shot: it holds until cleared, and
    // restarts the average every frame it is set.
    if let Some(mut pathtracer) = app.world_mut().get_mut::<Pathtracer>(camera) {
        pathtracer.reset = true;
        app.update();
        app.world_mut().get_mut::<Pathtracer>(camera).unwrap().reset = false;
    }
    // The last two frames are kept: the final image, and the one before it for
    // how much the picture still moves while standing still.
    let mut kept = Vec::new();
    for frame in 0..frames {
        // Only a moving camera is written to: the path tracer starts its
        // average over whenever the camera's transform changes, even to the
        // value it already had.
        if motion != Motion::Still {
            *app.world_mut().get_mut::<Transform>(camera).unwrap() = pose.at(motion, frame, frames);
        }
        #[cfg(feature = "dlss")]
        if frame == frames / 2 && matches!(renderer, Renderer::Realtime { dlss: Dlss::ThenOff, .. }) {
            app.world_mut().entity_mut(camera).remove::<RayReconstruction>();
        }
        each_frame(app.world_mut(), frame);
        app.update();
        if keep(frame) {
            kept.push(grab(&mut app, &target));
        }
    }
    // The diagnostics' figures arrive a few frames behind the frame they
    // describe; the camera holds still meanwhile.
    let stats = debug.filter(|d| d.stats).map(|_| {
        let output = app.world().resource::<SolariDebugOutput>().0.clone();
        output.lock().unwrap().take();
        for _ in 0..30 {
            app.update();
            if let Some(stats) = output.lock().unwrap().take() {
                return stats;
            }
        }
        panic!("the diagnostics never reported");
    });
    // Render diagnostics are read back from the GPU a few frames late, and a
    // run that follows another in the same process could look before the first
    // value arrived; so keep rendering, briefly, until there is one.
    let cache_cells = |app: &App| {
        app.world()
            .get_resource::<bevy::diagnostic::DiagnosticsStore>()
            .and_then(|store| {
                store
                    .iter()
                    .find(|d| d.path().as_str() == "render/solari_lighting/world_cache_active_cells_count")
                    .and_then(|d| d.value())
            })
    };
    let mut cells = cache_cells(&app);
    for _ in 0..30 {
        if cells.is_some() || !matches!(renderer, Renderer::Realtime { .. }) {
            break;
        }
        app.update();
        cells = cache_cells(&app);
    }
    *LAST_CACHE_CELLS.lock().unwrap_or_else(|e| e.into_inner()) = cells;
    (kept.iter().map(|bytes| decode(bytes)).collect(), stats)
}

/// The world cache's active cell count at the end of the last realtime run,
/// when it was recorded. A static rather than another return value, since
/// only one test reads it.
static LAST_CACHE_CELLS: Mutex<Option<f64>> = Mutex::new(None);

/// [`render_with`] without diagnostics.
fn render(renderer: Renderer, motion: Motion, frames: usize) -> (Frame, Frame) {
    let (previous, image, _) = render_with(renderer, motion, frames, None);
    (previous, image)
}

/// The render target as it stands after the frame just run, copied out and
/// waited for.
///
/// Bevy's `Readback` delivers frames later, more of them the bigger the image,
/// and a late-spawned one at 1280 x 1024 never arrived at all, so the frame it
/// scored was not reliably the one asked for. With pipelined rendering off the
/// frame's work is already submitted when `update` returns, so a copy queued
/// now sees exactly that frame.
fn grab(app: &mut App, target: &Handle<Image>) -> Vec<u8> {
    use bevy::render::render_asset::RenderAssets;
    use bevy::render::render_resource::{
        BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, MapMode, PollType,
        TexelCopyBufferInfo, TexelCopyBufferLayout,
    };
    use bevy::render::renderer::{RenderDevice, RenderQueue};
    use bevy::render::texture::GpuImage;

    let render = app.sub_app(bevy::render::RenderApp).world();
    let device = render.resource::<RenderDevice>();
    let queue = render.resource::<RenderQueue>();
    let image = render
        .resource::<RenderAssets<GpuImage>>()
        .get(target.id())
        .expect("the target is on the GPU");
    let (width, height) = size();
    // Eight bytes a pixel; `size` keeps a row a whole number of 256-byte rows.
    let row = width * 8;
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("denoise_quality_grab"),
        size: (row * height) as u64,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        image.texture.as_image_copy(),
        TexelCopyBufferInfo {
            buffer: &buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(MapMode::Read, |result| result.expect("the copy maps"));
    device.poll(PollType::wait_indefinitely()).expect("the GPU finishes the copy");
    let bytes = slice.get_mapped_range().expect("the copy is mapped").to_vec();
    buffer.unmap();
    bytes
}

/// RGBA half floats to RGB floats.
fn decode(bytes: &[u8]) -> Frame {
    let rgb = bytes
        .chunks_exact(8)
        .flat_map(|px| {
            (0..3).map(move |c| half_to_f32(u16::from_le_bytes([px[c * 2], px[c * 2 + 1]])))
        })
        .collect();
    Frame { rgb }
}

fn half_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = ((h >> 10) & 0x1F) as i32;
    let mantissa = (h & 0x3FF) as f32;
    match exponent {
        0 => sign * mantissa * 2f32.powi(-24),
        31 => f32::NAN,
        e => sign * (1.0 + mantissa / 1024.0) * 2f32.powi(e - 15),
    }
}

fn luminance(rgb: &[f32]) -> Vec<f32> {
    rgb.chunks_exact(3)
        .map(|p| 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2])
        .collect()
}

/// Box blur of a luminance image, `radius` pixels each way, clamped at edges.
fn blur(l: &[f32], radius: i32) -> Vec<f32> {
    let (w, h) = size();
    let (w, h) = (w as i32, h as i32);
    let mut out = vec![0.0; l.len()];
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0;
            let mut n = 0.0;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let (sx, sy) = (x + dx, y + dy);
                    if sx >= 0 && sy >= 0 && sx < w && sy < h {
                        sum += l[(sy * w + sx) as usize];
                        n += 1.0;
                    }
                }
            }
            out[(y * w + x) as usize] = sum / n;
        }
    }
    out
}

/// How an image compares with the reference.
#[derive(Debug)]
struct Score {
    /// Mean relative squared error against the reference, over the whole frame.
    /// Counts bias and noise alike.
    rel_mse: f64,
    /// Fine noise: the pixel-to-pixel variation the reference does not have,
    /// relative to brightness, on the smooth parts of the reference (flat
    /// walls and ceiling, away from edges and shadow boundaries). This is the
    /// grain the eye picks out.
    grain: f64,
    /// Mean relative change between the last two frames: flicker and crawl
    /// while the camera holds still.
    flicker: f64,
    /// [`rel_mse`](Self::rel_mse) over the pixels where the reference has
    /// detail rather than flat light: edges, contact shadows, reflections,
    /// wherever on screen they are. What a denoiser blurs away or smears.
    detail_rel_mse: f64,
}

fn score(image: &Frame, previous: &Frame, reference: &Frame) -> Score {
    let l = luminance(&image.rgb);
    let p = luminance(&previous.rgb);
    let r = luminance(&reference.rgb);
    let mean_r = r.iter().map(|&v| v as f64).sum::<f64>() / r.len() as f64;
    let floor = (0.02 * mean_r) as f32;

    // Errors are taken between 3x3 neighbourhood means, and the worst 0.1% of
    // pixels are left out. A reference of a few thousand samples is still
    // grainy pixel by pixel and keeps a scatter of unconverged fireflies (sun
    // glints off a glossy floor converge last); compared raw, its own noise
    // was most of every image's error and favoured whichever image blurred
    // most. Comparing neighbourhoods is also what the editor's error view does.
    let l_mid = blur(&l, 1);
    let r_mid = blur(&r, 1);
    let r_wide = blur(&r, 3);
    let l_wide = blur(&l, 3);
    let errors: Vec<f64> = l_mid
        .iter()
        .zip(&r_mid)
        .map(|(&a, &b)| {
            let d = (a - b) as f64;
            d * d / ((b as f64 + floor as f64).powi(2))
        })
        .collect();
    let mut sorted = errors.clone();
    sorted.sort_by(f64::total_cmp);
    let outlier = sorted[(sorted.len() * 999 / 1000).min(sorted.len() - 1)];

    let (mut sum, mut n) = (0.0, 0.0f64);
    let (mut detail_sum, mut detail_n) = (0.0, 0.0f64);
    let (mut grain_sum, mut grain_n) = (0.0, 0.0f64);
    for i in 0..l.len() {
        // Where the reference's light changes across a few pixels: an edge, a
        // shadow line, a reflection. Decided on the smoothed reference, so its
        // grain does not count as detail.
        let detail = (r_mid[i] - r_wide[i]).abs() > 0.05 * r_wide[i] + floor;
        if errors[i] <= outlier {
            sum += errors[i];
            n += 1.0;
            if detail {
                detail_sum += errors[i];
                detail_n += 1.0;
            }
        }
        // Grain: the image's pixel-to-pixel variation where the reference is
        // flat.
        if !detail && r_wide[i] > floor {
            let high = (l[i] - l_wide[i]) as f64;
            grain_sum += (high / (r_wide[i] as f64)).powi(2);
            grain_n += 1.0;
        }
    }
    let rel_mse = sum / n.max(1.0);
    let grain = (grain_sum / grain_n.max(1.0)).sqrt();
    let detail_rel_mse = detail_sum / detail_n.max(1.0);

    let flicker = l
        .iter()
        .zip(&p)
        .map(|(&a, &b)| ((a - b).abs() / (a.max(b) + floor)) as f64)
        .sum::<f64>()
        / l.len() as f64;

    Score { rel_mse, grain, flicker, detail_rel_mse }
}

/// A frame as a PPM, exposed and tonemapped for looking at.
fn write_ppm(name: &str, frame: &Frame) {
    let Ok(dir) = std::env::var("RENZORA_GPU_TEST_OUT") else { return };
    let (width, height) = size();
    let mut out = format!("P6\n{width} {height}\n255\n").into_bytes();
    for v in &frame.rgb {
        let mapped = (v / (1.0 + v)).max(0.0).powf(1.0 / 2.2);
        out.push((mapped * 255.0).round().clamp(0.0, 255.0) as u8);
    }
    std::fs::write(std::path::Path::new(&dir).join(format!("{name}.ppm")), out).unwrap();
}

/// The reference for the default pose. See [`reference_at`].
fn reference() -> Frame {
    reference_at(DEFAULT_POSE, REFERENCE_FRAMES)
}

/// The reference for a pose, cached on disk between runs when an output
/// directory is set: it takes far longer than everything else here. The cache
/// is named for the pose, the size and the sample count, so a change to any of
/// them makes a new one (a change to the scene does not: delete the cache).
fn reference_at(pose: Pose, samples: usize) -> Frame {
    let cache = std::env::var("RENZORA_GPU_TEST_OUT")
        .ok()
        .map(|dir| {
            let (w, h) = size();
            let key = [pose.eye, pose.target]
                .iter()
                .flat_map(|v| v.to_array())
                .fold(0u64, |h, f| h.wrapping_mul(1_000_003) ^ f.to_bits() as u64);
            std::path::Path::new(&dir).join(format!("reference_{w}x{h}_{samples}_{key:016x}.f32"))
        });
    if let Some(bytes) = cache.as_ref().and_then(|path| std::fs::read(path).ok()) {
        let rgb = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        return Frame { rgb };
    }
    let (_, frame, _) = render_at(pose, Renderer::Reference, Motion::Still, samples, None);
    if let Some(path) = cache {
        let bytes: Vec<u8> = frame.rgb.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(path, bytes).unwrap();
    }
    frame
}

#[test]
fn denoiser_quality() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());

    let reference = reference();
    write_ppm("reference", &reference);

    let mut raw_grain = 0.0;
    for (name, renderer, motion) in [
        ("raw_still", realtime(false, false), Motion::Still),
        ("denoised_still", realtime(true, false), Motion::Still),
        ("denoised_taa_still", realtime(true, true), Motion::Still),
        ("denoised_strafe", realtime(true, false), Motion::Strafe),
        ("denoised_taa_strafe", realtime(true, true), Motion::Strafe),
    ] {
        let (previous, image) = render(renderer, motion, REALTIME_FRAMES);
        write_ppm(name, &image);
        let s = report(name, &image, &previous, &reference);
        if name == "raw_still" {
            raw_grain = s.grain;
        } else {
            // Measured around 11 to 13 times less grain than raw at the
            // default size, and some 30 times at a viewport's; five is the
            // floor below which the image visibly fizzes.
            assert!(
                s.grain * 5.0 < raw_grain,
                "{name}: grain {:.4} is not a fifth of raw's {raw_grain:.4}",
                s.grain
            );
        }
    }
}

fn report(name: &str, image: &Frame, previous: &Frame, reference: &Frame) -> Score {
    let s = score(image, previous, reference);
    println!(
        "{name:20} rel_mse {:.4}  grain {:.4}  flicker {:.4}  detail {:.4}",
        s.rel_mse, s.grain, s.flicker, s.detail_rel_mse
    );
    s
}

/// The figures the denoiser is improved against: the editor's setup (TAA on,
/// so the camera jitters as it does in the viewport) standing still and moving,
/// from [`BENCHMARK_POSES`] random cameras, scored against each one's
/// reference, with the diagnostics' own history figures. Prints each pose and
/// the mean; asserts nothing, since it exists to compare one change with the
/// next. `RENZORA_GPU_TEST_OUT` caches the references, which are most of the
/// run's time.
#[test]
fn denoiser_benchmark() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut totals = [[0.0f64; 6]; 2];
    let poses = benchmark_poses(BENCHMARK_POSES);
    // `RENZORA_BENCH_POSE=<n>` runs one pose. Each renderer this test stands up
    // keeps GPU memory the driver frees only when the process ends, so all six
    // in one process can run out on a card the editor is also using.
    let only: Option<usize> = std::env::var("RENZORA_BENCH_POSE").ok().and_then(|v| v.parse().ok());
    for (i, &pose) in poses.iter().enumerate() {
        if only.is_some_and(|n| n != i) {
            continue;
        }
        let reference = reference_at(pose, BENCHMARK_REFERENCE_FRAMES);
        write_ppm(&format!("bench_{i}_reference"), &reference);
        // A random camera can land inside a piece of furniture and see only
        // its unlit inside; there is nothing to measure there.
        let mean = luminance(&reference.rgb).iter().sum::<f32>() / (reference.rgb.len() / 3) as f32;
        if mean < 1e-4 {
            println!("bench_{i} skipped: the camera sees nothing lit");
            continue;
        }
        for (m, (label, motion)) in [("still", Motion::Still), ("strafe", Motion::Strafe)].into_iter().enumerate() {
            let debug = SolariDebug {
                stats: true,
                ..default()
            };
            // `RENZORA_BENCH_RAW=1` measures Solari's raw estimate instead, to
            // tell what the denoiser adds from what it was given.
            // `RENZORA_BENCH_DLSS=1`, in a build with the `dlss` feature and
            // the NGX libraries on `PATH`, measures DLSS Ray Reconstruction
            // instead, the target the built-in denoiser is compared with.
            let denoise = std::env::var("RENZORA_BENCH_RAW").is_err();
            let dlss = if std::env::var("RENZORA_BENCH_DLSS").is_ok() { Dlss::On } else { Dlss::Off };
            let renderer = Renderer::Realtime { denoise, taa: true, dlss };
            let (previous, image, stats) = render_at(pose, renderer, motion, REALTIME_FRAMES, Some(debug));
            let suffix = match (denoise, dlss) {
                (_, Dlss::On) => "_dlss",
                (false, _) => "_raw",
                _ => "",
            };
            let name = format!("bench_{i}_{label}{suffix}");
            write_ppm(&name, &image);
            let s = report(&name, &image, &previous, &reference);
            let d = stats.unwrap();
            println!("{:20} history {:.1}  rejected {:.4}", "", d.mean_history, d.rejected_fraction);
            for (t, v) in totals[m].iter_mut().zip([
                s.rel_mse,
                s.grain,
                s.flicker,
                s.detail_rel_mse,
                d.mean_history as f64,
                d.rejected_fraction as f64,
            ]) {
                *t += v / poses.len() as f64;
            }
        }
    }
    for (label, t) in ["still", "strafe"].iter().zip(totals) {
        println!(
            "MEAN {label:7} rel_mse {:.4}  grain {:.4}  flicker {:.4}  detail {:.4}  history {:.1}  rejected {:.4}",
            t[0], t[1], t[2], t[3], t[4], t[5]
        );
    }
}

/// Cameras the benchmark averages over.
const BENCHMARK_POSES: usize = 8;

/// Samples per pixel in each benchmark reference: fewer than the regression
/// tests' single reference, since there are several. Its own leftover noise
/// adds the same amount to every run's error, so differences between runs
/// still mean what they say.
const BENCHMARK_REFERENCE_FRAMES: usize = 4096;

/// Switching DLSS Ray Reconstruction off hands the image back to the built-in
/// denoiser. It once did not: DLSS's guide textures outlived it on the view,
/// the denoiser took their presence to mean DLSS was still denoising, and with
/// neither running the raw image reached the screen.
///
/// Needs the `dlss` feature (and so the DLSS SDK to build) and a GPU, driver
/// and the NGX libraries on `PATH` that support Ray Reconstruction; skips
/// otherwise.
#[cfg(feature = "dlss")]
#[test]
fn switching_dlss_off_hands_back_to_the_denoiser() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let supported = app(realtime(true, false))
        .world()
        .contains_resource::<bevy::anti_alias::dlss::DlssRayReconstructionSupported>();
    if !supported {
        println!("DLSS Ray Reconstruction is not available here; skipped");
        return;
    }

    let reference = reference();
    let (_, raw) = render(realtime(false, false), Motion::Still, REALTIME_FRAMES);
    let raw = score(&raw, &raw, &reference);
    let renderer = Renderer::Realtime { denoise: true, taa: false, dlss: Dlss::ThenOff };
    let (previous, image) = render(renderer, Motion::Still, REALTIME_FRAMES);
    write_ppm("dlss_then_off", &image);
    let s = report("dlss_then_off", &image, &previous, &reference);
    assert!(
        s.grain * 5.0 < raw.grain,
        "after DLSS was switched off the image is undenoised: grain {:.4} against raw {:.4}",
        s.grain,
        raw.grain
    );
}

/// The diagnostics' figures describe the frame: every count adds up, and each
/// measure says what is known to be true of a still camera on this room.
#[test]
fn the_diagnostics_measure_the_frame() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());

    let debug = SolariDebug {
        stats: true,
        reference: true,
        ..default()
    };
    let (_, _, stats) = render_with(realtime(true, false), Motion::Still, REALTIME_FRAMES, Some(debug));
    let s = stats.unwrap();
    println!(
        "covered {} | rays: initial {} ({} shadow), reuse {} ({} shadow), world cache {} ({} shadow), debug {}",
        s.covered_pixels,
        s.rays_initial,
        s.shadow_rays_initial,
        s.rays_reuse,
        s.shadow_rays_reuse,
        s.rays_world_cache,
        s.shadow_rays_world_cache,
        s.rays_debug
    );
    println!(
        "history {:.1} ({:.3} short, {:.4} rejected) | confidence {:.2}/{} | grain {:.3} -> {:.3} | not traced {:.4} | reference {} samples, error {:?}",
        s.mean_history,
        s.short_history_fraction,
        s.rejected_fraction,
        s.mean_confidence,
        s.confidence_cap,
        s.grain_raw,
        s.grain_denoised,
        s.not_traced_fraction,
        s.reference_samples,
        s.reference_error
    );

    let (width, height) = size();
    let pixels = (width * height) as u64;
    // The window shows the sky, which Solari does not shade; everything else is
    // a surface.
    assert!(s.covered_pixels > pixels / 2 && s.covered_pixels <= pixels, "{} of {pixels} covered", s.covered_pixels);
    let total = |h: &[u32]| h.iter().map(|&b| b as u64).sum::<u64>();
    assert_eq!(total(&s.histogram_raw), s.covered_pixels, "every covered pixel lands in one raw bin");
    assert_eq!(total(&s.histogram_denoised), s.covered_pixels, "and in one denoised bin");

    assert!(s.rays_initial >= s.covered_pixels, "at least one ray per covered pixel");
    assert!(s.shadow_rays_initial <= s.rays_initial && s.shadow_rays_reuse <= s.rays_reuse);
    assert!(s.rays_reuse > 0, "ReSTIR's reuse traces visibility");
    assert!(s.rays_world_cache > 0, "the world cache updates");

    assert!(s.denoiser_active && s.restir);
    assert!(s.mean_history > 16.0, "a still camera keeps its history: {:.1} frames", s.mean_history);
    assert!(s.rejected_fraction < 0.05, "and throws little of it away: {:.3}", s.rejected_fraction);
    // The temporal history is capped, and the spatial merge adds a neighbour's
    // capped history and new sample on top.
    let max_confidence = 2.0 * s.confidence_cap + 1.0;
    assert!(
        s.mean_confidence >= 1.0 && s.mean_confidence <= max_confidence,
        "confidence {:.2} outside 1..{max_confidence}",
        s.mean_confidence
    );
    assert!(s.grain_denoised * 3.0 < s.grain_raw, "denoising measurably smooths: {:.3} -> {:.3}", s.grain_raw, s.grain_denoised);
    assert!(s.not_traced_fraction < 0.01, "every mesh in the room is traced: {:.4}", s.not_traced_fraction);
    assert!(s.reference_samples > 0 && s.reference_error.is_some());
}

/// The diagnostics' path-traced reference converges on the same image as
/// Solari's own offline path tracer: given as many samples, it is no further
/// from the long reference than the offline one is.
#[test]
fn the_diagnostics_reference_matches_the_offline_path_tracer() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    const SAMPLES: usize = 512;

    let long = reference();
    let (_, offline) = render(Renderer::Reference, Motion::Still, SAMPLES);
    write_ppm("offline_reference", &offline);
    let debug = SolariDebug {
        view: SolariDebugView::Reference,
        reference: true,
        ..default()
    };
    let (previous, ours, _) = render_with(realtime(true, false), Motion::Still, SAMPLES, Some(debug));
    write_ppm("diagnostics_reference", &ours);

    let offline = score(&offline, &offline, &long);
    let ours = report("diagnostics_reference", &ours, &previous, &long);
    println!("offline path tracer at {SAMPLES} samples: rel_mse {:.4}", offline.rel_mse);
    assert!(
        ours.rel_mse < offline.rel_mse * 1.5 + 0.01,
        "the diagnostics' reference is further from converged ({:.4}) than the offline path tracer at the same samples ({:.4})",
        ours.rel_mse,
        offline.rel_mse
    );
}

/// Every visualization runs without a GPU error and draws a real image.
#[test]
fn every_diagnostics_view_draws() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for view in [
        SolariDebugView::RawSplit,
        SolariDebugView::HistoryLength,
        SolariDebugView::Rejected,
        SolariDebugView::NoiseEstimate,
        SolariDebugView::ReservoirConfidence,
        SolariDebugView::WorldCache,
        SolariDebugView::NotTraced,
        SolariDebugView::Reference,
        SolariDebugView::ReferenceError,
    ] {
        let debug = SolariDebug {
            view,
            stats: true,
            reference: true,
            ..default()
        };
        let (_, image, _) = render_with(realtime(true, false), Motion::Still, 30, Some(debug));
        write_ppm(&format!("view_{view:?}"), &image);
        let finite = image.rgb.iter().all(|v| v.is_finite());
        let lit = image.rgb.iter().filter(|&&v| v > 0.0).count();
        assert!(finite, "{view:?} drew a non-finite value");
        assert!(lit > image.rgb.len() / 4, "{view:?} drew a mostly black image");
    }
}

/// Glass reflects the scene, not only the sky. Blended surfaces were once left
/// to Bevy's forward pass, which reflects nothing but the environment map, so a
/// car's windows and clear coat showed sky where the ground and the buildings
/// beside them should have been.
///
/// A glass wall stands on a grey ground with a box out in front of it. Each
/// seeded camera looks at a random point on the glass at a random angle, and
/// the box is placed where that point's mirror ray goes, so the middle of the
/// image is the box's reflection wherever the camera is. The scene is drawn
/// twice, with a red box and a green one; the box is never in view directly,
/// so the colour can only reach the middle of the image by being reflected.
#[test]
fn glass_reflects_the_scene() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());

    let mut state = 0x1B873593u32;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    for i in 0..GLASS_POSES {
        // A point on the glass (the plane x = 0, facing +x), seen from 2 to 3 m
        // away at 30 to 60 degrees off the normal: steep enough that the
        // reflected ray leaves well clear of the camera, shallow enough that
        // the glass is not edge-on.
        // Mostly sideways, a little up or down, so the box (four metres out
        // along the mirror ray) stays above the ground.
        let target = Vec3::new(0.0, 1.5 + 1.5 * random(), -1.5 + 3.0 * random());
        let off_normal = (30.0 + 30.0 * random()).to_radians();
        let around = std::f32::consts::TAU * random();
        let towards_eye = Vec3::new(
            off_normal.cos(),
            off_normal.sin() * around.sin() * 0.25,
            off_normal.sin() * around.cos(),
        )
        .normalize();
        let pose = Pose {
            eye: target + towards_eye * (2.0 + random()),
            target,
        };
        let mirrored = Vec3::new(towards_eye.x, -towards_eye.y, -towards_eye.z);
        let box_at = target + mirrored * 4.0;

        let centre = |colour: Color| {
            let mut scene = |world: &mut World| glass_scene(world, box_at, colour, Some(window_glass()));
            let (_, image, _) = render_in(&mut scene, pose, realtime(true, false), Motion::Still, 60, None);
            write_ppm(&format!("glass_{i}_{:?}", colour.to_srgba().red > 0.5), &image);
            centre_mean(&image)
        };
        let red = centre(Color::srgb(1.0, 0.02, 0.02));
        let green = centre(Color::srgb(0.02, 1.0, 0.02));
        // The box's colour in the reflection: how much redder the middle is
        // with the red box than with the green one, against how bright it is.
        let shift = ((red[0] - red[1]) - (green[0] - green[1])) / (red.iter().sum::<f32>() / 3.0).max(1e-6);
        println!("glass_{i}: red {red:?}  green {green:?}  colour shift {shift:.3}");
        assert!(
            shift > 0.05,
            "pose {i}: the glass does not reflect the box beside it (colour shift {shift:.3}); \
             it is reflecting the sky instead of the scene"
        );
    }
}

/// Seeded cameras [`glass_reflects_the_scene`] checks.
const GLASS_POSES: usize = 3;

/// A reflection is as bright as what it reflects. A see-through mirror (fully
/// opaque, metallic, white, smooth) is drawn reflecting the box, and the box is
/// drawn again, with no mirror, by a camera standing where the mirror image of
/// the first camera stands: what an ideal mirror shows is exactly that view.
/// The box's colour in the middle of each image should match.
///
/// The reflection once came out over three times too bright, because the
/// world cache's light (what arrives at a surface) was taken for what leaves
/// it, without the 1/pi a diffuse surface sends towards any one direction: car
/// paint under a clear coat read as chrome. [`glass_reflects_the_scene`] could
/// not see that, since it asks only whether the box is reflected at all.
#[test]
fn a_mirror_reflection_is_as_bright_as_what_it_reflects() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());

    let mut state = 0x85EBCA6Bu32;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    for i in 0..MIRROR_POSES {
        // As in `glass_reflects_the_scene`.
        let target = Vec3::new(0.0, 1.5 + 1.5 * random(), -1.5 + 3.0 * random());
        let off_normal = (30.0 + 30.0 * random()).to_radians();
        let around = std::f32::consts::TAU * random();
        let towards_eye = Vec3::new(
            off_normal.cos(),
            off_normal.sin() * around.sin() * 0.25,
            off_normal.sin() * around.cos(),
        )
        .normalize();
        let eye = target + towards_eye * (2.0 + random());
        let box_at = target + Vec3::new(towards_eye.x, -towards_eye.y, -towards_eye.z) * 4.0;

        // The box's colour at the middle of the image: how much redder it is
        // with a red box than with a green one, which cancels everything else
        // in view (sky, ground, highlights).
        let colour_at_centre = |pose: Pose, mirror: bool| {
            let mut shift = 0.0;
            for (sign, colour) in [(1.0, Color::srgb(1.0, 0.02, 0.02)), (-1.0, Color::srgb(0.02, 1.0, 0.02))] {
                let pane = mirror.then(mirror_glass);
                let mut scene = |world: &mut World| glass_scene(world, box_at, colour, pane.clone());
                let (_, image, _) = render_in(&mut scene, pose, realtime(true, false), Motion::Still, 60, None);
                write_ppm(&format!("mirror_{i}_{mirror}_{}", sign > 0.0), &image);
                let c = centre_mean(&image);
                shift += sign * (c[0] - c[1]);
            }
            shift
        };
        let reflected = colour_at_centre(Pose { eye, target }, true);
        let mirrored_eye = Vec3::new(-eye.x, eye.y, eye.z);
        let direct = colour_at_centre(Pose { eye: mirrored_eye, target }, false);
        let ratio = reflected / direct.max(1e-6);
        println!("mirror_{i}: reflected {reflected:.4}  seen directly {direct:.4}  ratio {ratio:.3}");
        // The reflection lights the box from Solari's world cache, a coarser
        // estimate than the direct view's, so it is not exact; a factor of pi
        // either way is what this exists to catch.
        assert!(
            (0.6..1.6).contains(&ratio),
            "pose {i}: the mirror shows the box {ratio:.2} times as bright as it is"
        );
    }
}

/// Seeded cameras [`a_mirror_reflection_is_as_bright_as_what_it_reflects`]
/// checks. Each takes four renders.
const MIRROR_POSES: usize = 2;

/// Every see-through layer along a ray is drawn, not only the nearest. Two
/// panes of half-opaque black glass stand between the camera and a box, so a
/// quarter of the box's colour should come through; drawing the nearest pane
/// alone lets half through. A car's headlight (clear lens, tinted lens,
/// reflector) once showed as a black hole for want of this.
#[test]
fn every_glass_layer_along_a_ray_is_drawn() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());

    let pose = Pose {
        eye: Vec3::new(4.0, 1.6, 0.3),
        target: Vec3::new(-2.0, 1.0, 0.0),
    };
    // The box's colour at the middle of the image, as a red box against a
    // green one, which cancels the panes' own reflections and tint.
    let colour_at_centre = |panes: usize, facing_camera: bool| {
        let mut shift = 0.0;
        for (sign, colour) in [(1.0, Color::srgb(1.0, 0.02, 0.02)), (-1.0, Color::srgb(0.02, 1.0, 0.02))] {
            let mut scene = |world: &mut World| layered_glass_scene(world, colour, panes, facing_camera);
            let (_, image, _) = render_in(&mut scene, pose, realtime(true, false), Motion::Still, 60, None);
            write_ppm(&format!("layers_{panes}_{facing_camera}_{}", sign > 0.0), &image);
            let c = centre_mean(&image);
            shift += sign * (c[0] - c[1]);
        }
        shift
    };
    let bare = colour_at_centre(0, true);
    let through_two = colour_at_centre(2, true);
    let ratio = through_two / bare.max(1e-6);
    println!("layers: bare {bare:.4}  through two panes {through_two:.4}  ratio {ratio:.3}");
    assert!(
        (0.15..0.35).contains(&ratio),
        "{ratio:.2} of the box shows through two half-opaque panes, not a quarter: \
         a layer behind the nearest is not drawn"
    );

    // The same panes turned to face away: single-sided, as a StandardMaterial
    // is by default, so a raster draw culls them and so must this pass.
    let behind_backs = colour_at_centre(2, false);
    let ratio = behind_backs / bare.max(1e-6);
    println!("layers: through two culled back faces {behind_backs:.4}  ratio {ratio:.3}");
    assert!(
        ratio > 0.9,
        "a single-sided pane seen from behind still tints what is behind it ({ratio:.2} shows through)"
    );
}

/// A glossy see-through surface stays steady while the camera moves. The pass
/// that lights glass has no denoiser after it, and lights what it reflects from
/// Solari's world cache, whose cells a moving camera keeps creating; read
/// evenly, the young ones (a single random bounce ray each) showed as splotches
/// crawling over a car's clear coat. Measured as the frame-to-frame change over
/// the pane, which a strafing camera barely moves across.
#[test]
fn glossy_glass_is_steady_while_the_camera_moves() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    // A car's clear coat: a dark, metallic, slightly rough shell, half opaque.
    let coat = StandardMaterial {
        base_color: Color::srgba(0.15, 0.15, 0.15, 0.46),
        metallic: 1.0,
        perceptual_roughness: 0.2,
        alpha_mode: AlphaMode::Blend,
        ..default()
    };
    let pose = Pose {
        eye: Vec3::new(3.0, 2.0, 1.5),
        target: Vec3::new(0.0, 2.0, 0.0),
    };
    let mut scene = |world: &mut World| glass_scene(world, Vec3::new(3.0, 1.0, -3.0), Color::srgb(0.8, 0.3, 0.1), Some(coat.clone()));
    let (previous, image, _) = render_in(&mut scene, pose, realtime(true, false), Motion::Strafe, 60, None);
    write_ppm("steady_glass", &image);
    let flicker = centre_flicker(&previous, &image);
    println!("glossy glass, strafing: frame-to-frame change {flicker:.4}");
    assert!(
        flicker < STEADY_GLASS_FLICKER,
        "glossy glass changes by {flicker:.3} a frame while the camera strafes: its lighting is splotchy"
    );
}

/// A sheer curtain is steady and as bright as its alpha says: a white, fully
/// rough sheet at 30% alpha over a black wall comes out at about 30% of the
/// same sheet made opaque, and the same from one still frame to the next. Its
/// tint was lit from the world cache at its own position, where nothing opaque
/// ever leaves a cell, and otherwise from one random light sample a frame with
/// no sky in it: sheer curtains in a window came and went from frame to frame.
#[test]
fn a_sheer_curtain_is_steady_and_lit() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (ratio, flicker) = sheer_curtain(realtime(true, false), "");
    assert!((0.2..0.45).contains(&ratio), "a sheer curtain shows at {ratio:.3} of the opaque one, expected about 0.3");
    assert!(flicker < SHEER_CURTAIN_FLICKER, "a still sheer curtain changes by {flicker:.3} a frame");
}

/// A sheer curtain shows under DLSS Ray Reconstruction as it does under the
/// built-in denoiser, over a wall and in a window. Ray Reconstruction denoises
/// an image the curtain is already drawn into, from guide buffers that
/// describe only what is behind it, so a faint layer could read to it as
/// noise; this holds it to showing the curtain.
///
/// Needs the `dlss` feature and Ray Reconstruction support, like
/// `switching_dlss_off_hands_back_to_the_denoiser`; skips otherwise.
#[cfg(feature = "dlss")]
#[test]
fn a_sheer_curtain_shows_under_dlss() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let supported = app(realtime(true, false))
        .world()
        .contains_resource::<bevy::anti_alias::dlss::DlssRayReconstructionSupported>();
    if !supported {
        println!("DLSS Ray Reconstruction is not available here; skipped");
        return;
    }
    let renderer = Renderer::Realtime { denoise: true, taa: false, dlss: Dlss::On };
    let (ratio, _) = sheer_curtain(renderer, "_dlss");
    assert!(
        (0.2..0.45).contains(&ratio),
        "under DLSS a sheer curtain shows at {ratio:.3} of the opaque one, expected about 0.3"
    );
    let backlit = backlit_sheer(renderer, "_dlss");
    assert!(
        (BACKLIT_CURTAIN_MIN..0.99).contains(&backlit),
        "under DLSS a sheer curtain in a window shows at {backlit:.3} of the open view, expected about 0.92"
    );
}

/// A sunlit sheer curtain is smooth with lamps elsewhere in the scene. Its
/// light samples each picked one light from the whole list, and every point
/// or spot light is an emissive stand-in in that list, so with seven lamps a
/// sample found the sun one time in eight, weighted eightfold. The samples
/// are fixed per point on the surface, so the points whose samples happened
/// to find the sun stayed several times brighter than their neighbours: an
/// apartment's sheers were covered in still white specks under the built-in
/// denoiser. Now every directional light is lit explicitly.
#[test]
fn a_sunlit_sheer_curtain_is_not_speckled_by_lamps() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let pose = Pose {
        eye: Vec3::new(3.0, 2.0, 0.0),
        target: Vec3::new(0.0, 2.0, 0.0),
    };
    let sheet = StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.3),
        perceptual_roughness: 1.0,
        alpha_mode: AlphaMode::Blend,
        ..default()
    };
    let mut scene = |world: &mut World| {
        glass_scene(world, Vec3::new(0.0, -20.0, 0.0), Color::BLACK, Some(sheet.clone()));
        // Small emissive boxes, which is what a lamp is to Solari (the editor
        // stands each point light in as an emissive sphere; this harness runs
        // Solari directly, without that). Far enough behind the camera that
        // they light the sheet very little: they matter here only as entries
        // in the list of lights.
        let lamp = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(2000.0, 1800.0, 1500.0),
            ..default()
        });
        for i in 0..SPECKLE_LAMPS {
            traced_box(world, Vec3::splat(0.05), Vec3::new(25.0, 3.0, i as f32 * 3.0 - 9.0), &lamp);
        }
    };
    let (_, image, _) = render_in(&mut scene, pose, realtime(true, false), Motion::Still, 60, None);
    write_ppm("sheer_with_lamps", &image);
    let specks = speck_fraction(&image);
    println!("sunlit sheer with {SPECKLE_LAMPS} lamps: {:.3}% of its pixels are specks", specks * 100.0);
    assert!(specks < SHEER_SPECKS, "{:.2}% of a sunlit sheer curtain is white specks", specks * 100.0);
}

/// The lamps of [`a_sunlit_sheer_curtain_is_not_speckled_by_lamps`].
const SPECKLE_LAMPS: u32 = 7;

/// Most of the sheer [`a_sunlit_sheer_curtain_is_not_speckled_by_lamps`] lets
/// be specks.
const SHEER_SPECKS: f32 = 0.002;

/// The share of the middle half of the image (both ways) brighter than one and
/// a half times the median of its 5 x 5 neighbourhood: still specks, which a
/// smooth surface has none of.
fn speck_fraction(image: &Frame) -> f32 {
    let (w, h) = size();
    let lum = luminance(&image.rgb);
    let (mut specks, mut n) = (0usize, 0usize);
    let mut window = Vec::with_capacity(25);
    for y in h / 4..h * 3 / 4 {
        for x in w / 4..w * 3 / 4 {
            window.clear();
            for dy in -2i32..=2 {
                for dx in -2i32..=2 {
                    window.push(lum[((y as i32 + dy) as u32 * w + (x as i32 + dx) as u32) as usize]);
                }
            }
            window.sort_by(f32::total_cmp);
            if lum[(y * w + x) as usize] > 1.5 * window[12] {
                specks += 1;
            }
            n += 1;
        }
    }
    specks as f32 / n as f32
}

/// A white, fully rough 30% sheet over a black wall, rendered by `renderer`
/// as it is and made opaque: its centre's brightness as a share of the opaque
/// one's, and how much the sheer one changes between the last two frames.
fn sheer_curtain(renderer: Renderer, tag: &str) -> (f32, f32) {
    let pose = Pose {
        eye: Vec3::new(3.0, 2.0, 0.0),
        target: Vec3::new(0.0, 2.0, 0.0),
    };
    let render = |alpha_mode: AlphaMode| {
        let sheet = StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 0.3),
            perceptual_roughness: 1.0,
            alpha_mode,
            ..default()
        };
        let mut scene = |world: &mut World| {
            glass_scene(world, Vec3::new(0.0, -20.0, 0.0), Color::BLACK, Some(sheet.clone()));
            // The opaque sheet is the rasterized G-buffer surface Solari lights
            // itself, so it must not carry the see-through marker, which takes
            // a mesh out of the raster passes.
            if alpha_mode == AlphaMode::Opaque {
                let marked: Vec<Entity> = world
                    .query_filtered::<Entity, With<renzora::SolariShadedTransparent>>()
                    .iter(world)
                    .collect();
                for entity in marked {
                    world.entity_mut(entity).remove::<renzora::SolariShadedTransparent>();
                }
            }
        };
        let (previous, image, _) = render_in(&mut scene, pose, renderer, Motion::Still, 60, None);
        write_ppm(&format!("curtain_{alpha_mode:?}{tag}"), &image);
        let [r, g, b] = centre_mean(&image);
        ((r + g + b) / 3.0, centre_flicker(&previous, &image))
    };
    let (opaque, _) = render(AlphaMode::Opaque);
    let (sheer, flicker) = render(AlphaMode::Blend);
    let ratio = sheer / opaque.max(1e-9);
    println!("sheer curtain{tag}: {sheer:.4} against opaque {opaque:.4}, {ratio:.3} of it, frame-to-frame change {flicker:.4}");
    (ratio, flicker)
}

/// A sheer curtain glows with the light behind it. The camera looks out of a
/// black box through its open end, which a white 30% sheet covers, at the sky
/// and the ground: the sheet's camera side sees only the box, so all its light
/// comes through the weave from behind. Lit from its camera side alone it
/// came out black, so it only dimmed the view by its alpha and read as a
/// slightly darker window rather than as a curtain. Measured against the same
/// view with the sheet hidden.
#[test]
fn a_sheer_curtain_glows_with_the_light_behind_it() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let ratio = backlit_sheer(realtime(true, false), "");
    // Unlit, the sheet lets 70% through (measured 0.700); painted over by the
    // sky, which drew after this pass, it vanished (measured 1.000).
    assert!(ratio > BACKLIT_CURTAIN_MIN, "a backlit sheer curtain shows at {ratio:.3} of the open view: it is not lit from behind");
    assert!(ratio < 0.99, "a sheer curtain in front of the sky is not drawn at all ({ratio:.3})");
}

/// The black box of [`a_sheer_curtain_glows_with_the_light_behind_it`], open
/// at one end onto the sky and the sunlit ground, rendered by `renderer` with
/// and without a white 30% sheet over the opening: the centre's brightness
/// with the sheet as a share of it without.
fn backlit_sheer(renderer: Renderer, tag: &str) -> f32 {
    let pose = Pose {
        eye: Vec3::new(3.5, 1.5, 0.0),
        target: Vec3::new(0.0, 1.5, 0.0),
    };
    let render = |with_sheet: bool| {
        let mut scene = |world: &mut World| {
            let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
            let ground = materials.add(plain(Color::srgb(0.4, 0.4, 0.4), 0.9));
            let black = materials.add(plain(Color::BLACK, 1.0));
            let sheet = materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 1.0, 1.0, 0.3),
                perceptual_roughness: 1.0,
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
            traced_box(world, Vec3::new(40.0, 0.2, 40.0), Vec3::new(0.0, -0.1, 0.0), &ground);
            // The box, x 0.1..5, y 0.1..3, z -2..2, open at x = 0.1.
            traced_box(world, Vec3::new(5.0, 0.1, 4.2), Vec3::new(2.6, 0.05, 0.0), &black);
            traced_box(world, Vec3::new(5.0, 0.1, 4.2), Vec3::new(2.6, 3.05, 0.0), &black);
            traced_box(world, Vec3::new(5.0, 3.1, 0.1), Vec3::new(2.6, 1.55, -2.05), &black);
            traced_box(world, Vec3::new(5.0, 3.1, 0.1), Vec3::new(2.6, 1.55, 2.05), &black);
            traced_box(world, Vec3::new(0.1, 3.1, 4.2), Vec3::new(5.15, 1.55, 0.0), &black);
            // Solari lights nothing in a scene with no light at all; a sun
            // behind the sheet, which the box keeps off its camera side.
            world.spawn((
                DirectionalLight {
                    illuminance: 10_000.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(-6.0, 4.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
            ));
            if with_sheet {
                let pane = traced_box(world, Vec3::new(0.02, 2.9, 4.0), Vec3::new(0.0, 1.55, 0.0), &sheet);
                world.entity_mut(pane).insert(renzora::SolariShadedTransparent::default());
            }
        };
        let (_, image, _) = render_in(&mut scene, pose, renderer, Motion::Still, 60, None);
        write_ppm(&format!("backlit_curtain_{with_sheet}{tag}"), &image);
        let [r, g, b] = centre_mean(&image);
        (r + g + b) / 3.0
    };
    let open = render(false);
    let curtained = render(true);
    let ratio = curtained / open.max(1e-9);
    println!("backlit sheer curtain{tag}: {curtained:.4} against the open view {open:.4}, ratio {ratio:.3}");
    ratio
}

/// Least [`a_sheer_curtain_glows_with_the_light_behind_it`] accepts: measured
/// 0.924.
const BACKLIT_CURTAIN_MIN: f32 = 0.78;

/// Most frame-to-frame change [`a_sheer_curtain_is_steady_and_lit`] accepts.
const SHEER_CURTAIN_FLICKER: f32 = 0.05;

/// Glass does not crowd the world cache. The see-through pass reads the cache
/// at every point it lights, several times over; when each of those reads made
/// a cell, the Bistro's cache ran to 95% of its capacity, lookups collided, and
/// the cache's per-frame update budget spread so thin that the opaque image's
/// bounce light went blotchy everywhere, a pavement nowhere near any glass
/// included. Measured as the cell count with a glossy see-through pane in view
/// against the same pane opaque, which the opaque path lights from cells of its
/// own.
#[test]
fn glass_does_not_crowd_the_world_cache() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let pose = Pose {
        eye: Vec3::new(3.0, 2.0, 1.5),
        target: Vec3::new(0.0, 2.0, 0.0),
    };
    let cells = |alpha_mode: AlphaMode| {
        let pane = StandardMaterial {
            base_color: Color::srgba(0.15, 0.15, 0.15, 0.46),
            metallic: 1.0,
            perceptual_roughness: 0.2,
            alpha_mode,
            ..default()
        };
        let mut scene = |world: &mut World| glass_scene(world, Vec3::new(3.0, 1.0, -3.0), Color::srgb(0.8, 0.3, 0.1), Some(pane.clone()));
        render_in(&mut scene, pose, realtime(true, false), Motion::Strafe, 60, None);
        LAST_CACHE_CELLS.lock().unwrap_or_else(|e| e.into_inner()).expect("the cell count is recorded")
    };
    let opaque = cells(AlphaMode::Opaque);
    let see_through = cells(AlphaMode::Blend);
    let ratio = see_through / opaque.max(1.0);
    println!("world cache cells: opaque pane {opaque:.0}, see-through {see_through:.0}, ratio {ratio:.2}");
    // Measured 1.24: the opaque image sees the wall behind the glass, which
    // has cells of its own. Filling the cache from the see-through pass took it
    // past 1.5.
    assert!(ratio < 1.35, "a see-through pane multiplies the world cache's cells by {ratio:.2}");
}

/// The world cache stays well short of full in a scene far larger than the
/// cache could hold at full detail. Its lookups give up after three probes, so
/// a nearly full cache fails most lookups for a new cell, and each failure is
/// a spot of missing bounce light: in the Bistro the cache crept to 95% over a
/// minute with the camera standing still, and the pavement went to dark,
/// crawling patches as soon as the camera moved. The scene is a town of
/// blocks around a street, seen from the street, and the count is taken after
/// long enough for the cache to stop growing.
#[test]
fn the_world_cache_never_fills_in_a_large_scene() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let pose = Pose {
        eye: Vec3::new(0.0, 1.7, 0.0),
        target: Vec3::new(8.0, 1.5, -20.0),
    };
    let debug = SolariDebug {
        view: SolariDebugView::WorldCache,
        ..default()
    };
    let (_, image, _) = render_in(&mut town, pose, realtime(true, false), Motion::Still, LARGE_SCENE_FRAMES, Some(debug));
    let cells = LAST_CACHE_CELLS.lock().unwrap_or_else(|e| e.into_inner()).expect("the cell count is recorded");
    let full = cells / WORLD_CACHE_CAPACITY;
    // A lookup that found no cell, and could not make one, reads exactly zero
    // in the World cache view: a hole of missing bounce light.
    let holes = image.rgb.chunks_exact(3).filter(|p| p.iter().all(|&c| c == 0.0)).count() as f32
        / (image.rgb.len() / 3) as f32;
    write_ppm("large_scene_world_cache", &image);
    println!(
        "world cache cells after {LARGE_SCENE_FRAMES} frames: {cells:.0} ({:.0}% full), holes {:.2}% of pixels",
        full * 100.0,
        holes * 100.0
    );
    assert!(full < 0.6, "the world cache is {:.0}% full", full * 100.0);
    // Only a real viewport's worth of pixels fills the cache enough for
    // lookups to fail: at the default test size the few zero pixels are the
    // same with three probe steps as with eight, so they are not holes.
    if size().0 >= 1280 {
        assert!(holes < LARGE_SCENE_HOLES, "{:.2}% of the world cache view is holes", holes * 100.0);
    }
}

/// Most of [`the_world_cache_never_fills_in_a_large_scene`]'s view that may
/// read as holes, as a fraction: measured at 1920 x 1088
/// (`RENZORA_SOLARI_TEST_SIZE`), 0.04% with eight probe steps, 1.56% with the
/// three upstream used.
const LARGE_SCENE_HOLES: f32 = 0.005;

/// The world cache holds steady in a room lit only by sky through slits in
/// a shutter, with the camera still. Most of its cells see that light only
/// when a bounce ray happens to leave through a slit, so their samples swing
/// between nothing and a lot, and the cache wobbles about three times as much
/// as in sunlight. Measured on the World cache view, blurred so per-pixel
/// jitter does not count, as how much each spot's light varies over two
/// seconds against how bright it is.
///
/// Tried and reverted on 2026-10-27: telling a cell's noise from a change in
/// its lighting, so noisy cells keep their full history. The cache wobbled
/// less (0.177 to 0.158) but the denoised image moved further from the
/// reference ([`a_dim_room_matches_the_reference`], 0.1225 to 0.1415), so a
/// steadier cache alone is not what a dim room needs.
#[test]
fn the_world_cache_holds_steady_in_low_light() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let cache = Some(SolariDebugView::WorldCache);
    let (wobble, brightness) = wobble_over_time(&mut sky_lit_room, cache);
    let (sunlit_wobble, sunlit_brightness) = wobble_over_time(&mut room, cache);
    let (image_wobble, image_brightness) = wobble_over_time(&mut sky_lit_room, None);
    println!("world cache in a sky-lit room: wobble {wobble:.3}, mean brightness {brightness:.4}");
    println!("world cache in the sunlit room: wobble {sunlit_wobble:.3}, mean brightness {sunlit_brightness:.4}");
    println!("denoised image of the sky-lit room: wobble {image_wobble:.3}, mean brightness {image_brightness:.5}");
    assert!(wobble < LOW_LIGHT_WOBBLE, "the world cache wobbles by {wobble:.3} in low light");
}

/// A room lit only by sky through slits comes out close to the path-traced
/// reference: the measure of low light that counts, since it is the image
/// that is seen and not the cache behind it.
#[test]
fn a_dim_room_matches_the_reference() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let reference = dim_room_reference();
    let (previous, image, _) = render_in(&mut sky_lit_room, DEFAULT_POSE, realtime(true, false), Motion::Still, REALTIME_FRAMES, None);
    let score = score(&image, &previous, &reference);
    let brightness = mean(&luminance(&image.rgb)) / mean(&luminance(&reference.rgb)).max(1e-9);
    println!(
        "dim room against the reference: rel_mse {:.4}, grain {:.4}, brightness {:.3} of the reference",
        score.rel_mse, score.grain, brightness
    );
    write_ppm("dim_room", &image);
    write_ppm("dim_room_reference", &reference);
    assert!(score.rel_mse < DIM_ROOM_REL_MSE, "the dim room's error against the reference is {:.4}", score.rel_mse);
}

/// A reflection on a glossy floor does not smear while the camera moves. A
/// metal floor reflects a bright panel and a row of posts; the camera strafes
/// past and the last frame is scored against the path-traced reference at
/// that pose, and against a still camera's last frame there. A reflection
/// moves across its surface as the camera does, unlike the surface itself, so
/// history carried forward by the surface's motion lags behind it: the
/// built-in denoiser dragged a glossy floor's reflections as smears that only
/// settled once the camera stopped, which DLSS, given the reflections' own
/// motion, did not.
#[test]
fn a_glossy_reflection_does_not_smear_while_moving() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let pose = Pose {
        eye: Vec3::new(0.0, 1.3, 3.0),
        target: Vec3::new(0.0, 0.2, -1.5),
    };
    let reference = {
        let (_, frame, _) = render_in(&mut glossy_floor, pose, Renderer::Reference, Motion::Still, GLOSSY_REFERENCE_FRAMES, None);
        frame
    };
    let (still_previous, still, _) = render_in(&mut glossy_floor, pose, realtime(true, true), Motion::Still, REALTIME_FRAMES, None);
    let (moving_previous, moving, _) = render_in(&mut glossy_floor, pose, realtime(true, true), Motion::Strafe, REALTIME_FRAMES, None);
    write_ppm("glossy_reference", &reference);
    write_ppm("glossy_still", &still);
    write_ppm("glossy_moving", &moving);
    let still_score = score(&still, &still_previous, &reference);
    let moving_score = score(&moving, &moving_previous, &reference);
    println!(
        "glossy floor: still rel_mse {:.4} detail {:.4}, moving rel_mse {:.4} detail {:.4}",
        still_score.rel_mse, still_score.detail_rel_mse, moving_score.rel_mse, moving_score.detail_rel_mse
    );
    assert!(
        moving_score.detail_rel_mse < GLOSSY_SMEAR_LIMIT * still_score.detail_rel_mse.max(0.005),
        "a glossy floor's reflections are {:.1}x as far from the reference while the camera moves",
        moving_score.detail_rel_mse / still_score.detail_rel_mse.max(0.005)
    );
}

/// How much further from the reference a moving glossy floor's reflections
/// may be than a still one's: measured 1.7 (0.1317 against 0.0786), the
/// smear still there. Following each reflection by the point it shows, and
/// choosing per pixel between that and the surface's motion by which agreed
/// with the frame's own samples, changed nothing measurable (2026-10-27) and
/// was taken out.
const GLOSSY_SMEAR_LIMIT: f64 = 2.0;

/// Samples [`a_glossy_reflection_does_not_smear_while_moving`]'s reference takes.
const GLOSSY_REFERENCE_FRAMES: usize = 4096;

/// A glossy metal floor, a sunlit white panel on a dark wall behind it and a
/// row of dark posts in front of the panel, all of whose reflections the floor
/// shows.
fn glossy_floor(world: &mut World) {
    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
    let floor = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.6, 0.6),
        metallic: 1.0,
        perceptual_roughness: 0.2,
        ..default()
    });
    // Sunlit white on a dark wall rather than emissive: Solari adds a
    // directly seen emissive surface after exposure and the reference before,
    // so an emissive panel would not compare.
    let panel = materials.add(plain(Color::srgb(0.95, 0.95, 0.95), 0.9));
    let post = materials.add(plain(Color::srgb(0.08, 0.08, 0.1), 0.8));
    let wall = materials.add(plain(Color::srgb(0.06, 0.06, 0.07), 0.9));
    traced_box(world, Vec3::new(12.0, 0.2, 12.0), Vec3::new(0.0, -0.1, 0.0), &floor);
    traced_box(world, Vec3::new(12.0, 4.0, 0.2), Vec3::new(0.0, 2.0, -4.0), &wall);
    traced_box(world, Vec3::new(3.0, 1.5, 0.1), Vec3::new(0.0, 1.6, -3.85), &panel);
    for i in 0..5 {
        let x = -1.6 + 0.8 * i as f32;
        traced_box(world, Vec3::new(0.12, 2.4, 0.12), Vec3::new(x, 1.2, -3.2), &post);
    }
    world.spawn((
        DirectionalLight {
            illuminance: 20_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-3.0, 6.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Most error [`a_dim_room_matches_the_reference`] accepts: measured 0.1225.
const DIM_ROOM_REL_MSE: f64 = 0.14;

/// Samples the dim room's reference takes: its light comes through slits,
/// which the path tracer finds as rarely as the realtime path does.
const DIM_ROOM_REFERENCE_FRAMES: usize = 16384;

/// The path-traced reference of [`sky_lit_room`] from [`DEFAULT_POSE`], kept
/// in `RENZORA_GPU_TEST_OUT` when it is set, since it takes minutes.
fn dim_room_reference() -> Frame {
    let (w, h) = size();
    let cache = std::env::var("RENZORA_GPU_TEST_OUT")
        .ok()
        .map(|dir| std::path::Path::new(&dir).join(format!("reference_dim_room_{w}x{h}_{DIM_ROOM_REFERENCE_FRAMES}.f32")));
    if let Some(bytes) = cache.as_ref().and_then(|path| std::fs::read(path).ok()) {
        let rgb = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
        return Frame { rgb };
    }
    let (_, frame, _) = render_in(&mut sky_lit_room, DEFAULT_POSE, Renderer::Reference, Motion::Still, DIM_ROOM_REFERENCE_FRAMES, None);
    if let Some(path) = cache {
        let bytes: Vec<u8> = frame.rgb.iter().flat_map(|v| v.to_le_bytes()).collect();
        std::fs::write(path, bytes).unwrap();
    }
    frame
}

/// Most wobble [`the_world_cache_holds_steady_in_low_light`] accepts: measured
/// 0.177.
const LOW_LIGHT_WOBBLE: f32 = 0.22;

/// The world cache follows a real change in the lighting promptly: any
/// attempt to steady it against noise must not make it slow to notice the sun
/// going out. Keeping every cell's full history took this to 120 frames. Counted as the frames the World cache view takes, after the sun is
/// switched off, to come within a tenth of the way from its old brightness to
/// its new one.
#[test]
fn the_world_cache_follows_the_sun_going_out() {
    if !gpu_enabled() {
        return;
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    const SWITCH: usize = 150;
    const FRAMES: usize = 330;
    let debug = SolariDebug {
        view: SolariDebugView::WorldCache,
        ..default()
    };
    let (kept, _) = render_frames(
        &mut room,
        DEFAULT_POSE,
        realtime(true, false),
        Motion::Still,
        FRAMES,
        Some(debug),
        &|frame| frame + 1 >= SWITCH,
        &mut |world, frame| {
            if frame == SWITCH {
                for mut sun in world.query::<&mut DirectionalLight>().iter_mut(world) {
                    sun.illuminance = 0.0;
                }
            }
        },
    );
    let levels: Vec<f32> = kept.iter().map(|frame| mean(&luminance(&frame.rgb))).collect();
    let before = levels[0];
    let after = levels[levels.len() - 20..].iter().sum::<f32>() / 20.0;
    let settled = levels[1..]
        .iter()
        .position(|&level| (level - after).abs() <= 0.1 * (before - after).abs())
        .map_or(levels.len(), |frames| frames + 1);
    println!("world cache after the sun goes out: {before:.4} -> {after:.4} in {settled} frames");
    assert!(settled <= SUN_OUT_FRAMES, "the world cache took {settled} frames to follow the sun going out");
}

/// Most frames [`the_world_cache_follows_the_sun_going_out`] accepts: measured
/// 54.
const SUN_OUT_FRAMES: usize = 65;

/// The test room with its sun off, and its window shuttered but for four
/// slits a few centimetres wide: lit by the sky alone, through openings a
/// bounce ray rarely finds, as a room behind framed and curtained windows is.
fn sky_lit_room(world: &mut World) {
    room(world);
    for mut sun in world.query::<&mut DirectionalLight>().iter_mut(world) {
        sun.illuminance = 0.0;
    }
    let shutter = world
        .resource_mut::<Assets<StandardMaterial>>()
        .add(plain(Color::srgb(0.62, 0.57, 0.52), 0.9));
    // Five slats across the 2.4 m opening (z -1.2..1.2, y 0.8..2.5), with
    // four 6 cm gaps between them.
    const GAP: f32 = 0.06;
    let slat = (2.4 - 4.0 * GAP) / 5.0;
    for i in 0..5 {
        let z = -1.2 + slat / 2.0 + i as f32 * (slat + GAP);
        traced_box(world, Vec3::new(0.05, 1.7, slat), Vec3::new(2.95, 1.65, z), &shutter);
    }
}

/// How much `scene` varies over time with the camera still, in `view` (the
/// denoised image when `None`), as the mean over the image of each spot's
/// standard deviation across a dozen frames, against the mean brightness; and
/// that mean brightness.
fn wobble_over_time(scene: &mut dyn FnMut(&mut World), view: Option<SolariDebugView>) -> (f32, f32) {
    const SETTLE: usize = 120;
    const EVERY: usize = 10;
    const FRAMES: usize = SETTLE + 12 * EVERY;
    let debug = view.map(|view| SolariDebug { view, ..default() });
    let (kept, _) = render_frames(
        scene,
        DEFAULT_POSE,
        realtime(true, false),
        Motion::Still,
        FRAMES,
        debug,
        &|frame| frame >= SETTLE && (frame - SETTLE).is_multiple_of(EVERY),
        &mut |_, _| {},
    );
    let blurred: Vec<Vec<f32>> = kept.iter().map(|frame| blur(&luminance(&frame.rgb), 4)).collect();
    let count = blurred.len() as f32;
    let pixels = blurred[0].len();
    let (mut deviation, mut brightness) = (0.0f32, 0.0f32);
    for pixel in 0..pixels {
        let samples = blurred.iter().map(|frame| frame[pixel]);
        let average = samples.clone().sum::<f32>() / count;
        let variance = samples.map(|v| (v - average).powi(2)).sum::<f32>() / count;
        deviation += variance.sqrt();
        brightness += average;
    }
    (deviation / brightness.max(1e-9), brightness / pixels as f32)
}

fn mean(values: &[f32]) -> f32 {
    values.iter().sum::<f32>() / values.len().max(1) as f32
}

/// Frames [`the_world_cache_never_fills_in_a_large_scene`] runs: in the Bistro
/// the cache took most of a minute to fill, at about 25 frames a second.
const LARGE_SCENE_FRAMES: usize = 1500;

/// Cells the world cache holds, Solari's `WORLD_CACHE_SIZE`.
const WORLD_CACHE_CAPACITY: f64 = (1u32 << 20) as f64;

/// A town: blocks of every height on a 12 metre grid across 360 metres of
/// ground, the camera's street left clear.
fn town(world: &mut World) {
    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
    let ground = materials.add(plain(Color::srgb(0.45, 0.42, 0.38), 0.8));
    let walls = [
        materials.add(plain(Color::srgb(0.7, 0.62, 0.5), 0.7)),
        materials.add(plain(Color::srgb(0.55, 0.6, 0.65), 0.6)),
        materials.add(plain(Color::srgb(0.75, 0.7, 0.68), 0.9)),
    ];
    traced_box(world, Vec3::new(360.0, 0.2, 360.0), Vec3::new(0.0, -0.1, 0.0), &ground);
    traced_box(world, Vec3::new(360.0, 0.2, 360.0), Vec3::new(0.0, 30.0, 0.0), &ground);
    let mut index = 0usize;
    for x in -15..=15 {
        for z in -15..=15 {
            index += 1;
            if x == 0 {
                continue;
            }
            let height = 4.0 + ((index * 7919) % 23) as f32;
            let at = Vec3::new(x as f32 * 12.0, height * 0.5, z as f32 * 12.0);
            traced_box(world, Vec3::new(8.0, height, 8.0), at, &walls[index % walls.len()]);
        }
    }
    // Street clutter turned every way, as a real town's chairs, railings and
    // leaves are: each face is surface the cache needs cells for.
    let clutter = traced_box(world, Vec3::splat(1.0), Vec3::ZERO, &walls[0]);
    let mesh = world.get::<Mesh3d>(clutter).expect("traced_box gives a mesh").clone();
    let mut state = 0x1234567u32;
    let mut random = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as f32 / u32::MAX as f32
    };
    for i in 0..4000 {
        let at = Vec3::new((random() - 0.5) * 100.0, random() * 6.0, (random() - 0.5) * 100.0);
        let rotation = Quat::from_euler(EulerRot::XYZ, random() * 6.3, random() * 6.3, random() * 6.3);
        world.spawn((
            mesh.clone(),
            RaytracingMesh3d(mesh.0.clone()),
            MeshMaterial3d(walls[i % walls.len()].clone()),
            Transform::from_translation(at).with_rotation(rotation).with_scale(Vec3::splat(0.2 + random() * 0.8)),
        ));
    }
    world.spawn((
        DirectionalLight {
            illuminance: 40_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(8.0, 6.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Most frame-to-frame change [`glossy_glass_is_steady_while_the_camera_moves`]
/// accepts.
const STEADY_GLASS_FLICKER: f32 = 1.0;

/// Mean relative change between two frames over the middle half of the image.
fn centre_flicker(a: &Frame, b: &Frame) -> f32 {
    let (w, h) = size();
    let (la, lb) = (luminance(&a.rgb), luminance(&b.rgb));
    let mut sum = 0.0;
    let mut n = 0.0;
    for y in h / 4..h * 3 / 4 {
        for x in w / 4..w * 3 / 4 {
            let i = (y * w + x) as usize;
            sum += (la[i] - lb[i]).abs() / (la[i].max(lb[i]) + 1e-3);
            n += 1.0;
        }
    }
    sum / n
}

/// A grey ground, a 1.5 m box of `colour` at x = -2, the sun, and `panes`
/// sheets of half-opaque black glass between it and the camera, side by side
/// along x, single-sided and facing the camera or away from it.
fn layered_glass_scene(world: &mut World, colour: Color, panes: usize, facing_camera: bool) {
    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
    let ground = materials.add(plain(Color::srgb(0.4, 0.4, 0.4), 0.9));
    let painted = materials.add(plain(colour, 0.6));
    // Rough, so their reflections are dim and smooth: the box's colour is what
    // is measured, and the red-green difference cancels the rest.
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.0, 0.0, 0.5),
        perceptual_roughness: 0.8,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    traced_box(world, Vec3::new(40.0, 0.2, 40.0), Vec3::new(0.0, -0.1, 0.0), &ground);
    traced_box(world, Vec3::splat(1.5), Vec3::new(-2.0, 0.75, 0.0), &painted);
    for i in 0..panes {
        // A flat quad, not a slab, so one pane is one layer.
        let normal = if facing_camera { Vec3::X } else { Vec3::NEG_X };
        let mut mesh = Mesh::from(Plane3d::new(normal, Vec2::new(2.0, 2.0)));
        mesh.asset_usage = RenderAssetUsages::default();
        mesh.generate_tangents().expect("a plane has UVs");
        if let Some(bevy::mesh::Indices::U16(indices)) = mesh.indices() {
            let wide = indices.iter().map(|&i| i as u32).collect();
            mesh.insert_indices(bevy::mesh::Indices::U32(wide));
        }
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        world.spawn((
            Mesh3d(mesh.clone()),
            RaytracingMesh3d(mesh),
            MeshMaterial3d(glass.clone()),
            Transform::from_xyz(0.5 + 0.6 * i as f32, 1.2, 0.0),
            renzora::SolariShadedTransparent::default(),
        ));
    }
    world.spawn((
        DirectionalLight {
            illuminance: 40_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(6.0, 8.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// A dark-tinted window glass: a window's reflection is plain to see because
/// what is behind it is darker than what it reflects. (A blended material's
/// whole shading, reflection included, is weighted by its alpha, as in Bevy's
/// forward pass.)
fn window_glass() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(0.05, 0.05, 0.06, 0.5),
        perceptual_roughness: 0.04,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

/// A blended material that is a perfect mirror: fully opaque, metallic and
/// white, as smooth as Solari allows.
fn mirror_glass() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        metallic: 1.0,
        perceptual_roughness: 0.0,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

/// A grey ground, a 1.5 m box of `colour` at `box_at`, and the sun; with
/// `pane`, a 6 x 4 m sheet of it standing at x = 0 facing +x, and a dark wall
/// two metres behind it so what shows through is not sky.
fn glass_scene(world: &mut World, box_at: Vec3, colour: Color, pane: Option<StandardMaterial>) {
    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
    let ground = materials.add(plain(Color::srgb(0.4, 0.4, 0.4), 0.9));
    let backdrop = materials.add(plain(Color::srgb(0.03, 0.03, 0.03), 0.9));
    let painted = materials.add(plain(colour, 0.6));
    let glass = pane.map(|pane| materials.add(pane));
    traced_box(world, Vec3::new(40.0, 0.2, 40.0), Vec3::new(0.0, -0.1, 0.0), &ground);
    traced_box(world, Vec3::splat(1.5), box_at, &painted);
    if let Some(glass) = glass {
        traced_box(world, Vec3::new(0.2, 4.0, 8.0), Vec3::new(-2.0, 2.0, 0.0), &backdrop);
        add_pane(world, &glass);
    }
    world.spawn((
        DirectionalLight {
            illuminance: 40_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(6.0, 8.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// The see-through sheet of [`glass_scene`].
fn add_pane(world: &mut World, glass: &Handle<StandardMaterial>) {
    let pane = traced_box(world, Vec3::new(0.02, 4.0, 6.0), Vec3::new(0.0, 2.0, 0.0), glass);
    // What `renzora_solari` puts on a mesh it lights as see-through, so the
    // forward pass does not draw the pane a second time over Solari's.
    world.entity_mut(pane).insert(renzora::SolariShadedTransparent::default());
}

/// The mean colour of the middle 15 x 15 pixels.
fn centre_mean(image: &Frame) -> [f32; 3] {
    let (w, h) = size();
    let (cx, cy) = (w as i32 / 2, h as i32 / 2);
    let mut sum = [0.0f32; 3];
    let mut n = 0.0;
    for y in cy - 7..=cy + 7 {
        for x in cx - 7..=cx + 7 {
            let i = (y as u32 * w + x as u32) as usize * 3;
            for (s, v) in sum.iter_mut().zip(&image.rgb[i..i + 3]) {
                *s += v;
            }
            n += 1.0;
        }
    }
    sum.map(|s| s / n)
}
