//! Offscreen preview sphere for the selected material.
//!
//! Selecting a material in the import window renders it here so you can see
//! what it actually looks like before accepting the import — a list of slot
//! names does not tell you that a base-colour map came through inverted, or
//! that a normal map is flat.
//!
//! ## Why this builds a `StandardMaterial` and not a `GraphMaterial`
//!
//! The `.material` graph files are written on *commit*, so during inspection
//! they do not exist yet — there is nothing for the graph resolver to load. But
//! everything needed is already in hand: the PBR factors travel in the staged
//! import's material rows, and the `.rmip` textures are sitting in the staging
//! tree. Assembling a `StandardMaterial` from those renders the same surface
//! through Bevy's own PBR path, with no dependency on the graph pipeline.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::core_pipeline::prepass::{DepthPrepass, MotionVectorPrepass, NormalPrepass};
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};

use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera};

/// Its own layer. See the registry in the contract crate for what is already
/// taken — sharing one with another preview means seeing its contents, because
/// every one of them parks geometry at the world origin.
use renzora::core::viewport_types::IMPORT_MATERIAL_PREVIEW_LAYER as MAT_LAYER;

/// Initial size only — the target is resized to the panel each frame, both so
/// the sphere is crisp and so it is not letterboxed inside a differently-shaped
/// region.
const RTT: u32 = 512;

/// Marker for the UI node the material preview is drawn into. Orbit input is
/// gated on its `Interaction`.
#[derive(Component)]
pub struct MaterialPreviewViewport;

/// Where the material camera is looking from.
#[derive(Resource)]
pub struct MaterialPreviewOrbit {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for MaterialPreviewOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.5,
            pitch: 0.25,
            // The sphere has radius 1; this frames it with a little margin.
            distance: 2.9,
        }
    }
}

#[derive(Resource)]
pub struct MaterialPreviewImage {
    pub handle: Handle<Image>,
}

#[derive(Resource)]
struct MaterialPreviewRig {
    sphere: Entity,
}

/// Which material is currently built, so the sphere is only rebuilt when the
/// selection actually changes rather than every frame.
#[derive(Resource, Default)]
struct MaterialPreviewShown {
    index: Option<usize>,
    /// Staging directory the textures were resolved against; a new staged file
    /// reuses index 0, so the directory has to be part of the identity.
    dir: Option<std::path::PathBuf>,
}

#[derive(Component)]
struct MatPreviewCamera;
#[derive(Component)]
struct MatPreviewSphere;

pub(crate) fn register(app: &mut App) {
    app.init_resource::<MaterialPreviewShown>()
        .init_resource::<MaterialPreviewOrbit>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                sync_camera_active,
                match_viewport_size,
                rebuild_selected,
                orbit_input,
                apply_orbit,
            )
                .chain(),
        );
}

pub fn preview_image(world: &World) -> Option<Handle<Image>> {
    world
        .get_resource::<MaterialPreviewImage>()
        .map(|i| i.handle.clone())
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size = Extent3d {
        width: RTT,
        height: RTT,
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
    let handle = images.add(image);
    commands.insert_resource(MaterialPreviewImage {
        handle: handle.clone(),
    });

    commands.spawn((
        Camera3d::default(),
        // Grouped to stay under the bundle-tuple limit. This matches the model
        // preview and the editor viewport: a camera without the prepasses
        // specializes the PBR pipeline differently from every other view, which
        // is what left this sphere rendering unlit.
        (Hdr, NormalPrepass, DepthPrepass, MotionVectorPrepass),
        Msaa::Off,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.09, 0.10, 0.12)),
            order: -10,
            is_active: false,
            ..default()
        },
        RenderTarget::Image(handle.into()),
        Transform::from_xyz(0.0, 0.35, 2.6).looking_at(Vec3::ZERO, Vec3::Y),
        AmbientLight {
            color: Color::srgb(0.85, 0.88, 1.0),
            // Generous on purpose. Without an environment map a PBR surface has
            // nothing to reflect, and this is a swatch to judge a texture by,
            // not a lighting study.
            brightness: 1200.0,
            affects_lightmapped_meshes: false,
        },
        RenderLayers::layer(MAT_LAYER),
        MatPreviewCamera,
        IsolatedCamera,
        HideInHierarchy,
        EditorLocked,
        Name::new("Material Preview Camera"),
    ));

    for (transform, illuminance) in [
        (
            Transform::from_xyz(3.0, 4.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
            12000.0,
        ),
        (
            Transform::from_xyz(-3.0, 1.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
            5000.0,
        ),
        // A light from behind the camera, so the face you are looking at is
        // never the unlit one however far you spin the sphere.
        (
            Transform::from_xyz(0.0, 0.5, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
            4000.0,
        ),
    ] {
        commands.spawn((
            DirectionalLight {
                illuminance,
                shadow_maps_enabled: false,
                ..default()
            },
            transform,
            RenderLayers::layer(MAT_LAYER),
            HideInHierarchy,
            EditorLocked,
            Name::new("Material Preview Light"),
        ));
    }

    // A UV sphere rather than an icosphere: the seam and pole layout is what
    // makes a wrongly-oriented or wrongly-tiled texture obvious.
    let sphere = commands
        .spawn((
            Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(48, 32))),
            MeshMaterial3d(materials.add(StandardMaterial::default())),
            Transform::default(),
            MatPreviewSphere,
            // Hidden until a material is actually selected. An idle sphere is
            // wasted work, and it sits at the world origin — which is exactly
            // where the model preview re-centres its model, so anything that
            // did leak across would land in the middle of it.
            Visibility::Hidden,
            RenderLayers::layer(MAT_LAYER),
            HideInHierarchy,
            EditorLocked,
            Name::new("Material Preview Sphere"),
        ))
        .id();
    commands.insert_resource(MaterialPreviewRig { sphere });
}

/// Only render while a material is actually selected — an always-on offscreen
/// camera costs a pass every frame for nothing.
fn sync_camera_active(
    shown: Res<MaterialPreviewShown>,
    rig: Option<Res<MaterialPreviewRig>>,
    mut cameras: Query<&mut Camera, With<MatPreviewCamera>>,
    mut visibility: Query<&mut Visibility>,
) {
    let want = shown.index.is_some();
    for mut cam in &mut cameras {
        if cam.is_active != want {
            cam.is_active = want;
        }
    }
    if let Some(rig) = rig {
        if let Ok(mut v) = visibility.get_mut(rig.sphere) {
            let target = if want {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            if *v != target {
                *v = target;
            }
        }
    }
}

/// Keep the render texture the same pixel size as the panel, so the sphere is
/// neither upscaled nor letterboxed when the columns are resized.
fn match_viewport_size(
    q: Query<&bevy::ui::ComputedNode, With<MaterialPreviewViewport>>,
    image: Option<Res<MaterialPreviewImage>>,
    mut images: ResMut<Assets<Image>>,
    mut current: Local<UVec2>,
) {
    let Some(image) = image else { return };
    let Some(cn) = q.iter().next() else { return };
    let size = cn.size();
    let want = UVec2::new(
        (size.x as u32).clamp(64, 4096),
        (size.y as u32).clamp(64, 4096),
    );
    if *current == want {
        return;
    }
    if let Some(mut img) = images.get_mut(&image.handle) {
        img.resize(Extent3d {
            width: want.x,
            height: want.y,
            depth_or_array_layers: 1,
        });
        *current = want;
    }
}

/// Drag to spin the sphere, wheel to zoom. Left *or* right drag both rotate —
/// there is nothing else to do in here, so a modifier would only be a thing to
/// remember.
fn orbit_input(
    hover: Query<&Interaction, With<MaterialPreviewViewport>>,
    shown: Res<MaterialPreviewShown>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut orbit: ResMut<MaterialPreviewOrbit>,
    mut dragging: Local<bool>,
) {
    if shown.index.is_none() {
        *dragging = false;
        return;
    }
    let over = hover
        .iter()
        .any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
    if !*dragging
        && over
        && (mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right))
    {
        *dragging = true;
    }
    if !mouse.pressed(MouseButton::Left) && !mouse.pressed(MouseButton::Right) {
        *dragging = false;
    }
    if *dragging {
        let d = motion.delta;
        orbit.yaw -= d.x * 0.008;
        // Short of the poles, or `looking_at` degenerates and the view rolls.
        orbit.pitch = (orbit.pitch + d.y * 0.008).clamp(-1.45, 1.45);
    }
    if over && scroll.delta.y != 0.0 {
        orbit.distance = (orbit.distance * 0.9f32.powf(scroll.delta.y)).clamp(1.35, 8.0);
    }
}

fn apply_orbit(
    orbit: Res<MaterialPreviewOrbit>,
    mut cameras: Query<&mut Transform, With<MatPreviewCamera>>,
) {
    if !orbit.is_changed() {
        return;
    }
    let (sy, cy) = orbit.yaw.sin_cos();
    let (sp, cp) = orbit.pitch.sin_cos();
    let eye = Vec3::new(
        orbit.distance * cp * sy,
        orbit.distance * sp,
        orbit.distance * cp * cy,
    );
    for mut t in &mut cameras {
        t.translation = eye;
        *t = t.looking_at(Vec3::ZERO, Vec3::Y);
    }
}

fn rebuild_selected(world: &mut World) {
    let selected = world
        .get_resource::<crate::window::ImportNav>()
        .and_then(|n| n.sel_material);
    let staged = world
        .get_resource::<crate::overlay::ImportOverlayState>()
        .and_then(|s| s.current().cloned());

    let (want, dir) = match (&staged, selected) {
        (Some(st), Some(i)) if i < st.materials.len() => (Some(i), Some(st.staging_dir.clone())),
        _ => (None, None),
    };
    {
        let shown = world.resource::<MaterialPreviewShown>();
        if shown.index == want && shown.dir == dir {
            return;
        }
    }
    {
        let mut shown = world.resource_mut::<MaterialPreviewShown>();
        shown.index = want;
        shown.dir = dir.clone();
    }
    let (Some(idx), Some(dir), Some(st)) = (want, dir, staged) else {
        return;
    };
    let row = &st.materials[idx];

    let asset_server = world.resource::<AssetServer>().clone();
    // Texture URIs are model-relative (`textures/foo.rmip`); the staging tree is
    // where they currently live.
    let load = |uri: &Option<String>| -> Option<Handle<Image>> {
        uri.as_ref().map(|u| asset_server.load::<Image>(dir.join(u)))
    };

    let material = StandardMaterial {
        base_color: Color::srgba(
            row.base_color[0],
            row.base_color[1],
            row.base_color[2],
            row.base_color[3],
        ),
        base_color_texture: load(&row.base_color_uri),
        normal_map_texture: load(&row.normal_uri),
        metallic_roughness_texture: load(&row.metallic_roughness_uri),
        occlusion_texture: load(&row.occlusion_uri),
        emissive_texture: load(&row.emissive_uri),
        emissive: LinearRgba::rgb(row.emissive[0], row.emissive[1], row.emissive[2]),
        metallic: row.metallic,
        perceptual_roughness: row.roughness.max(0.03),
        double_sided: row.double_sided,
        // A two-sided material needs its back faces lit, or the preview shows
        // black wherever you can see through to the far side of the sphere.
        cull_mode: if row.double_sided {
            None
        } else {
            Some(bevy::render::render_resource::Face::Back)
        },
        alpha_mode: match row.alpha_mode.as_str() {
            "Mask" => AlphaMode::Mask(0.5),
            "Blend" => AlphaMode::Blend,
            _ => AlphaMode::Opaque,
        },
        ..default()
    };

    let handle = world.resource_mut::<Assets<StandardMaterial>>().add(material);
    let sphere = world.resource::<MaterialPreviewRig>().sphere;
    if let Ok(mut e) = world.get_entity_mut(sphere) {
        e.insert(MeshMaterial3d(handle));
    }
}
