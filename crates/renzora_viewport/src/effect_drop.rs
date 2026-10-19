//! Dropping a post-process effect file on the viewport: it joins the scene's
//! Post Process list.
//!
//! An effect has no place in the scene to land on, unlike a material or a
//! model, so the drop goes to whichever list the cameras already read. Before
//! this an effect `.wesl` was taken for a hand-written material shader and put
//! on the mesh under the cursor, which crashed the editor: compiled as a mesh's
//! surface, the effect binds the screen texture where the mesh pipeline puts
//! the view uniform.

use std::path::PathBuf;

use bevy::prelude::*;
use renzora::core::{CurrentProject, DefaultCamera, HideInHierarchy, SceneCamera};
use renzora::{PostEffects, WorldEnvironment};
use renzora_editor_framework::EditorSelection;

/// Add the effect at `path` to the end of the scene's Post Process list.
///
/// The list it joins, in order of preference: one that already exists, so a
/// second drop lands beside the first; the World Environment's, which every
/// camera reads; the default scene camera's. With none of those there is no
/// camera to see it, and the drop says so in the Console.
pub(crate) fn commit_effect_drop(world: &mut World, path: PathBuf) {
    let Some(target) = effect_target(world) else {
        renzora::console_log::console_warn(
            "Post Process",
            "Nothing to add the effect to: the scene has no camera or World Environment.",
        );
        return;
    };
    let rel = match world.get_resource::<CurrentProject>() {
        Some(project) => project.make_asset_relative(&path),
        None => path.to_string_lossy().replace('\\', "/"),
    };
    match world.get_mut::<PostEffects>(target) {
        Some(mut list) => list.push(rel.clone()),
        None => {
            let mut list = PostEffects::default();
            list.push(rel.clone());
            world.entity_mut(target).insert(list);
        }
    }
    // Select it, so the inspector shows the effect that was just added.
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(target));
    }
    let on = world
        .get::<Name>(target)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| format!("{target:?}"));
    renzora::console_log::console_info("Post Process", format!("Added {rel} to {on}"));
}

fn effect_target(world: &mut World) -> Option<Entity> {
    let mut lists = world.query_filtered::<Entity, (With<PostEffects>, Without<HideInHierarchy>)>();
    if let Some(e) = lists.iter(world).next() {
        return Some(e);
    }
    let mut envs = world.query_filtered::<Entity, (With<WorldEnvironment>, Without<HideInHierarchy>)>();
    if let Some(e) = envs.iter(world).next() {
        return Some(e);
    }
    let mut cameras = world.query_filtered::<(Entity, Has<DefaultCamera>), With<SceneCamera>>();
    let cameras: Vec<(Entity, bool)> = cameras.iter(world).collect();
    cameras
        .iter()
        .find(|(_, default)| *default)
        .or_else(|| cameras.first())
        .map(|(e, _)| *e)
}
