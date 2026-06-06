//! Procedural synthwave city behind the splash — a grid of box "buildings" lit
//! by coloured rim lights with bloom + distance fog, rendered by an isolated
//! `Camera3d` to an offscreen image that the splash shows as its background
//! ([`CityView`]). The camera clears to **transparent**, so the buildings
//! composite over the animated grid/network shader background ([`crate::native_bg`])
//! rather than hiding it. Render-to-texture + a dedicated render layer keep it
//! isolated from the editor's cameras; it only exists while in
//! [`SplashState::Splash`] (spawned/despawned by [`manage_city`]).

use bevy::asset::Asset;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureFormat, TextureUsages};
use bevy::render::view::Hdr;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use crate::SplashState;

/// Free render layer (5 = vello, 7 = material thumbs, 8 = model thumbs).
const CITY_LAYER: usize = 6;
const RES: UVec2 = UVec2::new(1920, 1080);
const GRID: i32 = 31; // buildings per side
const SPACING: f32 = 9.0;
/// Seconds each camera "shot" holds before cutting to the next angle.
const SHOT_SECS: f32 = 9.0;
/// Count of small drifting/spinning cubes floating over the city.
const SMALL_CUBES: usize = 2400;
/// Half-extent the small cubes scatter across (x/z).
const CITY_EXTENT: f32 = 160.0;

/// The fullscreen UI image node (in the splash root) that shows the city render.
#[derive(Component)]
pub(crate) struct CityView;

#[derive(Component)]
struct CityCamera;

/// A building's rest height + a position-based phase, so [`animate_buildings`]
/// can ripple the skyline up and down like a waveform.
#[derive(Component)]
struct CityBuilding {
    base_h: f32,
    base_w: f32,
    base_d: f32,
    phase: f32,
    /// Base elevation, so buildings don't all sit on the same floor level.
    floor_y: f32,
}

/// A small floating cube that spins in place (and breathes with the rest).
#[derive(Component)]
struct SpinCube {
    axis: Vec3,
    speed: f32,
    phase: f32,
    scale: f32,
}

/// Marker on every world entity the city owns, for one-shot teardown.
#[derive(Component)]
struct CityEntity;

#[derive(Resource, Default)]
struct CityScene {
    image: Handle<Image>,
    built: bool,
}

/// Glitch state — `amount` spikes to 1 on each camera cut and decays.
#[derive(Resource, Default)]
struct CityGlitch {
    last_shot: i64,
    amount: f32,
}

/// UI material that runs the city image through `glitch.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct GlitchMaterial {
    /// x = time, y = glitch intensity.
    #[uniform(0)]
    params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    image: Option<Handle<Image>>,
}

impl UiMaterial for GlitchMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_splash/glitch.wgsl".into()
    }
}

pub(crate) fn register(app: &mut App) {
    bevy::asset::embedded_asset!(app, "glitch.wgsl");
    app.init_resource::<CityScene>()
        .init_resource::<CityGlitch>()
        .add_plugins(UiMaterialPlugin::<GlitchMaterial>::default())
        .add_systems(
            Update,
            (manage_city, attach_city_view, animate_city, animate_buildings, animate_spincubes, update_glitch),
        );
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

fn manage_city(
    mut commands: Commands,
    state: Res<State<SplashState>>,
    mut scene: ResMut<CityScene>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cam: Query<Entity, With<CityCamera>>,
    owned: Query<Entity, With<CityEntity>>,
) {
    let want = matches!(state.get(), SplashState::Splash);
    let built = !cam.is_empty();

    if want && !built {
        if scene.image == Handle::default() {
            scene.image = images.add(make_target(RES));
        }
        let image = scene.image.clone();
        spawn_city(&mut commands, &mut meshes, &mut materials, image);
        scene.built = true;
    } else if !want && built {
        for e in &owned {
            commands.entity(e).try_despawn();
        }
        scene.built = false;
    }
}

fn make_target(size: UVec2) -> Image {
    let extent = Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 };
    let mut img = Image {
        data: Some(vec![0u8; (extent.width * extent.height * 4) as usize]),
        ..default()
    };
    img.texture_descriptor.size = extent;
    img.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    img.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    img
}

/// Attach the glitch material (sampling the rendered city image) to the splash
/// background node once both exist.
fn attach_city_view(
    mut commands: Commands,
    scene: Res<CityScene>,
    mut mats: ResMut<Assets<GlitchMaterial>>,
    views: Query<Entity, (With<CityView>, Without<MaterialNode<GlitchMaterial>>)>,
) {
    if !scene.built {
        return;
    }
    for e in &views {
        let handle = mats.add(GlitchMaterial { params: Vec4::ZERO, image: Some(scene.image.clone()) });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

/// Spike the glitch on each camera cut, then decay it; feed time + intensity to
/// the material.
fn update_glitch(
    time: Res<Time>,
    mut glitch: ResMut<CityGlitch>,
    mut mats: ResMut<Assets<GlitchMaterial>>,
    views: Query<&MaterialNode<GlitchMaterial>, With<CityView>>,
) {
    let t = time.elapsed_secs();
    let shot = (t / SHOT_SECS).floor() as i64;
    if shot != glitch.last_shot {
        glitch.last_shot = shot;
        glitch.amount = 1.0;
    }
    glitch.amount = (glitch.amount - time.delta_secs() / 0.45).max(0.0);
    for mat in &views {
        if let Some(m) = mats.get_mut(&mat.0) {
            m.params = Vec4::new(t, glitch.amount, 0.0, 0.0);
        }
    }
}

// ── Scene ────────────────────────────────────────────────────────────────────

/// Cheap deterministic hash → 0..1 (no rng crate; stable across runs).
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    x = ((x >> ((x >> 28).wrapping_add(4))) ^ x).wrapping_mul(277_803_737);
    (((x >> 22) ^ x) & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// Build the city geometry, lights and camera.
fn spawn_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    image: Handle<Image>,
) {
    let layer = RenderLayers::layer(CITY_LAYER);

    // Camera — HDR + bloom + fog; clears transparent so the grid/network show
    // through. Animated in `animate_city`.
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        Msaa::Sample4,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::NONE),
            order: -50,
            ..default()
        },
        RenderTarget::Image(image.into()),
        DistanceFog {
            color: Color::srgb(0.10, 0.05, 0.18),
            falloff: FogFalloff::Exponential { density: 0.008 },
            ..default()
        },
        AmbientLight { color: Color::srgb(0.6, 0.7, 1.0), brightness: 220.0, affects_lightmapped_meshes: false },
        Transform::from_xyz(85.0, 30.0, 85.0).looking_at(Vec3::new(0.0, 10.0, 0.0), Vec3::Y),
        layer.clone(),
        CityCamera,
        CityEntity,
        renzora::HideInHierarchy,
        Name::new("Splash City Camera"),
    ));

    // Coloured rim lights (synthwave magenta + cyan).
    commands.spawn((
        DirectionalLight { color: Color::srgb(1.0, 0.25, 0.7), illuminance: 4500.0, ..default() },
        Transform::from_xyz(-60.0, 50.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer.clone(),
        CityEntity,
        renzora::HideInHierarchy,
        Name::new("Splash City Light M"),
    ));
    commands.spawn((
        DirectionalLight { color: Color::srgb(0.25, 0.7, 1.0), illuminance: 4500.0, ..default() },
        Transform::from_xyz(60.0, 40.0, -20.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer.clone(),
        CityEntity,
        renzora::HideInHierarchy,
        Name::new("Splash City Light C"),
    ));

    // Shared unit cube, scaled per building. No ground plane — the shader grid
    // shows through as the floor.
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // All buildings share one dark material — lit only by the coloured rim
    // lights, so they read as dark silhouettes against the glowing sky.
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.035, 0.05),
        perceptual_roughness: 0.55,
        metallic: 0.2,
        ..default()
    });

    let half = GRID / 2;
    for i in -half..=half {
        for j in -half..=half {
            let seed = (((i + 64) as u32) << 16) | (j + 64) as u32;
            if hash01(seed) < 0.12 {
                continue; // carve roads/plazas
            }
            let jx = (hash01(seed ^ 0x1111) - 0.5) * SPACING * 0.25;
            let jz = (hash01(seed ^ 0x2222) - 0.5) * SPACING * 0.25;
            let x = i as f32 * SPACING + jx;
            let z = j as f32 * SPACING + jz;
            let h = 4.0 + hash01(seed ^ 0x3333).powf(2.0) * 34.0;
            let w = SPACING * (0.5 + hash01(seed ^ 0x4444) * 0.25);
            let d = SPACING * (0.5 + hash01(seed ^ 0x5555) * 0.25);
            // Radial phase → waves ripple outward from the centre (long, gentle
            // wavelength).
            let phase = (x * x + z * z).sqrt() * 0.10;
            // Varied base elevation so the floor isn't a single flat plane.
            let floor_y = hash01(seed ^ 0x8888).powf(1.6) * 20.0 - 4.0;

            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(dark.clone()),
                Transform {
                    translation: Vec3::new(x, floor_y + h * 0.5, z),
                    scale: Vec3::new(w, h, d),
                    ..default()
                },
                CityBuilding { base_h: h, base_w: w, base_d: d, phase, floor_y },
                layer.clone(),
                CityEntity,
                renzora::HideInHierarchy,
                Name::new("Splash City Building"),
            ));
        }
    }

    // ── Thousands of small spinning shapes (mixed primitives, muted colours) ──
    let shapes: [Handle<Mesh>; 5] = [
        cube.clone(),
        meshes.add(Sphere::new(0.6)),
        meshes.add(Cylinder::new(0.45, 1.1)),
        meshes.add(Capsule3d::new(0.35, 0.8)),
        meshes.add(Torus::new(0.22, 0.5)),
    ];
    // Muted, low-intensity emissive palette (not bright).
    let palette = [
        (0.18, 0.10, 0.22), // violet
        (0.09, 0.18, 0.20), // teal
        (0.20, 0.13, 0.07), // amber
        (0.11, 0.19, 0.13), // green
        (0.17, 0.09, 0.16), // magenta
        (0.09, 0.13, 0.22), // blue
    ];
    let small_mats: Vec<Handle<StandardMaterial>> = palette
        .iter()
        .map(|(r, g, b)| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.02, 0.02, 0.03),
                emissive: LinearRgba::new(*r, *g, *b, 1.0),
                perceptual_roughness: 0.35,
                metallic: 0.3,
                ..default()
            })
        })
        .collect();
    for k in 0..SMALL_CUBES {
        let s = (k as u32).wrapping_mul(2_654_435_761).wrapping_add(101);
        let x = (hash01(s ^ 0xA1) - 0.5) * 2.0 * CITY_EXTENT;
        let z = (hash01(s ^ 0xB2) - 0.5) * 2.0 * CITY_EXTENT;
        let y = 3.0 + hash01(s ^ 0xC3) * 90.0;
        let axis = Vec3::new(
            hash01(s ^ 0xD4) - 0.5,
            hash01(s ^ 0xE5) - 0.5,
            hash01(s ^ 0xF6) - 0.5,
        )
        .normalize_or_zero();
        let axis = if axis == Vec3::ZERO { Vec3::Y } else { axis };
        let scale = 0.35 + hash01(s ^ 0x17) * 1.1;
        let mesh = shapes[(hash01(s ^ 0x4A) * shapes.len() as f32) as usize % shapes.len()].clone();
        let mat = small_mats[(hash01(s ^ 0x5B) * small_mats.len() as f32) as usize % small_mats.len()].clone();
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform { translation: Vec3::new(x, y, z), scale: Vec3::splat(scale), ..default() },
            SpinCube {
                axis,
                speed: 0.4 + hash01(s ^ 0x28) * 1.8,
                phase: hash01(s ^ 0x39) * std::f32::consts::TAU,
                scale,
            },
            layer.clone(),
            CityEntity,
            renzora::HideInHierarchy,
            Name::new("Splash City Spark"),
        ));
    }
}

/// Slow global breathing factor shared by buildings and sparks.
fn breathe(t: f32) -> f32 {
    1.0 + 0.08 * (t * 0.4).sin()
}

// ── Animation ────────────────────────────────────────────────────────────────

/// Periodically cut between a few camera "shots" (each slowly drifting), like a
/// demo reel — mid orbit, top-down, far wide push-in, high distant orbit.
fn animate_city(time: Res<Time>, mut cam: Query<&mut Transform, With<CityCamera>>) {
    let t = time.elapsed_secs();
    let shot = (t / SHOT_SECS).floor();
    let lt = t - shot * SHOT_SECS; // local time within this shot
    let idx = (shot as i64).rem_euclid(4);

    let (pos, look, up) = match idx {
        0 => {
            // Mid orbit with a vertical bob.
            let a = lt * 0.14;
            (
                Vec3::new(a.cos() * 82.0, 26.0 + 5.0 * (lt * 0.2).sin(), a.sin() * 82.0),
                Vec3::new(0.0, 11.0, 0.0),
                Vec3::Y,
            )
        }
        1 => {
            // Top-down, slowly rotating.
            let a = lt * 0.12;
            (Vec3::new(a.cos() * 18.0, 125.0, a.sin() * 18.0), Vec3::ZERO, Vec3::Z)
        }
        2 => {
            // Far, wide, slow push-in.
            let r = 150.0 - lt * 3.0;
            (Vec3::new(28.0 * (lt * 0.1).sin(), 22.0, r), Vec3::new(0.0, 12.0, 0.0), Vec3::Y)
        }
        _ => {
            // High distant orbit.
            let a = lt * 0.07;
            (Vec3::new(a.cos() * 135.0, 72.0, a.sin() * 135.0), Vec3::new(0.0, 5.0, 0.0), Vec3::Y)
        }
    };

    for mut tr in &mut cam {
        *tr = Transform::from_translation(pos).looking_at(look, up);
    }
}

/// Ripple the skyline up and down like a waveform — each building oscillates
/// around its rest height, phased by its distance from the centre. Buildings
/// grow upward from the ground (base stays at y = 0).
fn animate_buildings(time: Res<Time>, mut q: Query<(&CityBuilding, &mut Transform)>) {
    let t = time.elapsed_secs();
    let br = breathe(t);
    for (b, mut tr) in &mut q {
        let amp = 1.5 + b.base_h * 0.18;
        let h = (b.base_h + amp * (b.phase - t * 0.45).sin()).max(2.0) * br;
        tr.scale = Vec3::new(b.base_w * br, h, b.base_d * br);
        tr.translation.y = b.floor_y + h * 0.5;
    }
}

/// Spin the small cubes in place and breathe them (a touch more than buildings).
fn animate_spincubes(time: Res<Time>, mut q: Query<(&SpinCube, &mut Transform)>) {
    let t = time.elapsed_secs();
    let br = 1.0 + 0.2 * (t * 0.5).sin();
    for (s, mut tr) in &mut q {
        tr.rotation = Quat::from_axis_angle(s.axis, t * s.speed + s.phase);
        tr.scale = Vec3::splat(s.scale * br);
    }
}
