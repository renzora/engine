//! Rendering a shape's thumbnail to a PNG.
//!
//! The same code runs in two places. The companion runs it for the editor
//! (job kind `"shape_thumbnail"`), which keeps its cameras out of the editor's
//! frame. The editor runs it itself only when no companion is there to ask.
//! Either way the result is a PNG in the machine-wide cache
//! ([`shape_thumb_path`]), so a shape is rendered once per machine, not once
//! per session.
//!
//! It used to be once per session, in the editor, with ten offscreen cameras
//! rendering for about five seconds straight after a project opened. Each was
//! a full 3D view, and with a large scene loading at the same moment the
//! editor ran at 25 fps instead of 42 for those seconds.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

use renzora::core::viewport_types::SHAPE_THUMBNAIL_LAYER;
use renzora::core::{EditorLocked, HideInHierarchy, IsolatedCamera, ShapeRegistry};

/// Thumbnail resolution. Above the largest tile the size slider allows, so the
/// image is only ever scaled down.
const THUMB_PX: u32 = 192;
/// Cameras alive at once.
const BATCH: usize = 10;
/// Frames a camera renders before its picture is taken. Generous on purpose:
/// the mesh draws nothing until its pipeline has compiled, and a picture taken
/// before then is a blank tile that the cache would then keep.
const WARMUP_FRAMES: u32 = 45;
/// Distance between shapes on the thumbnail layer, so one camera never frames a
/// neighbour. The largest built-in shape is a few metres across.
const SPACING: f32 = 40.0;
/// Part of every cached file's name. Bump it when the lighting, material or
/// framing below changes, so pictures made the old way are not reused.
const RENDER_VERSION: u32 = 1;

/// Where the thumbnail of a shape with this `mesh`, drawn on `clear`, is kept.
///
/// Machine-wide rather than per project: the shapes are the engine's, not the
/// project's. The name carries a hash of the mesh and the background, so a
/// shape whose generator changed, or a theme with another tile colour, gets a
/// new file instead of an old picture. `None` when there is no home directory
/// to put a cache in.
pub fn shape_thumb_path(id: &str, mesh: &Mesh, clear: Color) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    // FNV-1a, written out rather than `DefaultHasher`, whose output may change
    // between Rust releases and would then throw the whole cache away.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(&RENDER_VERSION.to_le_bytes());
    feed(&THUMB_PX.to_le_bytes());
    for c in clear.to_linear().to_f32_array() {
        feed(&c.to_le_bytes());
    }
    if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|a| a.as_float3()) {
        for p in positions {
            for c in p {
                feed(&c.to_le_bytes());
            }
        }
    }
    if let Some(indices) = mesh.indices() {
        for i in indices.iter() {
            feed(&(i as u32).to_le_bytes());
        }
    }
    Some(
        PathBuf::from(home)
            .join(".renzora")
            .join("cache")
            .join("thumbnails")
            .join("shapes")
            .join(format!("{id}-{hash:016x}.png")),
    )
}

/// One shape to render.
#[derive(Clone, Debug)]
pub struct ShapeCaptureRequest {
    pub id: String,
    /// Where the PNG goes.
    pub path: PathBuf,
    /// The tile colour behind the shape.
    pub clear: Color,
}

/// Shapes waiting to be rendered, being rendered, and done.
#[derive(Resource, Default)]
pub struct ShapeCaptures {
    queue: VecDeque<ShapeCaptureRequest>,
    active: Vec<Active>,
    finished: Vec<(String, Result<PathBuf, String>)>,
    rig: Vec<Entity>,
    material: Option<Handle<StandardMaterial>>,
    slot: usize,
}

struct Active {
    id: String,
    path: PathBuf,
    camera: Entity,
    mesh: Entity,
    target: Handle<Image>,
    frames: u32,
    shot: bool,
}

impl ShapeCaptures {
    /// Render `request`, unless that shape is already queued or rendering.
    pub fn request(&mut self, request: ShapeCaptureRequest) {
        let known = self.queue.iter().any(|r| r.id == request.id) || self.active.iter().any(|a| a.id == request.id);
        if !known {
            self.queue.push_back(request);
        }
    }

    /// The shapes finished since the last call: the PNG's path, or why not.
    pub fn take_finished(&mut self) -> Vec<(String, Result<PathBuf, String>)> {
        std::mem::take(&mut self.finished)
    }

    /// Whether anything is queued or rendering.
    pub fn busy(&self) -> bool {
        !self.queue.is_empty() || !self.active.is_empty()
    }

    /// Give up on `id`, if it is still waiting to start.
    pub fn cancel(&mut self, id: &str) {
        self.queue.retain(|r| r.id != id);
    }
}

/// Renders the shapes [`ShapeCaptures`] is asked for.
pub struct ShapeCapturePlugin;

impl Plugin for ShapeCapturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShapeCaptures>().add_systems(Update, run_captures);
    }
}

/// Start queued shapes while there is room in the batch, and take the picture
/// of each one that has rendered long enough.
fn run_captures(
    mut commands: Commands,
    registry: Option<Res<ShapeRegistry>>,
    mut captures: ResMut<ShapeCaptures>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let captures = &mut *captures;
    if !captures.busy() {
        // Nothing left: take the lights down too, so the thumbnail layer holds
        // nothing between batches.
        for e in captures.rig.drain(..) {
            commands.entity(e).try_despawn();
        }
        return;
    }
    let layer = RenderLayers::layer(SHAPE_THUMBNAIL_LAYER);

    for a in &mut captures.active {
        a.frames += 1;
        if a.frames < WARMUP_FRAMES || a.shot {
            continue;
        }
        a.shot = true;
        let (id, path, camera, mesh) = (a.id.clone(), a.path.clone(), a.camera, a.mesh);
        commands.spawn((Screenshot::image(a.target.clone()), HideInHierarchy, EditorLocked)).observe(
            move |captured: On<ScreenshotCaptured>, mut commands: Commands, mut captures: ResMut<ShapeCaptures>| {
                commands.entity(camera).try_despawn();
                commands.entity(mesh).try_despawn();
                let result = write_png(&captured.image, &path).map(|()| path.clone());
                captures.active.retain(|a| a.id != id);
                captures.finished.push((id.clone(), result));
            },
        );
    }

    if captures.queue.is_empty() {
        return;
    }
    let Some(registry) = registry else {
        for r in captures.queue.drain(..) {
            captures.finished.push((r.id, Err("there are no shapes registered here".into())));
        }
        return;
    };

    if captures.rig.is_empty() {
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
            let light = commands
                .spawn((
                    DirectionalLight { color, illuminance, shadow_maps_enabled: false, ..default() },
                    // A directional light shines along its forward (-Z), so
                    // face it from `from` toward the origin.
                    Transform::default().looking_to(-from.normalize(), Vec3::Y),
                    layer.clone(),
                    HideInHierarchy,
                    EditorLocked,
                    Name::new(format!("Shape Thumbnail {label} Light")),
                ))
                .id();
            captures.rig.push(light);
        }
    }
    // One neutral clay for every thumbnail, rather than each shape's spawn
    // colour: the grid is for comparing forms, and thirty different tints
    // made it read as a palette.
    let material = captures
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

    while captures.active.len() < BATCH {
        let Some(request) = captures.queue.pop_front() else { break };
        let Some(mesh) = registry.create_mesh(&request.id, &mut meshes) else {
            captures.finished.push((request.id.clone(), Err(format!("no shape is called {}", request.id))));
            continue;
        };
        let extent = Extent3d { width: THUMB_PX, height: THUMB_PX, depth_or_array_layers: 1 };
        // No CPU copy: the GPU draws into it and the screenshot reads it back.
        let mut image = Image { data: None, ..default() };
        image.texture_descriptor.size = extent;
        image.texture_descriptor.format = TextureFormat::Bgra8UnormSrgb;
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::COPY_SRC
            | TextureUsages::RENDER_ATTACHMENT;
        let target = images.add(image);

        // Frame the mesh by its bounding sphere so a 4 m wall and a 1 m cube
        // both fill the tile.
        let (center, radius) = meshes.get(&mesh).and_then(bounds).unwrap_or((Vec3::ZERO, 1.0));
        let origin = Vec3::new(captures.slot as f32 * SPACING, 0.0, 0.0);
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
                Name::new(format!("Shape Thumbnail ({})", request.id)),
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
                    clear_color: ClearColorConfig::Custom(request.clear),
                    order: -2000 - (captures.slot % 1000) as isize,
                    ..default()
                },
                RenderTarget::Image(target.clone().into()),
                Transform::from_translation(focus + dir * dist).looking_at(focus, Vec3::Y),
                layer.clone(),
                IsolatedCamera,
                HideInHierarchy,
                EditorLocked,
                Name::new("Shape Thumbnail Camera"),
            ))
            .id();
        captures.slot += 1;
        captures.active.push(Active {
            id: request.id,
            path: request.path,
            camera,
            mesh: mesh_e,
            target,
            frames: 0,
            shot: false,
        });
    }
}

/// Save a screenshot's pixels as a PNG, making the folder if need be.
fn write_png(image: &Image, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not make {}: {e}", dir.display()))?;
    }
    let pixels = image
        .clone()
        .try_into_dynamic()
        .map_err(|e| format!("the picture is in a format that cannot be saved: {e}"))?
        .to_rgba8();
    // Written beside and renamed into place, so an editor reading the cache
    // never meets half a file.
    // Still ending `.png`: the encoder picks its format from the extension.
    let partial = path.with_extension("partial.png");
    pixels
        .save(&partial)
        .map_err(|e| format!("could not write {}: {e}", partial.display()))?;
    std::fs::rename(&partial, path).map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// Load a cached thumbnail into an image the UI can draw.
pub fn load_png(path: &Path) -> Result<Image, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Image::from_buffer(
        &bytes,
        bevy::image::ImageType::Extension("png"),
        bevy::image::CompressedImageFormats::NONE,
        true,
        bevy::image::ImageSampler::Default,
        RenderAssetUsages::RENDER_WORLD,
    )
    .map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_name_follows_the_mesh_and_the_background() {
        let cube = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        let wide = Mesh::from(Cuboid::new(2.0, 1.0, 1.0));
        let grey = Color::srgb(0.2, 0.2, 0.2);
        let a = shape_thumb_path("cube", &cube, grey).unwrap();
        assert_eq!(a, shape_thumb_path("cube", &cube, grey).unwrap(), "the same shape keeps its file");
        assert_ne!(a, shape_thumb_path("cube", &wide, grey).unwrap(), "a changed mesh gets a new file");
        assert_ne!(a, shape_thumb_path("cube", &cube, Color::BLACK).unwrap(), "another tile colour gets a new file");
        assert!(a.file_name().unwrap().to_string_lossy().starts_with("cube-"));
    }
}
