//! `renzora --render-host <file>`: render the editor's viewport in a child
//! process.
//!
//! The editor spawns this binary with `--no-editor --project <dir> --scene
//! <path> --render-host <file>`. It runs with no window, loads the scene the
//! editor has open, renders it from the pose the editor writes into the link
//! file every frame, reads each frame back from the GPU and publishes it into
//! the same file. The editor shows those pixels in its viewport panel and never
//! renders the 3D scene itself, so a slow scene no longer slows the editor UI.
//! The layout is `renzora_render_link`.
//!
//! # Rendering through the scene's own camera
//!
//! The World Environment's effects (atmosphere, Solari, bloom, TAA) reach a
//! shipped game's camera through `update_runtime_effect_routing`, which routes
//! them to the active `SceneCamera`. So the host reuses that camera: it
//! overwrites its pose and projection with the editor's every frame and points
//! it at an offscreen image. A scene with no camera gets a stand-in, which the
//! normal camera rehydration fills in like any other.
//!
//! # A viewer, not a game
//!
//! Scripts and physics stay paused (a `PlayModeState` that is editing, and
//! `PausePhysics`): the editor is editing, and the host only shows it.
//!
//! # Following the editor's edits
//!
//! The scene is first loaded from disk, so there is something to show at once.
//! The editor then sends the scene as it actually is (unsaved edits included)
//! on this process's stdin, followed by a patch whenever something changes; see
//! `renzora_render_link::SceneMessage`. Patches land on the entities already
//! here, through the map from the editor's entity ids to ours, so an edit never
//! reloads the scene.
//!
//! Editor overlays (gizmos, grid, selection outlines) are not drawn here.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;

use bevy::ecs::entity::EntityHashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use renzora::core::{CurrentProject, DefaultCamera, HideInHierarchy, PlayModeState, SceneCamera};
use renzora_engine::scene_io::{apply_entity_snapshot, is_document_entity, remove_components_by_path};
use renzora_render_link::{Link, SceneMessage, ViewRequest};

/// The link file named by `--render-host <file>`, when this launch is a
/// render host. Read before the app is assembled, because a render host boots
/// without a window.
pub fn requested() -> Option<PathBuf> {
    arg_value("--render-host").map(PathBuf::from)
}

fn arg_value(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

/// Installs the render host. Add it after the engine plugins, so the project
/// is already open when `build` runs.
pub struct RenderHostPlugin {
    pub link: PathBuf,
}

#[derive(Resource)]
struct HostLink(Link);

/// The offscreen image the camera renders into, and its size.
#[derive(Resource)]
struct HostTarget {
    image: Handle<Image>,
    size: UVec2,
}

/// What the stdin reader hands the app: a scene message, or the editor gone.
enum Inbound {
    Scene(SceneMessage),
    EditorGone,
}

/// The receiving end of the stdin reader thread. Behind a mutex only because a
/// resource has to be `Sync`; one system ever touches it.
#[derive(Resource)]
struct Inbox(Mutex<Receiver<Inbound>>);

/// The editor's entity ids mapped to ours, so a patch lands on the entity it
/// describes.
#[derive(Resource, Default)]
struct SyncMap(EntityHashMap<Entity>);

/// The camera the host spawned because the scene has none of its own.
#[derive(Component)]
struct StandInCamera;

impl Plugin for RenderHostPlugin {
    fn build(&self, app: &mut App) {
        let link = match Link::open(&self.link) {
            Ok(link) => link,
            Err(e) => {
                // Nothing to render for. Exiting beats a window-less process the
                // editor cannot talk to and nobody can see.
                error!("[render-host] cannot open {}: {e}", self.link.display());
                std::process::exit(2);
            }
        };
        info!("[render-host] rendering for the editor through {}", self.link.display());
        app.insert_resource(HostLink(link));

        // Show the scene the editor has open, not the project's main scene.
        let scene = arg_value("--scene");
        let world = app.world_mut();
        if let (Some(scene), Some(mut project)) = (scene.as_ref(), world.get_resource_mut::<CurrentProject>()) {
            project.config.main_scene = scene.clone();
        }

        // Editing, not playing: scripts stay quiet.
        world.insert_resource(PlayModeState::default());

        app.insert_resource(Inbox(Mutex::new(read_stdin())));
        app.init_resource::<SyncMap>();

        app.add_systems(Startup, (create_target, pause_physics))
            .add_systems(Update, (apply_scene_messages, ensure_camera, apply_view).chain());
    }
}

/// Read scene messages off stdin on a thread of its own. The editor holds the
/// pipe open for as long as it runs; when it exits, or crashes, the pipe ends
/// and the reader says so.
fn read_stdin() -> Receiver<Inbound> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        loop {
            match SceneMessage::read_from(&mut stdin) {
                Ok(Some(message)) => {
                    if tx.send(Inbound::Scene(message)).is_err() {
                        return;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    // A malformed message means the two sides disagree about
                    // the format; carrying on would misread everything after.
                    error!("[render-host] unreadable message from the editor: {e}");
                    break;
                }
            }
        }
        let _ = tx.send(Inbound::EditorGone);
    });
    rx
}

/// Apply what the editor sent since last frame, in order.
fn apply_scene_messages(world: &mut World) {
    let messages: Vec<Inbound> = {
        let inbox = world.resource::<Inbox>();
        let rx = inbox.0.lock().unwrap_or_else(|e| e.into_inner());
        rx.try_iter().collect()
    };
    for message in messages {
        match message {
            Inbound::EditorGone => {
                info!("[render-host] the editor has gone, exiting");
                // Straight out, like the editor's own fast exit: there is
                // nothing to save, and unwinding the world only risks a hang.
                std::process::exit(0);
            }
            Inbound::Scene(SceneMessage::Reset { bsn }) => {
                // Clear what the disk load (or an earlier reset) put here, the
                // same set a save would have written.
                let mut q = world.query_filtered::<Entity, With<Name>>();
                let doomed: Vec<Entity> = q
                    .iter(world)
                    .filter(|&e| is_document_entity(world, e))
                    .collect();
                for entity in doomed {
                    if let Ok(e) = world.get_entity_mut(entity) {
                        e.despawn();
                    }
                }
                let map = apply_entity_snapshot(world, &bsn, &EntityHashMap::default());
                info!("[render-host] showing the editor's scene ({} entities)", map.len());
                world.resource_mut::<SyncMap>().0 = map;
            }
            Inbound::Scene(SceneMessage::Patch { bsn, removed, despawned }) => {
                let mut map = std::mem::take(&mut world.resource_mut::<SyncMap>().0);
                for id in despawned {
                    if let Some(ours) = map.remove(&Entity::from_bits(id)) {
                        if let Ok(e) = world.get_entity_mut(ours) {
                            e.despawn();
                        }
                    }
                }
                if !bsn.is_empty() {
                    let written = apply_entity_snapshot(world, &bsn, &map);
                    map.extend(written);
                }
                for (id, paths) in removed {
                    if let Some(&ours) = map.get(&Entity::from_bits(id)) {
                        remove_components_by_path(world, ours, &paths);
                    }
                }
                world.resource_mut::<SyncMap>().0 = map;
            }
        }
    }
}

fn pause_physics(mut commands: Commands) {
    commands.trigger(renzora::PausePhysics);
}

/// A `Bgra8UnormSrgb` render target the readback can copy from: the same
/// format as the editor's viewport images, so frames cross unconverted.
///
/// No CPU copy: only the GPU draws into or reads back from a render target, and
/// wgpu zero-fills one created without data. The readback copies from the GPU
/// texture, never from `data`.
fn make_target(size: UVec2) -> Image {
    let mut image = Image::new_uninit(
        Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST
        | TextureUsages::COPY_SRC
        | TextureUsages::RENDER_ATTACHMENT;
    image
}

fn create_target(mut commands: Commands, mut images: ResMut<Assets<Image>>, link: Res<HostLink>) {
    let size = link
        .0
        .read_view()
        .map(|v| UVec2::new(v.width, v.height))
        .unwrap_or(UVec2::new(1280, 720));
    let image = images.add(make_target(size));
    commands.insert_resource(HostTarget { image: image.clone(), size });
    // Left in place, the readback repeats every frame.
    commands.spawn(Readback::texture(image)).observe(publish_frame);
}

/// Strip the GPU's row padding and publish the frame into the link.
fn publish_frame(event: On<ReadbackComplete>, link: Res<HostLink>, target: Res<HostTarget>) {
    let (width, height) = (target.size.x as usize, target.size.y as usize);
    let row = width * 4;
    // Copies are padded to 256-byte rows.
    let padded = row.div_ceil(256) * 256;
    if event.data.len() != padded * height {
        // A frame from before the last resize. The next one will fit.
        return;
    }
    link.0.publish_frame(target.size.x, target.size.y, |out| {
        if padded == row {
            out.copy_from_slice(&event.data[..row * height]);
        } else {
            for (y, dst) in out.chunks_exact_mut(row).enumerate() {
                dst.copy_from_slice(&event.data[y * padded..y * padded + row]);
            }
        }
    });
}

/// Give a camera-less scene a stand-in, and drop it once the scene has one of
/// its own. Unnamed, so a scene reload's sweep leaves it alone, and hidden from
/// the hierarchy, which is what keeps an unnamed scene helper alive in a game.
fn ensure_camera(
    mut commands: Commands,
    authored: Query<(), (With<SceneCamera>, Without<StandInCamera>)>,
    stand_in: Query<Entity, With<StandInCamera>>,
) {
    let has_own = !authored.is_empty();
    match (has_own, stand_in.iter().next()) {
        (false, None) => {
            commands.spawn((
                SceneCamera,
                DefaultCamera,
                StandInCamera,
                HideInHierarchy,
                Transform::default(),
                Visibility::default(),
            ));
        }
        (true, Some(entity)) => commands.entity(entity).despawn(),
        _ => {}
    }
}

/// Point the active scene camera at the editor's view, and at our image.
fn apply_view(
    link: Res<HostLink>,
    mut target: ResMut<HostTarget>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<
        (
            &mut Camera,
            &mut Transform,
            &mut Projection,
            Option<&mut RenderTarget>,
            Option<&ChildOf>,
            Entity,
            Has<DefaultCamera>,
        ),
        With<SceneCamera>,
    >,
    parents: Query<&GlobalTransform>,
    mut commands: Commands,
) {
    let Some(view) = link.0.read_view() else { return };

    let size = UVec2::new(view.width, view.height);
    if size != target.size {
        if let Some(mut image) = images.get_mut(&target.image) {
            image.resize(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 });
            target.size = size;
        }
    }

    // The scene's default camera, or its first one. A camera arriving from the
    // editor comes switched off (scene cameras do not render in the editor), so
    // "whichever is active" would find nothing; choose one and switch it on.
    let chosen = cameras
        .iter()
        .find(|(.., is_default)| *is_default)
        .or_else(|| cameras.iter().next())
        .map(|(.., entity, _)| entity);
    let Some(chosen) = chosen else { return };

    let world = world_transform(&view);
    for (mut camera, mut transform, mut projection, render_target, parent, entity, _) in &mut cameras {
        let wanted_active = entity == chosen;
        if camera.is_active != wanted_active {
            camera.is_active = wanted_active;
        }
        if !wanted_active {
            continue;
        }
        // The pose is the editor camera's WORLD pose; a parented scene camera
        // needs it in its parent's space.
        let local = match parent.and_then(|p| parents.get(p.parent()).ok()) {
            Some(parent) => {
                Transform::from_matrix(parent.affine().inverse() * world.to_matrix())
            }
            None => world,
        };
        if *transform != local {
            *transform = local;
        }
        let wanted = projection_from(&view);
        if !same_projection(&projection, &wanted) {
            *projection = wanted;
        }
        let image_target = RenderTarget::Image(target.image.clone().into());
        match render_target {
            Some(mut current) => {
                if !matches!(&*current, RenderTarget::Image(t) if t.handle == target.image) {
                    *current = image_target;
                }
            }
            None => {
                commands.entity(entity).insert(image_target);
            }
        }
    }
}

fn world_transform(view: &ViewRequest) -> Transform {
    let [x, y, z, w] = view.rotation;
    Transform {
        translation: Vec3::from_array(view.translation),
        rotation: Quat::from_xyzw(x, y, z, w).normalize(),
        scale: Vec3::ONE,
    }
}

fn projection_from(view: &ViewRequest) -> Projection {
    if view.perspective == 1 {
        Projection::Perspective(PerspectiveProjection {
            fov: view.fov_y,
            near: view.near,
            far: view.far,
            ..default()
        })
    } else {
        let mut ortho = OrthographicProjection::default_3d();
        ortho.scale = view.ortho_scale;
        ortho.near = view.near;
        ortho.far = view.far;
        Projection::Orthographic(ortho)
    }
}

/// Compare only what the link carries, so the aspect ratio Bevy computes from
/// the target size does not count as a difference every frame.
fn same_projection(current: &Projection, wanted: &Projection) -> bool {
    match (current, wanted) {
        (Projection::Perspective(a), Projection::Perspective(b)) => {
            a.fov == b.fov && a.near == b.near && a.far == b.far
        }
        (Projection::Orthographic(a), Projection::Orthographic(b)) => {
            a.scale == b.scale && a.near == b.near && a.far == b.far
        }
        _ => false,
    }
}
