//! Procedural "terrain flyover" behind the splash — an endless low-altitude
//! flight over rim-lit, fog-shrouded ridges, rendered by an isolated `Camera3d`
//! to an offscreen image that the splash shows as its background ([`TerrainView`]).
//!
//! The forward motion is faked by *scrolling the heightfield* rather than moving
//! the camera: the camera holds (with a gentle bob) while [`animate_terrain`]
//! re-evaluates every vertex from a noise function whose sample-Z advances with
//! time. That keeps the flight perfectly seamless and endless — there are no tiles
//! to recycle and never a visible repeat or pop. The mesh is a fixed wedge in front
//! of the camera; only its Y/normals change each frame (cheap: a few hundred
//! thousand noise evals, trivial for a GPU-light splash).
//!
//! The camera clears to **transparent**, so the moonlit ridges composite over the
//! night sky shader ([`crate::native_bg`]); distant ridges are fully fogged into
//! the horizon band so the two layers read as one continuous night. Render-to-texture +
//! a dedicated render layer keep it isolated from the editor's cameras; it only
//! exists while in [`SplashState::Splash`] (spawned/despawned by [`manage_terrain`]).

use bevy::asset::{Asset, RenderAssetUsages};
use bevy::camera::visibility::RenderLayers;
use bevy::camera::Hdr;
use bevy::camera::RenderTarget;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureFormat, TextureUsages};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use crate::SplashState;

/// Free render layer (5 = vello, 7 = material thumbs, 8 = model thumbs).
const TERRAIN_LAYER: usize = 6;
const RES: UVec2 = UVec2::new(1920, 1080);

// ── Terrain field extents (world units, camera at the origin looking down -Z) ──
/// Vertices across (X) and into the distance (Z).
const NX: usize = 128;
const NZ: usize = 180;
/// Half-width of the strip in X.
const HALF_W: f32 = 120.0;
/// Z of the nearest row (kept behind the camera so the bottom edge stays filled
/// as the camera flies forward) and the farthest row (well into the fog), both
/// measured relative to the mesh's snapped origin.
const FRONT_Z: f32 = 24.0;
const BACK_Z: f32 = -360.0;

/// Camera eye height and the height it aims at. The eye sits comfortably above the
/// tallest possible ridge (see `MAX_H`) so no peak ever clips through the lens as
/// we fly over it — that headroom is what lets the terrain stay STATIC (no
/// near-flattening hack), which is what makes the flight read as flight.
const CAM_Y: f32 = 46.0;
const LOOK_Y: f32 = 6.0;
const LOOK_Z: f32 = -150.0;

/// Forward flight speed (world units / sec). The terrain is fixed in world space;
/// the camera moves through it.
const SPEED: f32 = 16.0;

/// Peak ridge height and the horizontal noise frequency. Max actual height is
/// ~1.35·MAX_H (see [`height`]), kept below `CAM_Y` for lens clearance.
const MAX_H: f32 = 26.0;
const FREQ: f32 = 0.025;
/// Epsilon (world units) for the analytic-normal central difference.
const NORMAL_EPS: f32 = 1.6;

/// World-space spacing between consecutive depth rows — the quantum the mesh
/// snaps forward by, so a snap shifts the field by exactly one row (seamless).
fn cell_dz() -> f32 {
    (FRONT_Z - BACK_Z) / (NZ as f32 - 1.0)
}

/// The fullscreen UI image node (in the splash root) that shows the terrain render.
#[derive(Component)]
pub(crate) struct TerrainView;

#[derive(Component)]
struct TerrainCamera;

/// Marker on the terrain mesh entity (its `Transform` is snapped forward as the
/// camera flies, so the mesh always wraps the camera).
#[derive(Component)]
struct TerrainMesh;

/// Marker on every world entity the flyover owns, for one-shot teardown.
#[derive(Component)]
struct TerrainEntity;

#[derive(Resource, Default)]
struct TerrainScene {
    image: Handle<Image>,
    mesh: Handle<Mesh>,
    built: bool,
    /// The world-Z origin the current heights were built for; `None` until built.
    /// Heights only need rebuilding when the camera crosses into a new row.
    built_offset: Option<f32>,
}

/// UI material that runs the terrain image through `flyover.wgsl` (vignette + grain).
#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct FlyoverMaterial {
    /// x = time.
    #[uniform(0)]
    params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    image: Option<Handle<Image>>,
}

impl UiMaterial for FlyoverMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_splash/flyover.wgsl".into()
    }
}

pub(crate) fn register(app: &mut App) {
    bevy::asset::embedded_asset!(app, "flyover.wgsl");
    app.init_resource::<TerrainScene>()
        .add_plugins(UiMaterialPlugin::<FlyoverMaterial>::default())
        .add_systems(
            Update,
            (
                manage_terrain,
                attach_terrain_view,
                animate_camera,
                animate_terrain,
                update_flyover_material,
            ),
        );
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)] // a Bevy system; each param is a distinct world access
fn manage_terrain(
    mut commands: Commands,
    state: Res<State<SplashState>>,
    mut scene: ResMut<TerrainScene>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cam: Query<Entity, With<TerrainCamera>>,
    owned: Query<Entity, With<TerrainEntity>>,
) {
    let want = matches!(state.get(), SplashState::Splash);
    let built = !cam.is_empty();

    if want && !built {
        if scene.image == Handle::default() {
            scene.image = images.add(make_target(RES));
        }
        let image = scene.image.clone();
        let mesh = meshes.add(build_terrain_mesh(0.0));
        scene.mesh = mesh.clone();
        scene.built_offset = Some(0.0);
        spawn_terrain(&mut commands, &mut materials, image, mesh);
        scene.built = true;
    } else if !want && built {
        for e in &owned {
            commands.entity(e).try_despawn();
        }
        scene.built = false;
        scene.built_offset = None;
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

/// Attach the flyover material (sampling the rendered terrain image) to the splash
/// background node once both exist.
fn attach_terrain_view(
    mut commands: Commands,
    scene: Res<TerrainScene>,
    mut mats: ResMut<Assets<FlyoverMaterial>>,
    views: Query<Entity, (With<TerrainView>, Without<MaterialNode<FlyoverMaterial>>)>,
) {
    if !scene.built {
        return;
    }
    for e in &views {
        let handle = mats.add(FlyoverMaterial { params: Vec4::ZERO, image: Some(scene.image.clone()) });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn update_flyover_material(
    time: Res<Time>,
    mut mats: ResMut<Assets<FlyoverMaterial>>,
    views: Query<&MaterialNode<FlyoverMaterial>, With<TerrainView>>,
) {
    let t = time.elapsed_secs();
    for mat in &views {
        if let Some(mut m) = mats.get_mut(&mat.0) {
            m.params = Vec4::new(t, 0.0, 0.0, 0.0);
        }
    }
}

// ── Scene ────────────────────────────────────────────────────────────────────

fn spawn_terrain(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    image: Handle<Image>,
    mesh: Handle<Mesh>,
) {
    let layer = RenderLayers::layer(TERRAIN_LAYER);

    // Camera — HDR + bloom (so moonlit crests bloom). Clears transparent so the
    // night sky shows through above the ridgeline. (No distance fog; the far mesh
    // edge is hidden instead by the [`far_fade`] height taper sinking it flat.)
    commands.spawn((
        Camera3d::default(),
        Hdr,
        // Gentle bloom only — the default NATURAL preset blew bright moonlit crests
        // out to white, which read as "snow". Keep it subtle so highlights glow
        // without clipping.
        Bloom { intensity: 0.06, ..Bloom::NATURAL },
        Msaa::Sample4,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::NONE),
            order: -50,
            ..default()
        },
        RenderTarget::Image(image.into()),
        AmbientLight { color: Color::srgb(0.45, 0.5, 0.85), brightness: 130.0, affects_lightmapped_meshes: false },
        Transform::from_xyz(0.0, CAM_Y, 0.0).looking_at(Vec3::new(0.0, LOOK_Y, LOOK_Z), Vec3::Y),
        layer.clone(),
        TerrainCamera,
        TerrainEntity,
        renzora::HideInHierarchy,
        Name::new("Splash Terrain Camera"),
    ));

    // Warm "sun" key low and far behind the ridges → rim-lights every crest and
    // renders the dark ground as warm dark brown (not the desaturated grey that
    // cool light gave it).
    commands.spawn((
        DirectionalLight { color: Color::srgb(0.95, 0.72, 0.55), illuminance: 8000.0, ..default() },
        Transform::from_xyz(40.0, 55.0, -300.0).looking_at(Vec3::new(0.0, 0.0, -120.0), Vec3::Y),
        layer.clone(),
        TerrainEntity,
        renzora::HideInHierarchy,
        Name::new("Splash Terrain Sun"),
    ));
    // Cool fill from the near side so the ridge faces don't go fully black.
    commands.spawn((
        DirectionalLight { color: Color::srgb(0.30, 0.45, 0.95), illuminance: 2200.0, ..default() },
        Transform::from_xyz(-80.0, 40.0, 60.0).looking_at(Vec3::new(0.0, 0.0, -120.0), Vec3::Y),
        layer.clone(),
        TerrainEntity,
        renzora::HideInHierarchy,
        Name::new("Splash Terrain Fill"),
    ));

    // Dark, faintly-metallic ground so ridges read as plain silhouettes carrying
    // only the moonlight — no procedural texture. Double-sided + no culling so
    // winding can never hide it.
    let ground = materials.add(StandardMaterial {
        base_color: Color::srgb(0.035, 0.04, 0.06),
        perceptual_roughness: 0.7,
        metallic: 0.15,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(ground),
        Transform::IDENTITY,
        layer.clone(),
        TerrainMesh,
        TerrainEntity,
        renzora::HideInHierarchy,
        Name::new("Splash Terrain"),
    ));
}

// ── Heightfield ────────────────────────────────────────────────────────────────

/// Cheap deterministic hash → 0..1 (no rng crate; stable across runs).
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    x = ((x >> ((x >> 28).wrapping_add(4))) ^ x).wrapping_mul(277_803_737);
    (((x >> 22) ^ x) & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// Hash of integer lattice coords → 0..1 (offset so negatives stay positive).
fn lattice(ix: i32, iz: i32) -> f32 {
    let x = ix.wrapping_add(8192) as u32;
    let z = iz.wrapping_add(8192) as u32;
    hash01(x.wrapping_mul(374_761_393) ^ z.wrapping_mul(668_265_263))
}

/// Smooth (cubic-interpolated) value noise → 0..1.
fn vnoise(x: f32, z: f32) -> f32 {
    let xi = x.floor();
    let zi = z.floor();
    let xf = x - xi;
    let zf = z - zi;
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = zf * zf * (3.0 - 2.0 * zf);
    let (ix, iz) = (xi as i32, zi as i32);
    let a = lattice(ix, iz);
    let b = lattice(ix + 1, iz);
    let c = lattice(ix, iz + 1);
    let d = lattice(ix + 1, iz + 1);
    let ab = a + (b - a) * u;
    let cd = c + (d - c) * u;
    ab + (cd - ab) * v
}

/// Ridged multi-octave noise → 0..1, peaking along sharp crests (good for
/// rim-lit ridgelines).
///
/// Octave count is deliberately low (3). The terrain is static in world space and
/// the camera flies through it, so this isn't about temporal flicker — it's that
/// distant detail finer than the grid's world spacing (~2 u) would crawl/alias as
/// the viewpoint moves. With base wavelength ~40 u, three octaves bottom out at
/// ~10 u (≈5 samples/wavelength), which stays stable under camera motion.
fn ridged(mut x: f32, mut z: f32) -> f32 {
    let mut amp = 0.5;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..3 {
        let n = vnoise(x, z); // 0..1
        let r = 1.0 - (2.0 * n - 1.0).abs(); // crest at n = 0.5
        sum += r * amp;
        norm += amp;
        amp *= 0.5;
        x *= 2.0;
        z *= 2.0;
    }
    sum / norm
}

/// Terrain height at world (x, z). A static function of position only — there is
/// no time term, so the landscape never morphs; the camera moving over it is the
/// only thing that changes. Max value is ~1.35·MAX_H (rolling 0.35 + ridge² 1.0).
fn height(x: f32, z: f32) -> f32 {
    let sx = x * FREQ;
    let sz = z * FREQ;
    let rolling = vnoise(sx * 0.45, sz * 0.45); // big soft swells
    let r = ridged(sx, sz); // sharp crests
    (rolling * 0.35 + r * r) * MAX_H
}

/// Smoothstep (glam doesn't give us one on bare `f32`).
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How much the terrain stands up at local depth `lz`. The last rows before the
/// far edge fade down to flat so the back of the mesh melts into the fog instead
/// of ending as a hard, "collapsing" silhouette at the horizon. Based on the
/// camera-relative depth (local z), so the fade always sits at the far edge.
fn far_fade(lz: f32) -> f32 {
    smoothstep(BACK_Z, BACK_Z + 140.0, lz)
}

/// Local (x, z) for grid indices (i across, j into the distance). These are the
/// mesh's *local* vertex coords; the mesh entity's `Transform` adds the snapped
/// world offset, so world z = local z + offset.
fn grid_xz(i: usize, j: usize) -> (f32, f32) {
    let fx = i as f32 / (NX - 1) as f32;
    let fz = j as f32 / (NZ - 1) as f32;
    let x = -HALF_W + fx * (2.0 * HALF_W);
    let z = FRONT_Z + fz * (BACK_Z - FRONT_Z);
    (x, z)
}

/// Analytic normal from a central difference of [`height`].
fn normal(x: f32, z: f32) -> Vec3 {
    let e = NORMAL_EPS;
    let hl = height(x - e, z);
    let hr = height(x + e, z);
    let hd = height(x, z - e);
    let hu = height(x, z + e);
    let n = Vec3::new(hl - hr, 2.0 * e, hd - hu);
    if n == Vec3::ZERO {
        Vec3::Y
    } else {
        n.normalize()
    }
}

/// Positions (in mesh-local space) + smooth normals for the whole grid, sampling
/// the static heightfield at world z = local z + `offset_z`. When `offset_z`
/// advances by exactly one row ([`cell_dz`]), every vertex inherits its
/// neighbour's world height, so the visible surface is unchanged across a snap —
/// seamless.
fn field(offset_z: f32) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    let count = NX * NZ;
    let mut positions = Vec::with_capacity(count);
    let mut normals = Vec::with_capacity(count);
    for j in 0..NZ {
        for i in 0..NX {
            let (x, lz) = grid_xz(i, j);
            let wz = lz + offset_z;
            let y = height(x, wz) * far_fade(lz);
            positions.push([x, y, lz]);
            normals.push(normal(x, wz).to_array());
        }
    }
    (positions, normals)
}

fn build_terrain_mesh(offset_z: f32) -> Mesh {
    let (positions, normals) = field(offset_z);

    // Static index buffer (two triangles per grid quad).
    let mut indices: Vec<u32> = Vec::with_capacity((NX - 1) * (NZ - 1) * 6);
    for j in 0..NZ - 1 {
        for i in 0..NX - 1 {
            let a = (j * NX + i) as u32;
            let b = a + 1;
            let c = a + NX as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    let uvs = vec![[0.0f32, 0.0]; NX * NZ];

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

// ── Animation ────────────────────────────────────────────────────────────────

/// Snap the static terrain forward with the camera. The mesh's world origin is
/// quantised to whole rows ([`cell_dz`]) so that between snaps the terrain is
/// perfectly still in world space — the camera glides through it, giving true
/// parallax — and a snap shifts the field by exactly one row (invisible). Heights
/// are only rebuilt when the snapped origin changes (a handful of times a second),
/// not every frame.
fn animate_terrain(
    time: Res<Time>,
    mut scene: ResMut<TerrainScene>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mesh_tf: Query<&mut Transform, With<TerrainMesh>>,
) {
    if !scene.built {
        return;
    }
    let cam_z = -SPEED * time.elapsed_secs(); // camera travels toward -Z (flies forward)
    let dz = cell_dz();
    let offset = (cam_z / dz).round() * dz;

    // The mesh entity rides the snapped origin every frame (the camera glides the
    // sub-row remainder), so it always wraps the camera.
    for mut tf in &mut mesh_tf {
        tf.translation.z = offset;
    }

    // Only re-sample heights when we've crossed into a new row.
    if scene.built_offset == Some(offset) {
        return;
    }
    let Some(mut mesh) = meshes.get_mut(&scene.mesh) else { return };
    let (positions, normals) = field(offset);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    scene.built_offset = Some(offset);
}

/// Fly the camera forward through the static terrain, with a gentle hand-held
/// bob/sway + a touch of roll so the shot doesn't feel locked off.
fn animate_camera(time: Res<Time>, mut cam: Query<&mut Transform, With<TerrainCamera>>) {
    let t = time.elapsed_secs();
    let cam_z = -SPEED * t; // matches animate_terrain
    let sway_x = (t * 0.21).sin() * 3.0;
    let sway_y = (t * 0.17).sin() * 1.5;
    let pos = Vec3::new(sway_x, CAM_Y + sway_y, cam_z);
    let look = Vec3::new(sway_x * 0.3, LOOK_Y + (t * 0.13).sin(), cam_z + LOOK_Z);
    let mut tr = Transform::from_translation(pos).looking_at(look, Vec3::Y);
    tr.rotate_local_z((t * 0.15).sin() * 0.02);
    for mut c in &mut cam {
        *c = tr;
    }
}
