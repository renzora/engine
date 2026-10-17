//! Give a dropped model's own cameras a scene camera to watch them through.
//!
//! An imported fly-through carries its camera as an animated node, and a node
//! is not something the engine renders from. The importer marks each such
//! camera's view node with [`renzora::IMPORTED_CAMERA_EXTRAS_KEY`] extras (its
//! name and vertical field of view); this module reads them when the model is
//! dropped and adds a scene camera with [`renzora::FollowNode`] on that node and
//! the source's lens, made the default camera so Play shows the shot the file
//! was authored with.
//!
//! It only runs for a drop, never for a scene load: the camera it adds is an
//! ordinary scene entity and is saved with the scene, so a reload already has
//! it, and a user who deleted it does not want it back.

use bevy::ecs::reflect::ReflectComponent;
use bevy::ecs::world::FilteredEntityRef;
use bevy::gltf::{Gltf, GltfNode};
use bevy::prelude::*;
use renzora::core::{DefaultCamera, SceneCamera};
use renzora::FollowNode;

/// Placed on a dropped model's root until its imported cameras are added.
#[derive(Component)]
pub struct PendingImportedCameras {
    pub gltf: Handle<Gltf>,
    /// Frames spent waiting for the glTF to load. Bounded, so a model whose
    /// load failed does not keep this system busy forever.
    pub frames_waited: u32,
}

impl PendingImportedCameras {
    pub fn new(gltf: Handle<Gltf>) -> Self {
        Self {
            gltf,
            frames_waited: 0,
        }
    }
}

/// Ten seconds at 60 Hz. A drop normally hands over a glTF that is already
/// loaded, so this only matters when something went wrong.
const MAX_WAIT_FRAMES: u32 = 600;

/// One camera found in a model's nodes.
struct ImportedCamera {
    /// The view node to follow.
    node: String,
    /// The camera's own name in the source file.
    name: String,
    /// Vertical field of view, radians.
    yfov: f32,
}

/// System: add the scene cameras for every model dropped since the last frame.
pub fn add_imported_cameras(world: &mut World) {
    let mut pending = world.query::<(Entity, &mut PendingImportedCameras)>();
    if pending.iter(world).next().is_none() {
        return;
    }
    let loaded: std::collections::HashSet<AssetId<Gltf>> =
        world.resource::<Assets<Gltf>>().ids().collect();
    let mut ready: Vec<(Entity, Handle<Gltf>)> = Vec::new();
    let mut expired: Vec<Entity> = Vec::new();
    for (entity, mut marker) in pending.iter_mut(world) {
        if loaded.contains(&marker.gltf.id()) {
            ready.push((entity, marker.gltf.clone()));
        } else if marker.frames_waited >= MAX_WAIT_FRAMES {
            expired.push(entity);
        } else {
            marker.frames_waited += 1;
        }
    }
    for entity in expired {
        world.entity_mut(entity).remove::<PendingImportedCameras>();
    }

    for (root, handle) in ready {
        world.entity_mut(root).remove::<PendingImportedCameras>();
        for camera in imported_cameras(world, &handle) {
            spawn_follow_camera(world, camera);
        }
    }
}

/// Every node of the glTF that the importer marked as a camera view.
fn imported_cameras(world: &World, handle: &Handle<Gltf>) -> Vec<ImportedCamera> {
    let Some(gltf) = world.resource::<Assets<Gltf>>().get(handle) else {
        return Vec::new();
    };
    let nodes = world.resource::<Assets<GltfNode>>();
    let mut out = Vec::new();
    for node in gltf.nodes.iter().filter_map(|h| nodes.get(h)) {
        let Some(extras) = node.extras.as_ref() else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&extras.value) else {
            continue;
        };
        let Some(camera) = value.get(renzora::IMPORTED_CAMERA_EXTRAS_KEY) else {
            continue;
        };
        let name = camera
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&node.name)
            .to_string();
        // Bevy's own default, for a file that recorded no lens.
        let yfov = camera
            .get("yfov")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(std::f32::consts::FRAC_PI_4);
        out.push(ImportedCamera {
            node: node.name.clone(),
            name,
            yfov,
        });
    }
    out
}

/// Spawn the scene camera for one imported camera and make it the default.
///
/// The camera takes the effect settings (bloom, exposure, tonemapping, ...) of
/// the camera that was the default until now. Those live on the scene camera
/// and the editor mirrors them from whichever camera is the default, so moving
/// the default without them would change how the whole scene looks.
fn spawn_follow_camera(world: &mut World, camera: ImportedCamera) {
    // A second drop of the same model must not stack a second camera on the
    // same node.
    let target = renzora::sanitize_id(&camera.node);
    let mut followers = world.query::<&FollowNode>();
    if followers
        .iter(world)
        .any(|f| renzora::sanitize_id(&f.target) == target)
    {
        return;
    }

    let mut scene_cameras = world.query_filtered::<(Entity, Has<DefaultCamera>), With<SceneCamera>>();
    let cameras: Vec<(Entity, bool)> = scene_cameras.iter(world).collect();
    let previous = cameras
        .iter()
        .find(|(_, is_default)| *is_default)
        .or_else(|| cameras.first())
        .map(|(e, _)| *e);

    // `rehydrate_cameras` adds the `Camera3d` and an inactive `Camera`, the
    // same as for a camera loaded from a scene, so the editor's rules about
    // which camera renders stay in one place.
    let entity = world
        .spawn((
            Name::new(format!("{}_camera", camera.name)),
            SceneCamera,
            DefaultCamera,
            FollowNode {
                target: camera.node.clone(),
            },
            Projection::Perspective(PerspectiveProjection {
                fov: camera.yfov,
                ..default()
            }),
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    if let Some(previous) = previous {
        copy_effect_settings(world, previous, entity);
    }
    for (other, is_default) in cameras {
        if is_default {
            world.entity_mut(other).remove::<DefaultCamera>();
        }
    }

    renzora::console_log::console_info(
        "Import",
        format!(
            "Added camera '{}' following '{}' ({:.1} degree field of view); it is now the default camera, so Play shows the imported shot",
            camera.name,
            camera.node,
            camera.yfov.to_degrees(),
        ),
    );
}

/// Copy every reflected `*Settings` component from one camera to another: the
/// same set `sync_scene_camera_to_editor_camera` mirrors onto the editor camera.
fn copy_effect_settings(world: &mut World, from: Entity, to: Entity) {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let mut copies: Vec<(ReflectComponent, Box<dyn Reflect>)> = Vec::new();
    let source = world.entity(from);
    for reg in registry.iter() {
        let Some(reflect_component) = reg.data::<ReflectComponent>() else {
            continue;
        };
        if !reg.type_info().type_path().ends_with("Settings") {
            continue;
        }
        if let Some(value) = reflect_component.reflect(FilteredEntityRef::from(source)) {
            if let Ok(value) = value.reflect_clone() {
                copies.push((reflect_component.clone(), value));
            }
        }
    }
    for (reflect_component, value) in &copies {
        reflect_component.insert(
            &mut world.entity_mut(to),
            value.as_partial_reflect(),
            &registry,
        );
    }
}
