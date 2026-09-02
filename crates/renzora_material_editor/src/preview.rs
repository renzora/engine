//! Material preview — renders the compiled material on a preview shape with an
//! orbit camera, displayed via render-to-texture. The panel chrome lives in the
//! `preview_panel` module; this file owns the render plugin,
//! resources, and the shader/texture hot-swap systems.

use std::marker::PhantomData;

use bevy::anti_alias::smaa::{Smaa, SmaaPreset};
use bevy::asset::RenderAssetUsages;
use bevy::camera::primitives::MeshAabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::camera::Hdr;
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::core_pipeline::Skybox;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::light::GeneratedEnvironmentMapLight;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    TextureViewDimension,
};
use uuid::Uuid;

use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};
use renzora_editor_framework::EditorSelection;
use renzora_shader::material::material_ref::MaterialRef;
use renzora_shader::material::runtime::{
    new_graph_material, FallbackTexture, GraphMaterial, GraphMaterialShaderState,
};

use crate::{MaterialEditMode, MaterialEditorState};

/// This preview's private render layer. Allocated in the contract crate
/// alongside every other offscreen rig's — see the registry there for why.
pub use renzora::core::viewport_types::MATERIAL_PREVIEW_LAYER;

/// Side length of the preview's render target, in pixels.
///
/// Comfortably above the size the panel displays it at, which makes the
/// `ImageNode`'s downscale a cheap supersample on top of the camera's SMAA pass.
/// That second helping matters here: SMAA only smooths edges it can find in the
/// final image, and the aliasing that makes a material look wrong is usually
/// specular sparkle off a normal map, which supersampling is the only cure for.
const PREVIEW_RESOLUTION: u32 = 1024;

/// SMAA quality for the preview. `Ultra` is the top preset and this is one small
/// off-screen view rendered only while the panel is open, so there's no reason
/// to economize.
const PREVIEW_SMAA_PRESET: SmaaPreset = SmaaPreset::Ultra;

/// Equirectangular HDRI used as the preview backdrop + image-based lighting.
/// Embedded in the binary so the preview's environment is always available —
/// the material editor is an editor-only feature with no guaranteed runtime
/// asset path, and baking the bytes in keeps it self-contained.
const BACKDROP_HDR: &[u8] = include_bytes!("../../../assets/images/pretoria_gardens_1k.hdr");

/// `Skybox.brightness` for the backdrop cube (cd/m²). The preview is lit by two
/// bright directional key lights (6000 + 2000 lux), so the camera exposure is
/// calibrated for that range — a skybox at ~1.0 cd/m² tonemaps to black against
/// it. ~1000 (the engine's panorama-skybox convention, `energy * 1000`) brings
/// the sky into the same exposure band as the lit shape. TUNABLE.
const BACKDROP_SKY_BRIGHTNESS: f32 = 1000.0;
/// IBL strength (cd/m²) the backdrop contributes via `GeneratedEnvironmentMapLight`.
/// Paired with `BACKDROP_SKY_BRIGHTNESS` (Bevy's environment-map examples set the
/// skybox brightness and env-map intensity to the same value) so reflections
/// match the visible sky. TUNABLE if reflections read too hot or too flat.
const BACKDROP_IBL_INTENSITY: f32 = 1000.0;
/// Ceiling for the **visible skybox** cube (per channel). An HDRI's sun core can
/// exceed f16's max (65504); stored as `+Inf` it would render the sun as a black
/// hole. 8000 keeps a punchy HDR sky (the sun still blows to white through the
/// `* brightness`) while staying finite.
const BACKDROP_SKY_MAX_RADIANCE: f32 = 8000.0;
/// Ceiling for the **IBL** cube (per channel) — far lower than the skybox's.
/// `GeneratedEnvironmentMapLight` *prefilters* the cube with a limited-sample GGX
/// integral; a lone super-bright texel (the 8000 sun, bright clouds) landing in a
/// rough sample cone blows that whole prefilter texel out, which — multiplied by
/// the high IBL intensity — paints hard white squares across glossy reflections.
/// Clamping the IBL source hard tames those fireflies: a clear sky (~3) still
/// reflects at full strength, the sun reflects as a soft white bloom instead of a
/// blocky hot-spot. TUNABLE — lower for cleaner reflections, higher for punchier
/// (blockier) ones. The visible backdrop is unaffected (separate cube).
const BACKDROP_ENV_MAX_RADIANCE: f32 = 50.0;

// ── Resources ───────────────────────────────────────────────────────────────

/// Cubemaps reprojected from [`BACKDROP_HDR`] at startup. Two of them, on purpose:
/// `sky` is the crisp HDR cube for the visible `Skybox`; `env` is an
/// aggressively-clamped cube for the `GeneratedEnvironmentMapLight` so its
/// prefilter doesn't paint firefly blocks into glossy reflections (see
/// [`BACKDROP_ENV_MAX_RADIANCE`]). `None` if the embedded HDR failed to decode.
#[derive(Resource, Default)]
pub struct MaterialPreviewEnv {
    pub sky: Option<Handle<Image>>,
    pub env: Option<Handle<Image>>,
}

#[derive(Resource)]
pub struct MaterialPreviewImage {
    pub handle: Handle<Image>,
    pub size: (u32, u32),
}

impl Default for MaterialPreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            size: (PREVIEW_RESOLUTION, PREVIEW_RESOLUTION),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PreviewShape {
    Sphere,
    Cube,
    Cylinder,
    Torus,
    Plane,
    /// The mesh of the entity whose material is being edited, normalized to the
    /// primitives' size. Only selectable while that entity exists and has a
    /// `Mesh3d` — a material authored for a specific model reads very
    /// differently on a sphere than on the geometry it actually ships on.
    Selected,
}

impl PreviewShape {
    /// The built-in primitives, always available. [`PreviewShape::Selected`] is
    /// deliberately absent: the picker appends it only when there's a mesh to
    /// show, so a stale entry can never be chosen.
    pub const ALL: &[PreviewShape] = &[
        PreviewShape::Sphere,
        PreviewShape::Cube,
        PreviewShape::Cylinder,
        PreviewShape::Torus,
        PreviewShape::Plane,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Sphere => "Sphere",
            Self::Cube => "Cube",
            Self::Cylinder => "Cylinder",
            Self::Torus => "Torus",
            Self::Plane => "Plane",
            Self::Selected => "Selected Mesh",
        }
    }

    /// Phosphor icon *name* for this shape (resolved to a glyph by the native UI).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Sphere => "globe-hemisphere-east",
            Self::Cube => "cube",
            Self::Cylinder => "cylinder",
            Self::Torus => "circle-dashed",
            Self::Plane => "square",
            Self::Selected => "polygon",
        }
    }
}

#[derive(Resource)]
pub struct MaterialPreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    pub shape: PreviewShape,
    pub auto_rotate: bool,
    pub dark_bg: bool,
    /// Show the embedded HDRI as a skybox backdrop + use it for reflections.
    /// When off, the camera falls back to the flat `dark_bg` clear color and IBL
    /// collapses to zero intensity (kept attached — IBL bind slots can't be
    /// added back at runtime).
    pub show_backdrop: bool,
}

impl Default for MaterialPreviewOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.8,
            pitch: 0.3,
            distance: 3.0,
            target: Vec3::ZERO,
            shape: PreviewShape::Sphere,
            auto_rotate: false,
            dark_bg: true,
            show_backdrop: true,
        }
    }
}

/// Tracks the WGSL hash to detect when preview mesh material needs updating.
#[derive(Resource, Default)]
pub struct MaterialPreviewTracker {
    pub last_wgsl_hash: Option<u64>,
}

// ── Components ──────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct MaterialPreviewCamera;

#[derive(Component)]
pub struct MaterialPreviewLight;

#[derive(Component)]
pub struct MaterialPreviewMesh;

// ── Setup system ────────────────────────────────────────────────────────────

fn setup_material_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GraphMaterial>>,
    fallback: Res<FallbackTexture>,
    orbit: Res<MaterialPreviewOrbit>,
) {
    let size = Extent3d {
        width: PREVIEW_RESOLUTION,
        height: PREVIEW_RESOLUTION,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        data: Some(vec![0u8; (size.width * size.height * 4) as usize]),
        ..default()
    };
    image.texture_descriptor.size = size;
    image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;

    let image_handle = images.add(image);

    commands.insert_resource(MaterialPreviewImage {
        handle: image_handle.clone(),
        size: (PREVIEW_RESOLUTION, PREVIEW_RESOLUTION),
    });

    // Decode the embedded HDRI and reproject it into two power-of-two cubes once,
    // up front (one crisp for the skybox, one clamped for IBL). CPU work, but it's
    // a single startup reprojection and lets us attach IBL to the camera from spawn.
    let (sky_cube, env_cube) = decode_backdrop_cubes(&mut images);
    commands.insert_resource(MaterialPreviewEnv {
        sky: sky_cube.clone(),
        env: env_cube.clone(),
    });

    // Camera
    let camera = commands
        .spawn((
            Camera3d::default(),
            Hdr,
            (NormalPrepass, DepthPrepass, MotionVectorPrepass),
            // SMAA rather than MSAA: MSAA only covers geometry edges and would
            // have to multisample the three prepass targets as well, while SMAA
            // is a single post pass that also catches the shading and alpha-mask
            // edges a material preview is full of. It requires MSAA to be off,
            // which this camera already wanted.
            //
            // Grouped into nested tuples (here and for the prepasses above)
            // because a spawn bundle tops out at 16 elements and this camera was
            // already sitting on the ceiling.
            (
                Msaa::Off,
                Smaa {
                    preset: PREVIEW_SMAA_PRESET,
                },
            ),
            Camera {
                clear_color: ClearColorConfig::Custom(Color::srgba(0.08, 0.08, 0.1, 1.0)),
                order: -5,
                is_active: false,
                ..default()
            },
            RenderTarget::Image(image_handle.into()),
            Transform::from_xyz(0.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
            RenderLayers::layer(MATERIAL_PREVIEW_LAYER),
            MaterialPreviewCamera,
            IsolatedCamera,
            HideInHierarchy,
            EditorLocked,
            Name::new("Material Preview Camera"),
        ))
        .id();

    // Backdrop + IBL. The `GeneratedEnvironmentMapLight` must be present from the
    // camera's first render — Bevy locks the mesh-view layout (which includes the
    // IBL bind slots) on first render and adding them later crashes wgpu — so we
    // attach it here even when the backdrop starts off, collapsing it to zero
    // intensity instead. The `Skybox` pass is standalone (not a mesh-view
    // binding), so it can be added/removed freely by `sync_preview_backdrop`.
    if let Some(env) = &env_cube {
        commands.entity(camera).insert(GeneratedEnvironmentMapLight {
            environment_map: env.clone(),
            intensity: if orbit.show_backdrop {
                BACKDROP_IBL_INTENSITY
            } else {
                0.0
            },
            ..default()
        });
    }
    if orbit.show_backdrop {
        if let Some(sky) = &sky_cube {
            commands.entity(camera).insert(Skybox {
                image: Some(sky.clone()),
                brightness: BACKDROP_SKY_BRIGHTNESS,
                rotation: Quat::IDENTITY,
            });
        }
    }

    // Directional light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.98, 0.95),
            illuminance: 6000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.4, 0.0)),
        RenderLayers::layer(MATERIAL_PREVIEW_LAYER),
        MaterialPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Preview Light"),
    ));

    // Fill light (softer, from the other side)
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.6, 0.7, 0.9),
            illuminance: 2000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, 0.3, -0.8, 0.0)),
        RenderLayers::layer(MATERIAL_PREVIEW_LAYER),
        MaterialPreviewLight,
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Preview Fill Light"),
    ));

    // Preview sphere — all texture slots filled with fallback for stable pipeline layout
    let sphere_mesh = meshes.add(with_tangents(Sphere::new(1.0).mesh().ico(5).unwrap()));
    let material = materials.add(new_graph_material(&fallback));

    commands.spawn((
        Mesh3d(sphere_mesh),
        MeshMaterial3d(material),
        Transform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        RenderLayers::layer(MATERIAL_PREVIEW_LAYER),
        MaterialPreviewMesh,
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Preview Sphere"),
    ));
}

// ── Backdrop (HDRI) ───────────────────────────────────────────────────────────

/// Decode the embedded equirectangular HDR and reproject it into the (sky, env)
/// cube pair. Returns `(None, None)` if decode/convert fails (the preview then
/// renders with no backdrop/IBL, exactly as before this feature).
fn decode_backdrop_cubes(
    images: &mut Assets<Image>,
) -> (Option<Handle<Image>>, Option<Handle<Image>>) {
    // Linear (`is_srgb = false`): HDR radiance is already linear light.
    let equirect = match Image::from_buffer(
        BACKDROP_HDR,
        ImageType::Extension("hdr"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::Default,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ) {
        Ok(img) => img,
        Err(e) => {
            warn!("[material_preview] failed to decode embedded backdrop HDR: {e}");
            return (None, None);
        }
    };
    match build_backdrop_cubes(&equirect) {
        Some((sky, env)) => (Some(images.add(sky)), Some(images.add(env))),
        None => (None, None),
    }
}

/// Largest power of two `<= n`. Keeps the cube faces filter-friendly regardless
/// of the source HDRI resolution.
fn prev_pot(n: u32) -> u32 {
    if n < 2 {
        1
    } else {
        1u32 << (31 - n.leading_zeros())
    }
}

/// Per-channel linear lerp between two colors.
fn mix_lin(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    LinearRgba {
        red: a.red + (b.red - a.red) * t,
        green: a.green + (b.green - a.green) * t,
        blue: a.blue + (b.blue - a.blue) * t,
        alpha: a.alpha + (b.alpha - a.alpha) * t,
    }
}

/// Bilinearly sample an equirect image at normalized `(u, v)` in linear space.
/// `u` wraps around the seam (it's a 360° panorama); `v` clamps at the poles.
/// `None` only if the center fetch fails — i.e. the source has no CPU-readable
/// pixels — so callers can bail instead of shipping a black cube.
fn sample_equirect_bilinear(src: &Image, u: f32, v: f32, sw: u32, sh: u32) -> Option<LinearRgba> {
    let px = u * sw as f32 - 0.5;
    let py = v * sh as f32 - 0.5;
    let x0 = px.floor();
    let y0 = py.floor();
    let tx = px - x0;
    let ty = py - y0;

    let xi0 = x0.rem_euclid(sw as f32) as u32;
    let xi1 = (x0 + 1.0).rem_euclid(sw as f32) as u32;
    let yi0 = y0.clamp(0.0, (sh - 1) as f32) as u32;
    let yi1 = (y0 + 1.0).clamp(0.0, (sh - 1) as f32) as u32;

    let c00 = src.get_color_at(xi0, yi0).ok()?.to_linear();
    let texel = |x: u32, y: u32| src.get_color_at(x, y).map(|c| c.to_linear()).unwrap_or(c00);
    let c10 = texel(xi1, yi0);
    let c01 = texel(xi0, yi1);
    let c11 = texel(xi1, yi1);

    let top = mix_lin(c00, c10, tx);
    let bot = mix_lin(c01, c11, tx);
    Some(mix_lin(top, bot, ty))
}

/// Allocate an empty 6-face `Rgba16Float` cube of `face`² — the format
/// `GeneratedEnvironmentMapLight`'s filter requires (POT + GPU-filterable) and
/// that `Skybox` samples cleanly.
fn empty_cube(face: u32) -> Image {
    Image::new_fill(
        Extent3d {
            width: face,
            height: face,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        // One black `Rgba16Float` texel (8 bytes) tiled across all 6 faces; the
        // loop below overwrites each, with `set_color_at_3d` handling f32→f16.
        &[0, 0, 0, 0, 0, 0, 0, 0],
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

fn clamp3(lin: LinearRgba, max: f32) -> LinearRgba {
    LinearRgba {
        red: lin.red.clamp(0.0, max),
        green: lin.green.clamp(0.0, max),
        blue: lin.blue.clamp(0.0, max),
        alpha: lin.alpha,
    }
}

/// Reproject the equirect HDR into TWO cubes in a single pass: a crisp `sky` cube
/// (lightly clamped, for the visible `Skybox`) and a hard-clamped `env` cube (for
/// the IBL prefilter, so super-bright texels don't become firefly blocks in
/// glossy reflections — see [`BACKDROP_ENV_MAX_RADIANCE`]). Returns `None` if the
/// source has no CPU-readable pixels.
fn build_backdrop_cubes(src: &Image) -> Option<(Image, Image)> {
    use std::f32::consts::PI;

    let (sw, sh) = (src.width(), src.height());
    if sw == 0 || sh == 0 {
        return None;
    }
    // Face resolution ≈ the equirect height (a face spans 90°, so this slightly
    // oversamples — fine, bilinear smooths it). 256–1024: low res made the bright
    // sky texels read as a hard grid of squares, so we lean toward full detail.
    let face = prev_pot(sh).clamp(256, 1024);

    let mut sky = empty_cube(face);
    let mut env = empty_cube(face);

    // +X, -X, +Y, -Y, +Z, -Z face bases (forward, up, right).
    let faces: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::NEG_Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::X),
        (Vec3::NEG_Z, Vec3::Y, Vec3::NEG_X),
    ];

    let mut wrote_any = false;
    for (layer, (forward, up, right)) in faces.iter().enumerate() {
        for y in 0..face {
            for x in 0..face {
                let u = (x as f32 + 0.5) / face as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / face as f32 * 2.0 - 1.0;
                let dir = (*forward + *right * u - *up * v).normalize();

                let theta = dir.z.atan2(dir.x);
                let phi = dir.y.asin();
                let eq_u = (theta + PI) / (2.0 * PI);
                let eq_v = (phi + PI / 2.0) / PI;

                // Bilinear fetch (u wraps horizontally, v clamps vertically) so the
                // reprojection doesn't expose hard equirect-texel squares, then
                // write each cube with its own radiance ceiling.
                if let Some(lin) = sample_equirect_bilinear(src, eq_u, 1.0 - eq_v, sw, sh) {
                    let l = layer as u32;
                    let _ = sky.set_color_at_3d(
                        x,
                        y,
                        l,
                        Color::from(clamp3(lin, BACKDROP_SKY_MAX_RADIANCE)),
                    );
                    if env
                        .set_color_at_3d(x, y, l, Color::from(clamp3(lin, BACKDROP_ENV_MAX_RADIANCE)))
                        .is_ok()
                    {
                        wrote_any = true;
                    }
                }
            }
        }
    }

    if !wrote_any {
        return None;
    }

    let cube_view = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    sky.texture_view_descriptor = cube_view.clone();
    env.texture_view_descriptor = cube_view;
    Some((sky, env))
}

/// Apply the `show_backdrop` toggle: add/remove the `Skybox` (the visible HDRI
/// background) and fade IBL intensity in/out. The `GeneratedEnvironmentMapLight`
/// itself stays attached — only its intensity moves — because the IBL bind slots
/// can't be re-added at runtime.
fn sync_preview_backdrop(
    mut commands: Commands,
    orbit: Res<MaterialPreviewOrbit>,
    env: Res<MaterialPreviewEnv>,
    mut camera: Query<
        (Entity, Option<&mut GeneratedEnvironmentMapLight>, Has<Skybox>),
        With<MaterialPreviewCamera>,
    >,
) {
    if !orbit.is_changed() {
        return;
    }
    for (entity, gen_light, has_skybox) in camera.iter_mut() {
        if let Some(mut g) = gen_light {
            let want = if orbit.show_backdrop {
                BACKDROP_IBL_INTENSITY
            } else {
                0.0
            };
            if g.intensity != want {
                g.intensity = want;
            }
        }
        match (orbit.show_backdrop, has_skybox, &env.sky) {
            (true, false, Some(sky)) => {
                commands.entity(entity).insert(Skybox {
                    image: Some(sky.clone()),
                    brightness: BACKDROP_SKY_BRIGHTNESS,
                    rotation: Quat::IDENTITY,
                });
            }
            (false, true, _) => {
                commands.entity(entity).remove::<Skybox>();
            }
            _ => {}
        }
    }
}

// ── Camera sync ─────────────────────────────────────────────────────────────

fn sync_preview_camera_active(
    editor_state: Res<MaterialEditorState>,
    mut camera: Query<&mut Camera, With<MaterialPreviewCamera>>,
) {
    let should_be_active = editor_state.compiled_wgsl.is_some();
    for mut cam in camera.iter_mut() {
        if cam.is_active != should_be_active {
            cam.is_active = should_be_active;
        }
    }
}

fn update_preview_camera_orbit(
    time: Res<Time>,
    mut orbit: ResMut<MaterialPreviewOrbit>,
    mut camera: Query<(&mut Transform, &mut Camera), With<MaterialPreviewCamera>>,
) {
    if orbit.auto_rotate {
        orbit.yaw += time.delta_secs() * 0.5;
    }

    for (mut transform, mut cam) in camera.iter_mut() {
        let x = orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();
        let y = orbit.distance * orbit.pitch.sin();
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);

        let bg = if orbit.dark_bg {
            Color::srgba(0.08, 0.08, 0.1, 1.0)
        } else {
            Color::srgba(0.45, 0.45, 0.5, 1.0)
        };
        cam.clear_color = ClearColorConfig::Custom(bg);
    }
}

/// Half-size the [`PreviewShape::Selected`] mesh is scaled to fit inside, in
/// world units. Picked to sit between the cube (0.75) and the sphere (1.0) so
/// switching between a primitive and the real mesh doesn't jump the framing.
const SELECTED_FIT_RADIUS: f32 = 0.9;

/// What [`swap_preview_shape`] last put on the preview entity, so it only
/// rebuilds when something actually changed. A plain `orbit.is_changed()` gate
/// isn't enough any more: with `Selected`, the mesh can change without the
/// resource being touched (the user picks a different entity in the hierarchy),
/// and the *fit* can't be computed until the mesh asset finishes loading.
#[derive(PartialEq)]
enum AppliedShape {
    Primitive(PreviewShape),
    /// `fitted` is false while the asset is still loading — its AABB isn't
    /// readable yet, so the transform is left at identity and retried.
    Selected { id: AssetId<Mesh>, fitted: bool },
}

/// The scene mesh [`PreviewShape::Selected`] draws, tracked once per change so
/// both the swap system and the shape picker agree on whether there is one.
///
/// It can't just read `MaterialEditorState::editing_entity`: opening a material
/// for editing puts it in a **document tab**, which is asset mode — the graph is
/// loaded from the `.material` path and `editing_entity` is deliberately cleared.
/// The hierarchy selection is what survives that, so the search starts there.
#[derive(Resource, Default)]
pub struct MaterialPreviewSource {
    /// The mesh entity the handle came from. Kept so the search re-runs when
    /// that entity is despawned without the selection itself changing.
    pub entity: Option<Entity>,
    pub mesh: Option<Handle<Mesh>>,
}

/// Find the scene mesh that best represents the material being edited: the
/// editor's own `editing_entity` when there is one (scene mode picks the exact
/// mesh for the active tab), otherwise a mesh in the selected entity's subtree —
/// preferring one that actually uses the material being edited, since a model
/// root's subtree usually holds several.
fn find_source_mesh(
    editor: &MaterialEditorState,
    selection: Option<&EditorSelection>,
    meshes: &Query<(&Mesh3d, Option<&MaterialRef>), Without<MaterialPreviewMesh>>,
    children: &Query<&Children>,
) -> Option<(Entity, Handle<Mesh>)> {
    let want = match &editor.edit_mode {
        MaterialEditMode::Existing { path, .. } | MaterialEditMode::EditingFile { path } => {
            Some(path.as_str())
        }
        _ => None,
    };

    let mut queue: std::collections::VecDeque<Entity> = editor.editing_entity.into_iter().collect();
    queue.extend(selection.map(|s| s.get_all()).unwrap_or_default());

    let mut fallback = None;
    let mut visited = std::collections::HashSet::new();
    while let Some(e) = queue.pop_front() {
        if !visited.insert(e) {
            continue;
        }
        if let Ok((mesh, mat)) = meshes.get(e) {
            let matches = want.is_some_and(|w| mat.is_some_and(|m| m.0 == w));
            if matches {
                return Some((e, mesh.0.clone()));
            }
            fallback.get_or_insert((e, mesh.0.clone()));
        }
        if let Ok(kids) = children.get(e) {
            queue.extend(kids.iter());
        }
    }
    fallback
}

fn track_preview_source(
    editor: Res<MaterialEditorState>,
    selection: Option<Res<EditorSelection>>,
    meshes: Query<(&Mesh3d, Option<&MaterialRef>), Without<MaterialPreviewMesh>>,
    children: Query<&Children>,
    mut source: ResMut<MaterialPreviewSource>,
    mut orbit: ResMut<MaterialPreviewOrbit>,
) {
    // The subtree walk is cheap but not free, so only redo it when something it
    // depends on moved — or when the mesh we settled on has gone away.
    let stale = source.entity.is_some_and(|e| meshes.get(e).is_err());
    let changed = editor.is_changed() || selection.as_ref().is_some_and(|s| s.is_changed());
    if !changed && !stale {
        return;
    }

    let found = find_source_mesh(&editor, selection.as_deref(), &meshes, &children);
    let (entity, mesh) = match found {
        Some((e, m)) => (Some(e), Some(m)),
        None => (None, None),
    };
    if source.entity == entity && source.mesh.as_ref().map(|m| m.id()) == mesh.as_ref().map(|m| m.id())
    {
        return;
    }
    let arrived = mesh.is_some();
    source.entity = entity;
    source.mesh = mesh;

    // A new mesh under the editor re-arms the default: the material's real
    // geometry is what you want to judge it on, so every fresh selection starts
    // there. Picking a primitive still sticks for as long as that mesh is the
    // one being edited — it's only a *different* mesh arriving that resets it.
    if arrived {
        orbit.shape = PreviewShape::Selected;
    }
}

fn swap_preview_shape(
    mut orbit: ResMut<MaterialPreviewOrbit>,
    source: Res<MaterialPreviewSource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut preview_mesh: Query<(&mut Mesh3d, &mut Transform), With<MaterialPreviewMesh>>,
    mut applied: Local<Option<AppliedShape>>,
) {
    // The selected mesh can disappear under us (deselect, despawn, or opening a
    // standalone `.material` with nothing selected). Fall back to the sphere
    // rather than freezing on the last mesh we happened to see.
    let selected = (orbit.shape == PreviewShape::Selected)
        .then(|| source.mesh.clone())
        .flatten();
    if orbit.shape == PreviewShape::Selected && selected.is_none() {
        orbit.shape = PreviewShape::Sphere;
    }

    for (mut mesh3d, mut transform) in preview_mesh.iter_mut() {
        match &selected {
            Some(handle) => {
                let id = handle.id();
                // Re-run while `fitted` is false: the AABB only becomes readable
                // once the asset has loaded.
                if *applied == (Some(AppliedShape::Selected { id, fitted: true })) {
                    continue;
                }
                if mesh3d.0.id() != id {
                    mesh3d.0 = handle.clone();
                }
                // Normalize into the primitives' size band and recentre on the
                // mesh's own centroid — an imported mesh can be authored at any
                // scale and pivoted anywhere, and the orbit camera's distance
                // range (1.5–10) assumes a ~1-unit subject.
                let fitted = match meshes.get(handle).and_then(|m| m.compute_aabb()) {
                    Some(aabb) => {
                        let extent = Vec3::from(aabb.half_extents).max_element().max(1e-6);
                        let scale = SELECTED_FIT_RADIUS / extent;
                        *transform = Transform::from_scale(Vec3::splat(scale))
                            .with_translation(-Vec3::from(aabb.center) * scale);
                        true
                    }
                    None => {
                        *transform = Transform::default();
                        false
                    }
                };
                *applied = Some(AppliedShape::Selected { id, fitted });
            }
            None => {
                if *applied == Some(AppliedShape::Primitive(orbit.shape)) {
                    continue;
                }
                let new_mesh = match orbit.shape {
                    PreviewShape::Cube => Cuboid::new(1.5, 1.5, 1.5).into(),
                    PreviewShape::Cylinder => Cylinder::new(0.8, 2.0).into(),
                    PreviewShape::Torus => Torus::new(0.5, 1.0).into(),
                    PreviewShape::Plane => Plane3d::new(Vec3::Y, Vec2::splat(1.5)).into(),
                    // `Selected` can't reach here (it fell back above), so the
                    // sphere doubles as its guard.
                    PreviewShape::Sphere | PreviewShape::Selected => {
                        Sphere::new(1.0).mesh().ico(5).unwrap()
                    }
                };
                mesh3d.0 = meshes.add(with_tangents(new_mesh));
                *transform = Transform::default();
                *applied = Some(AppliedShape::Primitive(orbit.shape));
            }
        }
    }
}

/// Give a preview primitive the tangents parallax mapping needs.
///
/// Bevy's shape meshes ship position/normal/UV and nothing else, and the
/// parallax march is gated on `VERTEX_TANGENTS` — so without this a material
/// with a displacement map renders flat in the preview and with relief on an
/// imported mesh, which reads as "displacement is broken" rather than as a
/// property of the preview shape.
///
/// A failure here is not worth refusing to draw over: `generate_tangents`
/// only fails on degenerate UVs, and the mesh is still perfectly renderable
/// without the attribute — it just loses parallax.
fn with_tangents(mut mesh: Mesh) -> Mesh {
    if mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none() && mesh.generate_tangents().is_err() {
        warn!("material preview: couldn't generate tangents, parallax will be inactive");
    }
    mesh
}

// ── Shader hot-swap ─────────────────────────────────────────────────────────

/// Update both the shader AND the material textures atomically in one system.
/// This prevents the pipeline layout mismatch where the shader declares texture
/// bindings but the material hasn't assigned them yet.
///
/// Uses a content hash to skip redundant work when only non-graph fields
/// of MaterialEditorState change (e.g. selected_node).
/// Hash a `PinValue` by its variant + payload bytes. Used to fingerprint
/// `param/*` defaults for the preview's change detector — float/color/
/// vec arrays are hashed via their `to_bits()` so semantically-equal
/// values produce the same hash even though `f32` doesn't impl `Hash`.
fn hash_pin_value<H: std::hash::Hasher>(
    value: &renzora_shader::material::graph::PinValue,
    state: &mut H,
) {
    use renzora_shader::material::graph::PinValue;
    use std::hash::Hash;
    match value {
        PinValue::Float(f) => {
            0u8.hash(state);
            f.to_bits().hash(state);
        }
        PinValue::Vec2(v) => {
            1u8.hash(state);
            for x in v {
                x.to_bits().hash(state);
            }
        }
        PinValue::Vec3(v) => {
            2u8.hash(state);
            for x in v {
                x.to_bits().hash(state);
            }
        }
        PinValue::Vec4(v) => {
            3u8.hash(state);
            for x in v {
                x.to_bits().hash(state);
            }
        }
        PinValue::Color(c) => {
            4u8.hash(state);
            for x in c {
                x.to_bits().hash(state);
            }
        }
        PinValue::Int(i) => {
            5u8.hash(state);
            i.hash(state);
        }
        PinValue::Bool(b) => {
            6u8.hash(state);
            b.hash(state);
        }
        PinValue::TexturePath(s) | PinValue::String(s) => {
            7u8.hash(state);
            s.hash(state);
        }
        PinValue::None => {
            8u8.hash(state);
        }
    }
}

fn update_preview_material(
    mut commands: Commands,
    editor_state: Res<MaterialEditorState>,
    asset_server: Res<AssetServer>,
    fallback: Res<FallbackTexture>,
    mut shaders: ResMut<Assets<Shader>>,
    mut shader_state: ResMut<GraphMaterialShaderState>,
    mut tracker: ResMut<MaterialPreviewTracker>,
    preview_mesh: Query<Entity, With<MaterialPreviewMesh>>,
    mut materials: ResMut<Assets<GraphMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
) {
    if !editor_state.is_changed() {
        return;
    }
    // Don't touch the shared shader when no material is being edited —
    // overwriting it with the empty default would break all scene materials.
    if matches!(editor_state.edit_mode, crate::MaterialEditMode::Inactive) {
        return;
    }
    if !editor_state.compile_errors.is_empty() {
        return;
    }

    // Compile once — used for both texture bindings and shader insertion.
    let result = renzora_shader::material::codegen::compile(&editor_state.graph);
    if !result.errors.is_empty() {
        return;
    }

    // Hash shader + texture bindings + param defaults to detect actual
    // graph changes. Skips redundant work when only selection/UI state
    // changed.
    //
    // Param defaults are included because changing a `param/*` node's
    // authored default *doesn't* change the emitted WGSL (defaults
    // live in the params UBO, not in the shader source) — without
    // hashing them, editing a Color Parameter's default in the graph
    // would silently no-op the preview until the user disconnected
    // and reconnected the cable to force a WGSL hash flip.
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    result.fragment_shader.hash(&mut hasher);
    for tb in &result.texture_bindings {
        tb.binding.hash(&mut hasher);
        tb.asset_path.hash(&mut hasher);
    }
    for p in &result.parameters {
        p.name.hash(&mut hasher);
        // PinValue's bytes — float/color defaults boil down to f32
        // arrays, so hash via their byte representation. Bools and
        // ints come through their integer hashes naturally.
        hash_pin_value(&p.default, &mut hasher);
    }
    let hash = hasher.finish();

    if tracker.last_wgsl_hash == Some(hash) {
        return;
    }
    tracker.last_wgsl_hash = Some(hash);

    info!(
        "[material_preview] Compiling graph: {} nodes, {} connections",
        editor_state.graph.nodes.len(),
        editor_state.graph.connections.len()
    );

    // Mirror the scene resolver's dispatch so the preview shows EXACTLY what a
    // mesh in the scene would. The resolver tries the trivial fast path first
    // (`resolve_material_file`): a graph that's just texture samples + factors
    // wired to PBR pins — which is what every imported glTF and most authored
    // materials are — compiles to a stock `StandardMaterial`, NOT the
    // `ExtendedMaterial`/codegen path. The preview used to always build a
    // `GraphMaterial`, so a trivial material that renders correctly in the
    // scene (via StandardMaterial) previewed as a flat, untextured sphere
    // because the extension's texture slots never resolved. Trying the same
    // fast path here keeps the two in lockstep.
    if let Some(std_mat) = renzora_shader::material::standard_build::try_build_standard_material(
        &editor_state.graph,
        &asset_server,
    ) {
        let handle = standard_materials.add(std_mat);
        for entity in preview_mesh.iter() {
            // Swap the sphere onto StandardMaterial; drop any GraphMaterial it
            // carried from a previous (procedural) edit. Removing an absent
            // component is a no-op, so this is safe on the first run too.
            commands
                .entity(entity)
                .remove::<MeshMaterial3d<GraphMaterial>>()
                .insert(MeshMaterial3d(handle.clone()));
        }
        shader_state.last_wgsl_hash = Some(hash);
        return;
    }

    // Procedural graph → codegen'd GraphMaterial. Create a unique
    // Uuid-addressed shader; the material's `shader_uuid` points at it and
    // `SurfaceGraphExt::specialize` reconstructs the `Handle::Uuid` and plugs
    // it into the fragment stage.
    let preview_uuid = Uuid::new_v4();
    let preview_handle: Handle<Shader> = Handle::Uuid(preview_uuid, PhantomData);
    let shader = Shader::from_wgsl(result.fragment_shader.clone(), "graph_material://preview");
    let _ = shaders.insert(&preview_handle, shader);

    // Seed the parameter UBO from the graph's authored defaults. Without the
    // seed, every `param/*` slot reads as zero — a graph whose BaseColor comes
    // from a `param/color` would render as black even though the user authored
    // an orange default.
    let default_slots =
        renzora_shader::material::instance::build_default_param_slots(&result.parameters);

    // Build a FRESH material and assign a new handle, rather than mutating the
    // existing preview material in place. The sphere is spawned at startup with
    // `shader_uuid: None`, so it specializes once against the default
    // StandardMaterial fragment; flipping `shader_uuid` to `Some(..)` in place
    // via `get_mut` did NOT reliably re-trigger pipeline specialization, leaving
    // the sphere on the default fragment (a plain white StandardMaterial).
    // Assigning a brand-new material handle forces the same fresh prepare +
    // `specialize` path the scene resolver uses (`assemble_graph_material`).
    //
    // `new_graph_material` fills every 2D slot with fallback-white and leaves
    // cube/array/3D as None (Bevy's FallbackImage covers the layout), so unused
    // slots stay valid without explicit resets.
    use renzora_shader::material::codegen::TextureKind;
    let mut material = new_graph_material(&fallback);
    material.extension.shader_uuid = Some(preview_uuid);
    material.extension.params.slots = default_slots;

    for tb in &result.texture_bindings {
        if tb.asset_path.is_empty() {
            continue;
        }
        let handle: Handle<Image> = asset_server.load(&tb.asset_path);
        match (tb.kind, tb.binding) {
            (TextureKind::D2, 0) => material.extension.texture_0 = Some(handle),
            (TextureKind::D2, 1) => material.extension.texture_1 = Some(handle),
            (TextureKind::D2, 2) => material.extension.texture_2 = Some(handle),
            (TextureKind::D2, 3) => material.extension.texture_3 = Some(handle),
            (TextureKind::D2, 4) => material.extension.texture_4 = Some(handle),
            (TextureKind::D2, 5) => material.extension.texture_5 = Some(handle),
            (TextureKind::Cube, 0) => material.extension.cube_0 = Some(handle),
            (TextureKind::D2Array, 0) => material.extension.array_0 = Some(handle),
            (TextureKind::D3, 0) => material.extension.volume_0 = Some(handle),
            _ => warn!(
                "[material_preview] Texture binding slot {:?}/{} not routed!",
                tb.kind, tb.binding
            ),
        }
    }

    let graph_handle = materials.add(material);
    for entity in preview_mesh.iter() {
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(graph_handle.clone()));
    }
    shader_state.last_wgsl_hash = Some(hash);
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct MaterialPreviewPlugin;

impl Plugin for MaterialPreviewPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MaterialPreviewPlugin");
        app.init_resource::<MaterialPreviewOrbit>()
            .init_resource::<MaterialPreviewImage>()
            .init_resource::<MaterialPreviewTracker>()
            .init_resource::<MaterialPreviewEnv>()
            .init_resource::<MaterialPreviewSource>()
            .add_systems(PostStartup, setup_material_preview)
            .add_systems(
                Update,
                (
                    sync_preview_camera_active,
                    update_preview_camera_orbit,
                    // Ordered: the swap reads the source the tracker just wrote,
                    // so a fresh selection shows up this frame instead of next.
                    (track_preview_source, swap_preview_shape).chain(),
                    sync_preview_backdrop,
                    update_preview_material,
                ),
            );
    }
}

