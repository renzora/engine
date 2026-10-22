//! Lit 3D thumbnails for the Shape Library.
//!
//! Each registered shape is rendered once, by an offscreen camera into an image
//! the panel then shows. The phosphor icons the grid used before were shared
//! between unrelated shapes (Sphere and Hemisphere were the same globe; Cone,
//! Wedge, Ramp and Funnel were all a triangle), so the label was often the only
//! way to tell two tiles apart. A render of the actual mesh needs no label.
//!
//! The renders are one-off: a camera lives for [`WARMUP_FRAMES`] and is then
//! despawned with its mesh, and the image keeps the last frame it drew. So the
//! steady-state cost is thirty small textures and nothing per frame. Cameras run
//! in batches of [`BATCH`] so opening the panel the first time doesn't put
//! thirty extra render passes on one frame.

use std::collections::HashMap;

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};

use renzora::core::viewport_types::SHAPE_THUMBNAIL_LAYER;
use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera, ShapeRegistry};
use renzora_ember::theme::{card_bg, rgb};

/// Thumbnail resolution. Above the largest tile the size slider allows, so the
/// image is only ever scaled down.
const THUMB_PX: u32 = 192;
/// Cameras alive at once.
const BATCH: usize = 10;
/// Frames a camera renders before it is retired. Generous on purpose: the mesh
/// draws nothing until its pipeline has compiled, and a camera retired before
/// then leaves a blank thumbnail for the rest of the session.
const WARMUP_FRAMES: u32 = 45;
/// Distance between shapes on the thumbnail layer, so one camera never frames a
/// neighbour. The largest built-in shape is a few metres across.
const SPACING: f32 = 40.0;

/// One image per shape id, filled in over the first second or so.
#[derive(Resource, Default)]
pub(crate) struct ShapeThumbs {
    pub(crate) images: HashMap<&'static str, Handle<Image>>,
    queue: Vec<&'static str>,
    active: Vec<Active>,
    material: Option<Handle<StandardMaterial>>,
    lights: bool,
    slot: usize,
}

struct Active {
    camera: Entity,
    mesh: Entity,
    frames: u32,
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ShapeThumbs>().add_systems(
        Update,
        (allocate_thumbs, render_thumbs)
            .chain()
            .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
    );
}

/// Give every registered shape an image (blank until rendered) and queue it.
/// Runs every frame but does work only when the registry has grown, which is
/// what picks up a shape a plugin registers after startup.
fn allocate_thumbs(
    registry: Option<Res<ShapeRegistry>>,
    mut thumbs: ResMut<ShapeThumbs>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(registry) = registry else { return };
    if registry.iter().count() == thumbs.images.len() {
        return;
    }
    for shape in registry.iter() {
        if thumbs.images.contains_key(shape.id) {
            continue;
        }
        let extent = Extent3d { width: THUMB_PX, height: THUMB_PX, depth_or_array_layers: 1 };
        let mut image = Image {
            data: Some(vec![0u8; (THUMB_PX * THUMB_PX * 4) as usize]),
            ..default()
        };
        image.texture_descriptor.size = extent;
        image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
        image.texture_descriptor.usage =
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
        let handle = images.add(image);
        thumbs.images.insert(shape.id, handle);
        thumbs.queue.push(shape.id);
    }
}

/// The centre and bounding-sphere radius of a mesh's vertex positions.
/// Bevy 0.20 keeps its own AABB helper private, and the positions are all a
/// framing needs.
fn bounds(mesh: &Mesh) -> Option<(Vec3, f32)> {
    let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?.as_float3()?;
    let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in positions {
        let p = Vec3::from(*p);
        min = min.min(p);
        max = max.max(p);
    }
    (min.x <= max.x).then(|| ((min + max) * 0.5, ((max - min) * 0.5).length().max(0.1)))
}

/// Start queued renders while there is room in the batch, and retire cameras
/// that have rendered long enough.
fn render_thumbs(
    mut commands: Commands,
    registry: Option<Res<ShapeRegistry>>,
    mut thumbs: ResMut<ShapeThumbs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let thumbs = &mut *thumbs;
    for a in &mut thumbs.active {
        a.frames += 1;
    }
    thumbs.active.retain(|a| {
        if a.frames < WARMUP_FRAMES {
            return true;
        }
        commands.entity(a.camera).try_despawn();
        commands.entity(a.mesh).try_despawn();
        false
    });
    if thumbs.queue.is_empty() {
        return;
    }
    let Some(registry) = registry else { return };
    let layer = RenderLayers::layer(SHAPE_THUMBNAIL_LAYER);

    if !thumbs.lights {
        thumbs.lights = true;
        // Three-point lighting placed against the camera (which looks from
        // +X+Y+Z): a strong key from above and to the left, so the top, front
        // and side faces the camera sees land on three clearly different
        // values; a weak cool fill from the right so the shadow side keeps its
        // shape; and a rim from behind to pull the silhouette off the tile.
        // The first rig lit from roughly the camera's own side, every visible
        // face caught about the same light, and the renders read as flat
        // silhouettes.
        let lights: [(Vec3, Color, f32, &str); 3] = [
            (Vec3::new(-0.45, 1.0, 0.65), Color::srgb(1.0, 0.97, 0.92), 9000.0, "Key"),
            (Vec3::new(1.0, 0.25, 0.1), Color::srgb(0.62, 0.72, 0.95), 1800.0, "Fill"),
            (Vec3::new(-0.3, 0.5, -1.0), Color::srgb(0.9, 0.93, 1.0), 4500.0, "Rim"),
        ];
        for (from, color, illuminance, label) in lights {
            commands.spawn((
                DirectionalLight { color, illuminance, shadow_maps_enabled: false, ..default() },
                // A directional light shines along its forward (-Z), so face
                // it from `from` toward the origin.
                Transform::default().looking_to(-from.normalize(), Vec3::Y),
                layer.clone(),
                HideInHierarchy,
                EditorLocked,
                Name::new(format!("Shape Thumbnail {label} Light")),
            ));
        }
    }
    // One neutral clay for every thumbnail, rather than each shape's spawn
    // colour: the grid is for comparing forms, and thirty different tints
    // made it read as a palette.
    let material = thumbs
        .material
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                // A mid value, not near-white: a light clay clipped toward
                // white on the key side and flattened the faces into one tone.
                base_color: Color::srgb(0.55, 0.6, 0.7),
                perceptual_roughness: 0.55,
                ..default()
            })
        })
        .clone();

    while thumbs.active.len() < BATCH {
        let Some(id) = thumbs.queue.pop() else { break };
        let (Some(target), Some(mesh)) = (thumbs.images.get(id).cloned(), registry.create_mesh(id, &mut meshes)) else {
            continue;
        };
        // Frame the mesh by its bounding sphere so a 4 m wall and a 1 m cube
        // both fill the tile.
        let (center, radius) = meshes.get(&mesh).and_then(bounds).unwrap_or((Vec3::ZERO, 1.0));
        let origin = Vec3::new(thumbs.slot as f32 * SPACING, 0.0, 0.0);
        let fov = std::f32::consts::FRAC_PI_4;
        let dist = radius / (fov * 0.5).sin() * 1.05;
        let dir = Vec3::new(1.0, 0.75, 1.3).normalize();
        let focus = origin + center;

        let mesh_e = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(origin),
                layer.clone(),
                HideInHierarchy,
                EditorLocked,
                Name::new(format!("Shape Thumbnail ({id})")),
            ))
            .id();
        let camera = commands
            .spawn((
                Camera3d::default(),
                // HDR like every other editor camera: Bevy keys the mesh
                // pipeline on a coarse hdr bit, and an LDR offscreen camera
                // can leave the viewport pass with a pipeline built for the
                // wrong target format (see the material thumbnails).
                Hdr,
                Msaa::Sample4,
                Projection::Perspective(PerspectiveProjection { fov, ..default() }),
                Camera {
                    clear_color: ClearColorConfig::Custom(rgb(card_bg())),
                    order: -2000 - thumbs.slot as isize,
                    ..default()
                },
                RenderTarget::Image(target.into()),
                Transform::from_translation(focus + dir * dist).looking_at(focus, Vec3::Y),
                layer.clone(),
                IsolatedCamera,
                HideInHierarchy,
                EditorLocked,
                Name::new("Shape Thumbnail Camera"),
            ))
            .id();
        thumbs.slot += 1;
        thumbs.active.push(Active { camera, mesh: mesh_e, frames: 0 });
    }
}
