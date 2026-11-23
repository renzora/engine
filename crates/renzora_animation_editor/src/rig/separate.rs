//! "Separate Parts" from the Hierarchy's right-click menu: a model part whose
//! pieces are merged (both wheels of an axle) split into one node per piece,
//! each centred on itself, so each can be moved and turned on its own.
//!
//! The Hierarchy only sends [`renzora_shell_api::SeparateModelParts`]; the work is
//! here because it is a rewrite of the model file, like rigging, and reuses
//! rigging's file lookup and its reload ([`super::respawn`]).

use bevy::gltf::GltfMeshName;
use bevy::prelude::*;

pub(super) fn register(app: &mut App) {
    app.add_observer(|ev: On<renzora_shell_api::SeparateModelParts>, mut commands: Commands| {
        let target = ev.event().entity;
        commands.queue(move |world: &mut World| separate(world, target));
    });
}

/// The model root `entity` belongs to: itself or the nearest ancestor that is
/// a placed model.
fn model_root(world: &World, mut entity: Entity) -> Option<Entity> {
    loop {
        if world.get::<renzora_scene_api::MeshInstanceData>(entity).is_some() {
            return Some(entity);
        }
        entity = world.get::<ChildOf>(entity)?.parent();
    }
}

fn separate(world: &mut World, target: Entity) {
    let Some(root) = model_root(world, target) else {
        super::tell(world, false, "Separate Parts works on a part of a placed model");
        return;
    };
    // Every mesh the clicked entity draws or holds, by its name in the file.
    let mut names: Vec<String> = Vec::new();
    for e in super::descendants(world, target) {
        if let Some(name) = world.get::<GltfMeshName>(e) {
            if !names.contains(&name.0) {
                names.push(name.0.clone());
            }
        }
    }
    if names.is_empty() {
        super::tell(world, false, "There are no meshes here to separate");
        return;
    }
    let Some((abs, rel)) = super::model_file(world, root) else {
        super::tell(world, false, "This model has no file to separate parts of");
        return;
    };
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    match renzora_rig::separate_file(&abs, &refs) {
        Ok(done) => {
            let parts: Vec<&str> = done.parts.iter().map(|p| p.name.as_str()).collect();
            super::tell(
                world,
                true,
                format!(
                    "Separated {} of {rel} into {} parts: {}. The original is kept beside it.",
                    done.parent,
                    parts.len(),
                    parts.join(", ")
                ),
            );
            super::respawn::request(world, &rel);
        }
        Err(e) => super::tell(world, false, format!("Could not separate parts of {rel}: {e}")),
    }
}
