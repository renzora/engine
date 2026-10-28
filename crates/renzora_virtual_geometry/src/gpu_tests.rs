//! End-to-end checks on a real GPU: the same lit sphere is rendered as virtual
//! geometry and as an ordinary mesh, and the two images must match.
//!
//! Opt-in like the rest of the engine's GPU tests: set `RENZORA_GPU_TESTS=1`.
//! Run one at a time (`--test-threads=1`); each builds a full renderer.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use bevy::core_pipeline::prepass::{DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::camera::visibility::VisibilityClass;
use bevy::camera::RenderTarget;
use bevy::pbr::DefaultOpaqueRendererMethod;
use bevy::prelude::*;
use bevy::render::render_resource::{CachedPipelineState, PipelineCache, TextureFormat, TextureUsages};
use bevy::render::RenderApp;
use bevy::shader::ShaderCacheError;
use bevy::window::{ExitCondition, WindowPlugin};
use renzora_rvg::bake::{build_mesh, MeshInput};
use renzora_rvg::RvgFile;

use crate::render::materials::VgViewMaterials;
use crate::render::pipelines::VgPipelines;
use crate::{VirtualGeometry, VirtualGeometryPlugin, VirtualGeometrySettings};

const SIZE: u32 = 128;

fn sphere(rings: u32, segments: u32) -> MeshInput {
    let mut positions = vec![[0.0, 1.0, 0.0]];
    for r in 1..rings {
        let phi = std::f32::consts::PI * r as f32 / rings as f32;
        for s in 0..segments {
            let theta = std::f32::consts::TAU * s as f32 / segments as f32;
            positions.push([phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()]);
        }
    }
    positions.push([0.0, -1.0, 0.0]);
    let ring = |r: u32, s: u32| 1 + (r - 1) * segments + s % segments;
    let bottom = positions.len() as u32 - 1;
    let mut indices = Vec::new();
    for s in 0..segments {
        indices.extend([0, ring(1, s + 1), ring(1, s)]);
        indices.extend([bottom, ring(rings - 1, s), ring(rings - 1, s + 1)]);
    }
    for r in 1..rings - 1 {
        for s in 0..segments {
            let (a, b, c, d) = (ring(r, s), ring(r, s + 1), ring(r + 1, s), ring(r + 1, s + 1));
            indices.extend([a, b, d, a, d, c]);
        }
    }
    let uv0 = positions
        .iter()
        .map(|p| [p[0] * 0.5 + 0.5, p[1] * 0.5 + 0.5])
        .collect();
    MeshInput {
        normals: positions.clone(),
        uv0,
        positions,
        indices,
        ..Default::default()
    }
}

fn ordinary_mesh(input: &MeshInput) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, input.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, input.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, input.uv0.clone())
        .with_inserted_indices(Indices::U32(input.indices.clone()))
}

#[derive(Clone, Copy, Debug)]
enum Setup {
    Forward,
    ForwardWithPrepass,
    Deferred,
    EditorViewport,
}

/// A converted sphere: the `.rvg` built from `input`, plus the ordinary mesh
/// it stands in for.
fn spawn_sphere(world: &mut World, input: &MeshInput, color: Color, transform: Transform) {
    spawn_converted(
        world,
        input,
        StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.6,
            ..default()
        },
        transform,
    );
}

/// Any converted mesh with any material.
fn spawn_converted(world: &mut World, input: &MeshInput, material: StandardMaterial, transform: Transform) {
    let rvg = build_mesh(input).expect("mesh builds");
    let file = world.resource_mut::<Assets<RvgFile>>().add(RvgFile {
        builder_version: renzora_rvg::BUILDER_VERSION,
        source_hash: 0,
        meshes: vec![Arc::new(rvg)],
        scene: None,
    });
    let mesh = world.resource_mut::<Assets<Mesh>>().add(ordinary_mesh(input));
    let material = world.resource_mut::<Assets<StandardMaterial>>().add(material);
    world.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(material),
        transform,
        VirtualGeometry {
            file,
            mesh: 0,
            source: mesh.id(),
        },
    ));
}

fn app(setup: Setup) -> Option<(App, Handle<Image>)> {
    app_with(setup, |world| {
        spawn_sphere(world, &sphere(64, 128), Color::srgb(0.9, 0.2, 0.1), Transform::default());
    })
}

/// One renderer at a time. Each test stands up a full one, and several at once
/// on CI's software rasterizer would mostly be measuring contention.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn app_with(setup: Setup, scene: impl FnOnce(&mut World)) -> Option<(App, Handle<Image>)> {
    app_with_plugins(setup, |_| {}, scene)
}

/// [`app_with`], with a chance to add plugins before the app is finished.
fn app_with_plugins(
    setup: Setup,
    plugins: impl FnOnce(&mut App),
    scene: impl FnOnce(&mut World),
) -> Option<(App, Handle<Image>)> {
    app_with_assets(setup, None, plugins, scene)
}

/// [`app_with_plugins`], loading assets from `assets` when given.
fn app_with_assets(
    setup: Setup,
    assets: Option<&std::path::Path>,
    plugins: impl FnOnce(&mut App),
    scene: impl FnOnce(&mut World),
) -> Option<(App, Handle<Image>)> {
    if std::env::var("RENZORA_GPU_TESTS").unwrap_or_default() != "1" {
        return None;
    }
    let mut app = App::new();
    let mut defaults = DefaultPlugins
        .build()
        .disable::<bevy::winit::WinitPlugin>()
        // Keeps the render world in this app, so the test can read the
        // pipeline cache.
        .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
        .set(WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            ..default()
        });
    if let Some(dir) = assets {
        defaults = defaults.set(AssetPlugin {
            file_path: dir.to_string_lossy().into_owned(),
            ..default()
        });
    }
    app.add_plugins(defaults);
    if matches!(setup, Setup::Deferred) {
        app.insert_resource(DefaultOpaqueRendererMethod::deferred());
    }
    // Any GPU error fails the test, as the engine's own handler fails the
    // editor. Bevy's default lets one pass with a log line, and a test could
    // then render the next frame fine and pass over a draw that would crash
    // a real session.
    app.insert_resource(bevy::render::error_handler::RenderErrorHandler(fail_on_gpu_error));
    app.add_plugins(VirtualGeometryPlugin);
    plugins(&mut app);
    app.finish();
    app.cleanup();

    let world = app.world_mut();
    let mut target = Image::new_target_texture(SIZE, SIZE, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = world.resource_mut::<Assets<Image>>().add(target);

    scene(world);
    world.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(2.0, 3.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let mut camera = world.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        RenderTarget::Image(target.clone().into()),
        Msaa::Off,
        Transform::from_xyz(0.0, 0.0, 3.5).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    match setup {
        Setup::Forward => {}
        Setup::ForwardWithPrepass => {
            camera.insert((NormalPrepass, DepthPrepass, MotionVectorPrepass));
        }
        Setup::Deferred => {
            camera.insert((DeferredPrepass, DepthPrepass, MotionVectorPrepass));
        }
        Setup::EditorViewport => {
            // What the editor's viewport camera carries, minus the plugins a
            // bare Bevy app does not have. Each of these changes the view bind
            // group's layout, which the material passes must match.
            camera.insert((
                bevy::camera::Hdr,
                (NormalPrepass, DepthPrepass, MotionVectorPrepass),
                bevy::pbr::ScreenSpaceAmbientOcclusion::default(),
                bevy::pbr::DistanceFog::default(),
                bevy::anti_alias::taa::TemporalAntiAliasing::default(),
            ));
        }
    }
    Some((app, target))
}

fn fail_on_gpu_error(
    error: &bevy::render::error_handler::RenderError,
    _main_world: &mut World,
    _render_world: &mut World,
) -> bevy::render::error_handler::RenderErrorPolicy {
    panic!("GPU error ({:?}): {}", error.ty, error.description);
}

/// A pipeline that failed for good. A shader still loading is reported as an
/// error too, but Bevy retries those, so they are not counted.
fn failure(state: &CachedPipelineState) -> Option<String> {
    match state {
        CachedPipelineState::Err(ShaderCacheError::ShaderNotLoaded(_))
        | CachedPipelineState::Err(ShaderCacheError::ShaderImportNotYetAvailable) => None,
        CachedPipelineState::Err(e) => Some(e.to_string()),
        _ => None,
    }
}

/// Panic with the shader error if any of this crate's pipelines failed.
fn assert_pipelines_ok(app: &mut App) {
    let render = app.sub_app_mut(RenderApp).world_mut();
    let mut failures = Vec::new();
    {
        let cache = render.resource::<PipelineCache>();
        if let Some(p) = render.get_resource::<VgPipelines>() {
            if let Some(e) = failure(cache.get_compute_pipeline_state(p.cull)) {
                failures.push(format!("cull: {e}"));
            }
            for (name, id) in [
                ("raster single", p.raster_single),
                ("raster double", p.raster_double),
                ("material depth", p.material_depth),
            ] {
                if let Some(e) = failure(cache.get_render_pipeline_state(id)) {
                    failures.push(format!("{name}: {e}"));
                }
            }
        }
    }
    let mut query = render.query::<&VgViewMaterials>();
    let cache = render.resource::<PipelineCache>();
    for materials in query.iter(render) {
        for (pass, list) in [
            ("forward", &materials.forward),
            ("prepass", &materials.prepass),
            ("deferred", &materials.deferred),
        ] {
            for (_, id, _) in list {
                if let Some(e) = failure(cache.get_render_pipeline_state(*id)) {
                    failures.push(format!("{pass} material: {e}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "pipelines failed:\n{}", failures.join("\n\n"));
}

fn capture(app: &mut App, target: &Handle<Image>) -> Vec<u8> {
    // Shaders compile asynchronously; give every pipeline time to exist. A
    // fixed count alone is not enough: a pipeline first queued late (a
    // transparent one, queued once its material is ready) can still be
    // compiling on a cold shader cache, and the image would miss what it
    // draws. So wait until nothing is waiting, then a few frames more.
    for _ in 0..120 {
        app.update();
        assert_pipelines_ok(app);
    }
    for _ in 0..600 {
        let waiting = app
            .sub_app(RenderApp)
            .world()
            .resource::<PipelineCache>()
            .waiting_pipelines()
            .count();
        if waiting == 0 {
            break;
        }
        app.update();
        assert_pipelines_ok(app);
    }
    for _ in 0..3 {
        app.update();
    }
    grab(app, target)
}

/// Write an image to `RENZORA_GPU_TEST_OUT`, when set, as `<name>.ppm`.
/// Called straight after a capture, so a failing assertion still leaves the
/// picture behind to look at.
fn dump(name: &str, data: &[u8]) {
    if let Ok(dir) = std::env::var("RENZORA_GPU_TEST_OUT") {
        let path = std::path::Path::new(&dir).join(format!("{}.ppm", name.to_lowercase()));
        let mut ppm = format!("P6 {SIZE} {SIZE} 255\n").into_bytes();
        ppm.extend(data.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
        std::fs::write(path, ppm).unwrap();
    }
}

fn pixel(data: &[u8], x: u32, y: u32) -> [u8; 4] {
    let i = ((y * SIZE + x) * 4) as usize;
    [data[i], data[i + 1], data[i + 2], data[i + 3]]
}

fn compare(setup: Setup) {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app(setup) else {
        return;
    };
    let virtual_image = capture(&mut app, &target);
    // Bevy's own passes must not have drawn the sphere too, or the comparison
    // below would pass on ordinary geometry alone.
    let world = app.world_mut();
    let mut classes = world.query_filtered::<&VisibilityClass, With<VirtualGeometry>>();
    let class = classes.single(world).expect("the sphere is still virtual geometry");
    assert!(
        !class.contains(&std::any::TypeId::of::<Mesh3d>()),
        "the sphere is still in Bevy's mesh passes"
    );

    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    let ordinary_image = capture(&mut app, &target);
    let world = app.world_mut();
    assert_eq!(world.query::<&VirtualGeometry>().iter(world).count(), 0);

    if let Ok(dir) = std::env::var("RENZORA_GPU_TEST_OUT") {
        let name = format!("{setup:?}").to_lowercase();
        for (kind, data) in [("virtual", &virtual_image), ("ordinary", &ordinary_image)] {
            let path = std::path::Path::new(&dir).join(format!("{name}_{kind}.ppm"));
            let mut ppm = format!("P6 {SIZE} {SIZE} 255\n").into_bytes();
            ppm.extend(data.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
            std::fs::write(path, ppm).unwrap();
        }
    }

    // The sphere covers the centre and not the corners.
    let centre = pixel(&virtual_image, SIZE / 2, SIZE / 2);
    assert!(centre[0] > 40 && centre[0] > centre[2], "centre is {centre:?}, not the red sphere");
    // Near black rather than black: tonemapping's debanding dither nudges the
    // clear colour by a step or two.
    let corner = pixel(&virtual_image, 2, 2);
    assert!(corner[..3].iter().all(|&c| c < 8), "corner is {corner:?}, not the clear colour");

    let mut total = 0u64;
    let mut worst = 0u8;
    for (a, b) in virtual_image.iter().zip(&ordinary_image) {
        let d = a.abs_diff(*b);
        total += d as u64;
        worst = worst.max(d);
    }
    let mean = total as f64 / virtual_image.len() as f64;
    // Silhouette pixels may differ by a rasterization rule or a level of
    // detail; the surface itself must not.
    assert!(mean < 1.5, "mean difference {mean:.3} (worst {worst}) between virtual and ordinary");
}

/// Two converted spheres with different materials, and an ordinary cube
/// standing in front of one of them: each material shades its own pixels, and
/// the two kinds of geometry occlude each other through the shared depth.
fn mixed_scene(setup: Setup) {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with(setup, |world| {
        let input = sphere(32, 64);
        spawn_sphere(
            world,
            &input,
            Color::srgb(0.9, 0.1, 0.1),
            Transform::from_xyz(-0.7, 0.0, 0.0).with_scale(Vec3::splat(0.6)),
        );
        spawn_sphere(
            world,
            &input,
            Color::srgb(0.1, 0.9, 0.1),
            Transform::from_xyz(0.7, 0.0, 0.0).with_scale(Vec3::splat(0.6)),
        );
        // On the line from the camera to the red sphere's centre, 2.5 units
        // out, so it covers that centre and nothing else of the sphere's.
        let cube = world
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::new(0.3, 0.3, 0.3)));
        let blue = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::from(Color::srgb(0.1, 0.1, 0.9)));
        world.spawn((Mesh3d(cube), MeshMaterial3d(blue), Transform::from_xyz(-0.5, 0.0, 1.0)));
    }) else {
        return;
    };
    let image = capture(&mut app, &target);
    if let Ok(dir) = std::env::var("RENZORA_GPU_TEST_OUT") {
        let path = std::path::Path::new(&dir).join(format!("mixed_{setup:?}.ppm").to_lowercase());
        let mut ppm = format!("P6 {SIZE} {SIZE} 255\n").into_bytes();
        ppm.extend(image.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
        std::fs::write(path, ppm).unwrap();
    }
    let dominant = |p: [u8; 4]| -> usize {
        (0..3).max_by_key(|&i| p[i]).unwrap()
    };
    let red_centre = pixel(&image, 33, SIZE / 2);
    let red_side = pixel(&image, 55, SIZE / 2);
    let green_centre = pixel(&image, 95, SIZE / 2);
    assert_eq!(dominant(red_centre), 2, "the ordinary cube should hide the red sphere's centre: {red_centre:?}");
    assert_eq!(dominant(red_side), 0, "red sphere beside the cube: {red_side:?}");
    assert_eq!(dominant(green_centre), 1, "green sphere: {green_centre:?}");
}

/// The debug view colours converted pixels and dims everything else,
/// including the ordinary cube standing in front of a converted sphere.
#[test]
fn debug_view_colours_only_visible_virtual_geometry() {
    use renzora::core::viewport_types::{ViewportSettings, VisualizationMode};
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with(Setup::ForwardWithPrepass, |world| {
        let input = sphere(32, 64);
        spawn_sphere(
            world,
            &input,
            Color::srgb(0.9, 0.1, 0.1),
            Transform::from_xyz(-0.7, 0.0, 0.0).with_scale(Vec3::splat(0.6)),
        );
        let cube = world
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::new(0.3, 0.3, 0.3)));
        let blue = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::from(Color::srgb(0.1, 0.1, 0.9)));
        world.spawn((Mesh3d(cube), MeshMaterial3d(blue), Transform::from_xyz(-0.5, 0.0, 1.0)));
        world.insert_resource(ViewportSettings {
            visualization_mode: VisualizationMode::VgClusters,
            ..default()
        });
    }) else {
        return;
    };
    let world = app.world_mut();
    let camera = world
        .query_filtered::<Entity, With<Camera3d>>()
        .single(world)
        .unwrap();
    world.entity_mut(camera).insert(renzora::EditorCamera);

    let image = capture(&mut app, &target);
    {
        let render = app.sub_app(RenderApp).world();
        let cache = render.resource::<PipelineCache>();
        let debug = render.resource::<crate::render::debug::DebugPipelines>();
        for id in debug.ids() {
            if let Some(e) = failure(cache.get_render_pipeline_state(id)) {
                panic!("debug pipeline failed: {e}");
            }
        }
    }
    if let Ok(dir) = std::env::var("RENZORA_GPU_TEST_OUT") {
        let path = std::path::Path::new(&dir).join("debug_clusters.ppm");
        let mut ppm = format!("P6 {SIZE} {SIZE} 255\n").into_bytes();
        ppm.extend(image.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
        std::fs::write(path, ppm).unwrap();
    }
    let grey = |p: [u8; 4]| {
        let (lo, hi) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
        hi - lo < 6
    };
    let cube = pixel(&image, 33, SIZE / 2);
    let sphere_side = pixel(&image, 55, SIZE / 2);
    assert!(grey(cube), "the ordinary cube in front should be dimmed grey: {cube:?}");
    assert!(!grey(sphere_side), "the sphere should be coloured by cluster: {sphere_side:?}");
    // Clusters, not one flat colour: several distinct colours across the sphere.
    let distinct: HashSet<[u8; 4]> = (10..56).map(|x| pixel(&image, x, SIZE / 2 - 10)).filter(|p| !grey(*p)).collect();
    assert!(distinct.len() > 3, "expected several cluster colours, got {}", distinct.len());
}

/// A flat card facing the camera, subdivided so it builds into a few clusters.
fn card(size: f32, cells: u32) -> MeshInput {
    let mut input = MeshInput::default();
    for y in 0..=cells {
        for x in 0..=cells {
            let (u, v) = (x as f32 / cells as f32, y as f32 / cells as f32);
            input.positions.push([(u - 0.5) * size, (0.5 - v) * size, 0.0]);
            input.normals.push([0.0, 0.0, 1.0]);
            input.uv0.push([u, v]);
        }
    }
    let row = cells + 1;
    for y in 0..cells {
        for x in 0..cells {
            let i = y * row + x;
            input.indices.extend([i, i + row, i + row + 1, i, i + row + 1, i + 1]);
        }
    }
    input
}

/// A leaf-like cut-out: an alpha-masked, double-sided card whose texture is
/// opaque inside a circle and transparent outside, in front of a converted
/// sphere. The cut-away corners must show the sphere, the circle must show the
/// card, and the whole must match ordinary rendering.
fn masked_card(setup: Setup) {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with(setup, |world| {
        spawn_sphere(world, &sphere(48, 96), Color::srgb(0.9, 0.1, 0.1), Transform::default());

        let size = 64u32;
        let mut pixels = Vec::with_capacity((size * size * 4) as usize);
        for y in 0..size {
            for x in 0..size {
                let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
                // A circle smaller than the sphere behind it, so the sphere
                // shows around it through the cut-away part of the card.
                let inside = dx * dx + dy * dy < 14.0 * 14.0;
                pixels.extend([255, 255, 255, if inside { 255 } else { 0 }]);
            }
        }
        let image = Image::new(
            bevy::render::render_resource::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::default(),
        );
        let texture = world.resource_mut::<Assets<Image>>().add(image);
        let mut leaf = card(1.6, 8);
        leaf.cutout = true;
        spawn_converted(
            world,
            &leaf,
            StandardMaterial {
                base_color: Color::srgb(0.1, 0.8, 0.2),
                base_color_texture: Some(texture),
                alpha_mode: AlphaMode::Mask(0.5),
                double_sided: true,
                cull_mode: None,
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 1.2),
        );
    }) else {
        return;
    };
    let virtual_image = capture(&mut app, &target);
    {
        let render = app.sub_app(RenderApp).world();
        let cache = render.resource::<PipelineCache>();
        let masked = render.resource::<crate::render::masked::MaskedDraws>();
        assert!(!masked.ids().is_empty(), "the card was not drawn by the masked pass");
        for id in masked.ids() {
            if let Some(e) = failure(cache.get_render_pipeline_state(id)) {
                panic!("masked raster pipeline failed: {e}");
            }
        }
    }
    let world = app.world_mut();
    assert_eq!(
        world.query::<&VirtualGeometry>().iter(world).count(),
        2,
        "both the sphere and the masked card should be virtual geometry"
    );
    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    let ordinary_image = capture(&mut app, &target);

    if let Ok(dir) = std::env::var("RENZORA_GPU_TEST_OUT") {
        for (kind, data) in [("virtual", &virtual_image), ("ordinary", &ordinary_image)] {
            let path = std::path::Path::new(&dir).join(format!("masked_{setup:?}_{kind}.ppm").to_lowercase());
            let mut ppm = format!("P6 {SIZE} {SIZE} 255\n").into_bytes();
            ppm.extend(data.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
            std::fs::write(path, ppm).unwrap();
        }
    }

    // Centre: the card's circle, green (about 24 pixels across in radius).
    // Forty pixels left of centre the card is cut away but the sphere (about
    // 44 pixels in radius) is behind it: red.
    let centre = pixel(&virtual_image, SIZE / 2, SIZE / 2);
    assert!(centre[1] > centre[0] && centre[1] > centre[2], "centre should be the green card: {centre:?}");
    let corner = pixel(&virtual_image, SIZE / 2 - 40, SIZE / 2);
    assert!(corner[0] > corner[1] && corner[0] > corner[2], "the cut-away corner should show the red sphere: {corner:?}");

    let total: u64 = virtual_image
        .iter()
        .zip(&ordinary_image)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum();
    let mean = total as f64 / virtual_image.len() as f64;
    assert!(mean < 2.0, "mean difference {mean:.3} between virtual and ordinary");
}

/// The back of a double-sided surface is shaded with its normal turned toward
/// the camera, as ordinary rendering does. Bevy's PBR shaders take every
/// visibility-buffer pixel as a front face, so the back of a thin double-sided
/// shell kept a normal facing away from the eye: dimmer than it should be in
/// raster, and black under Solari wherever that normal faced away from every
/// light (the back of the apartment's dining chairs). An opaque card turned to
/// face away from the camera must match ordinary rendering.
fn back_of_double_sided_card(setup: Setup) {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with(setup, |world| {
        spawn_converted(
            world,
            &card(1.6, 8),
            StandardMaterial {
                base_color: Color::srgb(0.8, 0.8, 0.8),
                double_sided: true,
                cull_mode: None,
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 1.2).with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        );
    }) else {
        return;
    };
    let virtual_image = capture(&mut app, &target);
    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    let ordinary_image = capture(&mut app, &target);
    if let Ok(dir) = std::env::var("RENZORA_GPU_TEST_OUT") {
        for (kind, data) in [("virtual", &virtual_image), ("ordinary", &ordinary_image)] {
            let path = std::path::Path::new(&dir).join(format!("back_card_{setup:?}_{kind}.ppm").to_lowercase());
            let mut ppm = format!("P6 {SIZE} {SIZE} 255\n").into_bytes();
            ppm.extend(data.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
            std::fs::write(path, ppm).unwrap();
        }
    }
    let centre = pixel(&virtual_image, SIZE / 2, SIZE / 2);
    let ordinary_centre = pixel(&ordinary_image, SIZE / 2, SIZE / 2);
    let total: u64 = virtual_image
        .iter()
        .zip(&ordinary_image)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum();
    let mean = total as f64 / virtual_image.len() as f64;
    println!("back of a double-sided card: virtual {centre:?}, ordinary {ordinary_centre:?}, mean difference {mean:.3}");
    assert!(mean < 2.0, "the back of a double-sided card differs from ordinary rendering by {mean:.3}");
}

#[test]
fn back_of_double_sided_card_forward() {
    back_of_double_sided_card(Setup::ForwardWithPrepass);
}

#[test]
fn back_of_double_sided_card_deferred() {
    back_of_double_sided_card(Setup::Deferred);
}

/// A node-graph material on virtual geometry: a checkerboard computed in the
/// graph from the UVs, so the graph body itself (not the base approximation)
/// is what has to come out the same through the visibility buffer.
fn graph_material(setup: Setup) {
    use renzora_shader::material::codegen::{compile, supports_deferred, supports_virtual_geometry};
    use renzora_shader::material::graph::{MaterialDomain, MaterialGraph, PinValue};
    use renzora_shader::material::surface_ext::{new_graph_material, GraphMaterial};

    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with_plugins(
        setup,
        |app| {
            use crate::VirtualGeometryAppExt;
            app.add_plugins(renzora_shader::material::runtime::GraphMaterialPlugin);
            app.add_virtual_geometry_material::<GraphMaterial>(|m| {
                m.extension
                    .virtual_geometry
                    .then(|| crate::classify_standard(&m.base))
                    .flatten()
            });
        },
        |world| {
            let mut graph = MaterialGraph::new("Checker", MaterialDomain::Surface);
            let out = graph.output_node().unwrap().id;
            let uv = graph.add_node("input/uv", [0.0, 0.0]);
            let checker = graph.add_node("procedural/checkerboard", [200.0, 0.0]);
            graph
                .get_node_mut(checker)
                .unwrap()
                .input_values
                .insert("scale".into(), PinValue::Float(6.0));
            let lerp = graph.add_node("color/lerp", [400.0, 0.0]);
            let node = graph.get_node_mut(lerp).unwrap();
            node.input_values.insert("a".into(), PinValue::Color([0.9, 0.1, 0.1, 1.0]));
            node.input_values.insert("b".into(), PinValue::Color([0.9, 0.8, 0.1, 1.0]));
            graph.connect(uv, "uv", checker, "uv");
            graph.connect(checker, "value", lerp, "t");
            graph.connect(lerp, "color", out, "base_color");
            let compiled = compile(&graph);
            assert!(compiled.errors.is_empty(), "{:?}", compiled.errors);
            assert!(
                supports_virtual_geometry(&compiled.fragment_shader),
                "the generated shader has no virtual geometry branch:\n{}",
                compiled.fragment_shader
            );

            let uuid = bevy::asset::uuid::Uuid::new_v4();
            let shader = renzora::shader_source::shader(compiled.fragment_shader.clone(), "test/checker.wesl");
            let _ = world
                .resource_mut::<Assets<Shader>>()
                .insert(&Handle::<Shader>::Uuid(uuid, std::marker::PhantomData), shader);
            let fallback = world.resource::<renzora_shader::material::runtime::FallbackTexture>().clone();
            let mut material = new_graph_material(&fallback);
            material.extension.shader_uuid = Some(uuid);
            material.extension.writes_gbuffer = supports_deferred(&compiled.fragment_shader);
            material.extension.virtual_geometry = true;
            let material = world.resource_mut::<Assets<GraphMaterial>>().add(material);

            let input = sphere(48, 96);
            let rvg = build_mesh(&input).expect("sphere builds");
            let file = world.resource_mut::<Assets<RvgFile>>().add(RvgFile {
                builder_version: renzora_rvg::BUILDER_VERSION,
                source_hash: 0,
                meshes: vec![Arc::new(rvg)],
                scene: None,
            });
            let mesh = world.resource_mut::<Assets<Mesh>>().add(ordinary_mesh(&input));
            world.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::default(),
                VirtualGeometry {
                    file,
                    mesh: 0,
                    source: mesh.id(),
                },
            ));
        },
    ) else {
        return;
    };
    let virtual_image = capture(&mut app, &target);
    let world = app.world_mut();
    assert_eq!(world.query::<&VirtualGeometry>().iter(world).count(), 1, "the graph mesh was not kept as virtual geometry");
    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    let ordinary_image = capture(&mut app, &target);

    if let Ok(dir) = std::env::var("RENZORA_GPU_TEST_OUT") {
        for (kind, data) in [("virtual", &virtual_image), ("ordinary", &ordinary_image)] {
            let path = std::path::Path::new(&dir).join(format!("graph_{setup:?}_{kind}.ppm").to_lowercase());
            let mut ppm = format!("P6 {SIZE} {SIZE} 255\n").into_bytes();
            ppm.extend(data.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
            std::fs::write(path, ppm).unwrap();
        }
    }
    // Both colours of the checkerboard appear, so the graph body ran.
    let mut red = false;
    let mut yellow = false;
    for x in 30..98 {
        let p = pixel(&virtual_image, x, SIZE / 2);
        // Lit, so the red squares come out pink: red well above green.
        // Yellow has the two close together.
        let (r, g) = (p[0] as i32, p[1] as i32);
        red |= r > 80 && r - g > 60;
        yellow |= r > 80 && (r - g).abs() < 40;
    }
    assert!(red && yellow, "the checkerboard did not render (red {red}, yellow {yellow})");
    let total: u64 = virtual_image
        .iter()
        .zip(&ordinary_image)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum();
    let mean = total as f64 / virtual_image.len() as f64;
    assert!(mean < 2.5, "mean difference {mean:.3} between virtual and ordinary");
}

/// A sphere skinned to two joints, the upper half following a joint that has
/// been moved right and the lower half one left where it was: the virtual
/// geometry image must show the bent shape, not the rest pose, and match
/// Bevy's own skinning of the same mesh.
fn skinned_sphere(setup: Setup) {
    use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with(setup, |world| {
        let mut input = sphere(48, 96);
        for p in &input.positions {
            // A soft band around the equator, so the bend is smooth.
            let upper = ((p[1] + 0.2) / 0.4).clamp(0.0, 1.0);
            input.joints.push([0, 1, 0, 0]);
            input.weights.push([1.0 - upper, upper, 0.0, 0.0]);
        }
        let rvg = build_mesh(&input).expect("skinned sphere builds");
        assert!(rvg.attributes.has(renzora_rvg::Attributes::SKIN), "the skin did not survive the build");
        let file = world.resource_mut::<Assets<RvgFile>>().add(RvgFile {
            builder_version: renzora_rvg::BUILDER_VERSION,
            source_hash: 0,
            meshes: vec![Arc::new(rvg)],
            scene: None,
        });
        let mesh = ordinary_mesh(&input)
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                bevy::mesh::VertexAttributeValues::Uint16x4(input.joints.clone()),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, input.weights.clone());
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        let bindposes = world
            .resource_mut::<Assets<SkinnedMeshInverseBindposes>>()
            .add(SkinnedMeshInverseBindposes::from(vec![Mat4::IDENTITY, Mat4::IDENTITY]));
        let root = world.spawn(Transform::default()).id();
        let moved = world.spawn(Transform::from_xyz(0.6, 0.0, 0.0)).id();
        let material = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.2, 0.1),
            perceptual_roughness: 0.6,
            ..default()
        });
        world.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
            SkinnedMesh {
                inverse_bindposes: bindposes,
                joints: vec![root, moved],
            },
            VirtualGeometry {
                file,
                mesh: 0,
                source: mesh.id(),
            },
        ));
    }) else {
        return;
    };
    let virtual_image = capture(&mut app, &target);
    {
        let render = app.sub_app(RenderApp).world();
        let cache = render.resource::<PipelineCache>();
        let pipelines = render.resource::<VgPipelines>();
        if let Some(e) = failure(cache.get_compute_pipeline_state(pipelines.skin)) {
            panic!("skin pipeline failed: {e}");
        }
        assert!(
            render.resource::<crate::render::scene::VgSkinBindGroup>().0.is_some(),
            "nothing was skinned"
        );
    }
    let world = app.world_mut();
    assert_eq!(world.query::<&VirtualGeometry>().iter(world).count(), 1, "the skinned sphere was not kept as virtual geometry");
    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    let ordinary_image = capture(&mut app, &target);

    if let Ok(dir) = std::env::var("RENZORA_GPU_TEST_OUT") {
        for (kind, data) in [("virtual", &virtual_image), ("ordinary", &ordinary_image)] {
            let path = std::path::Path::new(&dir).join(format!("skinned_{setup:?}_{kind}.ppm").to_lowercase());
            let mut ppm = format!("P6 {SIZE} {SIZE} 255\n").into_bytes();
            ppm.extend(data.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
            std::fs::write(path, ppm).unwrap();
        }
    }
    // Upper right, beyond the rest-pose sphere's edge: only there because the
    // upper half moved right. Lower left, inside the unmoved lower half.
    let moved = pixel(&virtual_image, 116, 40);
    assert!(moved[0] > 40 && moved[0] > moved[2], "the moved upper half is missing: {moved:?}");
    let unmoved = pixel(&virtual_image, 40, 90);
    assert!(unmoved[0] > 40 && unmoved[0] > unmoved[2], "the lower half is missing: {unmoved:?}");
    let total: u64 = virtual_image
        .iter()
        .zip(&ordinary_image)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum();
    let mean = total as f64 / virtual_image.len() as f64;
    assert!(mean < 2.0, "mean difference {mean:.3} between virtual and ordinary skinning");
}

/// Glass: a half-transparent blue sphere in front of an opaque red one. The
/// overlap must show both (the blend happened, over the opaque sphere drawn
/// first), and the whole must match ordinary rendering, which sorts and
/// blends the same sphere in Bevy's own transparent pass.
fn transparent_sphere(setup: Setup) {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with(setup, |world| {
        let input = sphere(32, 64);
        spawn_sphere(
            world,
            &input,
            Color::srgb(0.9, 0.1, 0.1),
            Transform::from_xyz(0.0, 0.0, -0.5),
        );
        spawn_converted(
            world,
            &input,
            StandardMaterial {
                base_color: Color::srgba(0.1, 0.2, 0.9, 0.5),
                alpha_mode: AlphaMode::Blend,
                ..default()
            },
            Transform::from_xyz(0.3, 0.0, 1.2).with_scale(Vec3::splat(0.5)),
        );
    }) else {
        return;
    };
    let virtual_image = capture(&mut app, &target);
    dump(&format!("transparent_{setup:?}_virtual"), &virtual_image);
    {
        let render = app.sub_app(RenderApp).world();
        let cache = render.resource::<PipelineCache>();
        let pipelines = render.resource::<crate::render::transparent::TransparentPipelines>();
        assert!(!pipelines.ids().is_empty(), "the glass sphere was not queued in the transparent pass");
        for id in pipelines.ids() {
            let state = cache.get_render_pipeline_state(id);
            if let Some(e) = failure(state) {
                panic!("transparent pipeline failed: {e}");
            }
            assert!(
                matches!(state, CachedPipelineState::Ok(_)),
                "transparent pipeline not ready after the warm-up frames: {state:?}"
            );
        }
    }
    let world = app.world_mut();
    assert_eq!(
        world.query::<&VirtualGeometry>().iter(world).count(),
        2,
        "both spheres should be virtual geometry"
    );
    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    let ordinary_image = capture(&mut app, &target);
    dump(&format!("transparent_{setup:?}_ordinary"), &ordinary_image);
    // Where the glass covers the red sphere, the two blend to purple (about
    // 185, 105, 185). Red alone (225, 68, 59) means the glass was not drawn;
    // blue alone (about 65, 100, 230) means it was drawn without blending.
    let overlap = pixel(&virtual_image, SIZE / 2 + 12, SIZE / 2);
    assert!(
        overlap[0] > 120 && overlap[2] > 120,
        "the glass over the red sphere should blend to purple: {overlap:?}"
    );
    let total: u64 = virtual_image
        .iter()
        .zip(&ordinary_image)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum();
    let mean = total as f64 / virtual_image.len() as f64;
    assert!(mean < 2.0, "mean difference {mean:.3} between virtual and ordinary glass");
}

/// (resident groups, all groups, resident bytes) of the streamed scene.
fn streaming_stats(app: &App) -> (usize, usize, u64) {
    let gpu = app.sub_app(RenderApp).world().resource::<crate::render::scene::GpuScene>();
    let (resident, total) = gpu.streaming.resident_groups();
    (resident, total, gpu.streaming.bytes().0)
}

/// A mesh arrives with only its coarsest groups on the GPU, asks for the
/// detail the camera needs, and ends up looking exactly like ordinary
/// geometry once it has it.
#[test]
fn streams_detail_in() {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app(Setup::ForwardWithPrepass) else {
        return;
    };
    app.update();
    app.update();
    let (first, total, _) = streaming_stats(&app);
    assert!(first > 0, "the roots should be resident from the start");
    assert!(first < total, "only the roots should be resident before any feedback: {first} of {total}");

    let virtual_image = capture(&mut app, &target);
    dump("streams_detail_in_virtual", &virtual_image);
    let (later, total, bytes) = streaming_stats(&app);
    assert!(later > first, "nothing was streamed in: {later} of {total} groups resident");
    assert!(bytes > 0);

    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    let ordinary_image = capture(&mut app, &target);
    let total_diff: u64 = virtual_image
        .iter()
        .zip(&ordinary_image)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum();
    let mean = total_diff as f64 / virtual_image.len() as f64;
    assert!(mean < 1.5, "mean difference {mean:.3} after streaming");
}

/// With a budget far below what full detail needs, residency stays within it
/// and the mesh still renders whole: coarser where the detail did not fit,
/// never with a hole.
#[test]
fn stays_within_a_small_budget() {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with(Setup::ForwardWithPrepass, |world| {
        spawn_sphere(world, &sphere(256, 512), Color::srgb(0.9, 0.2, 0.1), Transform::default());
        let mut settings = world.resource_mut::<VirtualGeometrySettings>();
        settings.budget_mb = 1;
        // Far below a pixel, so the camera asks for nearly every group and
        // the budget, not the view, is what stops it.
        settings.lod_error_pixels = 0.001;
    }) else {
        return;
    };
    let image = capture(&mut app, &target);
    dump("stays_within_a_small_budget", &image);
    let (resident, total, bytes) = streaming_stats(&app);
    assert!(resident < total, "everything fit, so the budget was not tested: {resident} of {total}");
    assert!(bytes <= 1 << 20, "{bytes} bytes resident over a 1 MB budget");
    assert!(bytes > 3 << 18, "only {bytes} bytes resident: the budget never came under pressure");

    // The sphere covers a disc of about 44 pixels' radius. Inside it, every
    // pixel is the sphere: any clear-colour pixel there is a hole.
    let centre = SIZE as f32 / 2.0;
    let mut holes = 0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f32 + 0.5 - centre, y as f32 + 0.5 - centre);
            if dx * dx + dy * dy < 38.0 * 38.0 && pixel(&image, x, y)[0] < 20 {
                holes += 1;
            }
        }
    }
    assert_eq!(holes, 0, "{holes} pixels inside the sphere show the background");
}

/// Two detailed spheres, a budget that holds the detail of only one, and a
/// camera that turns from the first to the second: the first's detail is
/// evicted to make room, and the second renders whole.
#[test]
fn evicts_what_the_camera_left() {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with(Setup::ForwardWithPrepass, |world| {
        let input = sphere(256, 512);
        spawn_sphere(world, &input, Color::srgb(0.9, 0.2, 0.1), Transform::from_xyz(-20.0, 0.0, 0.0));
        spawn_sphere(world, &input, Color::srgb(0.1, 0.8, 0.2), Transform::from_xyz(20.0, 0.0, 0.0));
        let mut settings = world.resource_mut::<VirtualGeometrySettings>();
        settings.budget_mb = 2;
        settings.lod_error_pixels = 0.001;
    }) else {
        return;
    };
    let aim = |app: &mut App, x: f32| {
        let world = app.world_mut();
        let mut cameras = world.query_filtered::<&mut Transform, With<Camera3d>>();
        for mut transform in cameras.iter_mut(world) {
            *transform = Transform::from_xyz(x, 0.0, 3.5).looking_at(Vec3::new(x, 0.0, 0.0), Vec3::Y);
        }
    };
    aim(&mut app, -20.0);
    let left = capture(&mut app, &target);
    dump("evicts_left", &left);
    let evicted_before = app.sub_app(RenderApp).world().resource::<crate::render::scene::GpuScene>().streaming.evicted;

    aim(&mut app, 20.0);
    let right = capture(&mut app, &target);
    dump("evicts_right", &right);
    let gpu = app.sub_app(RenderApp).world().resource::<crate::render::scene::GpuScene>();
    let evicted = gpu.streaming.evicted - evicted_before;
    let bytes = gpu.streaming.bytes().0;
    assert!(evicted > 0, "turning to the second sphere evicted nothing of the first");
    assert!(bytes <= 2 << 20, "{bytes} bytes resident over a 2 MB budget");
    let centre = pixel(&right, SIZE / 2, SIZE / 2);
    assert!(centre[1] > centre[0] && centre[1] > centre[2], "the green sphere is not in view: {centre:?}");
}

/// The residency view: under a budget too small for the detail asked of it,
/// stand-ins show red; with room, once streaming has caught up, none do.
#[test]
fn residency_view_shows_stand_ins() {
    use renzora::core::viewport_types::{ViewportSettings, VisualizationMode};
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let run = |budget_mb: u32, lod_error_pixels: f32| -> (usize, usize) {
        let (mut app, target) = app_with(Setup::ForwardWithPrepass, |world| {
            spawn_sphere(world, &sphere(256, 512), Color::srgb(0.5, 0.5, 0.5), Transform::default());
            let mut settings = world.resource_mut::<VirtualGeometrySettings>();
            settings.budget_mb = budget_mb;
            settings.lod_error_pixels = lod_error_pixels;
            world.insert_resource(ViewportSettings {
                visualization_mode: VisualizationMode::VgResidency,
                ..default()
            });
        })
        .expect("GPU tests are on");
        let world = app.world_mut();
        let camera = world.query_filtered::<Entity, With<Camera3d>>().single(world).unwrap();
        world.entity_mut(camera).insert(renzora::EditorCamera);
        let image = capture(&mut app, &target);
        dump(&format!("residency_{budget_mb}mb"), &image);
        let (mut red, mut green) = (0, 0);
        for p in image.chunks_exact(4) {
            if p[0] > 120 && p[1] < 80 {
                red += 1;
            } else if p[1] > 120 && p[0] < 80 {
                green += 1;
            }
        }
        (red, green)
    };
    if std::env::var("RENZORA_GPU_TESTS").unwrap_or_default() != "1" {
        return;
    }
    let (red, green) = run(1, 0.001);
    assert!(red > 0, "a starved budget should leave stand-ins (red {red}, green {green})");
    let (red, green) = run(512, 1.0);
    assert!(green > 0, "nothing coloured at all (red {red}, green {green})");
    assert_eq!(red, 0, "with room, every stand-in should have been replaced (green {green})");
}

/// Removing models while their transparent meshes are on screen. Three glass
/// spheres take buckets 0 to 2; despawning two renumbers what is left. The
/// draws queued for a frame must use that frame's numbering, or one points
/// past the end of the bucket buffer and the GPU device is lost.
#[test]
fn removing_transparent_meshes_keeps_drawing() {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Some((mut app, target)) = app_with(Setup::ForwardWithPrepass, |world| {
        let input = sphere(16, 32);
        for x in [-0.8, 0.0, 0.8] {
            spawn_converted(
                world,
                &input,
                StandardMaterial {
                    base_color: Color::srgba(0.1, 0.2, 0.9, 0.5),
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                },
                Transform::from_xyz(x, 0.0, 0.0).with_scale(Vec3::splat(0.35)),
            );
        }
    }) else {
        return;
    };
    let before = capture(&mut app, &target);
    dump("removing_transparent_before", &before);
    // All three are drawn: each is a sorted item of its own. They were once
    // merged into one batch by Bevy, and only the first appeared.
    for x in [SIZE / 2 - 31, SIZE / 2, SIZE / 2 + 31] {
        let p = pixel(&before, x, SIZE / 2);
        assert!(p[2] > 30 && p[2] > p[0], "glass sphere at x {x} is missing: {p:?}");
    }
    let world = app.world_mut();
    let spheres: Vec<Entity> = world.query_filtered::<Entity, With<VirtualGeometry>>().iter(world).collect();
    assert_eq!(spheres.len(), 3);
    for &entity in &spheres[..2] {
        world.despawn(entity);
    }
    // A device loss panics the test from inside these frames.
    let image = capture(&mut app, &target);
    dump("removing_transparent", &image);
    assert_pipelines_ok(&mut app);
}

/// `input` as a GLB: one node drawing one red primitive, as an imported model
/// would be.
fn model_glb(input: &MeshInput) -> Vec<u8> {
    let floats = |v: &[[f32; 3]]| -> Vec<u8> { v.iter().flatten().flat_map(|f| f.to_le_bytes()).collect() };
    let mut bin = floats(&input.positions);
    let normals_at = bin.len();
    bin.extend(floats(&input.normals));
    let uv_at = bin.len();
    bin.extend(input.uv0.iter().flatten().flat_map(|f| f.to_le_bytes()));
    let indices_at = bin.len();
    bin.extend(input.indices.iter().flat_map(|i| i.to_le_bytes()));
    let n = input.positions.len();
    let doc = serde_json::json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "name": "Statue", "mesh": 0 }],
        "materials": [{ "name": "Red", "pbrMetallicRoughness": {
            "baseColorFactor": [0.8, 0.05, 0.03, 1.0], "metallicFactor": 0.0, "roughnessFactor": 0.6 } }],
        "meshes": [{ "primitives": [{
            "attributes": { "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 }, "indices": 3, "material": 0 }] }],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": n, "type": "VEC3",
              "min": [-1.1, -1.1, -1.1], "max": [1.1, 1.1, 1.1] },
            { "bufferView": 1, "componentType": 5126, "count": n, "type": "VEC3" },
            { "bufferView": 2, "componentType": 5126, "count": n, "type": "VEC2" },
            { "bufferView": 3, "componentType": 5125, "count": input.indices.len(), "type": "SCALAR" }
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": normals_at },
            { "buffer": 0, "byteOffset": normals_at, "byteLength": uv_at - normals_at },
            { "buffer": 0, "byteOffset": uv_at, "byteLength": indices_at - uv_at },
            { "buffer": 0, "byteOffset": indices_at, "byteLength": bin.len() - indices_at }
        ],
        "buffers": [{ "byteLength": bin.len() }]
    });
    let mut json = serde_json::to_vec(&doc).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut glb = b"glTF".to_vec();
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
    glb.extend_from_slice(&json);
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E_4942u32.to_le_bytes());
    glb.extend_from_slice(&bin);
    glb
}

/// A model stored as `.rvg`, loaded the way a placed model is: as a `Gltf`
/// from its path, its default scene spawned under a root. Its mesh must come
/// out as virtual geometry, found in the same file, and render.
#[test]
fn an_rvg_model_loads_and_renders() {
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let dir = std::env::temp_dir().join(format!("renzora_rvg_model_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rvg = renzora_rvg::bake::build_model(&model_glb(&sphere(64, 128))).unwrap();
    assert!(rvg.scene.is_some());
    std::fs::write(dir.join("statue.rvg"), rvg.to_bytes()).unwrap();

    let Some((mut app, target)) = app_with_assets(
        Setup::ForwardWithPrepass,
        Some(&dir),
        |app| {
            app.add_plugins(renzora_rvg::model::RvgModelPlugin);
        },
        |_| {},
    ) else {
        return;
    };
    let handle: Handle<bevy::gltf::Gltf> = app.world().resource::<AssetServer>().load("statue.rvg");
    let mut scene = None;
    for _ in 0..600 {
        app.update();
        if let Some(gltf) = app.world().resource::<Assets<bevy::gltf::Gltf>>().get(&handle) {
            scene = gltf.default_scene.clone();
            break;
        }
        assert!(
            !app.world().resource::<AssetServer>().load_state(&handle).is_failed(),
            "the .rvg model failed to load: {:?}",
            app.world().resource::<AssetServer>().load_state(&handle)
        );
    }
    let scene = scene.expect("the .rvg model loaded with a default scene");
    app.world_mut().spawn((Transform::default(), bevy::world_serialization::WorldAssetRoot(scene)));

    let image = capture(&mut app, &target);
    dump("rvg_model", &image);
    let world = app.world_mut();
    let converted = world.query::<&VirtualGeometry>().iter(world).count();
    assert_eq!(converted, 1, "the model's mesh was not drawn as virtual geometry");
    let centre = pixel(&image, SIZE / 2, SIZE / 2);
    assert!(centre[0] > 40 && centre[0] > centre[2], "the model is not on screen: {centre:?}");

    // The scene holds only the proxy; drawn the ordinary way, the mesh must
    // be swapped for its full detail, so switching virtual geometry off
    // changes nothing on screen.
    let triangles = |app: &mut App| -> usize {
        let world = app.world_mut();
        let handle = world.query::<&Mesh3d>().single(world).unwrap().0.clone();
        world.resource::<Assets<Mesh>>().get(&handle).and_then(|m| m.indices()).map_or(0, |i| i.len() / 3)
    };
    let proxy = triangles(&mut app);
    let full = rvg.meshes[0].original_triangle_count();
    assert!(proxy < full, "the scene should hold a coarser proxy ({proxy} of {full} triangles)");
    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    let ordinary = capture(&mut app, &target);
    dump("rvg_model_ordinary", &ordinary);
    assert_eq!(triangles(&mut app), full, "drawn the ordinary way, the mesh is not at full detail");
    let total: u64 = image.iter().zip(&ordinary).map(|(a, b)| a.abs_diff(*b) as u64).sum();
    let mean = total as f64 / image.len() as f64;
    assert!(mean < 1.5, "mean difference {mean:.3} between virtual geometry and the full-detail fallback");

    // And back: virtual geometry takes the mesh again, with its proxy.
    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = true;
    let _ = capture(&mut app, &target);
    let world = app.world_mut();
    assert_eq!(world.query::<&VirtualGeometry>().iter(world).count(), 1, "not converted again");
    assert_eq!(triangles(&mut app), proxy, "the proxy was not put back");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Solari traces a model stored as `.rvg` as it is drawn from the moment it
/// appears. After a scene load the editor showed bright and black chunks over
/// such models for tens of seconds, settling on their own: what was drawn and
/// what was traced did not match. A bumpy sphere (bumps larger than the
/// diagnostics' match tolerance, so a coarse cut and the full detail differ)
/// is loaded with Solari already running, and the share of pixels the traced
/// scene does not match is followed frame by frame from when the model is
/// spawned. Solari traced the full detail from the third frame; it was virtual
/// geometry, streaming its detail in a level per feedback round trip, that
/// drew coarse levels for some twenty frames here (54% unmatched at first), and
/// for tens of seconds in a full scene.
#[test]
fn solari_traces_an_rvg_model_as_drawn_from_the_start() {
    if !solari_gpu_tests() {
        return;
    }
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (mut app, _, dir) = solari_rvg_app("start", true, Transform::from_xyz(0.0, 0.0, 3.5).looking_at(Vec3::ZERO, Vec3::Y));
    let mut timeline = Vec::new();
    spawn_bumpy_model(&mut app);
    // The diagnostics' figures arrive a few frames late; each is tagged with
    // the frame it was read on.
    for frame in 0..FIRST_FRAMES {
        app.update();
        if let Some(stats) = app.world_mut().resource_mut::<renzora::SolariDiagnostics>().frame.take() {
            timeline.push((frame, stats.not_traced_fraction, stats.covered_pixels));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    for (frame, fraction, covered) in timeline.iter().take(30) {
        println!("frame {frame:>3}: {:.2}% of {covered} pixels not traced as drawn", fraction * 100.0);
    }
    // Only frames that see the model count: before it is drawn there is
    // nothing to compare.
    let drawn: Vec<_> = timeline.iter().filter(|(_, _, covered)| *covered > 1000).collect();
    assert!(!drawn.is_empty(), "the model never appeared");
    let worst = drawn.iter().skip(SETTLE_FRAMES).map(|(_, f, _)| *f).fold(0.0f32, f32::max);
    assert!(
        worst < 0.01,
        "{:.1}% of the model's pixels are not traced as drawn, {SETTLE_FRAMES} frames after it appeared",
        worst * 100.0
    );
}

/// Switching virtual geometry off and on again while Solari runs keeps the
/// model in the ray-traced scene, traced as it is drawn. In the editor the
/// apartment's floor, ceiling and armchairs left the traced scene when it was
/// switched off, and its window frames when it came back, until Solari itself
/// was switched off and on.
#[test]
fn solari_keeps_tracing_a_model_while_virtual_geometry_is_switched() {
    if !solari_gpu_tests() {
        return;
    }
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (mut app, _, dir) = solari_rvg_app("switch", true, Transform::from_xyz(0.0, 0.0, 3.5).looking_at(Vec3::ZERO, Vec3::Y));
    spawn_bumpy_model(&mut app);
    // The worst figure over the last frames of each phase, by which point the
    // swap to or from full detail has long since landed.
    let settle = |app: &mut App| {
        let mut worst = 0.0f32;
        let mut covered = 0;
        for frame in 0..SWITCH_SETTLE_FRAMES {
            app.update();
            if let Some(stats) = app.world_mut().resource_mut::<renzora::SolariDiagnostics>().frame.take() {
                if frame >= SWITCH_SETTLE_FRAMES / 2 {
                    worst = worst.max(stats.not_traced_fraction);
                    covered = stats.covered_pixels;
                }
            }
        }
        (worst, covered)
    };
    let mut phases = Vec::new();
    phases.push(("on", settle(&mut app)));
    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    phases.push(("switched off", settle(&mut app)));
    app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = true;
    phases.push(("switched back on", settle(&mut app)));
    let _ = std::fs::remove_dir_all(&dir);
    for (phase, (worst, covered)) in &phases {
        println!("virtual geometry {phase}: {:.2}% of {covered} pixels not traced as drawn", worst * 100.0);
    }
    for (phase, (worst, covered)) in &phases {
        assert!(*covered > 1000, "the model is not on screen with virtual geometry {phase}");
        assert!(*worst < 0.01, "{:.1}% of the model is not traced as drawn with virtual geometry {phase}", worst * 100.0);
    }
}

const SWITCH_SETTLE_FRAMES: usize = 120;

/// A glowing mesh with more triangles than a light can be sampled over is
/// traced without failing. Light sampling packs a triangle index into 16 bits,
/// and registering such a mesh as a light panicked while the scene was being
/// prepared; the GPU then read half-written scene data and lost the device. A
/// character whose hair glowed faintly (82,008 triangles) reset the GPU as it
/// came into a Solari scene, and only with Glass Reflections on, since
/// blended hair is traced only then. Both an opaque and a blended one.
#[test]
fn solari_traces_a_glowing_mesh_too_big_to_sample_as_a_light() {
    if !solari_gpu_tests() {
        return;
    }
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (mut app, _, dir) = solari_rvg_app("big_light", true, Transform::from_xyz(0.0, 0.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y));
    let _ = std::fs::remove_dir_all(&dir);
    let world = app.world_mut();
    // 256 x 160 segments: about 81,000 triangles, past 65,535.
    let mesh = world.resource_mut::<Assets<Mesh>>().add(Sphere::new(1.0).mesh().uv(256, 160));
    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
    let glowing = materials.add(StandardMaterial {
        emissive: LinearRgba::rgb(2.0, 1.5, 1.0),
        ..default()
    });
    let glowing_blended = materials.add(StandardMaterial {
        base_color: Color::srgba(0.5, 0.4, 0.3, 0.5),
        alpha_mode: AlphaMode::Blend,
        emissive: LinearRgba::rgb(2.0, 1.5, 1.0),
        ..default()
    });
    world.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(glowing), Transform::from_xyz(-1.2, 0.0, 0.0)));
    world.spawn((Mesh3d(mesh), MeshMaterial3d(glowing_blended), Transform::from_xyz(1.2, 0.0, 0.0)));
    let mut traced = None;
    for _ in 0..SWITCH_SETTLE_FRAMES {
        app.update();
        if let Some(stats) = app.world_mut().resource_mut::<renzora::SolariDiagnostics>().frame.take() {
            traced = Some(stats.not_traced_fraction);
        }
    }
    let traced = traced.expect("the diagnostics reported, so Solari ran");
    println!("glowing meshes too big to sample as lights: {:.2}% not traced as drawn", traced * 100.0);
    assert!(traced < 0.01, "{:.1}% of the image is not traced as drawn", traced * 100.0);
}

const FILLER_INSTANCES: u32 = 400;

/// A surface given another's slot in the ray-traced scene is traced with its
/// own ray mask. Each instance's mask lives in a slot, and a slot is reused
/// once its instance leaves: the new mask is a small change to the GPU copy,
/// which was staged but never sent, so the slot kept the old one. Glass in a
/// solid surface's old slot was hit by every ray, and a solid surface in a
/// glass one's was skipped by every ray but the glass pass's. In the editor,
/// switching virtual geometry reshuffles every instance's slot, and whole
/// meshes dropped out of the traced scene that way.
#[test]
fn solari_traces_a_surface_with_its_own_mask_in_a_reused_slot() {
    if !solari_gpu_tests() {
        return;
    }
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let (mut app, _, dir) = solari_rvg_app("slot", true, Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y));
    let _ = std::fs::remove_dir_all(&dir);
    let world = app.world_mut();
    let mesh = world.resource_mut::<Assets<Mesh>>().add(Sphere::new(1.0).mesh().uv(32, 18));
    let backdrop = world.resource_mut::<Assets<Mesh>>().add(Cuboid::new(8.0, 8.0, 0.2));
    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.8, 0.9, 1.0, 0.3),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let opaque = materials.add(StandardMaterial::from_color(Color::srgb(0.8, 0.3, 0.3)));
    // A backdrop, traced throughout, so the scene is never empty.
    world.spawn((
        Mesh3d(backdrop),
        MeshMaterial3d(opaque.clone()),
        Transform::from_xyz(0.0, 0.0, -3.0),
    ));
    // Enough other instances, out of sight behind the backdrop, that one
    // changed mask is a small update rather than a whole-buffer upload, as in
    // any real scene. The whole-buffer upload was always right.
    for i in 0..FILLER_INSTANCES {
        world.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(opaque.clone()),
            Transform::from_xyz((i % 20) as f32 - 10.0, (i / 20) as f32 - 10.0, -6.0).with_scale(Vec3::splat(0.2)),
        ));
    }
    let solid = world.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(opaque), Transform::default())).id();
    let not_traced = |app: &mut App| {
        let mut last = None;
        for _ in 0..SWITCH_SETTLE_FRAMES {
            app.update();
            if let Some(stats) = app.world_mut().resource_mut::<renzora::SolariDiagnostics>().frame.take() {
                last = Some(stats.not_traced_fraction);
            }
        }
        last.expect("the diagnostics reported")
    };
    not_traced(&mut app);
    // In one frame, as a switch of virtual geometry does: the glass is given
    // the slot the solid sphere leaves.
    app.world_mut().despawn(solid);
    app.world_mut().spawn((Mesh3d(mesh), MeshMaterial3d(glass), Transform::default()));
    let fraction = not_traced(&mut app);
    println!("glass in the solid sphere's place: {:.2}% of the image not traced as drawn", fraction * 100.0);
    assert!(fraction < 0.01, "{:.1}% of the image is not traced as drawn", fraction * 100.0);
}

/// Under Solari, a model drawn as virtual geometry looks as it does drawn the
/// ordinary way. Virtual geometry draws a simplified level wherever it stays
/// within a pixel of the full detail, but Solari traces the full detail, and
/// a gap well under a pixel is still enough for light: where the drawn
/// surface lay just behind the traced one, its rays met the traced surface at
/// once and it went black; where it lay in front, they slipped through. The
/// apartment's curtain folds showed both, as dark patches and gaps. The bumpy
/// sphere is seen from far enough that coarse levels are drawn, and rendered
/// twice in fresh apps, with virtual geometry on and off.
#[test]
fn solari_lights_virtual_geometry_as_it_lights_ordinary_geometry() {
    if !solari_gpu_tests() {
        return;
    }
    let _serial = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let camera = Transform::from_xyz(0.0, 0.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y);
    let render = |virtual_geometry: bool| {
        let (mut app, target, dir) = solari_rvg_app(if virtual_geometry { "vg" } else { "plain" }, virtual_geometry, camera);
        spawn_bumpy_model(&mut app);
        // Long enough for streaming to settle and the denoiser's history to fill.
        for _ in 0..SOLARI_SETTLE_FRAMES {
            app.update();
        }
        let image = grab(&mut app, &target);
        let _ = std::fs::remove_dir_all(&dir);
        image
    };
    let virtual_image = render(true);
    let ordinary_image = render(false);
    dump("solari_vg", &virtual_image);
    dump("solari_ordinary", &ordinary_image);
    let luminance = |p: &[u8]| (p[0] as f32 + p[1] as f32 + p[2] as f32) / 3.0;
    // The model's pixels are those unlike the corner, which is background.
    let background = luminance(&ordinary_image[..4]);
    let (mut dark, mut covered) = (0usize, 0usize);
    for (v, o) in virtual_image.chunks_exact(4).zip(ordinary_image.chunks_exact(4)) {
        let (v, o) = (luminance(v), luminance(o));
        if o > 30.0 && (o - background).abs() > 10.0 {
            covered += 1;
            if v < 0.5 * o {
                dark += 1;
            }
        }
    }
    let fraction = dark as f32 / covered.max(1) as f32;
    println!("virtual geometry under Solari: {:.2}% of the model's pixels at under half the ordinary brightness ({covered} pixels)", fraction * 100.0);
    assert!(covered > 500, "the model is not on screen");
    // Drawing coarse levels left about 1.3% dark here; the finest, none.
    assert!(fraction < 0.004,"{:.2}% of the model goes dark drawn as virtual geometry under Solari", fraction * 100.0);
}

const SOLARI_SETTLE_FRAMES: usize = 200;

/// Whether the tests that trace with Solari can run: GPU tests are on, and the
/// adapter the app would pick can ray trace. CI's GPU job runs a software
/// Vulkan with no ray tracing, where building the app would panic asking for
/// features the device lacks; those tests say so and pass instead. Asked of a
/// Vulkan adapter, the backend `solari_rvg_app` requests.
fn solari_gpu_tests() -> bool {
    if std::env::var("RENZORA_GPU_TESTS").unwrap_or_default() != "1" {
        return false;
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = bevy::tasks::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()));
    let supported = adapter.is_ok_and(|a| a.features().contains(bevy::solari::SolariPlugins::required_wgpu_features()));
    if !supported {
        println!("this GPU cannot ray trace; skipped");
    }
    supported
}

/// The render target as it stands after the frame just run, copied out and
/// waited for. Bevy's `Readback` never delivered in the Solari app (the Solari
/// harness in `renzora_solari/tests` met the same), and with pipelined
/// rendering off the frame's work is already submitted when `update` returns,
/// so a copy queued now sees exactly that frame.
fn grab(app: &mut App, target: &Handle<Image>) -> Vec<u8> {
    use bevy::render::render_asset::RenderAssets;
    use bevy::render::render_resource::{
        BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, MapMode, PollType, TexelCopyBufferInfo,
        TexelCopyBufferLayout,
    };
    use bevy::render::renderer::{RenderDevice, RenderQueue};
    use bevy::render::texture::GpuImage;

    let render = app.sub_app(RenderApp).world();
    let device = render.resource::<RenderDevice>();
    let queue = render.resource::<RenderQueue>();
    let image = render
        .resource::<RenderAssets<GpuImage>>()
        .get(target.id())
        .expect("the target is on the GPU");
    // Four bytes a pixel; SIZE keeps a row a whole number of 256-byte rows.
    let row = SIZE * 4;
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some("vg_gpu_test_grab"),
        size: (row * SIZE) as u64,
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
        Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(MapMode::Read, |result| result.expect("the copy maps"));
    device.poll(PollType::wait_indefinitely()).expect("the GPU finishes the copy");
    let bytes = slice.get_mapped_range().expect("the copy is mapped").to_vec();
    buffer.unmap();
    bytes
}

/// A bumpy sphere stored as `.rvg`, written to the app's asset folder by
/// [`solari_rvg_app`], loaded and spawned. Its bumps are larger than the
/// diagnostics' match tolerance, so a coarse level and the full detail differ.
fn spawn_bumpy_model(app: &mut App) {
    let handle: Handle<bevy::gltf::Gltf> = app.world().resource::<AssetServer>().load("bumpy.rvg");
    let mut scene = None;
    for _ in 0..600 {
        app.update();
        if let Some(gltf) = app.world().resource::<Assets<bevy::gltf::Gltf>>().get(&handle) {
            scene = gltf.default_scene.clone();
            break;
        }
    }
    let scene = scene.expect("the .rvg model loaded");
    app.world_mut().spawn((Transform::default(), bevy::world_serialization::WorldAssetRoot(scene)));
}

/// An app with virtual geometry, the `.rvg` model loader and Renzora's Solari
/// plugin, a camera at `camera` routed a default `SolariGi`, the diagnostics
/// asked for, and every pipeline compiled. The bumpy model is written to its
/// asset folder (returned, to remove after). With `virtual_geometry` false,
/// the model draws the ordinary way, as with the project setting off.
fn solari_rvg_app(name: &str, virtual_geometry: bool, camera: Transform) -> (App, Handle<Image>, std::path::PathBuf) {
    use bevy::render::settings::{Backends, RenderCreation, WgpuSettings};
    use bevy::render::RenderPlugin;
    use bevy::solari::SolariPlugins;

    let dir = std::env::temp_dir().join(format!("renzora_rvg_solari_{name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut input = sphere(96, 192);
    for p in &mut input.positions {
        let v = Vec3::from(*p);
        let bump = 1.0 + 0.15 * ((v.x * 23.0).sin() * (v.y * 19.0).sin() * (v.z * 29.0).sin());
        *p = (v * bump).into();
    }
    let rvg = renzora_rvg::bake::build_model(&model_glb(&input)).unwrap();
    std::fs::write(dir.join("bumpy.rvg"), rvg.to_bytes()).unwrap();

    let mut app = App::new();
    app.insert_resource(renzora::GpuRaytracing { enabled: true });
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(AssetPlugin {
                file_path: dir.to_string_lossy().into_owned(),
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
    app.insert_resource(bevy::render::error_handler::RenderErrorHandler(fail_on_gpu_error));
    app.add_plugins((VirtualGeometryPlugin, renzora_rvg::model::RvgModelPlugin, renzora_solari::SolariPlugin));
    app.finish();
    app.cleanup();
    if !virtual_geometry {
        app.world_mut().resource_mut::<VirtualGeometrySettings>().enabled = false;
    }

    let world = app.world_mut();
    let mut target = Image::new_target_texture(SIZE, SIZE, TextureFormat::Rgba8UnormSrgb, None);
    target.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = world.resource_mut::<Assets<Image>>().add(target);
    world.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(2.0, 3.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let camera = world
        .spawn((
            Camera3d::default(),
            RenderTarget::Image(target.clone().into()),
            Msaa::Off,
            camera,
            // The figures come through the Ray Tracing panel's own path, which
            // measures the editor's focused camera.
            renzora::EditorCamera,
        ))
        .id();
    let source = world.spawn(renzora::SolariGi::default()).id();
    world.insert_resource(renzora::EffectRouting {
        routes: vec![(camera, vec![source])],
    });
    world.insert_resource(renzora::SolariDiagnosticsRequest {
        stats: true,
        ..default()
    });

    // Every pipeline up before the model arrives, so what is measured is the
    // model's own start, not the renderer's.
    for _ in 0..60 {
        app.update();
    }
    for _ in 0..6000 {
        let waiting = app.sub_app(RenderApp).world().resource::<PipelineCache>().waiting_pipelines().count();
        if waiting == 0 {
            break;
        }
        app.update();
    }
    (app, target, dir)
}

/// Frames [`solari_traces_an_rvg_model_as_drawn_from_the_start`] follows.
const FIRST_FRAMES: usize = 240;

/// Frames after a model first appears that it may take to be traced as drawn.
const SETTLE_FRAMES: usize = 10;

#[test]
fn transparent_forward() {
    transparent_sphere(Setup::ForwardWithPrepass);
}

#[test]
fn transparent_deferred() {
    transparent_sphere(Setup::Deferred);
}

#[test]
fn skinned_forward() {
    skinned_sphere(Setup::ForwardWithPrepass);
}

#[test]
fn skinned_deferred() {
    skinned_sphere(Setup::Deferred);
}

#[test]
fn graph_material_forward() {
    graph_material(Setup::ForwardWithPrepass);
}

#[test]
fn graph_material_deferred() {
    graph_material(Setup::Deferred);
}

#[test]
fn masked_card_forward() {
    masked_card(Setup::ForwardWithPrepass);
}

#[test]
fn masked_card_deferred() {
    masked_card(Setup::Deferred);
}

#[test]
fn mixed_scene_forward() {
    mixed_scene(Setup::ForwardWithPrepass);
}

#[test]
fn mixed_scene_deferred() {
    mixed_scene(Setup::Deferred);
}

#[test]
fn forward_matches_ordinary_geometry() {
    compare(Setup::Forward);
}

#[test]
fn forward_with_prepass_matches_ordinary_geometry() {
    compare(Setup::ForwardWithPrepass);
}

#[test]
fn editor_viewport_camera_matches_ordinary_geometry() {
    compare(Setup::EditorViewport);
}

#[test]
fn deferred_matches_ordinary_geometry() {
    compare(Setup::Deferred);
}
