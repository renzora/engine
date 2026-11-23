//! Putting a freshly rigged model into the scene.
//!
//! Rigging rewrites the model's file. The instances already in the scene were
//! spawned from the old one and do not change on their own: the asset is
//! reloaded, but a spawned glTF scene is a copy, not a view of the asset. So
//! after the reload finishes, every instance of that model is rebuilt: its
//! spawned children are removed and its `MeshInstanceData` re-added, which is
//! exactly the path a scene load takes, so the instance comes back with the
//! new skeleton in it and nothing else about the entity (its name, place,
//! components, animator) changes.
//!
//! An animator on the instance is told to start again, so it binds its clips
//! to the new bones instead of to entities that no longer exist.

use bevy::prelude::*;

/// Models reloaded and waiting to be respawned, by project-relative path.
#[derive(Resource, Default)]
pub(crate) struct PendingRespawn(Vec<String>);

pub(super) fn register(app: &mut App) {
    // Waits for a pending model or a buffered glTF event. With neither, a run
    // would only clear an empty reader, so skipping it loses no event: any
    // event still buffered keeps the condition true until the system has
    // read or cleared it.
    app.init_resource::<PendingRespawn>().add_systems(
        Update,
        respawn_when_reloaded.run_if(
            |p: Res<PendingRespawn>, ev: Option<Res<Messages<AssetEvent<bevy::gltf::Gltf>>>>| {
                !p.0.is_empty() || ev.is_some_and(|e| !e.is_empty())
            },
        ),
    );
}

/// Reload `rel` and rebuild its instances once the new version has loaded.
pub(crate) fn request(world: &mut World, rel: &str) {
    world.resource::<AssetServer>().reload(rel.to_string());
    world.resource_mut::<PendingRespawn>().0.push(rel.to_string());
}

fn respawn_when_reloaded(
    mut events: MessageReader<AssetEvent<bevy::gltf::Gltf>>,
    asset_server: Res<AssetServer>,
    mut pending: ResMut<PendingRespawn>,
    instances: Query<(Entity, &renzora_scene_api::MeshInstanceData, Option<&Children>)>,
    mut animators: Query<&mut renzora_animation::AnimatorState>,
    tracker: Option<ResMut<crate::studio_preview::StudioPreviewTracker>>,
    mut commands: Commands,
) {
    let mut tracker = tracker;
    if pending.0.is_empty() {
        events.clear();
        return;
    }
    for ev in events.read() {
        let (AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id }) = ev else { continue };
        let Some(path) = asset_server.get_path(*id) else { continue };
        let path = path.path().to_string_lossy().replace('\\', "/");
        let Some(at) = pending.0.iter().position(|p| p.replace('\\', "/") == path) else { continue };
        let rel = pending.0.remove(at);
        let mut count = 0;
        for (e, data, children) in &instances {
            if data.model_path.as_deref().map(|p| p.replace('\\', "/")) != Some(rel.replace('\\', "/")) {
                continue;
            }
            for c in children.into_iter().flatten() {
                commands.entity(*c).despawn();
            }
            let data = data.clone();
            commands.entity(e).remove::<renzora_scene_api::MeshInstanceData>().insert(data);
            if let Ok(mut a) = animators.get_mut(e) {
                a.initialized = false;
            }
            count += 1;
        }
        // The stage's copy is one more instance: forgetting what it was made
        // from has it made again from the new file.
        if let Some(t) = tracker.as_mut() {
            t.source_entity = None;
        }
        renzora_log_api::console_info("Rig", format!("Reloaded {rel} in {count} instance(s)"));
    }
}
