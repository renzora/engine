//! Procedural synthwave city behind the splash — a grid of box "buildings" lit
//! by coloured rim lights with bloom + distance fog, rendered by an isolated
//! `Camera3d` to an offscreen image that the splash shows as its background
//! ([`CityView`]). Render-to-texture + a dedicated render layer keep it fully
//! isolated from the editor's cameras; it only exists while in
//! [`SplashState::Splash`] (spawned/despawned by [`manage_city`]).

use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::render::view::Hdr;
use bevy::ui::widget::NodeImageMode;

use crate::SplashState;

/// Free render layer (5 = vello, 7 = material thumbs, 8 = model thumbs).
const CITY_LAYER: usize = 6;
const RES: UVec2 = UVec2::new(1920, 1080);
const GRID: i32 = 15; // buildings per side
const SPACING: f32 = 9.0;

/// The fullscreen UI image node (in the splash root) that shows the city render.
#[derive(Component)]
pub(crate) struct CityView;

#[derive(Component)]
struct CityCamera;

/// Marker on every world entity the city owns, for one-shot teardown.
#[derive(Component)]
struct CityEntity;

#[derive(Resource, Default)]
struct CityScene {
    image: Handle<Image>,
    built: bool,
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<CityScene>()
        .add_systems(Update, (manage_city, attach_city_view, animate_city));
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
        spawn_city(&mut commands, &mut meshes, &mut materials, scene.image.clone());
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

/// Attach the rendered image to the splash's background node once both exist.
fn attach_city_view(
    mut commands: Commands,
    scene: Res<CityScene>,
    views: Query<Entity, (With<CityView>, Without<ImageNode>)>,
) {
    if !scene.built {
        return;
    }
    for e in &views {
        commands.entity(e).insert(ImageNode {
            image: scene.image.clone(),
            image_mode: NodeImageMode::Stretch,
            ..default()
        });
    }
}

// ── Scene ────────────────────────────────────────────────────────────────────

/// Cheap deterministic hash → 0..1 (no rng crate; stable across runs).
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    x = ((x >> ((x >> 28).wrapping_add(4))) ^ x).wrapping_mul(277_803_737);
    (((x >> 22) ^ x) & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

fn spawn_city(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    image: Handle<Image>,
) {
    let layer = RenderLayers::layer(CITY_LAYER);
    let sky = Color::srgb(0.020, 0.015, 0.045);

    // Camera — HDR + bloom + fog; orbits the city (see `animate_city`).
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        Msaa::Sample4,
        Camera {
            clear_color: ClearColorConfig::Custom(sky),
            order: -50,
            ..default()
        },
        RenderTarget::Image(image.into()),
        DistanceFog {
            color: Color::srgb(0.10, 0.05, 0.18),
            falloff: FogFalloff::Exponential { density: 0.010 },
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

    // Coloured rim lights (synthwave magenta + cyan) and a soft cool key.
    commands.spawn((
        DirectionalLight { color: Color::srgb(1.0, 0.25, 0.7), illuminance: 4000.0, ..default() },
        Transform::from_xyz(-60.0, 50.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer.clone(),
        CityEntity,
        renzora::HideInHierarchy,
        Name::new("Splash City Light M"),
    ));
    commands.spawn((
        DirectionalLight { color: Color::srgb(0.25, 0.7, 1.0), illuminance: 4000.0, ..default() },
        Transform::from_xyz(60.0, 40.0, -20.0).looking_at(Vec3::ZERO, Vec3::Y),
        layer.clone(),
        CityEntity,
        renzora::HideInHierarchy,
        Name::new("Splash City Light C"),
    ));

    // Ground.
    let ground = meshes.add(Cuboid::new(600.0, 1.0, 600.0));
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.015, 0.015, 0.03),
        perceptual_roughness: 0.85,
        ..default()
    });
    commands.spawn((
        Mesh3d(ground),
        MeshMaterial3d(ground_mat),
        Transform::from_xyz(0.0, -0.5, 0.0),
        layer.clone(),
        CityEntity,
        renzora::HideInHierarchy,
        Name::new("Splash City Ground"),
    ));

    // Shared unit cube, scaled per building.
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // A small palette of materials: dark (lit by rim lights) + neon (emissive,
    // blooms). Picked per building by hash so the skyline has scattered lit towers.
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.035, 0.05),
        perceptual_roughness: 0.55,
        metallic: 0.2,
        ..default()
    });
    let neon_colors = [
        Color::srgb(0.0, 0.9, 1.0),  // cyan
        Color::srgb(1.0, 0.2, 0.8),  // magenta
        Color::srgb(1.0, 0.55, 0.1), // amber
        Color::srgb(0.5, 0.3, 1.0),  // violet
    ];
    let neon: Vec<Handle<StandardMaterial>> = neon_colors
        .iter()
        .map(|cc| {
            let l = cc.to_linear();
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.02, 0.02, 0.03),
                emissive: LinearRgba::new(l.red * 3.5, l.green * 3.5, l.blue * 3.5, 1.0),
                perceptual_roughness: 0.4,
                ..default()
            })
        })
        .collect();

    let half = GRID / 2;
    for i in -half..=half {
        for j in -half..=half {
            let seed = ((i + 64) as u32) << 16 | (j + 64) as u32;
            // Carve some plots into roads/plazas.
            if hash01(seed) < 0.12 {
                continue;
            }
            let jx = (hash01(seed ^ 0x1111) - 0.5) * SPACING * 0.25;
            let jz = (hash01(seed ^ 0x2222) - 0.5) * SPACING * 0.25;
            let x = i as f32 * SPACING + jx;
            let z = j as f32 * SPACING + jz;
            let h = 4.0 + hash01(seed ^ 0x3333).powf(2.0) * 34.0;
            let w = SPACING * (0.5 + hash01(seed ^ 0x4444) * 0.25);
            let d = SPACING * (0.5 + hash01(seed ^ 0x5555) * 0.25);

            let mat = if hash01(seed ^ 0x6666) > 0.62 {
                neon[(hash01(seed ^ 0x7777) * neon.len() as f32) as usize % neon.len()].clone()
            } else {
                dark.clone()
            };

            commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(mat),
                Transform {
                    translation: Vec3::new(x, h * 0.5, z),
                    scale: Vec3::new(w, h, d),
                    ..default()
                },
                layer.clone(),
                CityEntity,
                renzora::HideInHierarchy,
                Name::new("Splash City Building"),
            ));
        }
    }
}

// ── Animation ────────────────────────────────────────────────────────────────

fn animate_city(time: Res<Time>, mut cam: Query<&mut Transform, With<CityCamera>>) {
    let t = time.elapsed_secs();
    let a = t * 0.04;
    let r = 88.0;
    let pos = Vec3::new(a.cos() * r, 30.0 + 4.0 * (t * 0.15).sin(), a.sin() * r);
    for mut tr in &mut cam {
        *tr = Transform::from_translation(pos).looking_at(Vec3::new(0.0, 11.0, 0.0), Vec3::Y);
    }
}
